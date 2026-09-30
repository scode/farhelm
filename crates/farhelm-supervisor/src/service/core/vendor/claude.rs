//! Claude Code's stateful supervisor behavior: its legacy positional
//! admission check, which binds a report to the session's foreground process.

use super::super::*;

impl Supervisor {
    /// Claude's legacy positional admission check: the report must come
    /// from the session's foreground process (the pane process or its direct
    /// child ran the hook), or it is refused and logged.
    pub(in crate::service::core) async fn attribute_claude_report(
        &self,
        row: &StoredSession,
        peer: Option<crate::procs::ProcessIdentity>,
        id: &str,
        generation: i64,
        source: &str,
    ) -> Result<(), RequestError> {
        let peer = peer.ok_or_else(|| {
            RequestError::new(
                ErrorKind::Conflict,
                "the Claude report has no kernel-attributed local process",
            )
        })?;
        let emitter = self
            .claude_foreground(row, peer)
            .await
            .inspect_err(|error| {
                warn!(target: LOG_TARGET,
                    session = %id, generation, source = %source, error = %error,
                    "refused a Claude conversation report that did not come from this \
                     session's foreground process"
                );
            })?;
        info!(target: LOG_TARGET,
            session = %id, generation, emitter_pid = emitter.pid,
            "attributed a Claude foreground conversation report"
        );
        Ok(())
    }

    /// Bind the socket peer to the Claude session's foreground by position
    /// (see `procs::claude_corridor`), after the shared owned-pane lookup
    /// that also covers the publication gap Claude's startup report lands
    /// in. One pass suffices: unlike Codex, Claude admission reads no
    /// vendor file between attribution and the write, so a repeat walk
    /// would guard nothing. No lifecycle lock, for the same reason as
    /// [`Supervisor::codex_foreground`].
    pub(in crate::service::core) async fn claude_foreground(
        &self,
        row: &StoredSession,
        peer: crate::procs::ProcessIdentity,
    ) -> Result<crate::procs::ProcessIdentity, RequestError> {
        let pid = self.owned_pane_pid(row, "Claude").await?;
        tokio::task::spawn_blocking(move || crate::procs::foreground_claude_emitter(peer, pid))
            .await
            .map_err(|_| {
                RequestError::new(
                    ErrorKind::Internal,
                    "Claude process attribution could not complete",
                )
            })?
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))
    }
}
