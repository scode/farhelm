//! Session notifications: telling the user, on the session itself, that
//! Farhelm has lost track of that session's agent conversation, or that
//! something in the session still runs in an old terminal.
//!
//! SPEC.md (Status) has the product contract and SPEC_impl.md the storage
//! and transport. In short: a problem the supervisor notices is recorded in
//! its store (`session_notifications`, at most 10 per session, at most one per
//! kind per launch, and the old-terminal kind at most one unresolved per
//! session; see below), copied into the session's in-memory cell
//! ([`super::core::SessionCells::notifications`]) so every reply carries it,
//! and announced to connected helms with the ordinary `SessionsChanged` hint.
//! The helm keeps read and cleared state; nothing here knows about either.
//! The two problems that cease when Resume returns are reconciled on each
//! capture pass. Resolution keeps the history; recurrence reopens the same
//! launch/kind row with a newer sequence rather than adding another entry.
//!
//! # Who records what
//!
//! Each kind is recorded by the code that already notices the problem and
//! already logs it, so a notification is never a second, independent guess
//! at the same fact:
//!
//! - [`NotificationKind::HookSilent`]: the capture pass, when the 65-second
//!   tripwire (`capture::report_liveness_tripwire`) fires for a launch.
//! - [`NotificationKind::HookNotAdded`]: the spawn that decided not to add
//!   Farhelm's arguments to a launch, for a reason the user did not choose.
//! - [`NotificationKind::ResumeWithdrawn`]: the Codex and Grok refreshes that
//!   withdraw a resume offer because the agent's own record went missing or
//!   stopped matching.
//! - [`NotificationKind::ReporterMismatch`]: OMP admission, when the launch's
//!   recorded reporter does not match this build.
//! - [`NotificationKind::CodexPinCorrected`]: Codex admission, after a recorded
//!   ancestor's verified record corrects a child-first conversation choice.
//! - [`NotificationKind::ClaudePinCorrected`]: Claude admission, when a recorded
//!   ancestor corrects a child-first conversation choice.
//! - [`NotificationKind::MusePinCorrected`]: the same correction for Muse.
//! - [`NotificationKind::OldTerminal`]: the ticker's sample pass, which is
//!   the one place that already asks tmux, every couple of seconds, what
//!   runs in the private tmux server
//!   ([`Supervisor::reconcile_old_terminal_notifications`]).
//! - [`NotificationKind::OldTerminalEnded`]: the startup step that ends the
//!   old terminals (`super::core::old_terminals`), once for each session
//!   whose agent it found in one, when the reload converts that agent to
//!   interrupted.
//!
//! # The old-terminal notification is per session, not per launch
//!
//! Every other kind describes one launch of the agent, so the store keys it
//! by `(session, generation, kind)` and a Restart starts afresh. The
//! old-terminal warning is about the session: a Restart that moves the
//! agent into a terminal host while old shell tabs stay open must not add a
//! second warning, and the first must stay unresolved until those tabs are
//! gone too. So it is recorded, at the current launch, only while the
//! session has no unresolved one in any launch, and resolved across
//! launches. It is transitional: it goes when old terminals do (TODO.md,
//! "After tmux and xterm.js are dropped entirely").
//!
//! # The wording rule
//!
//! The maintainer's rule (the plan's M5): every text names something the
//! user can actually do, or, when there is nothing, says plainly what they
//! lose. Never a pointer to a log. Tracking failures explain Restart's lost
//! ability to resume; a corrected selection explains which conversation
//! an earlier Restart would have resumed instead.

use super::core::{SessionEntry, Supervisor, session_tmux_name};
use super::terminals::{Terminal, tabs_from_pane_states, tmux_tab_home};
use crate::store::StoredSessionNotification;
use crate::tmux::PaneState;
use farhelm_proto::{AgentKind, ReadinessWording};
use std::collections::HashMap;
use tracing::warn;

