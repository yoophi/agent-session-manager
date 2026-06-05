use anyhow::Result;

fn main() -> Result<()> {
    agent_sessions::inbound::cli::init_tracing();
    agent_sessions::inbound::cli::run(agent_sessions::inbound::cli::Cli::parse_args())
}
