pub struct ZcSetupState {
    pub status: ZcStatus,
    pub api_key: String,
    pub provider: String,
    pub pairing_code: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZcStatus {
    CheckingBinary,
    BinaryNotFound,
    Configuring,
    Pairing,
    Active,
    Error,
}

impl Default for ZcSetupState {
    fn default() -> Self {
        Self {
            status: ZcStatus::CheckingBinary,
            api_key: String::new(),
            provider: "openrouter".to_string(),
            pairing_code: None,
            error: None,
        }
    }
}
