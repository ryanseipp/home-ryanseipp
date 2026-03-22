use deadpool_postgres::Pool;
use uuid::Uuid;

use super::User;

#[derive(Debug, thiserror::Error)]
pub enum UserInfoError {
    #[error("database error")]
    Db(#[from] tokio_postgres::Error),

    #[error("pool error")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("user not found")]
    NotFound,
}

/// Fetch a user's profile by ID.
///
/// # Errors
///
/// Returns `UserInfoError` if the user is not found or a database error occurs.
#[tracing::instrument(skip(pool))]
pub async fn execute(pool: &Pool, user_id: Uuid) -> Result<User, UserInfoError> {
    let client = pool.get().await?;
    super::get_user_by_id(&client, user_id)
        .await?
        .ok_or(UserInfoError::NotFound)
}
