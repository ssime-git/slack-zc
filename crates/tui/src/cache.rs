use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use slack_zc_slack::types::Channel;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceChannelsCache {
    team_id: String,
    saved_at: DateTime<Utc>,
    channels: Vec<Channel>,
}

fn cache_dir() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "slack-zc", "slack-zc")
        .context("unable to resolve slack-zc cache directory")?;
    Ok(proj_dirs.cache_dir().to_path_buf())
}

fn workspace_cache_path(team_id: &str) -> Result<PathBuf> {
    Ok(cache_dir()?.join(format!("{team_id}.channels.json")))
}

pub fn load_workspace_channels(team_id: &str) -> Result<Option<Vec<Channel>>> {
    let path = workspace_cache_path(team_id)?;
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read workspace cache {}", path.display()))?;
    let cached: WorkspaceChannelsCache = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse workspace cache {}", path.display()))?;

    if cached.team_id != team_id {
        return Ok(None);
    }

    Ok(Some(cached.channels))
}

pub fn save_workspace_channels(team_id: &str, channels: &[Channel]) -> Result<()> {
    let path = workspace_cache_path(team_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create workspace cache directory {}",
                parent.display()
            )
        })?;
    }

    let payload = WorkspaceChannelsCache {
        team_id: team_id.to_string(),
        saved_at: Utc::now(),
        channels: channels.to_vec(),
    };

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, serde_json::to_vec_pretty(&payload)?)
        .with_context(|| format!("failed to write workspace cache {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &path).with_context(|| {
        format!(
            "failed to atomically replace workspace cache {}",
            path.display()
        )
    })?;

    Ok(())
}
