use prost::Message;
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto;
use crate::crypto::password::PasswordError;
use crate::outbox;
use crate::outbox::OutboxError;
use crate::proto::email::v1::{AuthEmailMessage, PasswordChanged, auth_email_message};

use super::validation;

#[derive(Debug, thiserror::Error)]
pub enum UpdatePasswordError {
    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("password error")]
    Password(#[from] PasswordError),

    #[error("outbox error")]
    Outbox(#[from] OutboxError),

    #[error("user not found")]
    NotFound,

    #[error("invalid current password")]
    InvalidCurrentPassword,

    #[error("invalid new password: {0}")]
    InvalidNewPassword(&'static str),
}

#[tracing::instrument(skip(pool, current_password, new_password))]
pub async fn execute(
    pool: &PgPool,
    user_id: Uuid,
    current_password: &str,
    new_password: &str,
) -> Result<(), UpdatePasswordError> {
    validation::validate_password(new_password).map_err(UpdatePasswordError::InvalidNewPassword)?;

    // Fetch user
    let mut conn = pool.acquire().await?;
    let user = super::get_user_by_id(&mut conn, user_id)
        .await?
        .ok_or(UpdatePasswordError::NotFound)?;
    drop(conn);

    // Verify current password on blocking thread
    let current_hash = user.password_hash.clone();
    let current_pw = current_password.to_string();
    tokio::task::spawn_blocking(move || crypto::verify_password(&current_pw, &current_hash))
        .await
        .map_err(|_| PasswordError::VerificationFailed)?
        .map_err(|_| UpdatePasswordError::InvalidCurrentPassword)?;

    // Hash new password on blocking thread
    let new_pw = new_password.to_string();
    let new_hash = tokio::task::spawn_blocking(move || crypto::hash_password(&new_pw))
        .await
        .map_err(|_| PasswordError::HashingFailed)??;

    // Capture trace context
    let (trace_id, span_id) = super::extract_trace_context();

    // Build outbox event
    let outbox_id = Uuid::now_v7();
    let event = AuthEmailMessage {
        idempotency_key: outbox_id.to_string(),
        recipient_email: user.email.clone(),
        recipient_name: format!("{} {}", user.given_name, user.family_name),
        trace_id: trace_id.clone().unwrap_or_default(),
        span_id: span_id.clone().unwrap_or_default(),
        produced_at_ms: chrono::Utc::now().timestamp_millis(),
        payload: Some(auth_email_message::Payload::PasswordChanged(
            PasswordChanged {
                changed_at_ms: chrono::Utc::now().timestamp_millis(),
                change_ip: String::new(),
                change_location: String::new(),
            },
        )),
    };
    let payload = event.encode_to_vec();

    // Transaction: update password + outbox event
    let mut tx = pool.begin().await.map_err(UpdatePasswordError::Db)?;

    sqlx::query!(
        "UPDATE users SET password_hash = $2, updated_at = NOW()
         WHERE id = $1 AND deleted_at IS NULL",
        user_id,
        new_hash,
    )
    .execute(&mut *tx)
    .await
    .map_err(UpdatePasswordError::Db)?;

    outbox::db::insert_event(
        &mut *tx,
        outbox_id,
        "user",
        user_id,
        "auth_email",
        &payload,
        trace_id.as_deref(),
        span_id.as_deref(),
    )
    .await
    .map_err(UpdatePasswordError::Outbox)?;

    tx.commit().await.map_err(UpdatePasswordError::Db)?;

    tracing::info!(%user_id, "password updated");
    Ok(())
}
