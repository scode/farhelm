//! Unseen setup and Update outcomes, independent of connection and busy state.
//!
//! This is presentation bookkeeping. Callers first apply the provisioning read
//! gate and target checks; nothing here admits work or decides whether a run
//! succeeded. Opening Hosts acknowledges an occurrence without deleting its
//! identity, otherwise the next read of a retained failure would relatch it.

use crate::api::{ProvisioningOperation, ProvisioningStatus, ProvisioningView};

/// One host's two independent reasons to visit Hosts. Uninstall keeps its
/// existing notices and is deliberately outside this warning contract.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HostAttention {
    setup: ActionAttention,
    update: ActionAttention,
}

/// A revision orders local attention occurrences, never backend run ids.
/// A recovery records the revision it can clear so a late success cannot
/// erase a newer planning failure or uncertainty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ActionAttention {
    revision: u64,
    unseen: bool,
    cause_run: Option<String>,
    failed_run: Option<String>,
    uncertain_run: Option<String>,
    /// A delayed receipt and an already observed run are independent evidence.
    /// Neither may replace the other's original recovery revision.
    accepted_recovery: Option<(String, u64)>,
    observed_recovery: Option<(String, u64)>,
    observed: Option<(String, ProvisioningStatus)>,
}

impl HostAttention {
    /// Keep the exhaustive action boundary in one place; a new operation
    /// must explicitly choose whether it belongs to this warning contract.
    fn action(&self, operation: ProvisioningOperation) -> Option<&ActionAttention> {
        match operation {
            ProvisioningOperation::Add => Some(&self.setup),
            ProvisioningOperation::Update => Some(&self.update),
            ProvisioningOperation::Uninstall => None,
        }
    }

    /// The mutable counterpart shares the same deliberately narrow boundary.
    fn action_mut(&mut self, operation: ProvisioningOperation) -> Option<&mut ActionAttention> {
        match operation {
            ProvisioningOperation::Add => Some(&mut self.setup),
            ProvisioningOperation::Update => Some(&mut self.update),
            ProvisioningOperation::Uninstall => None,
        }
    }

    /// Snapshot what an admitted submission could recover. Taking this
    /// snapshot acknowledges nothing; matching Completed evidence is still
    /// required when its receipt eventually installs.
    pub(crate) fn revision(&self, operation: ProvisioningOperation) -> u64 {
        self.action(operation).map_or(0, |action| action.revision)
    }

    /// The rail warns if either action has an unacknowledged outcome.
    pub(crate) fn unseen(&self) -> bool {
        self.setup.unseen || self.update.unseen
    }

    /// Acknowledge both actions without forgetting their retained failures.
    pub(crate) fn acknowledge(&mut self) {
        self.setup.unseen = false;
        self.update.unseen = false;
    }

    /// Install one definite failure or uncertain submission outcome. This is
    /// called once by its already-correlated callback, never from error text.
    pub(crate) fn failed_attempt(&mut self, operation: ProvisioningOperation, open: bool) {
        if let Some(action) = self.action_mut(operation) {
            action.raise(None, open);
        }
    }

    /// Pin a recovery run to the warning revision captured before its POST
    /// or setup discovery. Neither a 202 nor discovery alone clears it: an
    /// authoritative Completed view for that exact run must also exist. The
    /// view may have arrived before a delayed decoded acceptance.
    pub(crate) fn accepted(&mut self, operation: ProvisioningOperation, run: &str, revision: u64) {
        if !run.is_empty()
            && let Some(action) = self.action_mut(operation)
        {
            // Running may have arrived before this receipt. The exact same
            // run must keep its known admission revision in either arrival
            // order; a different observed retry keeps its independent ticket.
            if let Some((observed, captured)) = &mut action.observed_recovery
                && observed == run
            {
                *captured = (*captured).min(revision);
            }
            action.accepted_recovery = Some((run.to_owned(), revision));
            if revision == action.revision
                && action.observed.as_ref().is_some_and(|(seen, status)| {
                    seen == run && *status == ProvisioningStatus::Completed
                })
            {
                action.unseen = false;
                action.accepted_recovery = None;
                action.observed_recovery = None;
            }
        }
    }

