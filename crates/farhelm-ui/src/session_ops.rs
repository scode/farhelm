//! Session-local exclusion and outcomes shared by the persistent list and keyed view.
//!
//! The list owns requests; this state carries only their claims and observations.
//! A navigation change destroys no request and cannot release another session's claim.

use crate::launch_progress::PendingLaunch;
use crate::{HostId, Session};
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

/// Accepted header work, synchronously handed to the list's existing request paths.
pub(crate) enum HeaderAction {
    /// The tab/agent guards are the answered render's consent, not live facts.
    Stop(Session, bool, bool, bool, SessionGuard),
    /// A plain Restart never changes the session's stored launch settings.
    Restart(Session, bool, SessionGuard),
    /// Keep plain Replace's existing retained identity and optional YOLO answer.
    Replace(
        Session,
        crate::DeleteGuard,
        Option<Box<crate::replace::ReplaceAnswer>>,
        SessionGuard,
    ),
    /// Sandbox identity selects whole-resource Delete; ordinary rows use the guard.
    Delete(String, crate::DeleteGuard, Option<HostId>, SessionGuard),
    /// The direction was captured before the activity read, never toggled later.
    Sandbox(
        Session,
        HostId,
        crate::sandbox_sessions::Operation,
        SessionGuard,
    ),
}

/// A lifecycle boundary orders detail replies even when no keyed view is mounted.
#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) struct Lifecycle {
    pub(crate) revision: u32,
    pub(crate) restart: Option<crate::list::RestartOutcome>,
    pub(crate) restart_revision: u32,
}

/// AppBody-owned data for the two session surfaces; no request queue lives here.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SessionOps {
    /// Ordinary questions and admitted operations own their source continuously.
    pub(crate) held: Signal<HashSet<String>>,
    /// Plain Replace's unanswered YOLO lanes share the same admission boundary.
    /// The controller owns their tokens; this existing page-owned set lets every
    /// header and row handler refuse the source while its question is open.
    pub(crate) questions: Signal<HashSet<String>>,
    /// Request presentation excludes questions, whose own answers stay enabled.
    pub(crate) pending: Signal<HashSet<String>>,
    pub(crate) stopping: Signal<HashSet<String>>,
    pub(crate) restarting: Signal<HashMap<String, PendingLaunch>>,
    pub(crate) errors: Signal<HashMap<String, String>>,
    pub(crate) lifecycle: Signal<HashMap<String, Lifecycle>>,
    pub(crate) dispatch: Signal<Option<Callback<HeaderAction>>>,
}

impl SessionOps {
    /// Create once above both siblings, so navigation preserves every observation.
    pub(crate) fn new() -> Self {
        Self {
            held: Signal::new(HashSet::new()),
            questions: Signal::new(HashSet::new()),
            pending: Signal::new(HashSet::new()),
            stopping: Signal::new(HashSet::new()),
            restarting: Signal::new(HashMap::new()),
            errors: Signal::new(HashMap::new()),
            lifecycle: Signal::new(HashMap::new()),
            dispatch: Signal::new(None),
        }
    }

    /// Claim from live state before a handler yields or opens its question.
    /// An inline Replace answer first consumes its matching question token,
    /// then claims synchronously, so it can continue without admitting a peer.
    pub(crate) fn claim(mut self, id: &str) -> Option<SessionGuard> {
        if self.questions.peek().contains(id) || !self.held.write().insert(id.to_owned()) {
            return None;
        }
        Some(SessionGuard {
            operations: self,
            id: id.to_owned(),
        })
    }

    /// Tracked render reflection of this session's question or request claim.
    pub(crate) fn busy(self, id: &str) -> bool {
        self.held.read().contains(id) || self.questions.read().contains(id)
    }
    /// Handler-time exclusion, independent of the render that drew its button.
    pub(crate) fn busy_now(self, id: &str) -> bool {
        self.held.peek().contains(id) || self.questions.peek().contains(id)
    }

    /// A read captures and rechecks this value directly, independently of effects.
    pub(crate) fn revision(self, id: &str) -> u32 {
        self.lifecycle
            .peek()
            .get(id)
            .map_or(0, |value| value.revision)
    }

    /// Invalidate both reads already in flight and reads made during a mutation.
    pub(crate) fn boundary(mut self, id: &str, restart: Option<crate::list::RestartOutcome>) {
        let mut states = self.lifecycle.write();
        let state = states.entry(id.to_owned()).or_default();
        state.revision = state.revision.wrapping_add(1);
        if let Some(restart) = restart {
            state.restart = Some(restart);
            state.restart_revision = state.revision;
        }
    }

    /// Calls in the list's callback scope, which is also where spawned tasks live.
    pub(crate) fn send(self, action: HeaderAction) {
        let callback = *self.dispatch.peek();
        if let Some(callback) = callback {
            callback.call(action);
        }
    }
}

