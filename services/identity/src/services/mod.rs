pub mod auth;
pub mod get_jwks;
pub mod login;
pub mod resend_verification;
pub mod sign_up;
pub mod update_password;
pub mod update_profile;
pub mod user_info;
pub mod validation;
pub mod verify_email;

use std::sync::Arc;

use sqlx::{PgConnection, PgPool};
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::crypto;
use crate::crypto::Kek;
use crate::crypto::encryption::EncryptedKey;
use crate::db::DatabasePool;
use crate::proto::{
    GetJwksRequest, GetJwksResponse, LoginRequest, LoginResponse, ResendVerificationRequest,
    ResendVerificationResponse, SignUpRequest, SignUpResponse, UpdatePasswordRequest,
    UpdatePasswordResponse, UpdateProfileRequest, UpdateProfileResponse, UserInfoRequest,
    UserInfoResponse, VerifyEmailRequest, VerifyEmailResponse,
    identity_service_server::IdentityService,
};

/// Application state shared across all RPC handlers.
pub struct IdentityServiceImpl {
    db: DatabasePool,
    web_base_url: String,
    kek: Arc<Kek>,
}

impl IdentityServiceImpl {
    pub fn new(db: DatabasePool, web_base_url: String, kek: Arc<Kek>) -> Self {
        Self {
            db,
            web_base_url,
            kek,
        }
    }
}

#[tonic::async_trait]
impl IdentityService for IdentityServiceImpl {
    async fn sign_up(
        &self,
        request: Request<SignUpRequest>,
    ) -> Result<Response<SignUpResponse>, Status> {
        let req = request.into_inner();

        sign_up::execute(
            self.db.writer(),
            &req.username,
            &req.email,
            &req.given_name,
            &req.family_name,
            req.password.as_deref(),
            &self.web_base_url,
        )
        .await
        .map_err(|e| match &e {
            sign_up::SignUpError::UsernameTaken => Status::already_exists("username already taken"),
            sign_up::SignUpError::EmailTaken => Status::already_exists("email already taken"),
            sign_up::SignUpError::PasswordRequired
            | sign_up::SignUpError::PasswordTooShort
            | sign_up::SignUpError::PasswordTooLong
            | sign_up::SignUpError::InvalidUsername
            | sign_up::SignUpError::InvalidEmail => Status::invalid_argument(e.to_string()),
            other => {
                tracing::error!(error = %other, "sign_up failed");
                Status::internal("internal error")
            }
        })?;

        Ok(Response::new(SignUpResponse {}))
    }

    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let req = request.into_inner();

        let result = login::execute(self.db.writer(), &req.email, &req.password, &self.kek)
            .await
            .map_err(|e| match &e {
                login::LoginError::InvalidCredentials => {
                    Status::unauthenticated("invalid credentials")
                }
                login::LoginError::AccountNotActive => {
                    Status::permission_denied("account not active")
                }
                other => {
                    tracing::error!(error = %other, "login failed");
                    Status::internal("internal error")
                }
            })?;

        let expires_at = prost_types::Timestamp {
            seconds: result.expires_at.timestamp(),
            nanos: result.expires_at.timestamp_subsec_nanos() as i32,
        };

