//! Session notifications: telling the user, on the session itself, that
//! Farhelm has lost track of that session's agent conversation.
//!
//! SPEC.md (Status) has the product contract and SPEC_impl.md the storage
//! and transport. In short: a problem the supervisor notices is recorded in
//! its store (`session_notifications`, at most 10 per session, at most one per
//! kind per launch), copied into the session's in-memory cell
//! ([`super::core::SessionCells::notifications`]) so every reply carries it,
//! and announced to connected helms with the ordinary `SessionsChanged` hint.
//! The helm keeps read and cleared state; nothing here knows about either.
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
//!
//! # The wording rule
//!
//! The maintainer's rule (the plan's M5): every text names something the
//! user can actually do, or, when there is nothing, says plainly what they
//! lose. Never a pointer to a log. What the user loses in every case here is
//! the same thing, Restart's ability to resume the conversation, so each text
//! says so in those words.

use super::core::Supervisor;
use tracing::warn;

/// What kind of problem a notification reports.
///
/// The unit of the once-per-launch rule (the store's `(session, generation,
/// kind)` key) and the reason the record does not assume there is only one
/// kind of notification. It never travels on the wire: only the text does
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
}

impl NotificationKind {
    /// Whether this kind is only true while the session's row holds no
    /// captured conversation, which the store then checks in the same
    /// statement as the insert ([`crate::store::SessionStore::record_session_notification`]).
    fn requires_no_identity(self) -> bool {
        match self {
            // Both say Restart will not be able to resume, which is false for
            // a launch that carries a captured conversation (a Restart keeps
            // the one it resumed).
            NotificationKind::HookSilent | NotificationKind::HookNotAdded => true,
            NotificationKind::ResumeWithdrawn | NotificationKind::ReporterMismatch => false,
        }
    }

    /// The value stored in the `kind` column. Stable: it is part of the
    /// once-per-launch key of rows already on disk.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            NotificationKind::HookSilent => "hook_silent",
            NotificationKind::HookNotAdded => "hook_not_added",
            NotificationKind::ResumeWithdrawn => "resume_withdrawn",
            NotificationKind::ReporterMismatch => "reporter_mismatch",
        }
    }
}

/// The text of a [`NotificationKind::HookSilent`] notification.
pub(crate) const HOOK_SILENT_TEXT: &str = "Farhelm has not learned which conversation this \
     agent is in, a minute after the first line you sent it, so Restart will not be able to \
     resume this conversation.";

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
/// never withdrawn, so it must stay true afterwards.
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
    /// `id`, unless that launch already has one, and show it.
    ///
    /// Best effort, like every diagnostic it replaces: a store failure is
    /// logged and the session carries on. A notification that was added is
    /// copied into the session's cell and announced with the
    /// `SessionsChanged` hint; one the store turned away (already recorded
    /// for this launch, or the session moved on to another launch or was
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
            Some(entry) => entry
                .session
                .notifications
                .lock()
                .expect("notification cell poisoned")
                .clone(),
            None => Vec::new(),
        }
    }

    /// Replace the published entry's notification cell with what the store
    /// holds for session `id`, returning whether the cell changed.
    ///
    /// Called after a recording, and once for every entry the supervisor
    /// publishes from a stored row or a create, since a create's own spawn
    /// may have recorded before the entry existed. An entry that is not
    /// published, or a store that cannot be read, leaves things as they are.
    pub(crate) async fn reload_notification_cell(&self, id: &str) -> bool {
        let stored = match self.store.session_notifications(id).await {
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

/// Whether `stored` is at least as recent as `current`, judged by their newest
/// sequence numbers (the lists are newest first).
///
/// Two reloads of one session can overlap (an OMP refusal and a tripwire
/// firing in the same pass): each reads the store, then publishes, and the
/// one that read first may publish last. The store only ever gains newer
/// entries and drops the oldest, so the newest sequence number orders the
/// snapshots, and an older one must not overwrite a newer one already in the
/// cell.
fn newer_or_equal(
    stored: &[farhelm_proto::SessionNotification],
    current: &[farhelm_proto::SessionNotification],
) -> bool {
    let newest = |list: &[farhelm_proto::SessionNotification]| list.first().map(|n| n.seq);
    newest(stored) >= newest(current)
}
