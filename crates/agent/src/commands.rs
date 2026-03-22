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

    pub fn to_agent_prompt(&self, active_channel: &str, history: &str, user: &str) -> String {
        match self {
            CommandType::Resume { channel } => {
                let target_channel = channel
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(active_channel);
                format!(
                    "You are ZeroClaw helping inside Slack for user {user}.\n\
                     Summarize the recent discussion in channel #{target_channel}.\n\
                     Focus on decisions, action items, blockers, and open questions.\n\
                     If the context is insufficient, say that explicitly.\n\n\
                     Recent Slack messages:\n{history}"
                )
            }
            CommandType::Draft { intent } => {
                format!(
                    "You are ZeroClaw helping inside Slack for user {user} in channel #{active_channel}.\n\
                     Write a concise Slack message draft.\n\
                     User intent: {intent}\n\
                     Return only the draft message body, ready to send.\n\n\
                     Recent Slack context:\n{history}"
                )
            }
            CommandType::Search { query } => {
                format!(
                    "You are ZeroClaw helping inside Slack for user {user} in channel #{active_channel}.\n\
                     Answer this request using the Slack context below when relevant.\n\
                     Search/query: {query}\n\
                     Be concise and explicit about uncertainty.\n\n\
                     Recent Slack context:\n{history}"
                )
            }
            CommandType::Unknown(name) => {
                format!(
                    "The user sent an unsupported Slack agent command '/{name}' in channel #{active_channel}. \
                     Explain briefly which commands are supported and how to use them."
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CommandType;

    #[test]
    fn resume_without_arg_uses_active_channel() {
        let prompt = CommandType::Resume { channel: None }.to_agent_prompt(
            "general",
            "alice: hello",
            "U456",
        );
        assert!(prompt.contains("channel #general"));
        assert!(prompt.contains("alice: hello"));
        assert!(prompt.contains("U456"));
    }

    #[test]
    fn resume_with_arg_uses_requested_channel() {
        let prompt = CommandType::Resume {
            channel: Some("general".to_string()),
        }
        .to_agent_prompt("random", "alice: hello", "U456");
        assert!(prompt.contains("channel #general"));
        assert!(!prompt.contains("channel #random"));
    }
}