        Ok(Response::new(LoginResponse {
            access_token: result.access_token,
            refresh_token: result.refresh_token,
            id_token: result.id_token,
            expires_at: Some(expires_at),
        }))
    }

    async fn user_info(
        &self,
        request: Request<UserInfoRequest>,
    ) -> Result<Response<UserInfoResponse>, Status> {
        let user_id = auth::authenticate(&request, self.db.reader()).await?;

        let user = user_info::execute(self.db.reader(), user_id)
            .await
            .map_err(|e| match &e {
                user_info::UserInfoError::NotFound => Status::not_found("user not found"),
                other => {
                    tracing::error!(error = %other, "user_info failed");
                    Status::internal("internal error")
                }
            })?;

        let updated_at = prost_types::Timestamp {
            seconds: user.updated_at.timestamp(),
            nanos: user.updated_at.timestamp_subsec_nanos() as i32,
        };

        Ok(Response::new(UserInfoResponse {
            sub: user.id.to_string(),
            name: format!("{} {}", user.given_name, user.family_name),
            given_name: user.given_name,
            family_name: user.family_name,
            username: user.username,
            email: user.email,
            email_verified: user.email_verified,
            updated_at: Some(updated_at),
        }))
    }

    async fn verify_email(
        &self,
        request: Request<VerifyEmailRequest>,
    ) -> Result<Response<VerifyEmailResponse>, Status> {
        let req = request.into_inner();

        verify_email::execute(self.db.writer(), &req.token)
            .await
            .map_err(|e| match &e {
                verify_email::VerifyEmailError::InvalidToken => {
                    Status::not_found("invalid or expired verification token")
                }
                verify_email::VerifyEmailError::AlreadyConsumed => {
                    Status::already_exists("token already consumed")
                }
                verify_email::VerifyEmailError::UserNotFound => Status::not_found("user not found"),
                verify_email::VerifyEmailError::AlreadyVerified => {
                    Status::already_exists("email already verified")
                }
                other => {
                    tracing::error!(error = %other, "verify_email failed");
                    Status::internal("internal error")
                }
            })?;

        Ok(Response::new(VerifyEmailResponse {}))
    }

    async fn resend_verification(
        &self,
        request: Request<ResendVerificationRequest>,
    ) -> Result<Response<ResendVerificationResponse>, Status> {
        let req = request.into_inner();

        resend_verification::execute(self.db.writer(), &req.email, &self.web_base_url)
            .await
            .map_err(|e| match &e {
                resend_verification::ResendVerificationError::RateLimited => {
                    Status::resource_exhausted(e.to_string())
                }
                other => {
                    tracing::error!(error = %other, "resend_verification failed");
                    Status::internal("internal error")
                }
            })?;

        // Deliberately return OK regardless of whether the email exists
        Ok(Response::new(ResendVerificationResponse {}))
    }

    async fn get_jwks(
        &self,
        _request: Request<GetJwksRequest>,
    ) -> Result<Response<GetJwksResponse>, Status> {
        let json = get_jwks::execute(self.db.reader())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetJwksResponse { keys: json }))
    }

    async fn update_profile(
        &self,
        request: Request<UpdateProfileRequest>,
    ) -> Result<Response<UpdateProfileResponse>, Status> {
        let user_id = auth::authenticate(&request, self.db.reader()).await?;
        let req = request.into_inner();

        update_profile::execute(
            self.db.writer(),
            user_id,
            req.given_name.as_deref(),
            req.family_name.as_deref(),
            req.username.as_deref(),
        )
        .await
        .map_err(|e| match &e {
            update_profile::UpdateProfileError::NoFieldsProvided => {
                Status::invalid_argument("no fields provided")
            }
            update_profile::UpdateProfileError::InvalidUsername(_) => {
                Status::invalid_argument(e.to_string())
            }
            update_profile::UpdateProfileError::UsernameTaken => {
                Status::already_exists("username already taken")
            }
            other => {
                tracing::error!(error = %other, "update_profile failed");
                Status::internal("internal error")
            }
        })?;

        Ok(Response::new(UpdateProfileResponse {}))
    }

    async fn update_password(
        &self,
        request: Request<UpdatePasswordRequest>,
    ) -> Result<Response<UpdatePasswordResponse>, Status> {
        let user_id = auth::authenticate(&request, self.db.reader()).await?;
        let req = request.into_inner();

        update_password::execute(
            self.db.writer(),
            user_id,
            &req.current_password,
            &req.new_password,
        )
        .await
        .map_err(|e| match &e {
            update_password::UpdatePasswordError::InvalidCurrentPassword => {
                Status::unauthenticated("invalid current password")
            }
            update_password::UpdatePasswordError::InvalidNewPassword(_) => {
                Status::invalid_argument(e.to_string())
            }
            update_password::UpdatePasswordError::NotFound => Status::not_found("user not found"),
            other => {
                tracing::error!(error = %other, "update_password failed");
                Status::internal("internal error")
            }
        })?;

        Ok(Response::new(UpdatePasswordResponse {}))
    }
}

