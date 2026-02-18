pub fn process_command(text: &str) -> Option<(String, Vec<String>)> {
    if !text.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = text[1..].split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let command = parts[0].to_string();
    let args = parts[1..].iter().map(|s| s.to_string()).collect();

    Some((command, args))
}

pub fn is_agent_mention(text: &str) -> bool {
    text.to_lowercase().contains("@zeroclaw") || text.to_lowercase().contains("@zc")
}

#[derive(Debug, Clone)]
pub enum CommandType {
    Resume { channel: Option<String> },
    Draft { intent: String },
    Search { query: String },
    Unknown(String),
}

impl CommandType {
    pub fn from_command(name: &str, args: &[String]) -> Self {
        match name.to_lowercase().as_str() {
            "resume" | "résume" | "summarize" => Self::Resume {
                channel: args.first().map(|s| {
                    if let Some(stripped) = s.strip_prefix('#') {
                        stripped.to_string()
                    } else {
                        s.clone()
                    }
                }),
            },
            "draft" => Self::Draft {
                intent: args.join(" "),
            },
            "cherche" | "search" => Self::Search {
                query: args.join(" "),
            },
            _ => Self::Unknown(name.to_string()),
        }
    }

    pub fn to_webhook_payload(&self, active_channel: &str, user: &str) -> serde_json::Value {
        match self {
            CommandType::Resume { channel } => {
                let target_channel = channel
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(active_channel);
                let message = channel
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("/resume #{}", s))
                    .unwrap_or_else(|| "/resume".to_string());
                serde_json::json!({
                    "command": "resume",
                    "channel": target_channel,
                    "user": user,
                    "message": message
                })
            }
            CommandType::Draft { intent } => {
                serde_json::json!({
                    "command": "draft",
                    "intent": intent,
                    "user": user,
                    "channel": active_channel,
                    "message": format!("/draft {}", intent)
                })
            }
            CommandType::Search { query } => {
                serde_json::json!({
                    "command": "cherche",
                    "query": query,
                    "user": user,
                    "channel": active_channel,
                    "message": format!("/cherche {}", query)
                })
            }
            CommandType::Unknown(name) => {
                serde_json::json!({
                    "command": "unknown",
                    "raw": name,
                    "user": user,
                    "channel": active_channel
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CommandType;

    #[test]
    fn resume_without_arg_uses_active_channel() {
        let payload = CommandType::Resume { channel: None }.to_webhook_payload("C123", "U456");
        assert_eq!(payload["command"], "resume");
        assert_eq!(payload["channel"], "C123");
        assert_eq!(payload["user"], "U456");
        assert_eq!(payload["message"], "/resume");
    }

    #[test]
    fn resume_with_arg_uses_requested_channel() {
        let payload = CommandType::Resume {
            channel: Some("general".to_string()),
        }
        .to_webhook_payload("C123", "U456");
        assert_eq!(payload["command"], "resume");
        assert_eq!(payload["channel"], "general");
        assert_eq!(payload["message"], "/resume #general");
    }
}
