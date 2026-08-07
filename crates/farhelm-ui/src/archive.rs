//! Archive confirmation policy shared by the list and detail surfaces.
//!
//! Both controls must describe the same shutdown before they ask for
//! consent. Keeping that decision here prevents a session with a dead agent
//! but live tabs from confirming in one place and archiving immediately in
//! the other.

use crate::Session;

/// The protocol's deliberate-stop annotation, mirrored here because the UI
/// decodes the helm's JSON rather than depending on the supervisor protocol
/// crate directly.
const STOP_ANNOTATION: &str = "stopped by user";

/// Explain what archive will stop, or return `None` only when the UI has
/// positive evidence that no owned work remains.
///
/// `visible_tabs` comes from the surface asking the question. The detail
/// view includes optimistic opens and excludes optimistic closes, while a
/// list row has only the server snapshot; deriving it here from
/// `session.tabs` would make the two surfaces disagree about work already
/// visible to the user.
///
/// A natural exit is not proof that the whole process tree ended: an agent
/// can daemonize a child before its own pane exits, and archive's ownership
/// sweep will still kill that child. A completed user stop is different —
/// its annotation is written only after that same sweep succeeds — as are
/// interrupted and launch-error states, whose agent cannot still own a
/// running tree. `Unknown` stays conservative because absence of a
/// classification is not evidence of absence.
pub(crate) fn confirmation(session: &Session, visible_tabs: usize) -> Option<String> {
    enum AgentRisk {
        Live,
        PriorTree,
        None,
    }

    let agent_risk = match &session.status {
        crate::SessionStatus::Exited { .. }
            if session.annotation.as_deref() != Some(STOP_ANNOTATION) =>
        {
            AgentRisk::PriorTree
        }
        status if status.is_live() || matches!(status, crate::SessionStatus::Unknown) => {
            AgentRisk::Live
        }
        _ => AgentRisk::None,
    };
    match (agent_risk, visible_tabs) {
        (AgentRisk::None, 0) => None,
        (AgentRisk::Live, 0) => Some(
            "archiving stops the agent and its whole process tree, then removes the terminal"
                .to_string(),
        ),
        (AgentRisk::PriorTree, 0) => Some(
            "archiving stops any surviving processes from the prior agent's whole process tree, then removes the terminal"
                .to_string(),
        ),
        (AgentRisk::None, 1) => {
            Some("archiving stops 1 terminal tab and removes the terminal".to_string())
        }
        (AgentRisk::None, n) => Some(format!(
            "archiving stops {n} terminal tabs and removes the terminal"
        )),
        (AgentRisk::Live, 1) => Some(
            "archiving stops the agent, its whole process tree, and 1 terminal tab, then removes the terminal"
                .to_string(),
        ),
        (AgentRisk::Live, n) => Some(format!(
            "archiving stops the agent, its whole process tree, and {n} terminal tabs, then removes the terminal"
        )),
        (AgentRisk::PriorTree, 1) => Some(
            "archiving stops any surviving processes from the prior agent's whole process tree and 1 terminal tab, then removes the terminal"
                .to_string(),
        ),
        (AgentRisk::PriorTree, n) => Some(format!(
            "archiving stops any surviving processes from the prior agent's whole process tree and {n} terminal tabs, then removes the terminal"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RestartOffer, SessionStatus, Tab};

    /// An ended agent is not sufficient evidence for an immediate archive
    /// when a retained tab still names a shell archive will destroy.
    #[test]
    fn a_terminal_tab_keeps_the_archive_confirmation_required() {
        let mut session = specimen(SessionStatus::Exited { exit_code: Some(0) }, 1);
        session.annotation = Some(STOP_ANNOTATION.to_string());
        assert_eq!(
            confirmation(&session, 1).as_deref(),
            Some("archiving stops 1 terminal tab and removes the terminal")
        );
    }

    /// A completed stop is the one tabless exit with authoritative evidence
    /// that its owned process tree was already swept.
    #[test]
    fn a_tabless_stopped_session_needs_no_archive_confirmation() {
        let mut session = specimen(SessionStatus::Exited { exit_code: Some(0) }, 0);
        session.annotation = Some(STOP_ANNOTATION.to_string());
        assert!(confirmation(&session, 0).is_none());
    }

    /// A natural exit may have left daemonized children, so archive names
    /// the prior tree even with no tabs left to make the risk obvious.
    #[test]
    fn an_unannotated_exit_confirms_the_prior_process_tree() {
        let session = specimen(SessionStatus::Exited { exit_code: Some(0) }, 0);
        assert!(
            confirmation(&session, 0)
                .expect("natural exits confirm")
                .contains("prior agent's whole process tree")
        );
    }

    /// Live and unknown agents both confirm with zero and plural tabs; the
    /// latter is conservative because unclassified does not mean dead.
    #[test]
    fn live_and_unknown_agents_name_zero_and_multiple_tabs() {
        for status in [SessionStatus::Running, SessionStatus::Unknown] {
            let session = specimen(status, 3);
            let without_tabs = confirmation(&session, 0).expect("agent risk confirms");
            assert!(without_tabs.contains("agent") && !without_tabs.contains("terminal tabs"));
            let with_tabs = confirmation(&session, 3).expect("tabs confirm");
            assert!(with_tabs.contains("whole process tree"));
            assert!(with_tabs.contains("3 terminal tabs"));
        }
    }

    /// Build only the fields the confirmation policy reads while preserving
    /// a realistic session shape for future additions to that policy.
    fn specimen(status: SessionStatus, tabs: usize) -> Session {
        Session {
            id: "session-1".to_string(),
            title: "archive me".to_string(),
            cwd: "/tmp".to_string(),
            invocation: "agent".to_string(),
            status,
            annotation: None,
            restart_offer: RestartOffer::FreshOnly,
            archived: false,
            tabs: (0..tabs)
                .map(|index| Tab {
                    id: format!("tab-{index}"),
                })
                .collect(),
            host: None,
            host_name: None,
            stale: false,
            source_profile: None,
        }
    }
}
