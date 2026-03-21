use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::pool::ChannelError;

/// Maps gRPC or channel errors to HTTP responses.
///
/// Internal errors (unknown codes, `Internal`) return 500 with a generic
/// message to avoid leaking implementation details.
pub enum AppError {
    Grpc(tonic::Status),
    Channel(ChannelError),
}

impl From<tonic::Status> for AppError {
    fn from(status: tonic::Status) -> Self {
        Self::Grpc(status)
    }
}

impl From<ChannelError> for AppError {
    fn from(err: ChannelError) -> Self {
        Self::Channel(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Channel(err) => {
                tracing::error!(error = ?err, "failed to get identity client");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service temporarily unavailable".to_string(),
                )
            }
            AppError::Grpc(s) => {
                tracing::error!(
                    code = ?s.code(),
                    message = %s.message(),
                    "gRPC error from identity service"
                );
                match s.code() {
                    tonic::Code::InvalidArgument => {
                        (StatusCode::BAD_REQUEST, s.message().to_string())
                    }
                    tonic::Code::AlreadyExists => (StatusCode::CONFLICT, s.message().to_string()),
                    tonic::Code::NotFound => (StatusCode::NOT_FOUND, s.message().to_string()),
                    tonic::Code::Unauthenticated => {
                        (StatusCode::UNAUTHORIZED, s.message().to_string())
                    }
                    tonic::Code::PermissionDenied => {
                        (StatusCode::FORBIDDEN, s.message().to_string())
                    }
                    tonic::Code::Unavailable => {
                        (StatusCode::SERVICE_UNAVAILABLE, s.message().to_string())
                    }
                    _ => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal server error".to_string(),
                    ),
                }
            }
        };

        (status, axum::Json(json!({ "error": message }))).into_response()
    }
}
