use std::sync::Arc;

use aws_lc_rs::digest::{self, SHA256};
use aws_lc_rs::rand;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::{DateTime, Utc};
use scylla::client::session::Session as ScyllaSession;
use scylla::client::session_builder::SessionBuilder;
use scylla::errors::{ExecutionError, NewSessionError, PrepareError, UseKeyspaceError};
use scylla::response::query_result::{IntoRowsResultError, MaybeFirstRowError};
use scylla::statement::prepared::PreparedStatement;
use uuid::Uuid;

use crate::config::ScyllaConfig;

/// A stored user session with cached JWT tokens from the Identity service.
pub struct Session {
    pub token_hash: Vec<u8>,
    pub user_id: Uuid,
    pub access_token: String,
    pub id_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Errors from the session store.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("ScyllaDB connection error: {0}")]
    NewSession(#[from] NewSessionError),

    #[error("execution error: {0}")]
    Execution(#[from] Box<ExecutionError>),

    #[error("prepare error: {0}")]
    Prepare(#[from] PrepareError),

    #[error("use keyspace error: {0}")]
    UseKeyspace(#[from] UseKeyspaceError),

    #[error("rows result error: {0}")]
    IntoRowsResult(#[from] Box<IntoRowsResultError>),

    #[error("row deserialization error: {0}")]
    MaybeFirstRow(#[from] MaybeFirstRowError),

    #[error("failed to decode JWT payload: {0}")]
    JwtDecode(String),

    #[error("failed to generate session token")]
    TokenGeneration,

    #[error("invalid session token")]
    InvalidToken,
}

/// Generate a CSPRNG session token.
///
/// Returns `(cleartext_base64url, sha256_hash)` where:
/// - `cleartext_base64url` is 32 random bytes encoded as base64url (sent to the client as a cookie)
/// - `sha256_hash` is the SHA-256 hash of the raw bytes (stored in `ScyllaDB` as the primary key)
fn generate_session_token() -> Result<(String, Vec<u8>), SessionError> {
    let mut raw = [0u8; 32];
    rand::fill(&mut raw).map_err(|_| SessionError::TokenGeneration)?;

    let cleartext = Base64UrlUnpadded::encode_string(&raw);
    let hash = digest::digest(&SHA256, &raw);

    Ok((cleartext, hash.as_ref().to_vec()))
}

/// Hash a base64url-encoded session token to its SHA-256 digest for DB lookup.
///
/// # Errors
///
/// Returns `SessionError::InvalidToken` if the token is not valid base64url.
pub fn hash_session_token(token: &str) -> Result<Vec<u8>, SessionError> {
    let raw = Base64UrlUnpadded::decode_vec(token).map_err(|_| SessionError::InvalidToken)?;
    let hash = digest::digest(&SHA256, &raw);
    Ok(hash.as_ref().to_vec())
}

/// Session store backed by `ScyllaDB`.
///
/// Manages the lifecycle of user sessions: creation after login, lookup for
/// authenticated requests, and deletion on logout. Sessions auto-expire via
/// `ScyllaDB`'s TTL (7 days).
pub struct SessionStore {
    session: ScyllaSession,
    insert: PreparedStatement,
    select: PreparedStatement,
    delete: PreparedStatement,
}

impl SessionStore {
    /// Connect to `ScyllaDB`, create the schema if needed, and prepare statements.
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the connection, schema creation, or
    /// statement preparation fails.
    pub async fn connect(
        config: &ScyllaConfig,
        tls_config: Option<Arc<rustls::ClientConfig>>,
    ) -> Result<Self, SessionError> {
        let mut builder = SessionBuilder::new();
        for cp in &config.contact_points {
            builder = builder.known_node(cp);
        }
        if let Some(tls) = tls_config {
            builder = builder.tls_context(Some(tls));
        }

        let session = Box::pin(builder.build()).await?;

        Self::create_schema(&session, &config.keyspace).await?;

        session.use_keyspace(&config.keyspace, false).await?;

        let insert = session
            .prepare(
                "INSERT INTO sessions \
                 (token_hash, user_id, access_token, id_token, refresh_token, expires_at, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .await?;

        let select = session
            .prepare(
                "SELECT token_hash, user_id, access_token, id_token, refresh_token, expires_at, created_at \
                 FROM sessions WHERE token_hash = ?",
            )
            .await?;

        let delete = session
            .prepare("DELETE FROM sessions WHERE token_hash = ?")
            .await?;

        Ok(Self {
            session,
            insert,
            select,
            delete,
        })
    }

    async fn create_schema(session: &ScyllaSession, keyspace: &str) -> Result<(), SessionError> {
        session
            .query_unpaged(
                format!(
                    "CREATE KEYSPACE IF NOT EXISTS {keyspace} \
                     WITH replication = {{'class': 'NetworkTopologyStrategy', 'replication_factor': 1}}"
                ),
                (),
            )
            .await
            .map_err(Box::new)?;

        // Drop and recreate: schema changed from uuid PK to blob PK.
        // Safe pre-production; remove once deployed.
        session
            .query_unpaged(format!("DROP TABLE IF EXISTS {keyspace}.sessions"), ())
            .await
            .map_err(Box::new)?;

        session
            .query_unpaged(
                format!(
                    "CREATE TABLE IF NOT EXISTS {keyspace}.sessions ( \
                     token_hash blob PRIMARY KEY, \
                     user_id uuid, \
                     access_token text, \
                     id_token text, \
                     refresh_token text, \
                     expires_at timestamp, \
                     created_at timestamp \
                     ) WITH default_time_to_live = 604800"
                ),
                (),
            )
            .await
            .map_err(Box::new)?;

        Ok(())
    }

    /// Create a new session, returning the cleartext session token for the cookie.
    ///
    /// The token is generated from 32 bytes of CSPRNG output, base64url-encoded.
    /// Only the SHA-256 hash is stored in `ScyllaDB`.
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if token generation or the insert query fails.
    pub async fn create_session(
        &self,
        user_id: Uuid,
        access_token: String,
        id_token: String,
        refresh_token: String,
        expires_at: DateTime<Utc>,
    ) -> Result<String, SessionError> {
        let (cleartext, token_hash) = generate_session_token()?;
        let created_at = Utc::now();

        self.session
            .execute_unpaged(
                &self.insert,
                (
                    &token_hash,
                    user_id,
                    &access_token,
                    &id_token,
                    &refresh_token,
                    expires_at,
                    created_at,
                ),
            )
            .await
            .map_err(Box::new)?;

        Ok(cleartext)
    }

    /// Look up a session by token hash. Returns `None` if not found or expired (TTL).
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the select query or row deserialization fails.
    pub async fn get_session(&self, token_hash: Vec<u8>) -> Result<Option<Session>, SessionError> {
        let result = self
            .session
            .execute_unpaged(&self.select, (&token_hash,))
            .await
            .map_err(Box::new)?;

        let rows_result = result.into_rows_result().map_err(Box::new)?;

        let row = rows_result.maybe_first_row::<(
            Vec<u8>,
            Uuid,
            String,
            String,
            String,
            DateTime<Utc>,
            DateTime<Utc>,
        )>()?;

        let Some((hash, user_id, access_token, id_token, refresh_token, expires_at, created_at)) =
            row
        else {
            return Ok(None);
        };

        Ok(Some(Session {
            token_hash: hash,
            user_id,
            access_token,
            id_token,
            refresh_token,
            expires_at,
            created_at,
        }))
    }

    /// Delete a session (logout).
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the delete query fails.
    pub async fn delete_session(&self, token_hash: &[u8]) -> Result<(), SessionError> {
        self.session
            .execute_unpaged(&self.delete, (token_hash,))
            .await
            .map_err(Box::new)?;
        Ok(())
    }
}

/// Decode the `sub` claim from a JWT access token without cryptographic
/// verification. Safe because we trust the Identity service over mTLS.
///
/// # Errors
///
/// Returns `SessionError::JwtDecode` if the token is malformed, the payload
/// cannot be decoded, or the `sub` claim is missing or not a valid UUID.
pub fn decode_sub_from_jwt(token: &str) -> Result<Uuid, SessionError> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| SessionError::JwtDecode("missing payload segment".into()))?;

    let bytes = Base64UrlUnpadded::decode_vec(payload)
        .map_err(|e| SessionError::JwtDecode(format!("base64 decode: {e}")))?;

    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| SessionError::JwtDecode(format!("json parse: {e}")))?;

    let sub = value["sub"]
        .as_str()
        .ok_or_else(|| SessionError::JwtDecode("missing sub claim".into()))?;

    sub.parse::<Uuid>()
        .map_err(|e| SessionError::JwtDecode(format!("sub is not a UUID: {e}")))
}
