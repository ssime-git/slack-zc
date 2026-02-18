use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

#[derive(Clone)]
pub struct GatewayClient {
    http: Client,
    base_url: String,
    bearer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairResponse {
    pub token: String,
}

impl GatewayClient {
    pub fn new(port: u16) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            http,
            base_url: format!("http://localhost:{}", port),
            bearer: None,
        }
    }

    pub fn with_bearer(mut self, token: String) -> Self {
        self.bearer = Some(token);
        self
    }

    pub async fn pair(&mut self, code: &str) -> Result<String> {
        let response = self
            .http
            .post(format!("{}/pair", self.base_url))
            .header("X-Pairing-Code", code)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Pairing failed: {}", response.status()));
        }

        let data: PairResponse = response.json().await?;
        info!("Successfully paired with ZeroClaw gateway");
        self.bearer = Some(data.token.clone());
        Ok(data.token)
    }

    pub async fn health_check(&self) -> Result<bool> {
        let mut request = self.http.get(format!("{}/health", self.base_url));

        if let Some(ref bearer) = self.bearer {
            request = request.header("Authorization", format!("Bearer {}", bearer));
        }

        match request.send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => {
                debug!("Health check failed: {}", e);
                Ok(false)
            }
        }
    }

    pub async fn send_to_agent(&self, payload: &serde_json::Value) -> Result<String> {
        let bearer = self
            .bearer
            .as_ref()
            .ok_or_else(|| anyhow!("Not paired with gateway"))?;

        let response = self
            .http
            .post(format!("{}/webhook", self.base_url))
            .header("Authorization", format!("Bearer {}", bearer))
            .json(payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Webhook failed: {}", response.status()));
        }

        let text = response.text().await?;
        Ok(text)
    }

    pub fn is_paired(&self) -> bool {
        self.bearer.is_some()
    }
}
