use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Command to refresh all analytics materialized views
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RefreshViewsCommand {
    /// Optional specific view to refresh (if None, refreshes all)
    pub view_name: Option<String>,
    /// Whether to use CONCURRENTLY (slower but doesn't block reads)
    pub concurrent: bool,
}

impl Default for RefreshViewsCommand {
    fn default() -> Self {
        Self {
            view_name: None,
            concurrent: true,
        }
    }
}

pub type RefreshViewsResult = Result<RefreshViewsResponse, RefreshViewsError>;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RefreshViewsResponse {
    pub refreshed_views: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, thiserror::Error)]
pub enum RefreshViewsError {
    #[error("Database error: {message}")]
    Database { message: String },
    #[error("View not found: {view_name}")]
    ViewNotFound { view_name: String },
}