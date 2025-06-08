//! Shared HTTP error handling patterns for domain services.
//!
//! This module provides a common `AppError` type that can be used across
//! all domain HTTP handlers to ensure consistent error responses.

use std::fmt;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug)]
pub struct AppError(pub Box<dyn std::error::Error + Send + Sync>);

impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self(err)
    }
}

impl AppError {
    pub fn from_error<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self(Box::new(err))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = StatusCode::INTERNAL_SERVER_ERROR;
        let message = format!("Internal server error: {}", self.0);
        (status, message).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