/// What kind of problem a notification reports.
///
/// The unit of the one-row-per-launch rule (the store's `(session, generation,
/// kind)` key) and the reason the record does not assume there is only one
/// kind of notification. Kind stays private to the supervisor; peers receive the
/// text and resolved flag without interpreting a kind enum
/// (see [`farhelm_proto::SessionInfo::notifications`] for why).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationKind {
    /// A launch whose conversation hook should have reported, and has not,
    /// a minute after the user first submitted a line.
    HookSilent,
    /// A launch Farhelm could not add its conversation hook or reporter to.
    HookNotAdded,
    /// A resume offer withdrawn because the agent's own conversation record
    /// went missing or stopped matching.
    ResumeWithdrawn,
    /// A launch whose conversation reporter does not match this build, so
    /// its reports cannot be accepted.
    ReporterMismatch,
    /// A verified Codex ancestor corrected a conversation selected by its child.
    /// The warning preserves what a Restart before correction would have used.
    CodexPinCorrected,
    /// A recorded Claude ancestor corrected a conversation selected by its child.
    /// This records the prior Restart target, so later capture does not resolve it.
    ClaudePinCorrected,
    /// A recorded Muse ancestor corrected a conversation selected by its child.
    /// The prior Restart target remains relevant after successful capture.
    MusePinCorrected,
    /// Something in the session still runs in an old terminal (its agent in
    /// a tmux pane, or an open tmux tab), which an upcoming release will
    /// forcibly terminate. One per session, not per launch (module docs).
    /// Kept decodable for stored rows; this release resolves every one at
    /// startup, since it ends the old terminals.
    OldTerminal,
    /// The session's agent ran in an old terminal, which this release ended
    /// at startup. A kind of its own rather than [`Self::OldTerminal`]
    /// reused: every start resolves the old-terminal warnings, and the
    /// notice must stay unresolved until the user reads or clears it, across
    /// later starts. Nothing resolves it.
    OldTerminalEnded,
}

impl NotificationKind {
    const ALL: [Self; 9] = [
        Self::HookSilent,
        Self::HookNotAdded,
        Self::ResumeWithdrawn,
        Self::ReporterMismatch,
        Self::CodexPinCorrected,
        Self::ClaudePinCorrected,
        Self::MusePinCorrected,
        Self::OldTerminal,
        Self::OldTerminalEnded,
    ];

    /// Whether Resume becoming available ends this warning's condition.
    /// Hook setup and reporter mismatch have no such resolving event: a
    /// captured conversation does not prove either of those problems ended.
    fn resolves_on_resume(self) -> bool {
        match self {
            Self::HookSilent | Self::ResumeWithdrawn => true,
            // The old-terminal warning has nothing to do with the
            // conversation; its own check resolves it.
            Self::HookNotAdded
            | Self::ReporterMismatch
            | Self::CodexPinCorrected
            | Self::ClaudePinCorrected
            | Self::MusePinCorrected
            | Self::OldTerminal
            | Self::OldTerminalEnded => false,
        }
    }

    /// Whether this kind is only true while the session's row holds no
    /// captured conversation, which the store then checks in the same
    /// statement as the insert ([`crate::store::SessionStore::record_session_notification`]).
    fn requires_no_identity(self) -> bool {
        match self {
            // Both say Restart will not be able to resume, which is false for
            // a launch that carries a captured conversation (a Restart keeps
            // the one it resumed).
            NotificationKind::HookSilent | NotificationKind::HookNotAdded => true,
            NotificationKind::ResumeWithdrawn
            | NotificationKind::ReporterMismatch
            | NotificationKind::CodexPinCorrected
            | NotificationKind::ClaudePinCorrected
            | NotificationKind::MusePinCorrected
            | NotificationKind::OldTerminal
            | NotificationKind::OldTerminalEnded => false,
        }
    }

    /// The value stored in the `kind` column. Stable: it is part of the
    /// one-row-per-launch key of rows already on disk.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            NotificationKind::HookSilent => "hook_silent",
            NotificationKind::HookNotAdded => "hook_not_added",
            NotificationKind::ResumeWithdrawn => "resume_withdrawn",
            NotificationKind::ReporterMismatch => "reporter_mismatch",
            NotificationKind::CodexPinCorrected => "codex_pin_corrected",
            NotificationKind::ClaudePinCorrected => "claude_pin_corrected",
            NotificationKind::MusePinCorrected => "muse_pin_corrected",
            NotificationKind::OldTerminal => "old_terminal",
            NotificationKind::OldTerminalEnded => "old_terminal_ended",
        }
    }
}

