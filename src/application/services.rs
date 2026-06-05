use anyhow::Result;

use crate::application::ports::SessionRepository;
use crate::domain::{
    AgentSession, ListSessionsQuery, RemoveSessionCommand, RemoveSessionResult, SessionScope,
};

pub struct ListSessionsService<R> {
    repository: R,
}

pub struct RemoveSessionService<R> {
    repository: R,
}

impl<R> RemoveSessionService<R>
where
    R: SessionRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn execute(&self, command: RemoveSessionCommand) -> Result<RemoveSessionResult> {
        let matches: Vec<_> = self
            .repository
            .list(command.agent, &SessionScope::All)?
            .into_iter()
            .filter(|session| session.id == command.session_id)
            .collect();

        match matches.len() {
            0 => anyhow::bail!(
                "session not found: agent={}, session_id={}",
                command.agent,
                command.session_id
            ),
            1 => {
                let session = matches.into_iter().next().expect("len checked");
                if !command.dry_run {
                    self.repository.move_to_trash(&session.file)?;
                }
                Ok(RemoveSessionResult {
                    agent: session.agent,
                    session_id: session.id,
                    file: session.file,
                    dry_run: command.dry_run,
                })
            }
            count => anyhow::bail!(
                "session id is ambiguous: agent={}, session_id={}, matches={}",
                command.agent,
                command.session_id,
                count
            ),
        }
    }
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
        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::domain::{AgentKind, AgentSession};

    #[derive(Default)]
    struct StubRepository {
        sessions: Vec<AgentSession>,
        trashed: RefCell<Vec<PathBuf>>,
    }

    impl SessionRepository for StubRepository {
        fn list(
            &self,
            _agent: AgentKind,
            _scope: &SessionScope,
        ) -> anyhow::Result<Vec<AgentSession>> {
            Ok(self.sessions.clone())
        }

        fn move_to_trash(&self, path: &Path) -> anyhow::Result<()> {
            self.trashed.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }

    #[test]
    fn remove_session_dry_run_does_not_delete() {
        let repository = StubRepository {
            sessions: vec![session("abc")],
            ..Default::default()
        };
        let service = RemoveSessionService::new(repository);

        let result = service
            .execute(RemoveSessionCommand {
                agent: AgentKind::Claude,
                session_id: "abc".to_string(),
                dry_run: true,
            })
            .unwrap();

        assert!(result.dry_run);
        assert_eq!(result.session_id, "abc");
        assert!(service.repository.trashed.borrow().is_empty());
    }

    #[test]
    fn remove_session_rejects_missing_session() {
        let service = RemoveSessionService::new(StubRepository::default());

        let error = service
            .execute(RemoveSessionCommand {
                agent: AgentKind::Claude,
                session_id: "missing".to_string(),
                dry_run: true,
            })
            .unwrap_err();

        assert!(error.to_string().contains("session not found"));
    }

    #[test]
    fn remove_session_rejects_ambiguous_session_id() {
        let repository = StubRepository {
            sessions: vec![session("abc"), session("abc")],
            ..Default::default()
        };
        let service = RemoveSessionService::new(repository);

        let error = service
            .execute(RemoveSessionCommand {
                agent: AgentKind::Claude,
                session_id: "abc".to_string(),
                dry_run: true,
            })
            .unwrap_err();

        assert!(error.to_string().contains("ambiguous"));
    }

    fn session(id: &str) -> AgentSession {
        AgentSession {
            agent: AgentKind::Claude,
            id: id.to_string(),
            cwd: None,
            title: None,
            file: PathBuf::from(format!("/tmp/{id}.jsonl")),
            message_count: 1,
            created_at: None,
            updated_at: None,
            model: None,
            branch: None,
            source: None,
            is_subsession: false,
            parent_session_id: None,
        }
    }
}
