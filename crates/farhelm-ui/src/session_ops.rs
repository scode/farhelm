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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Lifecycle {
    pub(crate) revision: u32,
    /// The last finished Restart's outcome, kept so that a view opened later
    /// can stand in for a listing that has not caught up with the restarted
    /// run yet (`session_view.rs`, `relaunched` and `restart_terminal_kind`).
    ///
    /// It is forgotten as soon as any read shows the restarted run: the open
    /// view's detail read, or the sidebar's listing (`restarts_retired_by`).
    /// Kept longer, it would outlive its purpose: after a second host reboot
    /// every read says Interrupted again, and a view opened then would trust
    /// this old success and attach to a destroyed pane instead of showing the
    /// interrupted card.
    pub(crate) restart: Option<crate::list::RestartOutcome>,
    pub(crate) restart_revision: u32,
    /// The sidebar's newest listing generation when `restart` was recorded
    /// (`ops::ReadGate::latest_started`); only a read with a higher generation
    /// may retire `restart`.
    ///
    /// A read already in flight at that moment describes the run the Restart
    /// replaced, including its terminal kind. For a Restart of a running or
    /// exited session it is not Interrupted, so `read_confirms_run` alone
    /// would let it retire the outcome, and a view opened next would take the
    /// replaced run's terminal kind instead of the reply's (`agent_spec_kind`
    /// in `session_view.rs`). The helm records the Restart's reply before it
    /// answers, so any read started after the reply carries the new kind.
    pub(crate) restart_listing_floor: u64,
}

/// Whether a read of a session shows a run that a remembered Restart outcome
/// no longer needs to stand in for.
///
/// Until the helm next checks the restarted run, it keeps reporting the status
/// the session had before the Restart (`merged_status` in the helm's
/// `session_cache.rs`), so Interrupted is not proof that the Restart failed.
/// The outcome matters to the status only while that status is Interrupted:
/// it is what keeps a view from showing the interrupted card for a session
/// that is in fact running again. Any other status leaves it nothing to stand
/// in for. A stale session's host is gone, and stale outranks interrupted on
/// screen. The outcome's other job, the terminal kind, is guarded by
/// `Lifecycle::restart_listing_floor` instead. Shared by the open view's
/// detail commit and the sidebar's listing commit, so the two can never
/// disagree about when to forget.
pub(crate) fn read_confirms_run(session: &Session) -> bool {
    session.stale || !matches!(session.status, crate::SessionStatus::Interrupted)
}

