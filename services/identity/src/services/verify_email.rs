use base64ct::{Base64UrlUnpadded, Encoding};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::UserStatus;

#[derive(Debug, thiserror::Error)]
pub enum VerifyEmailError {
    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("invalid or expired verification token")]
    InvalidToken,

    #[error("verification token already consumed")]
    AlreadyConsumed,

    #[error("user not found")]
    UserNotFound,

    #[error("email already verified")]
    AlreadyVerified,
}

/// Verify a user's email using the raw token sent via email.
///
/// 1. Hash the raw token with SHA-256
/// 2. Look up the token by hash
/// 3. Check expiry
/// 4. Transaction: consume token + mark user email verified + set status active
#[tracing::instrument(skip(pool, raw_token))]
pub async fn execute(pool: &PgPool, raw_token: &str) -> Result<(), VerifyEmailError> {
    // Decode base64url token and hash it
    let token_bytes =
        Base64UrlUnpadded::decode_vec(raw_token).map_err(|_| VerifyEmailError::InvalidToken)?;
    let token_hash = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, &token_bytes);
    let token_hash_bytes = token_hash.as_ref();

    let mut tx = pool.begin().await?;

    // Look up the token
    let token_row = get_token_by_hash(&mut tx, token_hash_bytes)
        .await?
        .ok_or(VerifyEmailError::InvalidToken)?;

    // Check expiry
    if token_row.expires_at < chrono::Utc::now() {
        return Err(VerifyEmailError::InvalidToken);
    }

    // Consume token + mark user verified in one transaction
    consume_token(&mut tx, token_row.id).await?;
    mark_email_verified(&mut tx, token_row.user_id).await?;

    tx.commit().await?;

    tracing::info!(user_id = %token_row.user_id, "email verified");
    Ok(())
}

struct TokenRow {
    id: Uuid,
    user_id: Uuid,
    #[allow(dead_code)]
    token_hash: Vec<u8>,
    expires_at: chrono::DateTime<chrono::Utc>,
    #[allow(dead_code)]
    consumed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Look up an unconsumed token by its SHA-256 hash.
async fn get_token_by_hash(
    conn: &mut PgConnection,
    token_hash: &[u8],
) -> Result<Option<TokenRow>, VerifyEmailError> {
    let row = sqlx::query_as!(
        TokenRow,
        "SELECT id, user_id, token_hash, expires_at, consumed_at
         FROM email_verification_tokens
         WHERE token_hash = $1 AND consumed_at IS NULL",
        token_hash,
    )
    .fetch_optional(conn)
    .await?;

    Ok(row)
}

/// Mark a token as consumed within an existing transaction.
async fn consume_token(conn: &mut PgConnection, token_id: Uuid) -> Result<(), VerifyEmailError> {
    let result = sqlx::query!(
        "UPDATE email_verification_tokens SET consumed_at = NOW()
         WHERE id = $1 AND consumed_at IS NULL",
        token_id,
    )
    .execute(conn)
    .await?;

    if result.rows_affected() == 0 {
        return Err(VerifyEmailError::AlreadyConsumed);
    }
    Ok(())
}

/// Mark a user's email as verified and set status to active.
async fn mark_email_verified(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<(), VerifyEmailError> {
    let result = sqlx::query!(
        "UPDATE users SET email_verified = TRUE, status = $1, updated_at = NOW()
         WHERE id = $2 AND deleted_at IS NULL",
        UserStatus::Active.as_str(),
        user_id,
    )
    .execute(conn)
    .await?;

    if result.rows_affected() == 0 {
        return Err(VerifyEmailError::UserNotFound);
    }
    Ok(())
}
