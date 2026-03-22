use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header::SET_COOKIE;
use axum::response::{AppendHeaders, IntoResponse};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use super::AppState;
use super::error::AppError;
use super::session::{AuthSession, clear_session_cookie, set_session_cookie, with_auth};
use crate::proto::identity::v1::{
    LoginRequest as ProtoLoginRequest, ResendVerificationRequest as ProtoResendVerificationRequest,
    SignUpRequest as ProtoSignUpRequest, UpdatePasswordRequest as ProtoUpdatePasswordRequest,
    UpdateProfileRequest as ProtoUpdateProfileRequest, UserInfoRequest as ProtoUserInfoRequest,
    VerifyEmailRequest as ProtoVerifyEmailRequest,
};
use crate::session::{SessionError, decode_sub_from_jwt};

#[derive(Deserialize)]
pub struct SignUpRequest {
    pub username: String,
    pub given_name: String,
    pub family_name: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub expires_at: i64,
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdatePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Deserialize)]
pub struct ResendVerificationRequest {
    pub email: String,
}

#[derive(Serialize)]
pub struct UserInfoResponse {
    pub sub: String,
    pub name: String,
    pub given_name: String,
    pub family_name: String,
    pub username: String,
    pub email: String,
    pub email_verified: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sign-up", post(sign_up))
        .route("/api/v1/verify-email", post(verify_email))
        .route("/api/v1/login", post(login))
        .route("/api/v1/logout", post(logout))
        .route("/api/v1/userinfo", get(user_info))
        .route("/api/v1/profile", patch(update_profile))
        .route("/api/v1/update-password", post(update_password))
        .route("/api/v1/resend-verification", post(resend_verification))
}

async fn sign_up(
    State(state): State<AppState>,
    Json(body): Json<SignUpRequest>,
) -> Result<StatusCode, AppError> {
    let mut client = state.identity.client().await?;

    client
        .sign_up(ProtoSignUpRequest {
            username: body.username,
            given_name: body.given_name,
            family_name: body.family_name,
            email: body.email,
            password: Some(body.password),
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn verify_email(
    State(state): State<AppState>,
    Json(body): Json<VerifyEmailRequest>,
) -> Result<StatusCode, AppError> {
    let mut client = state.identity.client().await?;

    client
        .verify_email(ProtoVerifyEmailRequest { token: body.token })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut client = state.identity.client().await?;

    let resp = client
        .login(ProtoLoginRequest {
            email: body.email,
            password: body.password,
        })
        .await?
        .into_inner();

    let user_id = decode_sub_from_jwt(&resp.access_token).map_err(AppError::Session)?;

    let expires_at_ts = resp.expires_at.as_ref().ok_or_else(|| {
        AppError::Session(SessionError::JwtDecode(
            "missing expires_at in login response".into(),
        ))
    })?;

    let expires_at = chrono::DateTime::from_timestamp(
        expires_at_ts.seconds,
        expires_at_ts.nanos.try_into().unwrap_or(0),
    )
    .ok_or_else(|| {
        AppError::Session(SessionError::JwtDecode(
            "invalid expires_at timestamp".into(),
        ))
    })?;

    let session_token = state
        .sessions
        .create_session(
            user_id,
            resp.access_token,
            resp.id_token,
            resp.refresh_token,
            expires_at,
        )
        .await
        .map_err(AppError::Session)?;

    Ok((
        AppendHeaders([(SET_COOKIE, set_session_cookie(&session_token))]),
        Json(LoginResponse {
            expires_at: expires_at_ts.seconds,
        }),
    ))
}

async fn logout(
    State(state): State<AppState>,
    AuthSession(session): AuthSession,
) -> Result<impl IntoResponse, AppError> {
    state
        .sessions
        .delete_session(&session.token_hash)
        .await
        .map_err(AppError::Session)?;

    Ok((
        StatusCode::NO_CONTENT,
        AppendHeaders([(SET_COOKIE, clear_session_cookie())]),
    ))
}

async fn user_info(
    State(state): State<AppState>,
    AuthSession(session): AuthSession,
) -> Result<Json<UserInfoResponse>, AppError> {
    let mut client = state.identity.client().await?;

    let req = with_auth(&session, ProtoUserInfoRequest {});
    let resp = client.user_info(req).await?.into_inner();

    Ok(Json(UserInfoResponse {
        sub: resp.sub,
        name: resp.name,
        given_name: resp.given_name,
        family_name: resp.family_name,
        username: resp.username,
        email: resp.email,
        email_verified: resp.email_verified,
    }))
}

async fn update_profile(
    State(state): State<AppState>,
    AuthSession(session): AuthSession,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<StatusCode, AppError> {
    let mut client = state.identity.client().await?;

    let req = with_auth(
        &session,
        ProtoUpdateProfileRequest {
            given_name: body.given_name,
            family_name: body.family_name,
            username: body.username,
        },
    );
    client.update_profile(req).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn update_password(
    State(state): State<AppState>,
    AuthSession(session): AuthSession,
    Json(body): Json<UpdatePasswordRequest>,
) -> Result<StatusCode, AppError> {
    let mut client = state.identity.client().await?;

    let req = with_auth(
        &session,
        ProtoUpdatePasswordRequest {
            current_password: body.current_password,
            new_password: body.new_password,
        },
    );
    client.update_password(req).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn resend_verification(
    State(state): State<AppState>,
    Json(body): Json<ResendVerificationRequest>,
) -> Result<StatusCode, AppError> {
    let mut client = state.identity.client().await?;

    client
        .resend_verification(ProtoResendVerificationRequest { email: body.email })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