/// The text of a [`NotificationKind::OldTerminal`] notification, in the
/// maintainer's own words (2026-11-07), verbatim: do not reword it without
/// them. One message covers an old agent and old tabs alike, which is why
/// its first sentence also asks for the session to be restarted when only
/// its tabs are old; the maintainer accepted that. It never says tabs
/// "move": the maintainer ruled that meaningless to users. The docs link at
/// the end is clickable in the session list (`farhelm-ui`'s `list::bell`
/// links only `https://farhelm.io/docs/` addresses), and no date is named
/// because none is set.
pub(crate) const OLD_TERMINAL_TEXT: &str = "This session must be restarted, replaced, stopped or \
    deleted as soon as possible. Any of its shell tabs that are still old terminals (the note at \
    the top of the session names them) must also be closed and reopened, because restarting a \
    session restarts only its agent, not its tabs. Otherwise an upcoming Farhelm release will \
    forcibly terminate this session's old terminals as part of transitioning away from using tmux \
    to manage sessions. See https://farhelm.io/docs/using/new-terminals/";

/// The text of a [`NotificationKind::OldTerminalEnded`] notice: what
/// happened, what the user loses, what they can do, and the docs page the
/// old-terminal warning linked (clickable in the session list, as above).
/// "Its last screen is gone" because that screen lived in the ended server.
pub(crate) const OLD_TERMINAL_ENDED_TEXT: &str = "This session ran in an old terminal, which this \
    release of Farhelm ended when it started, so its agent is no longer running and its last \
    screen is gone. Restart starts the agent again in a new terminal and resumes its \
    conversation; if Restart is unavailable, Replace starts it anew. See \
    https://farhelm.io/docs/using/new-terminals/";

/// Describe the correction without promising that no further ancestor can report.
/// The notification remains unresolved so the store posts it once per launch.
pub(crate) const CODEX_PIN_CORRECTED_TEXT: &str = "Farhelm had been following a Codex conversation \
    started by another Codex process in this session. A Restart before now would have resumed \
    that conversation. Farhelm now follows the conversation of the Codex process that started it.";

/// Preserve the consequence of child-first capture after the ancestor takes over.
/// Like Codex's correction, this remains unresolved and appears once per launch.
pub(crate) const CLAUDE_PIN_CORRECTED_TEXT: &str = "Farhelm had been following a Claude conversation \
    started by another Claude process in this session. A Restart before now would have resumed \
    that conversation. Farhelm now follows the conversation of the Claude process that started it.";

/// Preserve the earlier Restart consequence, once per launch, in Muse's terms.
pub(crate) const MUSE_PIN_CORRECTED_TEXT: &str = "Farhelm had been following a Muse conversation \
    started by another Muse process in this session. A Restart before now would have resumed \
    that conversation. Farhelm now follows the conversation of the Muse process that started it.";

/// Explain the missed report using the same agent name and timing as Restart.
///
/// Called only for a kind whose report is due by the first prompt, which
/// guarantees both facts exist. The custom-command advice names the supported
/// hook placeholder even for legacy sessions whose command predates it; the
/// user can instead send feedback when Farhelm composed the launch itself.
pub(crate) fn hook_silent_text(kind: AgentKind) -> String {
    let agent = kind.display_name().expect("a silent-hook agent has a name");
    let timing = kind
        .restart_readiness()
        .expect("a silent-hook agent has report timing")
        .clause(ReadinessWording::ToTheUser);
    format!(
        "{agent} normally reports its conversation to Farhelm {timing}, so it should have by now. \
         Until it does, Restart cannot resume this conversation. If you launched {agent} with a \
         custom command, check that it passes on `{{farhelm_args}}`; otherwise, please send \
         feedback from the help (?) menu."
    )
}

/// The text of a [`NotificationKind::HookNotAdded`] notification for a
/// launch that was composed with `{farhelm_args}` but whose reporter could
/// not be set up: there is nothing in the user's command to change, so the
/// text says only what they lose.
pub(crate) const REPORTER_UNAVAILABLE_TEXT: &str = "Farhelm could not set up its conversation \
     reporter for this launch, so Restart will not be able to resume this conversation.";

