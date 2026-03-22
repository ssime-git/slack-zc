use crate::types::Workspace;
use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

fn zeroclaw_state_dir() -> Option<PathBuf> {
    std::env::var("ZEROCLAW_CONFIG_DIR")
        .map(PathBuf::from)
        .ok()
        .or_else(|| directories::BaseDirs::new().map(|base| base.home_dir().join(".zeroclaw")))
}

pub fn load_zeroclaw_gateway_port() -> Option<u16> {
    let config_file = zeroclaw_state_dir()?.join("config.toml");
    let content = fs::read_to_string(config_file).ok()?;
    let config: toml::Value = content.parse().ok()?;
    let gateway = config.get("gateway")?;
    let port = gateway.get("port")?.as_integer()?;
    u16::try_from(port).ok()
}

/// Try to load ZeroClaw bearer token from OpenClaw's configuration directory.
/// OpenClaw stores its state in ~/.openclaw/ or $OPENCLAW_STATE_DIR/
/// The token may be in agents/<agentId>/agent/auth-profiles.json or credentials/oauth.json
pub fn load_openclaw_bearer() -> Option<String> {
    if let Some(config_dir) = zeroclaw_state_dir() {
        let config_file = config_dir.join("config.toml");
        if config_file.exists() {
            if let Ok(content) = fs::read_to_string(&config_file) {
                if let Ok(config) = content.parse::<toml::Value>() {
                    if let Some(token) = config.get("gateway").and_then(|gateway| {
                        gateway.get("paired_tokens")?.as_array()?.first()?.as_str()
                    }) {
                        return Some(token.to_string());
                    }
                }
            }
        }
    }

    // Check for OPENCLAW_STATE_DIR environment variable first
    let state_dir = std::env::var("OPENCLAW_STATE_DIR")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            // Use directories crate to get home dir
            directories::BaseDirs::new().map(|base| base.home_dir().join(".openclaw"))
        })?;

    if !state_dir.exists() {
        return None;
    }

    // Try to find the token in various locations
    // 1. Look for agent auth-profiles.json files
    let agents_dir = state_dir.join("agents");
    if agents_dir.exists() {
        if let Ok(entries) = fs::read_dir(&agents_dir) {
            for entry in entries.flatten() {
                let auth_file = entry.path().join("agent").join("auth-profiles.json");
                if auth_file.exists() {
                    if let Ok(content) = fs::read_to_string(&auth_file) {
                        if let Ok(json) = serde_json::from_str::<Value>(&content) {
                            // Look for bearer token in various formats
                            if let Some(token) = json.get("bearer").and_then(|v| v.as_str()) {
                                return Some(token.to_string());
                            }
                            if let Some(token) = json.get("token").and_then(|v| v.as_str()) {
                                return Some(token.to_string());
                            }
                            // Check profiles array
                            if let Some(profiles) = json.get("profiles").and_then(|v| v.as_array())
                            {
                                for profile in profiles {
                                    if let Some(token) =
                                        profile.get("token").and_then(|v| v.as_str())
                                    {
                                        return Some(token.to_string());
                                    }
                                    if let Some(token) =
                                        profile.get("bearer").and_then(|v| v.as_str())
                                    {
                                        return Some(token.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Try legacy credentials/oauth.json
    let oauth_file = state_dir.join("credentials").join("oauth.json");
    if oauth_file.exists() {
        if let Ok(content) = fs::read_to_string(&oauth_file) {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                if let Some(token) = json.get("bearer").and_then(|v| v.as_str()) {
                    return Some(token.to_string());
                }
                if let Some(token) = json.get("token").and_then(|v| v.as_str()) {
                    return Some(token.to_string());
                }
            }
        }
    }

    // 3. Try main config openclaw.json
    let config_file = state_dir.join("openclaw.json");
    if config_file.exists() {
        if let Ok(content) = fs::read_to_string(&config_file) {
            if let Ok(json) = serde_json::from_str::<Value>(&content) {
                // Look in gateway config
                if let Some(gateway) = json.get("gateway") {
                    if let Some(token) = gateway.get("token").and_then(|v| v.as_str()) {
                        return Some(token.to_string());
                    }
                    if let Some(token) = gateway.get("bearer").and_then(|v| v.as_str()) {
                        return Some(token.to_string());
                    }
                }
            }
        }
    }

    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub workspaces: Vec<Workspace>,
    pub zeroclaw_bearer: Option<String>,
}

impl Session {
    pub fn load() -> Result<Option<Self>> {
        let path = Self::session_path()?;
        if !path.exists() {
            return Ok(None);
        }

        let encrypted = fs::read(&path)?;
        let decrypted = Self::decrypt(&encrypted)?;
        let session: Session = serde_json::from_slice(&decrypted)?;
        Ok(Some(session))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::session_path()?;
        let json = serde_json::to_vec(self)?;
        let encrypted = Self::encrypt(&json)?;
        Self::write_secure_file(&path, &encrypted)?;
        Ok(())
    }

    pub fn session_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "slack-zc", "slack-zc")
            .ok_or_else(|| anyhow!("Could not determine project directories"))?;
        let data_dir = proj_dirs.data_dir();
        fs::create_dir_all(data_dir)?;
        Ok(data_dir.join("session.json"))
    }

    fn secret_key_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "slack-zc", "slack-zc")
            .ok_or_else(|| anyhow!("Could not determine project directories"))?;
        let data_dir = proj_dirs.data_dir();
        Ok(data_dir.join(".secret_key"))
    }

    fn get_or_create_key() -> Result<[u8; 32]> {
        let path = Self::secret_key_path()?;

        if path.exists() {
            let key_bytes = fs::read(&path)?;
            if key_bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes);
                return Ok(key);
            }
        }

        let mut key = [0u8; 32];
        rand::thread_rng().fill(&mut key);

        let mut file = File::create(&path)?;
        file.write_all(&key)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(key)
    }

    fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>> {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };

        let key = Self::get_or_create_key()?;
        let cipher = Aes256Gcm::new(aes_gcm::aead::Key::<Aes256Gcm>::from_slice(&key));

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| anyhow!("Encryption failed"))?;

        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>> {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };

        if ciphertext.len() < 12 {
            return Err(anyhow!("Invalid ciphertext"));
        }

        let key = Self::get_or_create_key()?;
        let cipher = Aes256Gcm::new(aes_gcm::aead::Key::<Aes256Gcm>::from_slice(&key));

        let nonce = Nonce::from_slice(&ciphertext[..12]);
        let plaintext = cipher
            .decrypt(nonce, &ciphertext[12..])
            .map_err(|_| anyhow!("Decryption failed"))?;

        Ok(plaintext)
    }

    fn write_secure_file(path: &PathBuf, bytes: &[u8]) -> Result<()> {
        let mut file = File::create(path)?;
        file.write_all(bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms)?;
        }
        Ok(())
    }

    pub fn add_workspace(&mut self, workspace: Workspace) {
        if let Some(idx) = self
            .workspaces
            .iter()
            .position(|w| w.team_id == workspace.team_id)
        {
            self.workspaces[idx] = workspace;
        } else {
            self.workspaces.push(workspace);
        }
    }

    pub fn set_active_workspace(&mut self, team_id: &str) {
        for ws in &mut self.workspaces {
            ws.active = ws.team_id == team_id;
        }
    }

    pub fn get_active_workspace(&self) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.active)
    }

    pub fn get_active_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|w| w.active)
    }

    pub fn remove_workspace(&mut self, team_id: &str) -> bool {
        let initial_len = self.workspaces.len();
        self.workspaces.retain(|w| w.team_id != team_id);
        if self.workspaces.len() != initial_len {
            if self.workspaces.is_empty() {
                self.zeroclaw_bearer = None;
            } else if let Some(first) = self.workspaces.first_mut() {
                first.active = true;
            }
            true
        } else {
            false
        }
    }

    pub fn clear_all(&mut self) -> Result<()> {
        self.workspaces.clear();
        self.zeroclaw_bearer = None;
        self.save()
    }

    pub fn rotate_token(
        &mut self,
        team_id: &str,
        new_token: &str,
        new_app_token: &str,
    ) -> Result<()> {
        if let Some(ws) = self.workspaces.iter_mut().find(|w| w.team_id == team_id) {
            ws.xoxp_token = new_token.to_string();
            ws.xapp_token = new_app_token.to_string();
            self.save()
        } else {
            Err(anyhow!("Workspace not found"))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthResponse {
    pub ok: bool,
    pub access_token: String,
    pub authed_user: AuthedUser,
    pub team: Team,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthedUser {
    pub id: String,
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
}

pub async fn exchange_oauth_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<OAuthResponse> {
    use reqwest::Client;

    let client = Client::builder()
        .user_agent("slack-zc/0.2")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_else(|_| Client::new());
    let response = client
        .post("https://slack.com/api/oauth.v2.access")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await?;

    let data: OAuthResponse = response.json().await?;

    if !data.ok {
        return Err(anyhow!("OAuth exchange failed"));
    }

    Ok(data)
}
