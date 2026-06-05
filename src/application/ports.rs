use anyhow::Result;

use std::path::Path;

use crate::domain::{AgentKind, AgentSession, SessionScope};

pub trait SessionRepository {
    fn list(&self, agent: AgentKind, scope: &SessionScope) -> Result<Vec<AgentSession>>;
    fn move_to_trash(&self, path: &Path) -> Result<()>;
}
