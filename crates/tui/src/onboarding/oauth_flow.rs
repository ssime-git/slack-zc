pub struct OAuthFlowState {
    pub status: OAuthStatus,
    pub auth_url: String,
    pub code: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OAuthStatus {
    WaitingForBrowser,
    WaitingForCallback,
    ExchangingToken,
    Success,
    Error,
}

impl Default for OAuthFlowState {
    fn default() -> Self {
        Self {
            status: OAuthStatus::WaitingForBrowser,
            auth_url: String::new(),
            code: None,
            error: None,
        }
    }
}
