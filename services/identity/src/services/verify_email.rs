use aws_lc_rs::digest::{self, SHA256};
use base64ct::{Base64UrlUnpadded, Encoding};
use deadpool_postgres::{GenericClient, Pool};
use uuid::Uuid;

use super::UserStatus;

#[derive(Debug, thiserror::Error)]
pub enum VerifyEmailError {
    #[error("database error")]
    Db(#[from] tokio_postgres::Error),

    #[error("pool error")]
    Pool(#[from] deadpool_postgres::PoolError),

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
///
/// # Errors
///
/// Returns `VerifyEmailError` if the token is invalid, expired, already consumed, or the user is not found.
#[tracing::instrument(skip(pool, raw_token))]
pub async fn execute(pool: &Pool, raw_token: &str) -> Result<(), VerifyEmailError> {
    // Decode base64url token and hash it
    let token_bytes =
        Base64UrlUnpadded::decode_vec(raw_token).map_err(|_| VerifyEmailError::InvalidToken)?;
    let token_hash = digest::digest(&SHA256, &token_bytes);
    let token_hash_bytes = token_hash.as_ref();

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    // Look up the token
    let token_row = get_token_by_hash(&tx, token_hash_bytes)
        .await?
        .ok_or(VerifyEmailError::InvalidToken)?;

    // Check expiry
    if token_row.expires_at < chrono::Utc::now() {
        return Err(VerifyEmailError::InvalidToken);
    }

    // Consume token + mark user verified in one transaction
    consume_token(&tx, token_row.id).await?;
    mark_email_verified(&tx, token_row.user_id).await?;

    tx.commit().await?;

    tracing::info!(user_id = %token_row.user_id, "email verified");
    Ok(())
}

struct TokenRow {
    id: Uuid,
    user_id: Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
}

/// Look up an unconsumed token by its SHA-256 hash.
async fn get_token_by_hash(
    client: &impl GenericClient,
    token_hash: &[u8],
) -> Result<Option<TokenRow>, VerifyEmailError> {
    let row = client
        .query_opt(
            "SELECT id, user_id, expires_at
             FROM email_verification_tokens
             WHERE token_hash = $1 AND consumed_at IS NULL",
            &[&token_hash],
        )
        .await?;

    Ok(row.map(|r| TokenRow {
        id: r.get("id"),
        user_id: r.get("user_id"),
        expires_at: r.get("expires_at"),
    }))
}

/// Mark a token as consumed within an existing transaction.
async fn consume_token(
    client: &impl GenericClient,
    token_id: Uuid,
) -> Result<(), VerifyEmailError> {
    let rows_affected = client
        .execute(
            "UPDATE email_verification_tokens SET consumed_at = NOW()
             WHERE id = $1 AND consumed_at IS NULL",
            &[&token_id],
        )
        .await?;

    if rows_affected == 0 {
        return Err(VerifyEmailError::AlreadyConsumed);
    }
    Ok(())
}

/// Mark a user's email as verified and set status to active.
async fn mark_email_verified(
    client: &impl GenericClient,
    user_id: Uuid,
) -> Result<(), VerifyEmailError> {
    let rows_affected = client
        .execute(
            "UPDATE users SET email_verified = TRUE, status = $1, updated_at = NOW()
             WHERE id = $2 AND deleted_at IS NULL",
            &[&UserStatus::Active.as_str(), &user_id],
        )
        .await?;

    if rows_affected == 0 {
        return Err(VerifyEmailError::UserNotFound);
    }
    Ok(())
}