/// The text of a [`NotificationKind::HookNotAdded`] notification for a
/// session created before launch kinds, whose own command stopped Farhelm
/// adding its hook. `reason` is the injection's own explanation (which flag
/// or form of the command was in the way). The way out is Replace with,
/// whose launcher opens on the command with nothing else filled in: starting
/// it as an agent launch lets Farhelm compose the arguments itself.
pub(crate) fn legacy_hook_not_added_text(reason: &str) -> String {
    format!(
        "Farhelm could not add its conversation hook to this session's command ({reason}), so \
         Restart will not be able to resume this conversation. To have Farhelm follow it, use \
         Replace with and start it as an agent launch."
    )
}

/// Why a spawn did not add Farhelm's conversation hook, when the user should
/// hear about it (`crate::agent_kind::hook_skip_notifies` decides which skips
/// qualify; turning hooks off with `FARHELM_AGENT_HOOKS` never does).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookSkip {
    /// A launch composed with `{farhelm_args}` whose reporter extension could
    /// not be set up.
    ReporterUnavailable,
    /// A session from before launch kinds whose own command was in the way,
    /// with the injection's reason.
    Legacy(&'static str),
}

impl HookSkip {
    /// The notification text for this skip.
    pub(crate) fn text(self) -> String {
        match self {
            HookSkip::ReporterUnavailable => REPORTER_UNAVAILABLE_TEXT.to_string(),
            HookSkip::Legacy(reason) => legacy_hook_not_added_text(reason),
        }
    }
}

/// The text of a [`NotificationKind::ResumeWithdrawn`] notification.
/// `agent` is the agent's name as the user knows it ("Codex", "Grok"),
/// supplied by the vendor module that withdrew the offer.
///
/// Past tense on purpose: a later check can find the record again and bring
/// the resume offer back, and a notification is a record of what happened,
/// retained after resolution, so its wording must stay true afterwards.
pub(crate) fn resume_withdrawn_text(agent: &str) -> String {
    format!(
        "When Farhelm checked, {agent}'s own record of this conversation was missing or no longer \
         matched, so Restart stopped offering to resume it."
    )
}

/// Why an OMP launch's reports could not be accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReporterMismatch {
    /// The launch installed an older reporter than this build's (it started
    /// before an upgrade). A new launch gets the current one.
    OlderReporter,
    /// The reporter file Farhelm installed differs from this build's. Farhelm
    /// does not overwrite it, so a new launch does not fix this.
    FileDiffers,
}

/// The text of a [`NotificationKind::ReporterMismatch`] notification, by
/// cause and by whether the session already had a captured conversation:
/// with one, Restart still resumes that conversation, so the loss is only
/// that later switches go unseen.
pub(crate) fn reporter_mismatch_text(cause: ReporterMismatch, captured: bool) -> &'static str {
    match (cause, captured) {
        (ReporterMismatch::OlderReporter, true) => {
            "This OMP session was started with an older conversation reporter, so Farhelm no \
             longer follows which conversation it is in. Restart starts it again with the current \
             reporter and resumes the conversation Farhelm last knew."
        }
        (ReporterMismatch::OlderReporter, false) => {
            "This OMP session was started with an older conversation reporter, so Restart will \
             not be able to resume this conversation. Replace with starts it again with the \
             current reporter, in a new conversation."
        }
        (ReporterMismatch::FileDiffers, true) => {
            "The OMP conversation reporter installed for Farhelm differs from the one this version \
             ships, so Farhelm cannot follow which conversation this session is in. Restart can \
             only resume the conversation Farhelm knew before."
        }
        (ReporterMismatch::FileDiffers, false) => {
            "The OMP conversation reporter installed for Farhelm differs from the one this version \
             ships, so Restart will not be able to resume this conversation."
        }
    }
}

impl Supervisor {
    /// Record a notification of `kind` for launch `generation` of session
    /// `id`, unless that launch already has an unresolved one, and show it.
    ///
    /// Best effort, like every diagnostic it replaces: a store failure is
    /// logged and the session carries on. A notification added or reopened
    /// is copied into the session's cell and announced with the
    /// `SessionsChanged` hint; one the store turned away (already recorded
    /// and unresolved for this launch, or the session moved on to another launch or was
    /// deleted) changes nothing.
    pub(crate) async fn notify_session(
        &self,
        id: &str,
        generation: i64,
        kind: NotificationKind,
        text: &str,
    ) {
        // A supervisor that has been replaced, or is shutting down, has no
        // standing to write to the store, the same rule report admission
        // follows; the supervisor that records next notices again.
        if !self.may_record() {
            return;
        }
        let added = match self
            .store
            .record_session_notification(
                id,
                generation,
                kind.as_str(),
                text,
                crate::store::now_unix(),
                kind.requires_no_identity(),
            )
            .await
        {
            Ok(added) => added,
            Err(error) => {
                warn!(session = %id, kind = kind.as_str(), error = %format!("{error:#}"),
                    "could not record a session notification");
                return;
            }
        };
        if added && self.reload_notification_cell(id).await {
            self.hint_sessions_changed();
        }
    }

