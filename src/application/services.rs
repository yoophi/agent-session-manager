use anyhow::Result;

use crate::application::ports::SessionRepository;
use crate::domain::{AgentSession, ListSessionsQuery};

pub struct ListSessionsService<R> {
    repository: R,
}

impl<R> ListSessionsService<R>
where
    R: SessionRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn execute(&self, query: ListSessionsQuery) -> Result<Vec<AgentSession>> {
        let mut sessions = self.repository.list(query.agent, &query.scope)?;
        sessions.sort_by(|left, right| right.modified_at.cmp(&left.modified_at));
        Ok(sessions)
    }
}
