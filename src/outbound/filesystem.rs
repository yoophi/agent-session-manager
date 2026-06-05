use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use chrono::{DateTime, Utc};
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
    let mut created_at = None;
    let mut updated_at = None;
    let mut model = None;
    let mut branch = None;
    let mut source = None;

    for value in read_json_lines(path, 200)? {
        if let Some(session_id) = value.get("sessionId").and_then(Value::as_str) {
            id = session_id.to_string();
        }
        if let Some(timestamp) = value.get("timestamp").and_then(Value::as_str) {
            apply_timestamp(&mut created_at, &mut updated_at, timestamp);
        }
        if cwd.is_none() {
            cwd = value.get("cwd").and_then(Value::as_str).map(PathBuf::from);
        }
        if model.is_none() {
            model = value
                .get("message")
                .and_then(|message| message.get("model"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
        }
        if branch.is_none() {
            branch = value
                .get("gitBranch")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
        }
        if source.is_none() {
            source = value
                .get("entrypoint")
                .or_else(|| value.get("promptSource"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
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
        created_at: created_at.or_else(|| metadata.created().ok()),
        updated_at: updated_at.or_else(|| metadata.modified().ok()),
        model,
        branch,
        source,
        is_subsession: false,
        parent_session_id: None,
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
    let mut created_at = None;
    let mut updated_at = None;
    let mut model = None;
    let mut branch = None;
    let mut source = None;

    for value in read_json_lines(path, 200)? {
        if let Some(timestamp) = value.get("timestamp").and_then(Value::as_str) {
            apply_timestamp(&mut created_at, &mut updated_at, timestamp);
        }

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
            source = payload
                .get("source")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            branch = payload
                .get("git")
                .and_then(|git| git.get("branch"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            if let Some(timestamp) = payload.get("timestamp").and_then(Value::as_str) {
                apply_timestamp(&mut created_at, &mut updated_at, timestamp);
            }
        }

        if value.get("type").and_then(Value::as_str) == Some("response_item") {
            let Some(payload) = value.get("payload") else {
                continue;
            };
            if payload.get("type").and_then(Value::as_str) == Some("message") {
                message_count += 1;
                if let Some(payload_model) = payload.get("model").and_then(Value::as_str) {
                    model = Some(payload_model.to_string());
                }
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
        created_at: created_at.or_else(|| metadata.created().ok()),
        updated_at: updated_at.or_else(|| metadata.modified().ok()),
        model,
        branch,
        source,
        is_subsession: false,
        parent_session_id: None,
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
    let mut created_at = None;
    let mut updated_at = None;
    let mut model = None;
    let branch = None;
    let source = None;

    for value in read_json_lines(path, 200)? {
        if let Some(timestamp) = value.get("timestamp").and_then(Value::as_str) {
            apply_timestamp(&mut created_at, &mut updated_at, timestamp);
        }

        match value.get("type").and_then(Value::as_str) {
            Some("session") => {
                if let Some(header_id) = value.get("id").and_then(Value::as_str) {
                    id = header_id.to_string();
                }
                cwd = value.get("cwd").and_then(Value::as_str).map(PathBuf::from);
                if let Some(timestamp) = value.get("timestamp").and_then(Value::as_str) {
                    apply_timestamp(&mut created_at, &mut updated_at, timestamp);
                }
            }
            Some("model_change") => {
                model = value
                    .get("modelId")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
            }
            Some("message") => {
                message_count += 1;
                if let Some(message_model) = value
                    .get("message")
                    .and_then(|message| message.get("model"))
                    .and_then(Value::as_str)
                {
                    model = Some(message_model.to_string());
                }
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
        created_at: created_at.or_else(|| metadata.created().ok()),
        updated_at: updated_at.or_else(|| metadata.modified().ok()),
        model,
        branch,
        source,
        is_subsession: false,
        parent_session_id: None,
    }))
}

fn apply_timestamp(
    created_at: &mut Option<SystemTime>,
    updated_at: &mut Option<SystemTime>,
    value: &str,
) {
    let Some(parsed) = parse_timestamp(value) else {
        return;
    };

    if created_at.is_none_or(|current| parsed < current) {
        *created_at = Some(parsed);
    }
    if updated_at.is_none_or(|current| parsed > current) {
        *updated_at = Some(parsed);
    }
}

fn parse_timestamp(value: &str) -> Option<SystemTime> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc).into())
        .ok()
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
