use crate::gateway::GatewayClient;
use anyhow::{anyhow, Result};
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, info};

async fn pump_output<R>(
    reader: R,
    source: &'static str,
    tx: mpsc::UnboundedSender<(&'static str, String)>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = tx.send((source, line));
    }
}

fn extract_pairing_code(line: &str, re: &Regex) -> Option<String> {
    re.captures(line)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

#[derive(Debug, Deserialize)]
struct AuthProfilesFile {
    active_profiles: Option<std::collections::HashMap<String, String>>,
}

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

        let (tx, mut rx) = mpsc::unbounded_channel();
        tokio::spawn(pump_output(stdout, "stdout", tx.clone()));
        tokio::spawn(pump_output(stderr, "stderr", tx));

        let re = Regex::new(r"(?i)(?:pair(?:ing)?[\s._-]*code|code)\D{0,12}([0-9]{6})").unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let code = loop {
            if let Ok(Some(status)) = child.try_wait() {
                return Err(anyhow!(
                    "ZeroClaw exited before emitting a pairing code (status: {})",
                    status
                ));
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(anyhow!("Timeout waiting for ZeroClaw pairing code"));
            }

            match tokio::time::timeout(remaining.min(Duration::from_millis(250)), rx.recv()).await {
                Ok(Some((source, line))) => {
                    debug!("ZeroClaw {}: {}", source, line);
                    if let Some(code) = extract_pairing_code(&line, &re) {
                        break code;
                    }
                }
                Ok(None) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        return Err(anyhow!(
                            "ZeroClaw exited before emitting a pairing code (status: {})",
                            status
                        ));
                    }
                }
                Err(_) => {}
            }
        };

        info!("ZeroClaw pairing code obtained (redacted)");

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

        let gateway = GatewayClient::new(self.gateway_port).with_bearer(bearer.to_string());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut ready = false;

        while tokio::time::Instant::now() < deadline {
            if gateway.api_auth_check().await? {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        if !ready {
            return Err(anyhow!(
                "Gateway health check failed after waiting for startup"
            ));
        }

        self.child = Some(child);
        self.gateway = Some(gateway.clone());

        info!("ZeroClaw gateway started and authenticated");
        Ok(gateway)
    }

    fn zeroclaw_home_dir() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .map_err(|_| anyhow!("HOME is not set; cannot locate ZeroClaw config"))?;
        Ok(Path::new(&home).join(".zeroclaw"))
    }

    fn symlink_or_copy(src: &Path, dst: &Path) -> Result<()> {
        if dst.exists() {
            let _ = fs::remove_file(dst);
            let _ = fs::remove_dir_all(dst);
        }

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(src, dst)
                .or_else(|_| {
                    if src.is_dir() {
                        Err(std::io::Error::other("directory symlink failed"))
                    } else {
                        fs::copy(src, dst).map(|_| ())
                    }
                })
                .map_err(|e| {
                    anyhow!(
                        "failed to link {} -> {}: {}",
                        src.display(),
                        dst.display(),
                        e
                    )
                })?;
        }

        #[cfg(not(unix))]
        {
            if src.is_dir() {
                return Err(anyhow!(
                    "directory linking not implemented for this platform: {}",
                    src.display()
                ));
            }
            fs::copy(src, dst).map_err(|e| {
                anyhow!(
                    "failed to copy {} -> {}: {}",
                    src.display(),
                    dst.display(),
                    e
                )
            })?;
        }

        Ok(())
    }

    fn detect_embedded_provider(source_dir: &Path, config_text: &str) -> Option<String> {
        if std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .is_some()
        {
            return None;
        }

        let auth_profiles = source_dir.join("auth-profiles.json");
        let auth_text = fs::read_to_string(auth_profiles).ok()?;
        let auth: AuthProfilesFile = serde_json::from_str(&auth_text).ok()?;
        let active = auth.active_profiles?;

        if active.contains_key("openai-codex") {
            let current_provider = config_text
                .lines()
                .find_map(|line| {
                    line.trim()
                        .strip_prefix("default_provider = ")
                        .map(str::trim)
                        .map(|value| value.trim_matches('"').to_string())
                })
                .unwrap_or_default();

            if current_provider != "openai-codex" {
                return Some("openai-codex".to_string());
            }
        }

        None
    }

    fn prepare_embedded_config_dir(&self) -> Result<PathBuf> {
        let source_dir = Self::zeroclaw_home_dir()?;
        let target_dir =
            std::env::temp_dir().join(format!("slack-zc-zeroclaw-{}", self.gateway_port));
        fs::create_dir_all(&target_dir)?;

        let source_config = source_dir.join("config.toml");
        let target_config = target_dir.join("config.toml");
        let mut config_text = fs::read_to_string(&source_config).map_err(|e| {
            anyhow!(
                "failed to read ZeroClaw config {}: {}",
                source_config.display(),
                e
            )
        })?;

        if config_text.contains("require_pairing = true") {
            config_text = config_text.replace("require_pairing = true", "require_pairing = false");
        } else if !config_text.contains("require_pairing = false") {
            config_text.push_str("\n[gateway]\nrequire_pairing = false\n");
        }

        if let Some(provider) = Self::detect_embedded_provider(&source_dir, &config_text) {
            info!(
                "Adjusting embedded ZeroClaw provider from user config to {}",
                provider
            );

            if config_text.contains("default_provider = ") {
                config_text = config_text
                    .lines()
                    .map(|line| {
                        if line.trim_start().starts_with("default_provider = ") {
                            format!("default_provider = \"{}\"", provider)
                        } else {
                            line.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                config_text.push('\n');
            } else {
                config_text.push_str(&format!("\ndefault_provider = \"{}\"\n", provider));
            }
        }

        fs::write(&target_config, config_text).map_err(|e| {
            anyhow!(
                "failed to write embedded ZeroClaw config {}: {}",
                target_config.display(),
                e
            )
        })?;

        for file_name in ["auth-profiles.json", ".secret_key", "workspace"] {
            let src = source_dir.join(file_name);
            if src.exists() {
                let dst = target_dir.join(file_name);
                Self::symlink_or_copy(&src, &dst)?;
            }
        }

        Ok(target_dir)
    }

    pub async fn start_embedded_gateway(&mut self) -> Result<GatewayClient> {
        let config_dir = self.prepare_embedded_config_dir()?;
        let requested_port = self.gateway_port;
        let gateway_port = if GatewayClient::new(requested_port).health_check().await? {
            let listener = TcpListener::bind(("127.0.0.1", 0))
                .map_err(|e| anyhow!("failed to reserve free port for embedded gateway: {}", e))?;
            let port = listener
                .local_addr()
                .map_err(|e| anyhow!("failed to inspect reserved embedded gateway port: {}", e))?
                .port();
            drop(listener);
            port
        } else {
            requested_port
        };
        info!(
            "Starting embedded ZeroClaw gateway on port {} with pairing disabled",
            gateway_port
        );

        let child = Command::new(&self.binary_path)
            .arg("gateway")
            .arg("--config-dir")
            .arg(&config_dir)
            .arg("--port")
            .arg(gateway_port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let gateway = GatewayClient::new(gateway_port);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut ready = false;

        while tokio::time::Instant::now() < deadline {
            if gateway.health_check().await? {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        if !ready {
            return Err(anyhow!(
                "Embedded gateway health check failed after waiting for startup"
            ));
        }

        self.child = Some(child);
        self.gateway_port = gateway_port;
        self.gateway = Some(gateway.clone());

        info!("Embedded ZeroClaw gateway started");
        Ok(gateway)
    }

    pub async fn connect_with_bearer(&mut self, bearer: &str) -> Result<GatewayClient> {
        info!(
            "Attempting to connect to existing ZeroClaw gateway on port {} with bearer",
            self.gateway_port
        );

        let gateway = GatewayClient::new(self.gateway_port).with_bearer(bearer.to_string());

        if !gateway.api_auth_check().await? {
            return Err(anyhow!(
                "Existing ZeroClaw gateway is not reachable with a valid API bearer"
            ));
        }

        self.gateway = Some(gateway.clone());
        info!("Connected to existing ZeroClaw gateway");
        Ok(gateway)
    }

    pub async fn connect_to_running_gateway(&mut self) -> Result<GatewayClient> {
        info!(
            "Attempting to connect to existing ZeroClaw gateway on port {}",
            self.gateway_port
        );

        let gateway = GatewayClient::new(self.gateway_port);

        // Check if gateway is running and not paired
        match gateway.check_pairing_status().await {
            Ok(paired) => {
                if paired {
                    info!("Gateway is already paired but no bearer token stored");
                    return Err(anyhow!(
                        "Gateway already paired, but no valid runtime bearer is available."
                    ));
                }
                // Not paired - gateway is running and waiting for a pairing code
                info!("Gateway is running and waiting for pairing code");
                Err(anyhow!(
                    "Gateway needs pairing. Check your terminal for the 6-digit code."
                ))
            }
            Err(_) => {
                info!(
                    "No running ZeroClaw gateway detected on port {}",
                    self.gateway_port
                );
                Err(anyhow!(
                    "ZeroClaw gateway not accessible. Make sure it's running."
                ))
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
