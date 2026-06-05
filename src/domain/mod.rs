use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::SystemTime;

use anyhow::{anyhow, bail};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    Claude,
    Codex,
    Pi,
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Pi => "pi",
        };
        f.write_str(value)
    }
}

impl FromStr for AgentKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "pi" => Ok(Self::Pi),
            other => bail!("unsupported agent: {other}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentSession {
    pub agent: AgentKind,
    pub id: String,
    pub cwd: Option<PathBuf>,
    pub title: Option<String>,
    pub file: PathBuf,
    pub message_count: usize,
    pub created_at: Option<SystemTime>,
    pub updated_at: Option<SystemTime>,
    pub model: Option<String>,
    pub branch: Option<String>,
    pub source: Option<String>,
    pub is_subsession: bool,
    pub parent_session_id: Option<String>,
}

#[derive(Clone, Debug)]
pub enum SessionScope {
    All,
    Path(PathBuf),
}

#[derive(Clone, Debug)]
pub struct ListSessionsQuery {
    pub agent: AgentKind,
    pub scope: SessionScope,
}

pub fn home_dir() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))
}
