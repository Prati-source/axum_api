use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(thiserror::Error, Debug)]
pub enum SyncError {
    #[error("Redis failure: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Database failure: {0}")]
    Postgres(#[from] sqlx::Error),

    #[error("JSON failure: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

impl IntoResponse for SyncError {
    fn into_response(self) -> Response {
        // Log the actual error for the server admin
        tracing::error!("Sync Error: {:?}", self);

        // Map your internal errors to external HTTP statuses
        let (status, error_message) = match self {
            SyncError::Postgres(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            SyncError::Redis(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            SyncError::Json(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            SyncError::Other(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };

        // Return a JSON response or just a status code
        (status, error_message).into_response()
    }
}
