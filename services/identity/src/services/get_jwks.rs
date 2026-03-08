use sqlx::PgPool;

use crate::crypto::jwk::{self, Jwk};

#[derive(Debug, thiserror::Error)]
pub enum GetJwksError {
    #[error("database error")]
    Db(#[from] sqlx::Error),

    #[error("serialization failed")]
    Serialization,
}

/// Get the JWKS (all non-revoked public keys) as a JSON string.
pub async fn execute(pool: &PgPool) -> Result<String, GetJwksError> {
    let public_keys = list_public_jwks(pool).await?;
    let jwk_set = jwk::build_jwk_set(public_keys);
    serde_json::to_string(&jwk_set).map_err(|_| GetJwksError::Serialization)
}

/// Helper for queries that only select `public_jwk`.
pub(crate) struct JwkRow {
    pub(crate) public_jwk: serde_json::Value,
}

pub(crate) async fn list_public_jwks(pool: &PgPool) -> Result<Vec<Jwk>, GetJwksError> {
    let rows = sqlx::query_as!(
        JwkRow,
        "SELECT public_jwk FROM signing_keys WHERE status != 'revoked'",
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| serde_json::from_value(r.public_jwk).map_err(|_| GetJwksError::Serialization))
        .collect()
}
