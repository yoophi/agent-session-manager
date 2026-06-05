use anyhow::Result;

use crate::domain::{AgentKind, AgentSession, SessionScope};

pub trait SessionRepository {
    fn list(&self, agent: AgentKind, scope: &SessionScope) -> Result<Vec<AgentSession>>;
}
