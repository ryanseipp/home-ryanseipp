use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, Router, routing::post};
use serde::Deserialize;

use super::AppState;
use super::error::AppError;
use crate::proto::identity::v1::{
    SignUpRequest as ProtoSignUpRequest, VerifyEmailRequest as ProtoVerifyEmailRequest,
};

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

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sign-up", post(sign_up))
        .route("/api/v1/verify-email", post(verify_email))
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