    /// Lost progress is uncertainty only for an actual known action/run.
    /// Remembering it across acknowledgement bounds repeated failed polls to
    /// one occurrence rather than treating each network error as a new failure.
    pub(crate) fn unreadable(&mut self, open: bool) {
        for action in [&mut self.setup, &mut self.update] {
            if let Some(run) = action.known_nonterminal_run() {
                action.uncertain(&run, open);
            }
        }
    }

    /// Apply an authoritative, applicable progress view after the existing
    /// lifecycle checks. Completed history alone never establishes recovery.
    /// Setup discovery may complete its exact retained failed run in place.
    pub(crate) fn observe(&mut self, view: &ProvisioningView, open: bool) {
        // A matching current ticket is already correlated evidence. Resolve it
        // before treating a competing receipt or observation as missing.
        for (operation, action) in [
            (ProvisioningOperation::Add, &mut self.setup),
            (ProvisioningOperation::Update, &mut self.update),
        ] {
            if view.operation == Some(operation)
                && view
                    .run_id
                    .as_deref()
                    .is_some_and(|run| action.recovers(run))
            {
                continue;
            }
            if let Some(run) = action.known_nonterminal_run()
                && view.run_id.as_deref() != Some(run.as_str())
            {
                action.uncertain(&run, open);
                // Keep the last identity and its original ticket. Idle or
                // another action does not make a repeated old Running view
                // a new retry entitled to clear an intervening failure.
            }
        }
        self.apply_view(view, open);
    }

    /// Publish one applicable view after missing-run uncertainty is accounted
    /// for. Only first Running evidence can mint an observed recovery ticket.
    fn apply_view(&mut self, view: &ProvisioningView, open: bool) {
        let Some(operation) = view.operation else {
            return;
        };
        let Some(run) = view.run_id.as_deref().filter(|run| !run.is_empty()) else {
            return;
        };
        let Some(action) = self.action_mut(operation) else {
            return;
        };
        match view.status {
            ProvisioningStatus::Running => {
                if action.observed.as_ref().is_none_or(|(seen, _)| seen != run) {
                    // First Running is not a new admission when this exact
                    // run was already accepted. Carry its original ticket so
                    // progress cannot promote an old run over a newer failure.
                    let revision = action
                        .accepted_recovery
                        .as_ref()
                        .filter(|(accepted, _)| accepted == run)
                        .map_or(action.revision, |(_, revision)| *revision);
                    action.observed_recovery = Some((run.to_owned(), revision));
                }
            }
            ProvisioningStatus::Failed => {
                if action.failed_run.as_deref() != Some(run) {
                    action.failed_run = Some(run.to_owned());
                    action.raise(Some(run.to_owned()), open);
                }
                if action
                    .accepted_recovery
                    .as_ref()
                    .is_some_and(|(candidate, _)| candidate == run)
                {
                    action.accepted_recovery = None;
                }
                action.observed_recovery = None;
            }
            ProvisioningStatus::Completed => {
                let own_outcome = action.cause_run.as_deref() == Some(run)
                    && (operation == ProvisioningOperation::Add
                        || action.uncertain_run.as_deref() == Some(run));
                if own_outcome || action.recovers(run) {
                    action.unseen = false;
                    // A matching same-action result settles this occurrence.
                    // A competing delayed receipt must not turn the next idle
                    // read into another warning for the settled occurrence.
                    action.accepted_recovery = None;
                    action.observed_recovery = None;
                }
                if action
                    .accepted_recovery
                    .as_ref()
                    .is_some_and(|(candidate, _)| candidate == run)
                {
                    action.accepted_recovery = None;
                }
                if action
                    .observed_recovery
                    .as_ref()
                    .is_some_and(|(candidate, _)| candidate == run)
                {
                    action.observed_recovery = None;
                }
            }
        }
        action.observed = Some((run.to_owned(), view.status));
    }
}

