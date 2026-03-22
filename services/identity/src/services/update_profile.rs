use deadpool_postgres::Pool;
use uuid::Uuid;

use super::validation;

#[derive(Debug, thiserror::Error)]
pub enum UpdateProfileError {
    #[error("database error")]
    Db(#[from] tokio_postgres::Error),

    #[error("pool error")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("no fields provided")]
    NoFieldsProvided,

    #[error("invalid username: {0}")]
    InvalidUsername(&'static str),

    #[error("username already taken")]
    UsernameTaken,
}

/// Update a user's profile fields.
///
/// # Errors
///
/// Returns `UpdateProfileError` if no fields are provided, validation fails, or a database error occurs.
#[tracing::instrument(skip(pool))]
pub async fn execute(
    pool: &Pool,
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

    let client = pool.get().await?;
    client
        .execute(
            "UPDATE users
             SET given_name = COALESCE($2, given_name),
                 family_name = COALESCE($3, family_name),
                 username = COALESCE($4, username),
                 updated_at = NOW()
             WHERE id = $1 AND deleted_at IS NULL",
            &[&user_id, &given_name, &family_name, &username],
        )
        .await
        .map_err(|e| {
            if let Some(db_err) = e.as_db_error()
                && db_err.constraint() == Some("idx_users_username")
            {
                return UpdateProfileError::UsernameTaken;
            }
            UpdateProfileError::Db(e)
        })?;

    Ok(())
}