    /// The published entry's current notifications, for a lifecycle reply
    /// built from a separate `SessionInfo` (create, restart): empty when the
    /// session is not published.
    pub(crate) async fn published_notifications(
        &self,
        id: &str,
    ) -> Vec<farhelm_proto::SessionNotification> {
        match self.sessions.lock().await.get(id) {
            Some(entry) => notification_wire(
                &entry
                    .session
                    .notifications
                    .lock()
                    .expect("notification cell poisoned"),
            ),
            None => Vec::new(),
        }
    }

    /// Reconcile current-launch warnings against the same Resume promise
    /// listing and Restart use, including identities restored during startup.
    ///
    /// This belongs to the capture pass rather than the admission transition:
    /// an accepted startup report may precede publication, and a restored
    /// vendor record can regain Resume without a new report. The store compares
    /// the exact conversation before resolving, so a concurrent withdrawal
    /// wins over an earlier Resume observation. Older launches stay history.
    /// The in-memory kind/generation snapshot skips the database entirely when
    /// no unresolved current-launch warning has a resolving event.
    pub(crate) async fn resolve_resumable_notifications(
        &self,
        entries: &[std::sync::Arc<super::core::SessionEntry>],
    ) {
        if !self.may_record() {
            return;
        }
        for entry in entries {
            if !entry
                .session
                .notifications
                .lock()
                .expect("notification cell poisoned")
                .iter()
                .any(|stored| {
                    !stored.notification.resolved
                        && stored.generation == entry.generation
                        && NotificationKind::ALL
                            .into_iter()
                            .any(|kind| kind.resolves_on_resume() && kind.as_str() == stored.kind)
                })
            {
                continue;
            }
            let conversation = {
                let capture = entry.run.capture.lock().expect("capture mutex poisoned");
                if entry.snapshot.restart_offer(
                    capture.committed_conversation(),
                    capture.committed_ownership_version().unwrap_or(0),
                ) != farhelm_proto::RestartOffer::Resume
                {
                    continue;
                }
                capture.committed_conversation().map(str::to_owned)
            };
            let Some(conversation) = conversation else {
                continue;
            };
            let kinds: Vec<&str> = NotificationKind::ALL
                .into_iter()
                .filter(|kind| kind.resolves_on_resume())
                .map(NotificationKind::as_str)
                .collect();
            match self
                .store
                .resolve_session_notifications_if_current(
                    &entry.info.id,
                    entry.generation,
                    &conversation,
                    &kinds,
                )
                .await
            {
                Ok(true) => {
                    if self.reload_notification_cell(&entry.info.id).await {
                        self.hint_sessions_changed();
                    }
                }
                Ok(false) => {}
                Err(error) => warn!(session = %entry.info.id, error = %format!("{error:#}"),
                    "could not resolve a session's notifications"),
            }
        }
    }

