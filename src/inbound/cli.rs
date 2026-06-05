use std::path::PathBuf;
use std::time::SystemTime;
use std::{io, io::Write};

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use tracing_subscriber::{EnvFilter, fmt};

use crate::application::services::ListSessionsService;
use crate::domain::{AgentKind, AgentSession, ListSessionsQuery, SessionScope};
use crate::outbound::filesystem::FilesystemSessionRepository;

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List agent sessions.
    List {
        /// Agent session source.
        #[arg(long, value_enum)]
        agent: AgentArg,

        /// Show sessions for this working directory only.
        #[arg(long, conflicts_with = "all")]
        path: Option<PathBuf>,

        /// Show all sessions. This is the default.
        #[arg(long, default_value_t = true)]
        all: bool,

        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        output: OutputFormat,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AgentArg {
    Claude,
    Codex,
    Pi,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Csv,
    Json,
}

impl From<AgentArg> for AgentKind {
    fn from(value: AgentArg) -> Self {
        match value {
            AgentArg::Claude => Self::Claude,
            AgentArg::Codex => Self::Codex,
            AgentArg::Pi => Self::Pi,
        }
    }
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::List {
            agent,
            path,
            output,
            ..
        } => {
            let scope = match path {
                Some(path) => SessionScope::Path(path.canonicalize().unwrap_or(path)),
                None => SessionScope::All,
            };
            let query = ListSessionsQuery {
                agent: agent.into(),
                scope,
            };
            let service = ListSessionsService::new(FilesystemSessionRepository::default());
            let sessions = service.execute(query)?;
            print_sessions(&sessions, output)?;
        }
    }

    Ok(())
}

fn print_sessions(sessions: &[AgentSession], output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Text => print_text_sessions(sessions),
        OutputFormat::Csv => print_csv_sessions(sessions),
        OutputFormat::Json => print_json_sessions(sessions),
    }
}

fn print_text_sessions(sessions: &[AgentSession]) -> Result<()> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    write_line(
        &mut writer,
        "AGENT\tSESSION_ID\tMESSAGES\tUPDATED_AT\tCWD\tFILE\tTITLE",
    )?;

    for session in sessions {
        write_line(&mut writer, &format_session_row(session))?;
    }

    Ok(())
}

fn print_csv_sessions(sessions: &[AgentSession]) -> Result<()> {
    let mut bytes = Vec::new();
    {
        let mut writer = csv::Writer::from_writer(&mut bytes);
        for session in sessions {
            writer.serialize(SessionOutput::from(session))?;
        }
        writer.flush()?;
    }
    write_bytes(&bytes)
}

fn print_json_sessions(sessions: &[AgentSession]) -> Result<()> {
    let output = SessionsOutput {
        sessions: sessions.iter().map(SessionOutput::from).collect(),
    };
    let bytes = serde_json::to_vec_pretty(&output)?;
    write_bytes(&bytes)?;
    write_bytes(b"\n")
}

fn write_line(writer: &mut impl Write, line: &str) -> Result<()> {
    match writeln!(writer, "{line}") {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn write_bytes(bytes: &[u8]) -> Result<()> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    match writer.write_all(bytes) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn format_session_row(session: &AgentSession) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        session.agent,
        session.id,
        session.message_count,
        display_optional(format_system_time(session.updated_at)),
        session
            .cwd
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string()),
        session.file.display(),
        session.title.as_deref().unwrap_or("-")
    )
}

#[derive(Debug, Serialize)]
struct SessionsOutput {
    sessions: Vec<SessionOutput>,
}

#[derive(Debug, Serialize)]
struct SessionOutput {
    agent: String,
    session_id: String,
    title: Option<String>,
    cwd: Option<String>,
    file_path: String,
    message_count: usize,
    created_at: Option<String>,
    updated_at: Option<String>,
    model: Option<String>,
    branch: Option<String>,
    source: Option<String>,
    is_subsession: bool,
    parent_session_id: Option<String>,
}

impl From<&AgentSession> for SessionOutput {
    fn from(session: &AgentSession) -> Self {
        Self {
            agent: session.agent.to_string(),
            session_id: session.id.clone(),
            title: session.title.clone(),
            cwd: session.cwd.as_ref().map(|path| path.display().to_string()),
            file_path: session.file.display().to_string(),
            message_count: session.message_count,
            created_at: format_system_time(session.created_at),
            updated_at: format_system_time(session.updated_at),
            model: session.model.clone(),
            branch: session.branch.clone(),
            source: session.source.clone(),
            is_subsession: session.is_subsession,
            parent_session_id: session.parent_session_id.clone(),
        }
    }
}

fn format_system_time(value: Option<SystemTime>) -> Option<String> {
    value.map(|value| {
        let value: DateTime<Utc> = value.into();
        value.to_rfc3339()
    })
}

fn display_optional(value: Option<String>) -> String {
    value.unwrap_or_else(|| "-".to_string())
}
