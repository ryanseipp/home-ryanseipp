use sqlx::PgPool;
use uuid::Uuid;

use super::validation;

#[derive(Debug, thiserror::Error)]
pub enum UpdateProfileError {
    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("no fields provided")]
    NoFieldsProvided,

    #[error("invalid username: {0}")]
    InvalidUsername(&'static str),

    #[error("username already taken")]
    UsernameTaken,
}

#[tracing::instrument(skip(pool))]
pub async fn execute(
    pool: &PgPool,
    user_id: Uuid,
    given_name: Option<&str>,
    family_name: Option<&str>,
    username: Option<&str>,
) -> Result<(), UpdateProfileError> {
    if given_name.is_none() && family_name.is_none() && username.is_none() {
        return Err(UpdateProfileError::NoFieldsProvided);
    }

    if let Some(u) = username {
        validation::validate_username(u).map_err(UpdateProfileError::InvalidUsername)?;
    }

    sqlx::query!(
        "UPDATE users
         SET given_name = COALESCE($2, given_name),
             family_name = COALESCE($3, family_name),
             username = COALESCE($4, username),
             updated_at = NOW()
         WHERE id = $1 AND deleted_at IS NULL",
        user_id,
        given_name,
        family_name,
        username,
    )
    .execute(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db_err) if db_err.constraint() == Some("idx_users_username") => {
            UpdateProfileError::UsernameTaken
        }
        _ => UpdateProfileError::Db(e),
    })?;

    Ok(())
}