impl ActionAttention {
    /// Either accepted or observed evidence can settle its original occurrence.
    /// Run identities and equal revisions do not establish backend age.
    fn recovers(&self, run: &str) -> bool {
        [&self.accepted_recovery, &self.observed_recovery]
            .into_iter()
            .any(|ticket| {
                ticket.as_ref().is_some_and(|(candidate, revision)| {
                    candidate == run && *revision == self.revision
                })
            })
    }

    /// Prefer observed current progress when its ticket is at least as recent
    /// as the receipt's. A delayed older receipt cannot hide a read loss for
    /// that observed candidate. Matching terminal snapshots are settled.
    fn known_nonterminal_run(&self) -> Option<String> {
        if let Some((run, revision)) = &self.observed_recovery
            && self
                .observed
                .as_ref()
                .is_some_and(|(seen, status)| seen == run && *status == ProvisioningStatus::Running)
            && self
                .accepted_recovery
                .as_ref()
                .is_none_or(|(_, accepted_revision)| revision >= accepted_revision)
        {
            return Some(run.clone());
        }
        self.accepted_recovery
            .as_ref()
            .filter(|(candidate, _)| {
                self.observed.as_ref().is_none_or(|(seen, status)| {
                    seen != candidate || *status == ProvisioningStatus::Running
                })
            })
            .map(|(run, _)| run.clone())
            .or_else(|| {
                self.observed
                    .as_ref()
                    .filter(|(_, status)| *status == ProvisioningStatus::Running)
                    .map(|(run, _)| run.clone())
            })
    }

    /// Replacing a cause also prevents that cause's older recovery candidate
    /// from acknowledging this newer occurrence.
    fn raise(&mut self, run: Option<String>, open: bool) {
        self.revision += 1;
        self.unseen = !open;
        self.cause_run = run;
    }

