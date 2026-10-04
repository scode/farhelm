//! Claude Code's stateful supervisor behavior: its legacy positional
//! admission check, which binds a report to the session's foreground process.

use super::super::*;

impl Supervisor {
    /// Claude's legacy positional admission check: the report's recorded
    /// chain must show it came from the session's foreground process (the
    /// pane process or its direct child ran the hook), or it is refused and
    /// logged. The chain arrives already anchored at the current pane, so
    /// this reads only recorded evidence: no tmux or process inspection, and
    /// no repeat (Claude admission reads no vendor file a second walk could
    /// guard).
    pub(in crate::service::core) fn attribute_claude_report(
        &self,
        ancestry: Option<&[crate::procs::ChainLink]>,
        id: &str,
        generation: i64,
        source: &str,
    ) -> Result<(), RequestError> {
        let chain = ancestry.ok_or_else(|| {
            RequestError::new(
                ErrorKind::Conflict,
                "the Claude report carries no process ancestry reaching this session's pane",
            )
        })?;
        let emitter = crate::procs::claude_corridor(chain)
            .map_err(|reason| RequestError::new(ErrorKind::Conflict, reason))
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
}
