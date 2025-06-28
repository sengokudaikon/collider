use std::fmt;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiErrorResponse {
    pub error: ApiErrorInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiErrorInfo {
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug)]
pub enum AppError {
    BadRequest {
        code: String,
        message: String,
        details: Option<String>,
    },
    NotFound {
        code: String,
        message: String,
        details: Option<String>,
    },
    UnprocessableEntity {
        code: String,
        message: String,
        details: Option<String>,
    },
    InternalServerError {
        code: String,
        message: String,
        details: Option<String>,
    },
}

impl AppError {
    pub fn bad_request(code: &str, message: &str) -> Self {
        Self::BadRequest {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn bad_request_with_details(
        code: &str, message: &str, details: &str,
    ) -> Self {
        Self::BadRequest {
            code: code.to_string(),
            message: message.to_string(),
            details: Some(details.to_string()),
        }
    }

    pub fn not_found(code: &str, message: &str) -> Self {
        Self::NotFound {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn unprocessable_entity(code: &str, message: &str) -> Self {
        Self::UnprocessableEntity {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn internal_server_error(message: &str) -> Self {
        Self::InternalServerError {
            code: "INTERNAL_ERROR".to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn from_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        // Handle specific error types with appropriate HTTP status codes
        let error_str = err.to_string();

        // Check for common database connection errors
        if error_str.contains("connection") || error_str.contains("pool") {
            return Self::internal_server_error(&format!(
                "Database connection error: {}",
                err
            ));
        }

        // Check for query parameter validation errors
        if error_str.contains("deserialize")
            || error_str.contains("invalid characters")
        {
            return Self::bad_request_with_details(
                "INVALID_QUERY_PARAMS",
                "Invalid query parameters provided",
                &error_str,
            );
        }

        // Check for UUID parsing errors
        if error_str.contains("invalid character")
            && error_str.contains("uuid")
        {
            return Self::bad_request(
                "INVALID_UUID",
                "Invalid UUID format provided",
            );
        }

        // Default to internal server error for unknown errors
        Self::internal_server_error(&format!(
            "An unexpected error occurred: {}",
            err
        ))
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::UnprocessableEntity { .. } => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::InternalServerError { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn to_response_data(&self) -> ApiErrorResponse {
        let (code, message, details) = match self {
            Self::BadRequest {
                code,
                message,
                details,
            } => (code, message, details),
            Self::NotFound {
                code,
                message,
                details,
            } => (code, message, details),
            Self::UnprocessableEntity {
                code,
                message,
                details,
            } => (code, message, details),
            Self::InternalServerError {
                code,
                message,
                details,
            } => (code, message, details),
        };

        ApiErrorResponse {
            error: ApiErrorInfo {
                code: code.clone(),
                message: message.clone(),
                details: details.clone(),
            },
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest { message, .. } => write!(f, "{}", message),
            Self::NotFound { message, .. } => write!(f, "{}", message),
            Self::UnprocessableEntity { message, .. } => {
                write!(f, "{}", message)
            }
            Self::InternalServerError { message, .. } => {
                write!(f, "{}", message)
            }
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let response_data = self.to_response_data();
        (status, Json(response_data)).into_response()
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::internal_server_error(&format!(
            "An unexpected error occurred: {}",
            err
        ))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::internal_server_error(&format!(
            "An unexpected error occurred: {}",
            err
        ))
    }
}

pub type AppResult<T> = Result<T, AppError>;
