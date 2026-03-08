use prost::Message;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::crypto;
use crate::crypto::password::PasswordError;
use crate::outbox;
use crate::outbox::OutboxError;
use crate::proto::email::v1::{AuthEmailMessage, EmailVerification, auth_email_message};

/// Verification token validity in minutes.
const TOKEN_EXPIRES_MINUTES: i32 = 60;

#[derive(Debug, thiserror::Error)]
pub enum SignUpError {
    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("password error")]
    Password(#[from] PasswordError),

    #[error("crypto error")]
    Crypto(#[from] crypto::CryptoError),

    #[error("outbox error")]
    Outbox(#[from] OutboxError),

    #[error("username already taken")]
    UsernameTaken,

    #[error("email already taken")]
    EmailTaken,

    #[error("password is required")]
    PasswordRequired,

    #[error("password must be at least 8 characters")]
    PasswordTooShort,

    #[error("password must be at most 128 characters")]
    PasswordTooLong,

    #[error("invalid username: must be 1-64 characters, alphanumeric, underscore, or hyphen")]
    InvalidUsername,

    #[error("invalid email address")]
    InvalidEmail,
}

/// Execute user registration.
///
/// 1. Validate input
/// 2. Hash password (on blocking thread)
/// 3. Generate verification token
/// 4. Single transaction: insert user + verification token + outbox event
#[tracing::instrument(skip(pool, password, web_base_url), fields(username = %username, email = %email))]
pub async fn execute(
    pool: &PgPool,
    username: &str,
    email: &str,
    given_name: &str,
    family_name: &str,
    password: Option<&str>,
    web_base_url: &str,
) -> Result<(), SignUpError> {
    // -- Validate input --
    validate_username(username)?;
    validate_email(email)?;
    let password = password.ok_or(SignUpError::PasswordRequired)?;
    validate_password(password)?;

    // -- Hash password on blocking thread --
    let password_owned = password.to_string();
    let password_hash = tokio::task::spawn_blocking(move || crypto::hash_password(&password_owned))
        .await
        .map_err(|_| PasswordError::HashingFailed)??;

    // -- Generate IDs and verification token --
    let user_id = Uuid::now_v7();
    let token_id = Uuid::now_v7();
    let outbox_id = Uuid::now_v7();

    let (token_string, token_hash_bytes) = crypto::token::generate_verification_token()?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::minutes(i64::from(TOKEN_EXPIRES_MINUTES));

    // -- Capture OTEL trace context for span links --
    let (trace_id, span_id) = super::extract_trace_context();

    // -- Build outbox event payload --
    let verification_link = format!("{web_base_url}/verify-email?token={token_string}");
    let event = AuthEmailMessage {
        idempotency_key: outbox_id.to_string(),
        recipient_email: email.to_string(),
        recipient_name: format!("{given_name} {family_name}"),
        trace_id: trace_id.clone().unwrap_or_default(),
        span_id: span_id.clone().unwrap_or_default(),
        produced_at_ms: chrono::Utc::now().timestamp_millis(),
        payload: Some(auth_email_message::Payload::EmailVerification(
            EmailVerification {
                verification_code: token_string,
                verification_link,
                expires_in_minutes: TOKEN_EXPIRES_MINUTES,
            },
        )),
    };
    let payload = event.encode_to_vec();

    // -- Single transaction: user + token + outbox --
    let mut tx = pool.begin().await.map_err(SignUpError::Db)?;

    insert_user(
        &mut tx,
        user_id,
        username,
        email,
        given_name,
        family_name,
        &password_hash,
    )
    .await?;

    super::insert_token(&mut tx, token_id, user_id, &token_hash_bytes, expires_at)
        .await
        .map_err(SignUpError::Db)?;

    outbox::db::insert_event(
        &mut tx,
        outbox_id,
        "user",
        user_id,
        "auth_email",
        &payload,
        trace_id.as_deref(),
        span_id.as_deref(),
    )
    .await
    .map_err(SignUpError::Outbox)?;

    tx.commit().await.map_err(SignUpError::Db)?;

    tracing::info!(%user_id, "user registered, verification email queued");
    Ok(())
}

/// Insert a new user within an existing transaction.
async fn insert_user(
    conn: &mut PgConnection,
    id: Uuid,
    username: &str,
    email: &str,
    given_name: &str,
    family_name: &str,
    password_hash: &str,
) -> Result<(), SignUpError> {
    sqlx::query!(
        "INSERT INTO users (id, username, email, given_name, family_name, password_hash)
         VALUES ($1, $2, $3, $4, $5, $6)",
        id,
        username,
        email,
        given_name,
        family_name,
        password_hash,
    )
    .execute(conn)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.constraint() == Some("idx_users_username") => {
            SignUpError::UsernameTaken
        }
        sqlx::Error::Database(db_err) if db_err.constraint() == Some("idx_users_email") => {
            SignUpError::EmailTaken
        }
        _ => SignUpError::Db(e),
    })?;

    Ok(())
}

fn validate_username(username: &str) -> Result<(), SignUpError> {
    if username.is_empty() || username.len() > 64 {
        return Err(SignUpError::InvalidUsername);
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(SignUpError::InvalidUsername);
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), SignUpError> {
    if password.len() < 8 {
        return Err(SignUpError::PasswordTooShort);
    }
    if password.len() > 128 {
        return Err(SignUpError::PasswordTooLong);
    }
    Ok(())
}

fn validate_email(email: &str) -> Result<(), SignUpError> {
    if email.len() < 3 || email.len() > 254 || !email.contains('@') {
        return Err(SignUpError::InvalidEmail);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_usernames() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("bob-123").is_ok());
        assert!(validate_username("user_name").is_ok());
        assert!(validate_username("A").is_ok());
    }

    #[test]
    fn invalid_usernames() {
        assert!(validate_username("").is_err());
        assert!(validate_username("user name").is_err());
        assert!(validate_username("user@name").is_err());
        assert!(validate_username(&"a".repeat(65)).is_err());
    }

    #[test]
    fn valid_emails() {
        assert!(validate_email("a@b").is_ok());
        assert!(validate_email("user@example.com").is_ok());
    }

    #[test]
    fn invalid_emails() {
        assert!(validate_email("").is_err());
        assert!(validate_email("ab").is_err());
        assert!(validate_email("no-at-sign").is_err());
        assert!(validate_email(&format!("{}@b", "a".repeat(253))).is_err());
    }

    #[test]
    fn valid_passwords() {
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password(&"a".repeat(128)).is_ok());
    }

    #[test]
    fn invalid_passwords() {
        assert!(matches!(
            validate_password("short"),
            Err(SignUpError::PasswordTooShort)
        ));
        assert!(matches!(
            validate_password(&"a".repeat(129)),
            Err(SignUpError::PasswordTooLong)
        ));
    }
}
