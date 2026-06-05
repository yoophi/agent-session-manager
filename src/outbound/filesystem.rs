use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;
use walkdir::WalkDir;

use crate::application::ports::SessionRepository;
use crate::domain::{AgentKind, AgentSession, SessionScope, home_dir};

#[derive(Clone, Debug, Default)]
pub struct FilesystemSessionRepository {
    roots: SessionRoots,
}

#[derive(Clone, Debug, Default)]
pub struct SessionRoots {
    pub claude: Option<PathBuf>,
    pub codex: Option<PathBuf>,
    pub pi: Option<PathBuf>,
}

impl SessionRepository for FilesystemSessionRepository {
    fn list(&self, agent: AgentKind, scope: &SessionScope) -> Result<Vec<AgentSession>> {
        match agent {
            AgentKind::Claude => scan_agent(agent, self.claude_root()?, scope, parse_claude),
            AgentKind::Codex => scan_agent(agent, self.codex_root()?, scope, parse_codex),
            AgentKind::Pi => scan_agent(agent, self.pi_root()?, scope, parse_pi),
        }
    }
}

impl FilesystemSessionRepository {
    pub fn with_roots(roots: SessionRoots) -> Self {
        Self { roots }
    }

    fn claude_root(&self) -> Result<PathBuf> {
        if let Some(root) = &self.roots.claude {
            return Ok(root.clone());
        }

        let base = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or(home_dir()?.join(".claude"));
        Ok(base.join("projects"))
    }

    fn codex_root(&self) -> Result<PathBuf> {
        if let Some(root) = &self.roots.codex {
            return Ok(root.clone());
        }

        let base = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or(home_dir()?.join(".codex"));
        Ok(base.join("sessions"))
    }

    fn pi_root(&self) -> Result<PathBuf> {
        if let Some(root) = &self.roots.pi {
            return Ok(root.clone());
        }

        if let Some(value) = std::env::var_os("PI_CODING_AGENT_SESSION_DIR") {
            return Ok(PathBuf::from(value));
        }

        Ok(home_dir()?.join(".pi").join("agent").join("sessions"))
    }
}

fn scan_agent(
    agent: AgentKind,
    root: PathBuf,
    scope: &SessionScope,
    parser: fn(AgentKind, &Path) -> Result<Option<AgentSession>>,
) -> Result<Vec<AgentSession>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
    {
        if let Some(session) = parser(agent, entry.path())?
            && matches_scope(&session, scope)
        {
            sessions.push(session);
        }
    }

    Ok(sessions)
}

fn matches_scope(session: &AgentSession, scope: &SessionScope) -> bool {
    match scope {
        SessionScope::All => true,
        SessionScope::Path(path) => session.cwd.as_ref().is_some_and(|cwd| cwd == path),
    }
}

fn parse_claude(agent: AgentKind, path: &Path) -> Result<Option<AgentSession>> {
    if path
        .components()
        .any(|component| component.as_os_str() == OsStr::new("subagents"))
    {
        return Ok(None);
    }

    let metadata = fs::metadata(path)?;
    let mut id = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let mut cwd = None;
    let mut title = None;
    let mut message_count = 0;

    for value in read_json_lines(path, 200)? {
        if let Some(session_id) = value.get("sessionId").and_then(Value::as_str) {
            id = session_id.to_string();
        }
        if cwd.is_none() {
            cwd = value.get("cwd").and_then(Value::as_str).map(PathBuf::from);
        }
        if value.get("type").and_then(Value::as_str) == Some("user") {
            message_count += 1;
            if title.is_none() {
                title = extract_claude_user_text(&value);
            }
        } else if value.get("type").and_then(Value::as_str) == Some("assistant") {
            message_count += 1;
        }
    }

    Ok(Some(AgentSession {
        agent,
        id,
        cwd,
        title,
        file: path.to_path_buf(),
        message_count,
        modified_at: metadata.modified().ok(),
    }))
}

fn parse_codex(agent: AgentKind, path: &Path) -> Result<Option<AgentSession>> {
    let metadata = fs::metadata(path)?;
    let mut id = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let mut cwd = None;
    let mut title = None;
    let mut message_count = 0;

    for value in read_json_lines(path, 200)? {
        if value.get("type").and_then(Value::as_str) == Some("session_meta")
            && let Some(payload) = value.get("payload")
        {
            if let Some(meta_id) = payload.get("id").and_then(Value::as_str) {
                id = meta_id.to_string();
            }
            cwd = payload
                .get("cwd")
                .and_then(Value::as_str)
                .map(PathBuf::from);
        }

        if value.get("type").and_then(Value::as_str) == Some("response_item") {
            let Some(payload) = value.get("payload") else {
                continue;
            };
            if payload.get("type").and_then(Value::as_str) == Some("message") {
                message_count += 1;
                if title.is_none() && payload.get("role").and_then(Value::as_str) == Some("user") {
                    title = extract_codex_user_text(payload);
                }
            }
        }
    }

    Ok(Some(AgentSession {
        agent,
        id,
        cwd,
        title,
        file: path.to_path_buf(),
        message_count,
        modified_at: metadata.modified().ok(),
    }))
}

fn parse_pi(agent: AgentKind, path: &Path) -> Result<Option<AgentSession>> {
    let metadata = fs::metadata(path)?;
    let mut id = path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let mut cwd = None;
    let mut title = None;
    let mut message_count = 0;

    for value in read_json_lines(path, 200)? {
        match value.get("type").and_then(Value::as_str) {
            Some("session") => {
                if let Some(header_id) = value.get("id").and_then(Value::as_str) {
                    id = header_id.to_string();
                }
                cwd = value.get("cwd").and_then(Value::as_str).map(PathBuf::from);
            }
            Some("message") => {
                message_count += 1;
                if title.is_none() {
                    title = extract_pi_user_text(&value);
                }
            }
            Some("session_info") => {
                if let Some(name) = value.get("name").and_then(Value::as_str) {
                    title = Some(name.to_string());
                }
            }
            _ => {}
        }
    }

    Ok(Some(AgentSession {
        agent,
        id,
        cwd,
        title,
        file: path.to_path_buf(),
        message_count,
        modified_at: metadata.modified().ok(),
    }))
}

fn read_json_lines(path: &Path, max_lines: usize) -> Result<Vec<Value>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut values = Vec::new();

    for line in reader.lines().take(max_lines) {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            values.push(value);
        }
    }

    Ok(values)
}

fn extract_claude_user_text(value: &Value) -> Option<String> {
    let content = value.get("message")?.get("content")?;
    match content {
        Value::String(text) => Some(snippet(text)),
        Value::Array(items) => items
            .iter()
            .find_map(|item| item.get("text").and_then(Value::as_str).map(snippet)),
        _ => None,
    }
}

fn extract_codex_user_text(payload: &Value) -> Option<String> {
    payload
        .get("content")?
        .as_array()?
        .iter()
        .find_map(|item| item.get("text").and_then(Value::as_str).map(snippet))
}

fn extract_pi_user_text(value: &Value) -> Option<String> {
    let message = value.get("message")?;
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return None;
    }

    let content = message.get("content")?;
    match content {
        Value::String(text) => Some(snippet(text)),
        Value::Array(items) => items
            .iter()
            .find_map(|item| item.get("text").and_then(Value::as_str).map(snippet)),
        _ => None,
    }
}

fn snippet(value: &str) -> String {
    let value = value.trim().replace(['\n', '\t'], " ");
    const MAX: usize = 80;
    if value.chars().count() <= MAX {
        return value;
    }
    value.chars().take(MAX).collect::<String>()
}
