use sqlx::PgPool;
use uuid::Uuid;

use super::User;

#[derive(Debug, thiserror::Error)]
pub enum UserInfoError {
    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("user not found")]
    NotFound,
}

#[tracing::instrument(skip(pool))]
pub async fn execute(pool: &PgPool, user_id: Uuid) -> Result<User, UserInfoError> {
    let mut conn = pool.acquire().await?;
    super::get_user_by_id(&mut conn, user_id)
        .await?
        .ok_or(UserInfoError::NotFound)
}