    /// Record or resolve each session's old-terminal notification from one
    /// tmux pane-state snapshot ([`NotificationKind::OldTerminal`]).
    ///
    /// Called by the ticker's sample pass with the snapshot it already took,
    /// so the check costs no tmux call of its own; the pass runs it before
    /// it narrows to sessions whose agent is alive, since a stopped agent's
    /// session can still hold old tabs. `states` must be a definite answer:
    /// the pass does not call this when the probe failed, because "tmux
    /// could not be asked" is not "nothing old runs". An empty map from a
    /// pass that had no session to ask about is definite, and resolves.
    ///
    /// The session's notification cell is the precheck, so the store is
    /// touched only on a transition: a session with something old running
    /// and no unresolved warning of this kind in any launch gets one at its
    /// current launch, and one with nothing old running and an unresolved
    /// warning has it resolved, whichever launch recorded it. A recording the
    /// store turns away (the session moved to another launch meanwhile) is
    /// simply tried again on the next pass. The cells are loaded before the
    /// ticker starts and a relaunch shares its session's cell, so an empty
    /// cell here means no warning, not one not yet read.
    pub(crate) async fn reconcile_old_terminal_notifications(
        &self,
        entries: &[std::sync::Arc<SessionEntry>],
        states: &HashMap<String, PaneState>,
    ) {
        if !self.may_record() {
            return;
        }
        let kind = NotificationKind::OldTerminal;
        let by_session = panes_by_session(states);
        for entry in entries {
            let running = runs_in_old_terminal(entry, states, &by_session);
            let unresolved = entry
                .session
                .notifications
                .lock()
                .expect("notification cell poisoned")
                .iter()
                .any(|stored| stored.kind == kind.as_str() && !stored.notification.resolved);
            match (running, unresolved) {
                (true, false) => {
                    self.notify_session(&entry.info.id, entry.generation, kind, OLD_TERMINAL_TEXT)
                        .await;
                }
                (false, true) => match self
                    .store
                    .resolve_session_notifications_of_kind(&entry.info.id, kind.as_str())
                    .await
                {
                    Ok(true) => {
                        if self.reload_notification_cell(&entry.info.id).await {
                            self.hint_sessions_changed();
                        }
                    }
                    Ok(false) => {}
                    Err(error) => warn!(session = %entry.info.id, error = %format!("{error:#}"),
                        "could not resolve a session's old-terminal notification"),
                },
                (true, true) | (false, false) => {}
            }
        }
    }

    /// Replace the published entry's notification cell with what the store
    /// holds for session `id`, returning whether the cell changed.
    ///
    /// Called after a recording or resolution, and once for every entry the supervisor
    /// publishes from a stored row or a create, since a create's own spawn
    /// may have recorded before the entry existed. An entry that is not
    /// published, or a store that cannot be read, leaves things as they are.
    pub(crate) async fn reload_notification_cell(&self, id: &str) -> bool {
        let stored = match self.store.session_notification_records(id).await {
            Ok(stored) => stored,
            Err(error) => {
                warn!(session = %id, error = %format!("{error:#}"),
                    "could not read a session's notifications");
                return false;
            }
        };
        let Some(entry) = self.sessions.lock().await.get(id).cloned() else {
            return false;
        };
        let mut cell = entry
            .session
            .notifications
            .lock()
            .expect("notification cell poisoned");
        if *cell == stored || !newer_or_equal(&stored, &cell) {
            return false;
        }
        *cell = stored;
        true
    }
}

/// A pane-state snapshot grouped by the tmux session each pane belongs to,
/// so finding one session's tabs does not rescan every pane on the server
/// (the same partition `ticker::reap_dead_tabs` makes).
fn panes_by_session(states: &HashMap<String, PaneState>) -> HashMap<&str, Vec<&PaneState>> {
    let mut by_session: HashMap<&str, Vec<&PaneState>> = HashMap::new();
    for state in states.values() {
        by_session
            .entry(state.session_name.as_str())
            .or_default()
            .push(state);
    }
    by_session
}

/// Whether something in `entry`'s session still runs in an old terminal,
/// by one pane-state snapshot: its agent's tmux pane is alive, or one of
/// its tmux tabs is.
///
/// The agent counts with the predicate the sample pass uses to decide
/// whether to sample it: its pane is in the snapshot, under the session's
/// own tmux name, and not dead. A pane reported under another name (one
/// tmux recycled after its server restarted) is not this session's. A dead
/// pane, which Stop and an exited agent leave so their last screen stays
/// readable, is not running: there is nothing in it for a release to
/// terminate. Tabs are found as everywhere else, by their window markers in
/// the session's tmux session (`terminals::tmux_tab_home`, or the session's
/// own tmux name when it has no agent terminal at all, as the listing
/// does), and count while their shell is alive; a dead one is about to be
/// reaped.
fn runs_in_old_terminal(
    entry: &SessionEntry,
    states: &HashMap<String, PaneState>,
    by_session: &HashMap<&str, Vec<&PaneState>>,
) -> bool {
    let agent_alive = match &entry.terminal {
        Some(Terminal::Tmux(tmux)) => states
            .get(&tmux.pane)
            .is_some_and(|state| state.session_name == tmux.tmux_name && !state.dead),
        Some(Terminal::Host(_)) | None => false,
    };
    if agent_alive {
        return true;
    }
    let (tmux_name, agent_pane) = match &entry.terminal {
        Some(terminal) => tmux_tab_home(&entry.info.id, terminal),
        None => (
            std::borrow::Cow::Owned(session_tmux_name(&entry.info.id)),
            None,
        ),
    };
    by_session.get(tmux_name.as_ref()).is_some_and(|panes| {
        tabs_from_pane_states(panes.iter().copied(), &tmux_name, agent_pane)
            .iter()
            .any(|tab| !tab.dead)
    })
}