/// The sessions whose remembered Restart outcome one sidebar listing read
/// retires: listed, shown by `read_confirms_run` to be past the restart,
/// with no request pending for them, and read with a `generation` newer than
/// the outcome's `restart_listing_floor`.
///
/// Only sessions the listing positively shows count. A session the listing
/// omits (filtered out, past a truncation, or deleted) keeps its outcome here;
/// the listing commit's separate absence cleanup drops a deleted session's
/// whole entry once a complete listing proves it is gone.
pub(crate) fn restarts_retired_by(
    lifecycle: &HashMap<String, Lifecycle>,
    pending: &HashSet<String>,
    sessions: &[Session],
    generation: u64,
) -> Vec<String> {
    sessions
        .iter()
        .filter(|session| read_confirms_run(session))
        .map(|session| session.id.raw())
        .filter(|id| !pending.contains(*id))
        .filter(|id| {
            lifecycle.get(*id).is_some_and(|state| {
                state.restart.is_some() && generation > state.restart_listing_floor
            })
        })
        .map(str::to_owned)
        .collect()
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
    pub(crate) fn boundary(mut self, id: &str) {
        let mut states = self.lifecycle.write();
        let state = states.entry(id.to_owned()).or_default();
        state.revision = state.revision.wrapping_add(1);
    }

    /// Record a finished Restart POST's outcome behind a closing boundary.
    ///
    /// `listing_floor` is the sidebar's `ReadGate::latest_started` at this
    /// moment; see `Lifecycle::restart_listing_floor` for why the listing
    /// reads already in flight must not retire the outcome.
    pub(crate) fn finish_restart(
        mut self,
        id: &str,
        outcome: crate::list::RestartOutcome,
        listing_floor: u64,
    ) {
        let mut states = self.lifecycle.write();
        let state = states.entry(id.to_owned()).or_default();
        state.revision = state.revision.wrapping_add(1);
        state.restart = Some(outcome);
        state.restart_revision = state.revision;
        state.restart_listing_floor = listing_floor;
    }

    /// The remembered successful Restart a newly mounted view seeds itself
    /// from, if no read has shown the restarted run yet. Untracked: a view
    /// reads it once, at mount.
    pub(crate) fn retained_restart(self, id: &str) -> Option<crate::list::RestartOutcome> {
        self.lifecycle
            .peek()
            .get(id)
            .and_then(|state| state.restart)
            .filter(|outcome| outcome.succeeded)
    }

    /// Forget the remembered Restart outcomes a committed sidebar listing read
    /// shows to be past (`restarts_retired_by`).
    ///
    /// Writes the lifecycle map only when something is retired: every listing
    /// commit calls this, filtered ones included, and an unconditional write
    /// would add a wake of every reader of the map (each open view's Restart
    /// observer) to each of them.
    pub(crate) fn retire_confirmed_restarts(mut self, sessions: &[Session], generation: u64) {
        let retired = restarts_retired_by(
            &self.lifecycle.peek(),
            &self.pending.peek(),
            sessions,
            generation,
        );
        if retired.is_empty() {
            return;
        }
        let mut states = self.lifecycle.write();
        for id in retired {
            if let Some(state) = states.get_mut(&id) {
                state.restart = None;
            }
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
            operations.boundary("a");
            let during_read = operations.revision("a");
            let outcome = crate::list::RestartOutcome {
                succeeded: true,
                agent_terminal: None,
            };
            operations.finish_restart("a", outcome, 7);
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
                operations
                    .lifecycle
                    .peek()
                    .get("a")
                    .unwrap()
                    .restart_listing_floor,
                7,
                "the outcome remembers which listing reads were already in flight"
            );
            assert_eq!(
                operations.revision("b"),
                0,
                "unrelated reads remain current"
            );
        });
    }

    /// A listed session with the given status and reachability. Every other
    /// field is whatever is cheapest: only those two, and the id, decide
    /// whether a listing read retires a remembered Restart.
    fn listed(id: &str, status: crate::SessionStatus, stale: bool) -> Session {
        Session {
            screen_yolo: false,
            claude_trust_prompt: false,
            workspace_trust_prompt: false,
            id: id.into(),
            title: id.into(),
            cwd: "/tmp".into(),
            canonical_cwd: None,
            invocation: "agent".into(),
            agent_kind: crate::SessionAgentKind::Unrecognized,
            launch: None,
            status,
            annotation: None,
            restart_offer: crate::RestartOffer::NotCaptured,
            created_at: 0,
            last_activity_at: 0,
            tabs: Vec::new(),
            host: None,
            host_identity: None,
            host_name: None,
            stale,
            github_repo: None,
            working_copy: None,
            seen_activity_at: None,
            pinned: false,
            pin_position: None,
            notifications: Vec::new(),
            notifications_read_through: 0,
            agent_terminal: None,
            supervisor_creates_host_terminals: false,
        }
    }

    /// A remembered successful Restart, recorded when listing generation
    /// `floor` was the newest one started.
    fn remembered(floor: u64) -> Lifecycle {
        Lifecycle {
            revision: 2,
            restart: Some(crate::list::RestartOutcome {
                succeeded: true,
                agent_terminal: None,
            }),
            restart_revision: 2,
            restart_listing_floor: floor,
        }
    }

    /// Why this matters: a Restart that succeeded while another session was
    /// open used to be remembered until that session was next opened, so a
    /// host reboot in between made the newly opened view trust the old
    /// success and attach to a destroyed pane instead of showing the
    /// interrupted card. The sidebar's listing now retires it.
    ///
    /// What: a listing read retires a session's remembered outcome exactly
    /// when it lists the session as running (any status but Interrupted) or
    /// stale, no request for it is pending, and the read started after the
    /// outcome was recorded. A read that still says Interrupted has not
    /// caught up with the restarted run and keeps it (the window the outcome
    /// exists for), a read already in flight at the reply may describe the
    /// replaced run and keeps it, and a session the listing omits keeps it.
    #[farhelm_testtrace::test]
    fn a_listing_retires_a_remembered_restart_only_once_it_shows_the_new_run() {
        use crate::SessionStatus::{Interrupted, Running};
        let lifecycle: HashMap<String, Lifecycle> = [
            ("caught-up", remembered(3)),
            ("not-yet", remembered(3)),
            ("host-gone", remembered(3)),
            ("in-flight-read", remembered(4)),
            ("pending", remembered(3)),
            ("omitted", remembered(3)),
            (
                "nothing-remembered",
                Lifecycle {
                    restart: None,
                    ..remembered(3)
                },
            ),
        ]
        .into_iter()
        .map(|(id, state)| (id.to_owned(), state))
        .collect();
        let pending: HashSet<String> = ["pending".to_owned()].into();
        let listing = [
            listed("caught-up", Running, false),
            listed("not-yet", Interrupted, false),
            listed("host-gone", Interrupted, true),
            listed("in-flight-read", Running, false),
            listed("pending", Running, false),
            listed("nothing-remembered", Running, false),
            listed("never-restarted", Running, false),
        ];
        let mut retired = restarts_retired_by(&lifecycle, &pending, &listing, 4);
        retired.sort();
        assert_eq!(retired, ["caught-up", "host-gone"]);
        assert!(
            restarts_retired_by(&lifecycle, &pending, &listing, 5)
                .contains(&"in-flight-read".to_owned()),
            "a read started after the reply does retire it"
        );
    }

    /// Why this matters: every listing commit calls this, so it must clear
    /// only the outcome and leave the boundaries a detail read is ordered by.
    ///
    /// What: retiring drops the `restart` outcome and nothing else; the
    /// revisions stay, and another session's outcome is untouched.
    #[farhelm_testtrace::test]
    fn retiring_forgets_only_the_outcome_of_the_listed_session() {
        let mut dom = VirtualDom::new(owner);
        dom.rebuild_in_place();
        dom.in_runtime(|| {
            let operations = OPERATIONS.with(Cell::get).unwrap();
            let outcome = crate::list::RestartOutcome {
                succeeded: true,
                agent_terminal: None,
            };
            operations.finish_restart("a", outcome, 0);
            operations.finish_restart("b", outcome, 0);
            let before = *operations.lifecycle.peek().get("a").unwrap();
            assert_eq!(operations.retained_restart("a"), Some(outcome), "premise");
            operations
                .retire_confirmed_restarts(&[listed("a", crate::SessionStatus::Running, false)], 1);
            assert_eq!(operations.retained_restart("a"), None);
            assert_eq!(
                *operations.lifecycle.peek().get("a").unwrap(),
                Lifecycle {
                    restart: None,
                    ..before
                }
            );
            assert_eq!(operations.retained_restart("b"), Some(outcome));
        });
    }
}
