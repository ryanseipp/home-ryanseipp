use deadpool_postgres::Pool;

use crate::crypto::jwk::{self, Jwk};

#[derive(Debug, thiserror::Error)]
pub enum GetJwksError {
    #[error("database error")]
    Db(#[from] tokio_postgres::Error),

    #[error("pool error")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("serialization failed")]
    Serialization,
}

/// Get the JWKS (all non-revoked public keys) as a JSON string.
///
/// # Errors
///
/// Returns `GetJwksError` if the database query or JSON serialization fails.
pub async fn execute(pool: &Pool) -> Result<String, GetJwksError> {
    let public_keys = list_public_jwks(pool).await?;
    let jwk_set = jwk::build_jwk_set(public_keys);
    serde_json::to_string(&jwk_set).map_err(|_| GetJwksError::Serialization)
}

pub(crate) async fn list_public_jwks(pool: &Pool) -> Result<Vec<Jwk>, GetJwksError> {
    let client = pool.get().await?;
    let rows = client
        .query(
            "SELECT public_jwk FROM signing_keys WHERE status != 'revoked'",
            &[],
        )
        .await?;

    rows.into_iter()
        .map(|r| {
            let v: serde_json::Value = r.get("public_jwk");
            serde_json::from_value(v).map_err(|_| GetJwksError::Serialization)
        })
        .collect()
}