/// Project the stored history onto the protocol without leaking private kinds.
///
/// Keeping this projection shared ensures lifecycle replies and ordinary lists
/// carry identical fields even though the supervisor retains resolution metadata.
pub(crate) fn notification_wire(
    stored: &[StoredSessionNotification],
) -> Vec<farhelm_proto::SessionNotification> {
    stored
        .iter()
        .map(|stored| stored.notification.clone())
        .collect()
}

/// Whether `stored` is at least as recent as `current`, judged by their newest
/// sequence number, then their resolved count (the lists are newest first).
///
/// Two reloads of one session can overlap (an OMP refusal and a tripwire
/// firing in the same pass): each reads the store, then publishes, and the
/// one that read first may publish last. Recording or reopening raises the
/// newest sequence; with that sequence fixed, resolution only increases the
/// resolved count. That second ordering prevents an older unresolved snapshot
/// from undoing a resolution when neither reload added an entry.
fn newer_or_equal(
    stored: &[StoredSessionNotification],
    current: &[StoredSessionNotification],
) -> bool {
    let version = |list: &[StoredSessionNotification]| {
        (
            list.first().map(|n| n.notification.seq),
            list.iter()
                .filter(|stored| stored.notification.resolved)
                .count(),
        )
    };
    version(stored) >= version(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind eligible for the silent-hook warning gets its own name,
    /// Restart's timing, and actionable advice. Deriving the set from the
    /// eligibility rule keeps a future integration from silently inheriting
    /// an anonymous warning or sending the user to a log.
    #[test]
    fn silent_hook_advice_follows_each_eligible_agents_restart_timing() {
        for &kind in AgentKind::ALL {
            let Some(readiness) = kind.restart_readiness() else {
                continue;
            };
            if !readiness.due_by_first_prompt() {
                continue;
            }
            let text = hook_silent_text(kind);
            assert!(text.contains(kind.display_name().expect("eligible agent name")));
            assert!(text.contains(readiness.clause(ReadinessWording::ToTheUser)));
            assert!(text.contains("Restart cannot resume this conversation"));
            assert!(text.contains("custom command"));
            assert!(text.contains("`{farhelm_args}`"));
            assert!(text.contains("feedback from the help (?) menu"));
            assert!(!text.to_lowercase().contains("log"), "{kind:?}: {text}");
        }
    }

    /// An overlapping reload may read before a resolve and publish after it.
    /// Equal sequence numbers must preserve resolution; recurrence is newer
    /// even though its resolved count falls, because its sequence advances.
    #[farhelm_testtrace::test]
    fn notification_snapshots_order_resolution_and_recurrence() {
        let unresolved = vec![StoredSessionNotification {
            notification: farhelm_proto::SessionNotification {
                seq: 2,
                at: 100,
                text: "warning".into(),
                resolved: false,
            },
            generation: 1,
            kind: NotificationKind::HookSilent.as_str().into(),
        }];
        let mut resolved = unresolved.clone();
        resolved[0].notification.resolved = true;
        assert!(newer_or_equal(&resolved, &unresolved));
        assert!(!newer_or_equal(&unresolved, &resolved));
        assert!(newer_or_equal(&resolved, &resolved));
        let mut recurrence = unresolved.clone();
        recurrence[0].notification.seq = 3;
        assert!(newer_or_equal(&recurrence, &resolved));
        assert!(!newer_or_equal(&resolved, &recurrence));
        assert!(newer_or_equal(&unresolved, &[]));
        assert!(!newer_or_equal(&[], &resolved));
    }
}