    /// A run can become unreadable many times; only the first loss is a new
    /// occurrence until a different run supplies a different identity.
    fn uncertain(&mut self, run: &str, open: bool) {
        if self.uncertain_run.as_deref() != Some(run) {
            self.uncertain_run = Some(run.to_owned());
            // Losing an older candidate must not replace an intervening
            // failure with a cause that this candidate could then clear.
            let revision = [&self.accepted_recovery, &self.observed_recovery]
                .into_iter()
                .filter_map(|ticket| {
                    ticket
                        .as_ref()
                        .filter(|(candidate, _)| candidate == run)
                        .map(|(_, revision)| *revision)
                })
                .max();
            if revision.is_some_and(|revision| revision < self.revision)
                && self.cause_run.as_deref() != Some(run)
            {
                return;
            }
            self.raise(Some(run.to_owned()), open);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An accepted run's first Running snapshot can arrive after a newer
    /// failure. That snapshot must retain admission's old recovery revision;
    /// only a distinct later run may recover the intervening failure.
    #[test]
    fn first_running_keeps_the_same_accepted_runs_original_revision() {
        for operation in [ProvisioningOperation::Add, ProvisioningOperation::Update] {
            for acceptance_at in ["before failure", "before Running", "after Running"] {
                let mut attention = HostAttention::default();
                if acceptance_at == "before failure" {
                    attention.accepted(operation, "old", 0);
                }
                attention.failed_attempt(operation, false);
                if acceptance_at == "before Running" {
                    attention.accepted(operation, "old", 0);
                }
                let revision = attention.revision(operation);
                attention.observe(&view(operation, "old", ProvisioningStatus::Running), false);
                if acceptance_at == "after Running" {
                    attention.accepted(operation, "old", 0);
                }
                attention.observe(
                    &view(operation, "old", ProvisioningStatus::Completed),
                    false,
                );
                assert!(attention.unseen());
                assert_eq!(attention.revision(operation), revision);
                attention.observe(&view(operation, "new", ProvisioningStatus::Running), false);
                attention.observe(
                    &view(operation, "new", ProvisioningStatus::Completed),
                    false,
                );
                assert!(!attention.unseen());
            }
        }
    }

    /// A delayed receipt for an earlier setup must not steal a later retry's
    /// current recovery ticket. The matching completion also retires the
    /// competing receipt, so the following idle read cannot relatch it.
    #[test]
    fn old_acceptance_preserves_a_newer_observed_recovery() {
        let operation = ProvisioningOperation::Add;
        let mut attention = HostAttention::default();
        attention.observe(
            &view(operation, "failed-b", ProvisioningStatus::Failed),
            false,
        );
        attention.observe(
            &view(operation, "retry-c", ProvisioningStatus::Running),
            false,
        );
        attention.accepted(operation, "old-a", 0);
        attention.observe(
            &view(operation, "retry-c", ProvisioningStatus::Running),
            false,
        );
        attention.observe(
            &view(operation, "retry-c", ProvisioningStatus::Completed),
            false,
        );
        assert!(!attention.unseen());
        attention.observe(&idle(), false);
        assert!(!attention.unseen());
    }

    /// Receipt and observation arrival order cannot order backend runs. At an
    /// equal warning revision either exact completion can settle the warning;
    /// accepting one must not forget evidence for the other or invent a loss.
    #[test]
    fn equal_revision_receipt_and_observation_keep_both_recovery_paths() {
        for warned in [false, true] {
            for completed in ["observed", "accepted"] {
                let operation = ProvisioningOperation::Add;
                let mut attention = HostAttention::default();
                if warned {
                    attention.failed_attempt(operation, false);
                }
                let revision = attention.revision(operation);
                attention.observe(
                    &view(operation, "observed", ProvisioningStatus::Running),
                    false,
                );
                attention.accepted(operation, "accepted", revision);
                attention.observe(
                    &view(operation, "observed", ProvisioningStatus::Running),
                    false,
                );
                assert_eq!(attention.revision(operation), revision);
                attention.observe(
                    &view(operation, completed, ProvisioningStatus::Completed),
                    false,
                );
                assert!(!attention.unseen());
                attention.observe(&idle(), false);
                assert!(!attention.unseen());
            }
        }
    }

    /// Losing current observed progress is its own uncertainty even when an
    /// older receipt arrives late. Its eventual exact completion must recover
    /// that loss, without promoting the older ticket to a newer revision.
    #[test]
    fn read_loss_follows_current_observed_progress_despite_a_delayed_receipt() {
        let operation = ProvisioningOperation::Add;
        let mut attention = HostAttention::default();
        attention.observe(
            &view(operation, "current", ProvisioningStatus::Running),
            false,
        );
        attention.accepted(operation, "delayed", 0);
        attention.unreadable(false);
        assert!(attention.unseen());
        attention.observe(
            &view(operation, "current", ProvisioningStatus::Completed),
            false,
        );
        assert!(!attention.unseen());
    }

    /// The helm's no-run snapshot is distinct from Completed history with an id.
    fn idle() -> ProvisioningView {
        ProvisioningView {
            run_id: None,
            operation: None,
            status: ProvisioningStatus::Completed,
            steps: Vec::new(),
            message: None,
        }
    }

    /// A validated acceptance is already a known run. Its first read may be
    /// idle or another run; requiring a prior Running snapshot would hide
    /// that unknown outcome. Repeated mismatching reads retain acknowledgement.
    #[test]
    fn accepted_run_can_disappear_before_any_running_observation() {
        for operation in [ProvisioningOperation::Add, ProvisioningOperation::Update] {
            for missing in [
                ProvisioningView {
                    run_id: None,
                    operation: None,
                    status: ProvisioningStatus::Completed,
                    steps: Vec::new(),
                    message: None,
                },
                view(operation, "other", ProvisioningStatus::Completed),
            ] {
                let mut attention = HostAttention::default();
                attention.accepted(operation, "accepted", 0);
                assert!(!attention.unseen(), "acceptance alone is not a failure");
                attention.observe(&missing, false);
                assert!(attention.unseen());
                attention.acknowledge();
                attention.observe(&missing, false);
                assert!(!attention.unseen());

                let mut newer = HostAttention::default();
                newer.accepted(operation, "accepted", 0);
                newer.failed_attempt(operation, false);
                newer.observe(&missing, false);
                newer.observe(
                    &view(operation, "accepted", ProvisioningStatus::Completed),
                    false,
                );
                assert!(
                    newer.unseen(),
                    "an older accepted run cannot clear the newer failure"
                );
            }
        }
    }

    /// The same retained run must survive acknowledgement without becoming a
    /// new occurrence just because the fleet reader publishes it again.
    #[test]
    fn acknowledgement_remembers_failure_but_new_run_warns() {
        let mut attention = HostAttention::default();
        let failed = view(
            ProvisioningOperation::Update,
            "first",
            ProvisioningStatus::Failed,
        );
        attention.observe(&failed, false);
        assert!(attention.unseen());
        attention.acknowledge();
        attention.observe(&failed, false);
        attention.unreadable(false);
        assert!(!attention.unseen());
        attention.observe(
            &view(
                ProvisioningOperation::Update,
                "second",
                ProvisioningStatus::Failed,
            ),
            false,
        );
        assert!(attention.unseen());
    }

    /// A retry's own success can recover its captured occurrence, but neither
    /// unrelated Completed history nor repeating Running upgrades its ticket.
    #[test]
    fn only_correlated_success_clears_its_own_revision() {
        let mut attention = HostAttention::default();
        attention.failed_attempt(ProvisioningOperation::Update, false);
        attention.observe(
            &view(
                ProvisioningOperation::Update,
                "old",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(attention.unseen());
        let running = view(
            ProvisioningOperation::Update,
            "retry",
            ProvisioningStatus::Running,
        );
        attention.observe(&running, false);
        attention.failed_attempt(ProvisioningOperation::Update, false);
        attention.observe(&running, false);
        attention.observe(
            &view(
                ProvisioningOperation::Add,
                "unrelated",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        attention.observe(
            &view(
                ProvisioningOperation::Update,
                "retry",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(attention.unseen());
        let revision = attention.revision(ProvisioningOperation::Update);
        attention.accepted(ProvisioningOperation::Update, "next", revision);
        assert!(attention.unseen());
        attention.observe(
            &view(
                ProvisioningOperation::Update,
                "next",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(!attention.unseen());
    }

    /// Another client's actual Running→Completed retry can recover attention;
    /// an idle view and Uninstall never stand in for that same-action run.
    #[test]
    fn observed_recovery_requires_a_running_candidate() {
        let mut attention = HostAttention::default();
        attention.failed_attempt(ProvisioningOperation::Update, false);
        let idle = ProvisioningView {
            run_id: None,
            operation: None,
            status: ProvisioningStatus::Completed,
            steps: Vec::new(),
            message: None,
        };
        attention.observe(&idle, false);
        attention.observe(
            &view(
                ProvisioningOperation::Uninstall,
                "other",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(attention.unseen());
        attention.observe(
            &view(
                ProvisioningOperation::Update,
                "external",
                ProvisioningStatus::Running,
            ),
            false,
        );
        assert!(attention.unseen());
        attention.observe(
            &view(
                ProvisioningOperation::Update,
                "external",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(!attention.unseen());
    }

    /// Recovery on another action cannot dismiss setup, while setup discovery
    /// is allowed to complete the same retained failed Add without a new id.
    #[test]
    fn actions_are_independent_and_setup_discovery_recovers_in_place() {
        let mut attention = HostAttention::default();
        attention.observe(
            &view(
                ProvisioningOperation::Add,
                "setup",
                ProvisioningStatus::Failed,
            ),
            false,
        );
        attention.failed_attempt(ProvisioningOperation::Update, false);
        let revision = attention.revision(ProvisioningOperation::Update);
        attention.accepted(ProvisioningOperation::Update, "update", revision);
        attention.observe(
            &view(
                ProvisioningOperation::Update,
                "update",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(attention.unseen());
        attention.observe(
            &view(
                ProvisioningOperation::Add,
                "setup",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(!attention.unseen());
    }

    /// A later discovery can recover a failed setup even after an intervening
    /// planning failure; the unchanged old run id alone was not enough.
    #[test]
    fn discovery_ticket_recovers_newer_planning_failure_only_after_reread() {
        let mut attention = HostAttention::default();
        attention.observe(
            &view(
                ProvisioningOperation::Add,
                "setup",
                ProvisioningStatus::Failed,
            ),
            false,
        );
        attention.failed_attempt(ProvisioningOperation::Add, false);
        let revision = attention.revision(ProvisioningOperation::Add);
        attention.accepted(ProvisioningOperation::Add, "setup", revision);
        assert!(attention.unseen());
        attention.observe(
            &view(
                ProvisioningOperation::Add,
                "setup",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(!attention.unseen());
    }

    /// Registration/progress can outrun the initial Add's acceptance. Both
    /// exact identities together recover the captured occurrence; neither an
    /// unrelated receipt nor a ticket predating a newer failure may do so.
    #[test]
    fn completed_before_acceptance_still_requires_the_exact_current_ticket() {
        let mut attention = HostAttention::default();
        attention.failed_attempt(ProvisioningOperation::Add, false);
        let revision = attention.revision(ProvisioningOperation::Add);
        attention.observe(
            &view(
                ProvisioningOperation::Add,
                "accepted",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(attention.unseen());
        attention.accepted(ProvisioningOperation::Add, "different", revision);
        assert!(attention.unseen());
        attention.failed_attempt(ProvisioningOperation::Add, false);
        attention.accepted(ProvisioningOperation::Add, "accepted", revision);
        assert!(attention.unseen());
        attention.accepted(
            ProvisioningOperation::Add,
            "accepted",
            attention.revision(ProvisioningOperation::Add),
        );
        assert!(!attention.unseen());
    }

    /// A late acceptance may replace a candidate while the last observed run
    /// is still retained. Repeating that earlier Running view must not admit
    /// it again with permission to clear a newer failure.
    #[test]
    fn retained_running_does_not_regain_recovery_after_late_acceptance() {
        let mut attention = HostAttention::default();
        attention.failed_attempt(ProvisioningOperation::Update, false);
        let running = view(
            ProvisioningOperation::Update,
            "earlier",
            ProvisioningStatus::Running,
        );
        attention.observe(&running, false);
        let revision = attention.revision(ProvisioningOperation::Update);
        attention.failed_attempt(ProvisioningOperation::Update, false);
        attention.accepted(ProvisioningOperation::Update, "later-acceptance", revision);
        attention.observe(&running, false);
        attention.observe(
            &view(
                ProvisioningOperation::Update,
                "earlier",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        assert!(attention.unseen());
    }

    /// Poll failures require a known run and are deduplicated independently
    /// of their text. An open management surface acknowledges them at once.
    #[test]
    fn unreadable_known_run_warns_once_and_its_success_recovers() {
        let mut attention = HostAttention::default();
        attention.unreadable(false);
        assert!(!attention.unseen());
        attention.accepted(ProvisioningOperation::Add, "known", 0);
        attention.unreadable(false);
        assert!(attention.unseen());
        attention.acknowledge();
        attention.unreadable(false);
        assert!(!attention.unseen());
        attention.observe(
            &view(
                ProvisioningOperation::Add,
                "known",
                ProvisioningStatus::Completed,
            ),
            false,
        );
        attention.failed_attempt(ProvisioningOperation::Add, true);
        assert!(!attention.unseen());
    }

    /// Controlled domain views keep text out of the identity tests: different
    /// runs deliberately carry the same failure message.
    fn view(
        operation: ProvisioningOperation,
        run: &str,
        status: ProvisioningStatus,
    ) -> ProvisioningView {
        ProvisioningView {
            operation: Some(operation),
            run_id: Some(run.to_owned()),
            status,
            steps: Vec::new(),
            message: Some("same failure".into()),
        }
    }
}
