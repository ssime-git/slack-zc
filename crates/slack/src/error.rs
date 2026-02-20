use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    Auth(String),
    
    #[error("Rate limited. Retry after {retry_after}s")]
    RateLimited { retry_after: u64 },
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("API error: {0}")]
    Api(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
}

impl ApiError {
    pub fn user_message(&self) -> &'static str {
        match self {
            ApiError::Auth(_) => "Authentication failed. Please re-authenticate.",
            ApiError::RateLimited { .. } => "Rate limited. Please slow down.",
            ApiError::Network(_) => "Network error. Check your connection.",
            ApiError::Validation(_) => "Invalid input. Please check your message.",
            ApiError::Api(_) => "Server error. Please try again later.",
            ApiError::Timeout(_) => "Request timed out. Please try again.",
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, ApiError::RateLimited { .. } | ApiError::Network(_) | ApiError::Timeout(_))
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

pub fn map_anyhow_error(e: anyhow::Error) -> ApiError {
    let msg = e.to_string();
    if msg.contains("429") || msg.contains("rate_limit") {
        ApiError::RateLimited { retry_after: 60 }
    } else if msg.contains("not_authed") || msg.contains("invalid_auth") || msg.contains("token") {
        ApiError::Auth(msg)
    } else if msg.contains("timeout") || msg.contains("timed out") {
        ApiError::Timeout(msg)
    } else if msg.contains("validation") || msg.contains("invalid") {
        ApiError::Validation(msg)
    } else {
        ApiError::Api(msg)
    }
}
