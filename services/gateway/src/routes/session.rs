use axum::extract::{FromRef, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::header::COOKIE;
use axum::http::request::Parts;

use super::AppState;
use super::error::AppError;
use crate::session::{Session, hash_session_token};

/// Cookie name for session ID. Uses `__Host-` prefix to enforce Secure,
/// Path=/, and no Domain (strongest browser protections).
pub const SESSION_COOKIE: &str = "__Host-sid";

/// Max-Age for the session cookie (7 days in seconds).
const SESSION_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60;

/// Axum extractor that reads the session cookie, looks up the session in
/// `ScyllaDB`, and provides the authenticated `Session` to route handlers.
///
/// Returns 401 if the cookie is missing, invalid, or the session is not found.
pub struct AuthSession(pub Session);

impl<S> FromRequestParts<S> for AuthSession
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let token = extract_session_cookie(&parts.headers)?;
        let token_hash = hash_session_token(&token).map_err(|_| AppError::Unauthorized)?;

        let app_state = AppState::from_ref(state);
        let session = app_state
            .sessions
            .get_session(token_hash)
            .await
            .map_err(AppError::Session)?
            .ok_or(AppError::Unauthorized)?;

        Ok(AuthSession(session))
    }
}

/// Parse the `__Host-sid` value from the Cookie header.
fn extract_session_cookie(headers: &HeaderMap) -> Result<String, AppError> {
    let cookie_header = headers
        .get(COOKIE)
        .ok_or(AppError::Unauthorized)?
        .to_str()
        .map_err(|_| AppError::Unauthorized)?;

    // Cookie header format: "name1=value1; name2=value2; ..."
    for pair in cookie_header.split(';') {
        let pair = pair.trim();
        if let Some(value) = pair.strip_prefix(SESSION_COOKIE) {
            let value = value.strip_prefix('=').ok_or(AppError::Unauthorized)?;
            return Ok(value.to_owned());
        }
    }

    Err(AppError::Unauthorized)
}

/// Build a `Set-Cookie` header value for the session cookie.
#[must_use]
pub fn set_session_cookie(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={SESSION_MAX_AGE_SECS}"
    )
}

/// Build a `Set-Cookie` header value that clears the session cookie.
#[must_use]
pub fn clear_session_cookie() -> String {
    format!("{SESSION_COOKIE}=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0")
}

/// Wrap a protobuf request with Bearer authorization from the session's
/// access token, for forwarding to downstream gRPC services.
///
/// # Panics
///
/// Panics if the access token contains non-ASCII characters.
pub fn with_auth<T>(session: &Session, request: T) -> tonic::Request<T> {
    let mut req = tonic::Request::new(request);
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", session.access_token)
            .parse()
            .expect("Bearer token is valid ASCII"),
    );
    req
}
