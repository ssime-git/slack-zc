use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
            .user_agent("slack-zc/0.2")
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            http,
            base_url: format!("http://127.0.0.1:{}", port),
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

    pub async fn check_pairing_status(&self) -> Result<bool> {
        let response = self
            .http
            .get(format!("{}/health", self.base_url))
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(false);
        }

        #[derive(Deserialize)]
        struct HealthResponse {
            paired: bool,
        }

        let data: HealthResponse = response.json().await?;
        Ok(data.paired)
    }

    pub async fn api_auth_check(&self) -> Result<bool> {
        let bearer = match self.bearer.as_ref() {
            Some(bearer) => bearer,
            None => return Ok(false),
        };

        let response = self
            .http
            .get(format!("{}/api/status", self.base_url))
            .header("Authorization", format!("Bearer {}", bearer))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    pub async fn send_to_agent(&self, payload: &serde_json::Value) -> Result<String> {
        let mut request = self.http.post(format!("{}/webhook", self.base_url));
        if let Some(bearer) = self.bearer.as_ref() {
            request = request.header("Authorization", format!("Bearer {}", bearer));
        }

        let response = request
            .timeout(Duration::from_secs(55))
            .json(payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let body = body.trim();
            if body.is_empty() {
                return Err(anyhow!("Webhook failed: {}", status));
            }
            return Err(anyhow!("Webhook failed: {}: {}", status, body));
        }

        let text = response.text().await?;
        let parsed_text = match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => map
                .get("response")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| {
                    map.get("message")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .unwrap_or(text),
            _ => text,
        };
        let bounded = if parsed_text.chars().count() > 20_000 {
            parsed_text.chars().take(20_000).collect()
        } else {
            parsed_text
        };
        Ok(bounded)
    }

    pub fn is_paired(&self) -> bool {
        self.bearer.is_some()
    }

    pub fn get_bearer(&self) -> Option<&String> {
        self.bearer.as_ref()
    }
}
