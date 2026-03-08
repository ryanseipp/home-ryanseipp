use base64ct::{Base64UrlUnpadded, Encoding};
use sqlx::PgPool;
use tonic::{Request, Status};
use uuid::Uuid;

use crate::crypto::jwk::Algorithm;
use crate::crypto::jwt::{JwtHeader, VerificationOptions};

/// Authenticate a gRPC request by validating the Bearer JWT.
///
/// Returns the user's UUID from the `sub` claim on success.
pub(crate) async fn authenticate<T>(request: &Request<T>, pool: &PgPool) -> Result<Uuid, Status> {
    let token = extract_bearer_token(request)?;

    // Decode header to get kid
    let header = decode_jwt_header(token)?;

    // Fetch public JWKs from DB
    let jwks = super::get_jwks::list_public_jwks(pool)
        .await
        .map_err(|_| Status::internal("failed to load signing keys"))?;

    // Find matching JWK
    let jwk = jwks
        .iter()
        .find(|k| k.kid == header.kid)
        .ok_or_else(|| Status::unauthenticated("unknown signing key"))?;

    // Verify JWT
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Status::internal("clock error"))?
        .as_secs();

    let options = VerificationOptions {
        allowed_algorithms: &[Algorithm::ES256, Algorithm::ES384],
        issuer: Some("https://home.ryanseipp.com"),
        audience: Some("https://api.home.ryanseipp.com"),
        current_time: now,
        leeway: 30,
    };

    let claims = crate::crypto::jwt::verify_jwt(token, jwk, &options)
        .map_err(|_| Status::unauthenticated("invalid token"))?;

    // Parse sub as UUID
    let sub = claims
        .sub
        .as_deref()
        .ok_or_else(|| Status::unauthenticated("missing sub claim"))?;

    sub.parse::<Uuid>()
        .map_err(|_| Status::unauthenticated("invalid sub claim"))
}

fn extract_bearer_token<T>(request: &Request<T>) -> Result<&str, Status> {
    let value = request
        .metadata()
        .get("authorization")
        .ok_or_else(|| Status::unauthenticated("missing authorization header"))?
        .to_str()
        .map_err(|_| Status::unauthenticated("invalid authorization header"))?;

    value
        .strip_prefix("Bearer ")
        .ok_or_else(|| Status::unauthenticated("invalid authorization scheme"))
}

fn decode_jwt_header(token: &str) -> Result<JwtHeader, Status> {
    let header_b64 = token
        .split('.')
        .next()
        .ok_or_else(|| Status::unauthenticated("malformed token"))?;

    let header_bytes = Base64UrlUnpadded::decode_vec(header_b64)
        .map_err(|_| Status::unauthenticated("malformed token"))?;

    serde_json::from_slice(&header_bytes).map_err(|_| Status::unauthenticated("malformed token"))
}
