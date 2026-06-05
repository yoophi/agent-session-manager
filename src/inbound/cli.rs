use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{io, io::Write};

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{EnvFilter, fmt};

use crate::application::services::ListSessionsService;
use crate::domain::{AgentKind, ListSessionsQuery, SessionScope};
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
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AgentArg {
    Claude,
    Codex,
    Pi,
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
        Commands::List { agent, path, .. } => {
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
            print_sessions(&sessions)?;
        }
    }

    Ok(())
}

fn print_sessions(sessions: &[crate::domain::AgentSession]) -> Result<()> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    write_line(
        &mut writer,
        "AGENT\tID\tMESSAGES\tMODIFIED\tCWD\tFILE\tTITLE",
    )?;

    for session in sessions {
        write_line(&mut writer, &format_session_row(session))?;
    }

    Ok(())
}

fn write_line(writer: &mut impl Write, line: &str) -> Result<()> {
    match writeln!(writer, "{line}") {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn format_session_row(session: &crate::domain::AgentSession) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        session.agent,
        session.id,
        session.message_count,
        format_system_time(session.modified_at),
        session
            .cwd
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string()),
        session.file.display(),
        session.title.as_deref().unwrap_or("-")
    )
}

fn format_system_time(value: Option<SystemTime>) -> String {
    let Some(value) = value else {
        return "-".to_string();
    };

    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "-".to_string(),
    }
}
