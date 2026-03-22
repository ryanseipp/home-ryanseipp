use deadpool_postgres::Pool;
use prost::Message;
use uuid::Uuid;

use crate::crypto;
use crate::outbox;
use crate::outbox::OutboxError;
use crate::proto::email::v1::{AuthEmailMessage, EmailVerification, auth_email_message};

use super::UserStatus;

/// Maximum verification emails per user per hour.
const MAX_TOKENS_PER_HOUR: i64 = 3;

/// Verification token validity in minutes.
const TOKEN_EXPIRES_MINUTES: i32 = 60;

#[derive(Debug, thiserror::Error)]
pub enum ResendVerificationError {
    #[error("database error")]
    Db(#[from] tokio_postgres::Error),

    #[error("pool error")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("crypto error")]
    Crypto(#[from] crypto::CryptoError),

    #[error("outbox error")]
    Outbox(#[from] OutboxError),

    #[error("too many verification requests, try again later")]
    RateLimited,
}

/// Resend email verification for a user.
///
/// Returns OK even if the email doesn't exist (prevents user enumeration).
/// Only returns errors for rate limiting.
///
/// # Errors
///
/// Returns `ResendVerificationError` on rate limiting or internal failures.
#[tracing::instrument(skip(pool, web_base_url))]
pub async fn execute(
    pool: &Pool,
    email: &str,
    web_base_url: &str,
) -> Result<(), ResendVerificationError> {
    let mut client = pool.get().await?;

    // Look up user — if not found, return Ok (prevent enumeration)
    let Ok(Some(user)) = super::get_user_by_email(&client, email).await else {
        return Ok(());
    };

    // Only resend for pending_verification users
    if user.status != UserStatus::PendingVerification {
        // Already verified or other status — silently return Ok
        return Ok(());
    }

    // Rate limit: max 3 tokens per hour
    let recent_count = count_recent_tokens(&client, user.id).await?;
    if recent_count >= MAX_TOKENS_PER_HOUR {
        return Err(ResendVerificationError::RateLimited);
    }

    // Generate new verification token
    let token_id = Uuid::now_v7();
    let outbox_id = Uuid::now_v7();

    let (token_string, token_hash_bytes) = crypto::token::generate_verification_token()?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::minutes(i64::from(TOKEN_EXPIRES_MINUTES));

    // Capture OTEL trace context for span links
    let (trace_id, span_id) = super::extract_trace_context();

    // Build outbox event
    let verification_link = format!("{web_base_url}/verify-email?token={token_string}");
    let event = AuthEmailMessage {
        idempotency_key: outbox_id.to_string(),
        recipient_email: user.email.clone(),
        recipient_name: format!("{} {}", user.given_name, user.family_name),
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

    // Transaction: insert token + insert outbox event
    let tx = client.transaction().await?;

    super::insert_token(&tx, token_id, user.id, &token_hash_bytes, expires_at)
        .await
        .map_err(ResendVerificationError::Db)?;

    outbox::db::insert_event(
        &tx,
        outbox_id,
        "user",
        user.id,
        "auth_email",
        &payload,
        trace_id.as_deref(),
        span_id.as_deref(),
    )
    .await
    .map_err(ResendVerificationError::Outbox)?;

    tx.commit().await?;

    tracing::info!(user_id = %user.id, "verification email resend queued");
    Ok(())
}

/// Count recent tokens for a user (for rate limiting).
/// Returns the number of tokens created in the last hour.
async fn count_recent_tokens(
    client: &impl deadpool_postgres::GenericClient,
    user_id: Uuid,
) -> Result<i64, tokio_postgres::Error> {
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM email_verification_tokens
             WHERE user_id = $1 AND created_at > NOW() - INTERVAL '1 hour'",
            &[&user_id],
        )
        .await?;

    Ok(row.get::<_, Option<i64>>(0).unwrap_or(0))
}