/// One question/request's release authority; a refused claimant owns nothing.
/// Sandbox Suspend and Resume hold only this session claim: they are not host
/// registry changes and carry no page token (see the `ops` module).
pub(crate) struct SessionGuard {
    operations: SessionOps,
    id: String,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if let Ok(mut held) = self.operations.held.try_write() {
            held.remove(&self.id);
        }
        if let Ok(mut pending) = self.operations.pending.try_write() {
            pending.remove(&self.id);
        }
    }
}

/// The selected session's small Copy adapter for existing confirmation surfaces.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SessionGate {
    pub(crate) id: Signal<String>,
    pub(crate) operations: SessionOps,
}

impl SessionGate {
    /// A local question holds the same source claim as its sidebar counterpart.
    pub(crate) fn claim_guard(&mut self) -> Option<SessionGuard> {
        self.operations.claim(&self.id.peek())
    }
    /// Keep the question's accepted claim until its synchronous list handoff.
    pub(crate) fn claim_into(&mut self, slot: &mut Signal<Option<SessionGuard>>) -> bool {
        if let Some(claim) = self.claim_guard() {
            slot.set(Some(claim));
            true
        } else {
            false
        }
    }
    /// Only this selected source disables its lifecycle controls.
    pub(crate) fn busy(self) -> bool {
        self.operations.busy(self.id.read().as_str())
    }
    /// Queued handlers must recheck the source rather than trusting markup.
    pub(crate) fn busy_now(self) -> bool {
        self.operations.busy_now(self.id.peek().as_str())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        static OPERATIONS: Cell<Option<SessionOps>> = const { Cell::new(None) };
        static SELECTED: Cell<Option<Signal<String>>> = const { Cell::new(None) };
    }

    /// Own signals outside the child surface whose navigation is being modeled.
    fn owner() -> Element {
        let operations = use_hook(SessionOps::new);
        let selected = use_signal(|| "a".to_owned());
        OPERATIONS.with(|slot| slot.set(Some(operations)));
        SELECTED.with(|slot| slot.set(Some(selected)));
        rsx! {}
    }

    /// A question and a request exclude the same source synchronously, while a
    /// refused same-source claim cannot release that question or another source.
    #[farhelm_testtrace::test]
    fn claims_exclude_only_their_source_and_release_only_their_owner() {
        let mut dom = VirtualDom::new(owner);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let operations = OPERATIONS.with(Cell::get).unwrap();
            let first = operations.claim("a").expect("first source is idle");
            let other = operations
                .claim("b")
                .expect("another source is independent");
            assert!(operations.claim("a").is_none());
            assert!(operations.busy_now("a"));
            drop(first);
            assert!(!operations.busy_now("a"));
            assert!(operations.busy_now("b"), "dropping A cannot release B");
            let later = operations
                .claim("a")
                .expect("A can proceed while B remains held");
            drop(other);
            assert!(
                operations.busy_now("a"),
                "dropping B cannot release the later A owner"
            );
            drop(later);
        });
    }

    /// Inline Replace questions participate in the same render and handler
    /// boundary as ordinary questions. Their source is unavailable through a
    /// newly mounted header too, while another session remains independent.
    #[farhelm_testtrace::test]
    fn inline_question_excludes_common_claims_and_selected_gate() {
        let mut dom = VirtualDom::new(owner);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let mut operations = OPERATIONS.with(Cell::get).unwrap();
            operations.questions.write().insert("a".to_owned());
            let mut gate = SessionGate {
                id: SELECTED.with(Cell::get).unwrap(),
                operations,
            };
            assert!(operations.busy("a"));
            assert!(operations.busy_now("a"));
            assert!(gate.busy());
            assert!(gate.busy_now());
            assert!(operations.claim("a").is_none());
            assert!(gate.claim_guard().is_none());
            let other = operations.claim("b").expect("another session is usable");
            operations.questions.write().remove("a");
            let answered = gate.claim_guard().expect("own answer can now claim");
            assert!(operations.busy_now("a"));
            drop(answered);
            assert!(!operations.busy_now("a"));
            assert!(operations.busy_now("b"));
            drop(other);
        });
    }

    /// A reopened view needs both mutation boundaries and the successful reply's
    /// terminal kind even when no view was mounted when the result arrived.
    #[farhelm_testtrace::test]
    fn lifecycle_boundaries_and_restart_reply_outlive_the_view() {
        let mut dom = VirtualDom::new(owner);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let operations = OPERATIONS.with(Cell::get).unwrap();
            let old_read = operations.revision("a");
            operations.boundary("a", None);
            let during_read = operations.revision("a");
            let outcome = crate::list::RestartOutcome {
                succeeded: true,
                agent_terminal: None,
            };
            operations.boundary("a", Some(outcome));
            let finished = operations.revision("a");
            assert_ne!(old_read, during_read);
            assert_ne!(during_read, finished);
            assert_eq!(
                operations.lifecycle.peek().get("a").unwrap().restart,
                Some(outcome)
            );
            assert_eq!(
                operations
                    .lifecycle
                    .peek()
                    .get("a")
                    .unwrap()
                    .restart_revision,
                finished
            );
            assert_eq!(
                operations.revision("b"),
                0,
                "unrelated reads remain current"
            );
        });
    }
}
