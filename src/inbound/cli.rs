use std::path::PathBuf;
use std::time::SystemTime;
use std::{env, io, io::Write};

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use tracing_subscriber::{EnvFilter, fmt};

use crate::application::services::{ListSessionsService, RemoveSessionService};
use crate::domain::{
    AgentKind, AgentSession, ListSessionsQuery, RemoveSessionCommand, RemoveSessionResult,
    SessionScope,
};
use crate::outbound::filesystem::FilesystemSessionRepository;

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(flatten)]
    list: ListArgs,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List agent sessions.
    List(ListArgs),
    /// Remove an agent session transcript.
    Rm {
        /// Agent session source.
        #[arg(long, value_enum)]
        agent: AgentArg,

        /// Exact session id to remove.
        #[arg(long)]
        session_id: String,

        /// Show the target without moving it to trash.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Parser)]
struct ListArgs {
    /// Agent session source.
    #[arg(long, value_enum)]
    agent: Option<AgentArg>,

    /// Show sessions for this path. Defaults to the current working directory.
    #[arg(long, conflicts_with = "all_paths")]
    path: Option<PathBuf>,

    /// Show sessions for all agents. This is the default when --agent is omitted.
    #[arg(long, conflicts_with = "agent")]
    all_agents: bool,

    /// Show sessions across all directories instead of only the current path.
    #[arg(long)]
    all_paths: bool,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output: OutputFormat,
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
        Some(Commands::List(args)) => run_list(args)?,
        Some(Commands::Rm {
            agent,
            session_id,
            dry_run,
        }) => {
            let service = RemoveSessionService::new(FilesystemSessionRepository::default());
            let result = service.execute(RemoveSessionCommand {
                agent: agent.into(),
                session_id,
                dry_run,
            })?;
            print_remove_result(&result)?;
        }
        None => run_list(cli.list)?,
    }

    Ok(())
}

fn run_list(args: ListArgs) -> Result<()> {
    let scope = resolve_scope(args.path, args.all_paths)?;
    let service = ListSessionsService::new(FilesystemSessionRepository::default());
    let sessions = match args.agent {
        Some(agent) => service.execute(ListSessionsQuery {
            agent: agent.into(),
            scope,
        })?,
        None => list_all_agents(&service, scope)?,
    };
    print_sessions(&sessions, args.output)
}

fn resolve_scope(path: Option<PathBuf>, all_paths: bool) -> Result<SessionScope> {
    if all_paths {
        return Ok(SessionScope::All);
    }

    Ok(SessionScope::Path(resolve_scope_path(path)?))
}

fn resolve_scope_path(path: Option<PathBuf>) -> Result<PathBuf> {
    let path = match path {
        Some(path) => path,
        None => env::current_dir()?,
    };
    Ok(path.canonicalize().unwrap_or(path))
}

fn list_all_agents(
    service: &ListSessionsService<FilesystemSessionRepository>,
    scope: SessionScope,
) -> Result<Vec<AgentSession>> {
    let mut sessions = Vec::new();
    for agent in AgentKind::ALL {
        sessions.extend(service.execute(ListSessionsQuery {
            agent,
            scope: scope.clone(),
        })?);
    }
    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(sessions)
}

fn print_sessions(sessions: &[AgentSession], output: OutputFormat) -> Result<()> {
    match output {
        OutputFormat::Text => print_text_sessions(sessions),
        OutputFormat::Csv => print_csv_sessions(sessions),
        OutputFormat::Json => print_json_sessions(sessions),
    }
}

fn print_remove_result(result: &RemoveSessionResult) -> Result<()> {
    let status = if result.dry_run {
        "would remove"
    } else {
        "removed"
    };
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_line(
        &mut writer,
        &format!(
            "{} session: agent={} session_id={} file={}",
            status,
            result.agent,
            result.session_id,
            result.file.display()
        ),
    )
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
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(&mut bytes);
        writer.write_record([
            "agent",
            "session_id",
            "title",
            "cwd",
            "file_path",
            "message_count",
            "created_at",
            "updated_at",
            "model",
            "branch",
            "source",
            "is_subsession",
            "parent_session_id",
        ])?;
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
