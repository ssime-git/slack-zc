use crate::gateway::GatewayClient;
use anyhow::{anyhow, Result};
use regex::Regex;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info};

pub struct AgentRunner {
    binary_path: String,
    gateway_port: u16,
    child: Option<tokio::process::Child>,
    gateway: Option<GatewayClient>,
}

#[derive(Debug, Clone)]
pub enum AgentStatus {
    Unavailable,
    Starting,
    Pairing,
    Active,
    Error(String),
}

impl AgentRunner {
    pub fn new(binary_path: String, gateway_port: u16) -> Self {
        Self {
            binary_path,
            gateway_port,
            child: None,
            gateway: None,
        }
    }

    pub async fn check_binary(&self) -> Result<()> {
        let output = Command::new(&self.binary_path)
            .arg("--version")
            .output()
            .await?;

        if output.status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "ZeroClaw binary not found or not executable: {}",
                self.binary_path
            ))
        }
    }

    pub async fn start_and_pair(&mut self) -> Result<GatewayClient> {
        info!("Starting ZeroClaw gateway on port {}", self.gateway_port);

        let mut child = Command::new(&self.binary_path)
            .arg("gateway")
            .arg("--port")
            .arg(self.gateway_port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stderr"))?;
        
        let mut reader = BufReader::new(stdout).lines();

        let re = Regex::new(r"(?i)(?:pairing.code|pairing.code).*?(\d{6})").unwrap();

        let code = timeout(Duration::from_secs(10), async {
            while let Some(line) = reader.next_line().await? {
                debug!("ZeroClaw stdout: {}", line);
                if let Some(caps) = re.captures(&line) {
                    return Ok::<_, anyhow::Error>(caps[1].to_string());
                }
            }
            Err(anyhow!("Pairing code not found in output"))
        })
        .await
        .map_err(|_| anyhow!("Timeout waiting for pairing code"))??;

        info!("ZeroClaw pairing code obtained (redacted)");

        tokio::spawn(async move { while reader.next_line().await.ok().flatten().is_some() {} });

        let mut gateway = GatewayClient::new(self.gateway_port);
        gateway.pair(&code).await?;

        self.child = Some(child);
        self.gateway = Some(gateway.clone());

        Ok(gateway)
    }

    pub async fn start_with_bearer(&mut self, bearer: &str) -> Result<GatewayClient> {
        info!("Starting ZeroClaw gateway with existing bearer");

        let child = Command::new(&self.binary_path)
            .arg("gateway")
            .arg("--port")
            .arg(self.gateway_port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        tokio::time::sleep(Duration::from_millis(500)).await;

        let gateway = GatewayClient::new(self.gateway_port).with_bearer(bearer.to_string());

        if !gateway.health_check().await? {
            return Err(anyhow!("Gateway health check failed"));
        }

        self.child = Some(child);
        self.gateway = Some(gateway.clone());

        info!("ZeroClaw gateway started and authenticated");
        Ok(gateway)
    }

    pub async fn connect_to_running_gateway(&mut self) -> Result<GatewayClient> {
        info!("Attempting to connect to existing ZeroClaw gateway on port {}", self.gateway_port);
        
        let gateway = GatewayClient::new(self.gateway_port);
        
        // Check if gateway is running and not paired
        match gateway.check_pairing_status().await {
            Ok(paired) => {
                if paired {
                    info!("Gateway is already paired but no bearer token stored");
                    return Err(anyhow!("Gateway already paired. Please check configuration."));
                }
                // Not paired - gateway is running and waiting for a pairing code
                info!("Gateway is running and waiting for pairing code");
                Err(anyhow!("Gateway needs pairing. Check your terminal for the 6-digit code."))
            }
            Err(_) => {
                info!("No running ZeroClaw gateway detected on port {}", self.gateway_port);
                Err(anyhow!("ZeroClaw gateway not accessible. Make sure it's running."))
            }
        }
    }


    pub fn get_gateway(&self) -> Option<&GatewayClient> {
        self.gateway.as_ref()
    }

    pub fn status(&self) -> AgentStatus {
        match &self.gateway {
            None => AgentStatus::Unavailable,
            Some(g) if !g.is_paired() => AgentStatus::Pairing,
            Some(_) => AgentStatus::Active,
        }
    }

    pub async fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
            info!("ZeroClaw gateway stopped");
        }
        self.gateway = None;
    }
}

impl Drop for AgentRunner {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        self.gateway = None;
    }
}
