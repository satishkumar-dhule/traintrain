use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// A CAPTCHA challenge surfaced by an upstream source.
#[derive(Debug, Clone, Serialize)]
pub struct CaptchaContext {
    pub text: Option<String>,
    pub session_id: Option<String>,
}

/// Error carrying a CAPTCHA challenge back to the client (HTTP 428).
#[derive(Debug, Clone)]
pub struct CaptchaRequiredError {
    pub source: String,
    pub image: String,
    pub session_id: String,
    pub message: String,
}

impl CaptchaRequiredError {
    pub fn new(
        source: impl Into<String>,
        image: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        let source = source.into();
        Self {
            message: format!("Captcha required by {source}"),
            source: source.clone(),
            image: image.into(),
            session_id: session_id.into(),
        }
    }
}

#[derive(Debug)]
pub enum AppError {
    /// Client supplied a bad query. HTTP 400.
    BadRequest(String),
    /// Resource does not exist / could not be resolved upstream. HTTP 404.
    NotFound(String),
    /// A live upstream source is unreachable or failed. HTTP 502.
    SourceUnavailable { source: String, reason: String },
    /// CAPTCHA challenge from a source. HTTP 428.
    CaptchaRequired(CaptchaRequiredError),
    /// Any other server-side failure. HTTP 500.
    Internal(String),
}

impl AppError {
    pub fn source_unavailable(source: impl Into<String>, reason: impl Into<String>) -> Self {
        AppError::SourceUnavailable {
            source: source.into(),
            reason: reason.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        AppError::Internal(msg.into())
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        AppError::BadRequest(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError::NotFound(msg.into())
    }

    /// Human readable summary, used in logs and as the `error` field.
    pub fn message(&self) -> String {
        match self {
            AppError::BadRequest(m) => m.clone(),
            AppError::NotFound(m) => m.clone(),
            AppError::SourceUnavailable { source, reason } => {
                format!("Live source {source} unavailable: {reason}")
            }
            AppError::CaptchaRequired(e) => e.message.clone(),
            AppError::Internal(m) => m.clone(),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for AppError {}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::internal(format!("upstream request failed: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::internal(format!("upstream response could not be decoded: {e}"))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, Json(ErrorBody { error: msg })).into_response()
            }
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, Json(ErrorBody { error: msg })).into_response()
            }
            AppError::SourceUnavailable { source, reason } => {
                let msg = format!("Live source {source} unavailable: {reason}");
                tracing::warn!(%source, %reason, "upstream source unavailable");
                (StatusCode::BAD_GATEWAY, Json(ErrorBody { error: msg })).into_response()
            }
            AppError::Internal(msg) => {
                tracing::error!(%msg, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody { error: msg }),
                )
                    .into_response()
            }
            AppError::CaptchaRequired(e) => {
                let body = CaptchaBody {
                    error: "captcha_required".to_string(),
                    source: e.source,
                    image: e.image,
                    session_id: e.session_id,
                    message: e.message,
                };
                (StatusCode::PRECONDITION_REQUIRED, Json(body)).into_response()
            }
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Serialize)]
struct CaptchaBody {
    error: String,
    source: String,
    image: String,
    session_id: String,
    message: String,
}
