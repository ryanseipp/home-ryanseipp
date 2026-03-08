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
    super::validation::validate_username(username).map_err(|_| SignUpError::InvalidUsername)?;
    super::validation::validate_email(email).map_err(|_| SignUpError::InvalidEmail)?;
    let password = password.ok_or(SignUpError::PasswordRequired)?;
    super::validation::validate_password(password).map_err(|e| {
        if e.contains("at least") {
            SignUpError::PasswordTooShort
        } else {
            SignUpError::PasswordTooLong
        }
    })?;

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