/// Status of a user account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    PendingVerification,
    Active,
    Suspended,
    Locked,
    Deleted,
}

impl UserStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserStatus::PendingVerification => "pending_verification",
            UserStatus::Active => "active",
            UserStatus::Suspended => "suspended",
            UserStatus::Locked => "locked",
            UserStatus::Deleted => "deleted",
        }
    }
}

impl std::str::FromStr for UserStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending_verification" => Ok(UserStatus::PendingVerification),
            "active" => Ok(UserStatus::Active),
            "suspended" => Ok(UserStatus::Suspended),
            "locked" => Ok(UserStatus::Locked),
            "deleted" => Ok(UserStatus::Deleted),
            _ => Err(format!("invalid user status: {s}")),
        }
    }
}

/// A stored user account.
#[derive(Clone)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub given_name: String,
    pub family_name: String,
    pub password_hash: String,
    pub email_verified: bool,
    pub status: UserStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("username", &self.username)
            .field("email", &self.email)
            .field("given_name", &self.given_name)
            .field("family_name", &self.family_name)
            .field("password_hash", &"[REDACTED]")
            .field("email_verified", &self.email_verified)
            .field("status", &self.status)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

pub(crate) struct UserRow {
    id: Uuid,
    username: String,
    email: String,
    given_name: String,
    family_name: String,
    password_hash: String,
    email_verified: bool,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<UserRow> for User {
    type Error = String;

    fn try_from(row: UserRow) -> Result<Self, Self::Error> {
        Ok(User {
            id: row.id,
            username: row.username,
            email: row.email,
            given_name: row.given_name,
            family_name: row.family_name,
            password_hash: row.password_hash,
            email_verified: row.email_verified,
            status: row.status.parse()?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Insert a verification token within an existing transaction.
pub(crate) async fn insert_token(
    conn: &mut sqlx::PgConnection,
    id: Uuid,
    user_id: Uuid,
    token_hash: &[u8],
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO email_verification_tokens (id, user_id, token_hash, expires_at)
         VALUES ($1, $2, $3, $4)",
        id,
        user_id,
        token_hash,
        expires_at,
    )
    .execute(conn)
    .await?;

    Ok(())
}

/// Extract the current OTEL trace ID and span ID for outbox span links.
pub(crate) fn extract_trace_context() -> (Option<String>, Option<String>) {
    use opentelemetry::trace::TraceContextExt;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let span = tracing::Span::current();
    let context = span.context();
    let otel_span = context.span();
    let span_context = otel_span.span_context();

    if span_context.is_valid() {
        (
            Some(span_context.trace_id().to_string()),
            Some(span_context.span_id().to_string()),
        )
    } else {
        (None, None)
    }
}

/// Look up a user by ID. Only returns non-deleted users.
pub(crate) async fn get_user_by_id(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as!(
        UserRow,
        "SELECT id, username, email, given_name, family_name, password_hash,
                email_verified, status, created_at, updated_at
         FROM users
         WHERE id = $1 AND deleted_at IS NULL",
        id,
    )
    .fetch_optional(conn)
    .await?;

    match row {
        Some(r) => Ok(Some(
            User::try_from(r).map_err(|e| sqlx::Error::Decode(e.into()))?,
        )),
        None => Ok(None),
    }
}

/// Look up a user by email (case-insensitive). Only returns non-deleted users.
pub(crate) async fn get_user_by_email(
    conn: &mut PgConnection,
    email: &str,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as!(
        UserRow,
        "SELECT id, username, email, given_name, family_name, password_hash,
                email_verified, status, created_at, updated_at
         FROM users
         WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL",
        email,
    )
    .fetch_optional(conn)
    .await?;

    match row {
        Some(r) => Ok(Some(
            User::try_from(r).map_err(|e| sqlx::Error::Decode(e.into()))?,
        )),
        None => Ok(None),
    }
}

/// A decrypted signing key ready for JWT signing.
pub(crate) struct ActiveSigningKey {
    pub kid: String,
    pub algorithm: crypto::Algorithm,
    pub key_pair: aws_lc_rs::signature::EcdsaKeyPair,
}

struct SigningKeyRow {
    kid: String,
    algorithm: String,
    encrypted_private_key: Vec<u8>,
    #[allow(dead_code)]
    public_jwk: serde_json::Value,
}

/// Fetch the most recent active signing key, decrypt it, and return it ready for signing.
pub(crate) async fn get_active_signing_key(
    pool: &PgPool,
    kek: &Kek,
) -> Result<ActiveSigningKey, crypto::CryptoError> {
    let row = sqlx::query_as!(
        SigningKeyRow,
        "SELECT kid, algorithm, encrypted_private_key, public_jwk
         FROM signing_keys
         WHERE status = 'active'
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| crypto::CryptoError::KeyNotFound)?
    .ok_or(crypto::CryptoError::KeyNotFound)?;

    let algorithm: crypto::Algorithm = row.algorithm.parse()?;
    let encrypted = EncryptedKey::from_bytes(row.encrypted_private_key)?;
    let pkcs8_der =
        crypto::encryption::decrypt_private_key(kek.as_bytes(), &encrypted, row.kid.as_bytes())?;
    let key_pair = crypto::jwt::key_pair_from_pkcs8(algorithm, &pkcs8_der)?;

    Ok(ActiveSigningKey {
        kid: row.kid,
        algorithm,
        key_pair,
    })
}

/// Ensure at least one active signing key exists in the database.
///
/// Generates an ES256 key pair, encrypts the private key with the KEK,
/// and stores it. No-op if an active key already exists.
pub async fn ensure_signing_key(
    pool: &PgPool,
    kek: &Kek,
) -> Result<(), Box<dyn std::error::Error>> {
    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM signing_keys WHERE status = 'active'")
        .fetch_one(pool)
        .await?;

    if count.unwrap_or(0) > 0 {
        tracing::info!("active signing key already exists");
        return Ok(());
    }

    let generated = crypto::jwk::generate_key_pair(crypto::Algorithm::ES256)?;
    let encrypted = crypto::encryption::encrypt_private_key(
        kek.as_bytes(),
        generated.private_key_pkcs8.as_bytes(),
        generated.kid.as_bytes(),
    )?;
    let public_jwk_json = serde_json::to_value(&generated.public_jwk)?;

    sqlx::query!(
        "INSERT INTO signing_keys (kid, algorithm, encrypted_private_key, public_jwk, status)
         VALUES ($1, $2, $3, $4, 'active')",
        generated.kid,
        generated.algorithm.as_str(),
        encrypted.as_bytes(),
        public_jwk_json,
    )
    .execute(pool)
    .await?;

    tracing::info!(kid = %generated.kid, "auto-created signing key");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_redacts_password_hash() {
        let user = User {
            id: Uuid::nil(),
            username: "testuser".into(),
            email: "test@example.com".into(),
            given_name: "Test".into(),
            family_name: "User".into(),
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$secret_salt$secret_hash".into(),
            email_verified: false,
            status: UserStatus::Active,
            created_at: chrono::DateTime::UNIX_EPOCH,
            updated_at: chrono::DateTime::UNIX_EPOCH,
        };

        let debug_output = format!("{:?}", user);
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("secret_salt"));
        assert!(!debug_output.contains("secret_hash"));
    }
}
