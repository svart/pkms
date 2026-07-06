use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    detail: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    detail: String,
}

impl ApiError {
    pub(super) fn not_found(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            detail: detail.into(),
        }
    }

    pub(super) fn bad_request(detail: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            detail,
        }
    }

    pub(super) fn json_rejection(rejection: JsonRejection) -> Self {
        Self::bad_request(rejection.body_text())
    }

    pub(super) fn internal(operation: &'static str, err: anyhow::Error) -> Self {
        tracing::error!(
            event = "rag_api_request_failed",
            operation,
            error = %err,
            "RAG API request failed"
        );
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            detail: err.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                detail: self.detail,
            }),
        )
            .into_response()
    }
}
