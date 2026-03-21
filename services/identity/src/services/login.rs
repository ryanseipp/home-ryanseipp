use chrono::{Duration, Utc};
use deadpool_postgres::Pool;
use tokio::task;
use uuid::Uuid;

use crate::crypto;
use crate::crypto::Kek;
use crate::crypto::jwt::{Audience, Claims};
use crate::crypto::password::PasswordError;

use super::{UserStatus, get_active_signing_key, get_user_by_email};

/// Access token lifetime: 15 minutes.
const ACCESS_TOKEN_LIFETIME_SECS: i64 = 900;

/// ID token lifetime: same as access token (15 minutes).
const ID_TOKEN_LIFETIME_SECS: i64 = 900;

/// Refresh token lifetime: 7 days.
const REFRESH_TOKEN_LIFETIME_DAYS: i64 = 7;

/// JWT issuer claim.
const ISSUER: &str = "https://home.ryanseipp.com";

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("database error")]
    Db(#[from] tokio_postgres::Error),

    #[error("pool error")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("account not active")]
    AccountNotActive,

    #[error("crypto error")]
    Crypto(#[from] crypto::CryptoError),

    #[error("password error")]
    Password(#[from] PasswordError),

    #[error("no signing key available")]
    NoSigningKey,
}

/// Result of a successful login.
pub struct LoginResult {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_at: chrono::DateTime<Utc>,
}

/// Execute user login by email and password.
///
/// 1. Look up user by email
/// 2. Verify password (on blocking thread)
/// 3. Check user status is Active
/// 4. Fetch and decrypt active signing key
/// 5. Generate access_token (JWT), id_token (JWT), refresh_token (opaque)
/// 6. Store refresh_token hash in database
/// 7. Return all three tokens
#[tracing::instrument(skip(pool, password, kek))]
pub async fn execute(
    pool: &Pool,
    email: &str,
    password: &str,
    kek: &Kek,
) -> Result<LoginResult, LoginError> {
    // 1. Look up user by email
    let client = pool.get().await?;
    let user = get_user_by_email(&client, email)
        .await?
        .ok_or(LoginError::InvalidCredentials)?;
    drop(client);

    // 2. Verify password on blocking thread
    let password_hash = user.password_hash.clone();
    let password_owned = password.to_string();
    task::spawn_blocking(move || crypto::verify_password(&password_owned, &password_hash))
        .await
        .map_err(|_| PasswordError::VerificationFailed)?
        .map_err(|_| LoginError::InvalidCredentials)?;

    // 3. Check user status
    if user.status != UserStatus::Active {
        return Err(LoginError::AccountNotActive);
    }

    // 4. Fetch active signing key
    let signing_key = get_active_signing_key(pool, kek)
        .await
        .map_err(|_| LoginError::NoSigningKey)?;

    // 5. Build tokens
    let now = Utc::now();
    let now_secs = now.timestamp() as u64;
    let access_exp = now + Duration::seconds(ACCESS_TOKEN_LIFETIME_SECS);
    let id_exp = now + Duration::seconds(ID_TOKEN_LIFETIME_SECS);
    let refresh_exp = now + Duration::days(REFRESH_TOKEN_LIFETIME_DAYS);

    // Access token: minimal claims for authorization
    let access_claims = Claims {
        iss: Some(ISSUER.to_string()),
        sub: Some(user.id.to_string()),
        aud: Some(Audience::Single(
            "https://api.home.ryanseipp.com".to_string(),
        )),
        exp: Some(access_exp.timestamp() as u64),
        nbf: Some(now_secs),
        iat: Some(now_secs),
        jti: Some(Uuid::new_v4().to_string()),
        ..Default::default()
    };

    let access_jwt = crypto::jwt::sign_jwt(
        signing_key.algorithm,
        &signing_key.kid,
        &access_claims,
        &signing_key.key_pair,
    )?;

    // ID token: OIDC-style with user profile claims
    let id_claims = Claims {
        iss: Some(ISSUER.to_string()),
        sub: Some(user.id.to_string()),
        aud: Some(Audience::Single(
            "https://api.home.ryanseipp.com".to_string(),
        )),
        exp: Some(id_exp.timestamp() as u64),
        nbf: Some(now_secs),
        iat: Some(now_secs),
        jti: Some(Uuid::new_v4().to_string()),
        name: Some(format!("{} {}", user.given_name, user.family_name)),
        given_name: Some(user.given_name.clone()),
        family_name: Some(user.family_name.clone()),
        email: Some(user.email.clone()),
        email_verified: Some(user.email_verified),
        preferred_username: Some(user.username.clone()),
        ..Default::default()
    };

    let id_jwt = crypto::jwt::sign_jwt(
        signing_key.algorithm,
        &signing_key.kid,
        &id_claims,
        &signing_key.key_pair,
    )?;

    // Refresh token: opaque random string, store hash in DB
    let (refresh_token_string, refresh_token_hash) = crypto::token::generate_verification_token()?;

    // 6. Store refresh token hash
    let refresh_token_id = Uuid::now_v7();
    insert_refresh_token(
        pool,
        refresh_token_id,
        user.id,
        &refresh_token_hash,
        refresh_exp,
    )
    .await?;

    tracing::info!(user_id = %user.id, "user logged in");

    Ok(LoginResult {
        access_token: access_jwt.token,
        refresh_token: refresh_token_string,
        id_token: id_jwt.token,
        expires_at: access_exp,
    })
}

/// Insert a refresh token hash into the database.
async fn insert_refresh_token(
    pool: &Pool,
    id: Uuid,
    user_id: Uuid,
    token_hash: &[u8],
    expires_at: chrono::DateTime<Utc>,
) -> Result<(), LoginError> {
    let client = pool.get().await?;
    client
        .execute(
            "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at)
             VALUES ($1, $2, $3, $4)",
            &[&id, &user_id, &token_hash, &expires_at],
        )
        .await?;

    Ok(())
}
