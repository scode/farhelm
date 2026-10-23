//! Plain Replace's retry identity and YOLO continuation, shared by both panes.
//!
//! Admission, tasks and successful selection remain with the surface: sidebar
//! rows can run independently, whereas the header owns a page operation guard.
//! This controller owns the rules between those boundaries. A submitted source
//! and its destructive precondition are snapshots of consent, never live reads.

use std::collections::HashSet;

use dioxus::prelude::*;

use crate::api::{self, RefusalOutcome};
use crate::ops::{ConfirmSlot, ConfirmToken};
use crate::yolo_confirm::{YoloAsk, YoloReason};
use crate::{DeleteGuard, HostId, Session};

/// The exact Replace refused by the helm, including the consent it continues.
///
/// A restarted source or a new terminal tab must not broaden an earlier
/// nothing-alive answer. The key also matters independently of the source:
/// A → B → A can leave an old A question beside a newer A retry identity.
#[derive(Clone, PartialEq)]
pub(crate) struct PendingReplace {
    key: String,
    pub(crate) ask: YoloAsk,
    pub(crate) source: Session,
    pub(crate) guard: DeleteGuard,
}

/// One local opening of a Replace question: the shared prompt mechanism's
/// token, whose key is the captured operation the question displays.
///
/// The opening number comes from the controller's [`ConfirmSlot`], like
/// every other prompt in the app; Replace has no counter of its own. Every
/// answer names the token its render drew, on the sidebar and in the session
/// header alike, so a queued answer behind Cancel, or behind a reopen of the
/// same request, cannot dispatch: the reopen is a new opening even when its
/// [`PendingReplace`] is equal. (The header used to answer whatever question
/// was current instead; that was reversed on 2026-10-21 so both surfaces
/// answer only what they showed.) Read what the question shows with
/// [`ConfirmToken::key`].
pub(crate) type ReplaceYoloQuestion = ConfirmToken<PendingReplace>;

/// Approval to continue one captured Replace, optionally after marking its host.
/// The host comes from the question, never a newly fetched source session.
pub(crate) struct ReplaceAnswer {
    pub(crate) pending: PendingReplace,
    stop_asking: Option<(HostId, String)>,
}

/// An admitted operation whose retry reservation was published synchronously.
/// Keep this value in the surface-owned task; creating its future alone must
/// not postpone reserving the source until the executor first polls it.
pub(crate) struct ReplaceAttempt {
    source: Session,
    guard: DeleteGuard,
    retained: Option<String>,
    answer: Option<ReplaceAnswer>,
}

/// The only results an adapter needs after the common request sequence ends.
/// Question failures are already displayed on the question; ordinary refusal
/// text still uses the surface's existing row or header presentation.
pub(crate) enum ReplaceResult {
    /// The bare reply and delete notice; boxing keeps other results small.
    Replaced(Box<Session>, Option<String>),
    /// The adapter presents this beside the source using its existing prefix.
    Refused(String),
    /// Local question state already represents the refusal or host-save failure.
    QuestionHandled,
}

/// Existing surface differences, not independently configurable behaviors.
/// Sidebar consumes either answer before dispatch; the session surface keeps a
/// permanent-answer question mounted until its host preference write succeeds.
#[derive(Clone, Copy, PartialEq)]
enum QuestionPolicy {
    Sidebar,
    Session,
}

/// Per-surface Replace state over page-owned retry and question-exclusion slots.
///
/// Copying this handle does not create another state owner. Components create
/// the local question slot and error signal, and release the current
/// question's exclusion on unmount; neither unmount nor navigation discards
/// the page's unresolved retry identity.
///
/// The question is a [`ConfirmSlot`] keyed by the [`PendingReplace`] it
/// displays, with no payload: the slot alone mints each opening's identity
/// and decides whether an answer's token still names the open question. The
/// exclusion, the error text and the retry key live beside it because they
/// follow rules the slot does not know (a duplicate source refuses to open,
/// a permanent answer may keep the question up, a cancel ends one key).
#[derive(Clone, Copy)]
pub(crate) struct ReplaceController {
    retry: Signal<Option<(String, Option<String>)>>,
    exclusions: Signal<HashSet<String>>,
    question: ConfirmSlot<PendingReplace>,
    error: Signal<Option<String>>,
    policy: QuestionPolicy,
}

impl ReplaceController {
    /// Bind a sidebar's local question to the existing page-wide slots.
    /// Its token-bound answers consume before either kind of continuation.
    pub(crate) fn sidebar(
        retry: Signal<Option<(String, Option<String>)>>,
        exclusions: Signal<HashSet<String>>,
        question: ConfirmSlot<PendingReplace>,
        error: Signal<Option<String>>,
    ) -> Self {
        Self {
            retry,
            exclusions,
            question,
            error,
            policy: QuestionPolicy::Sidebar,
        }
    }

    /// Bind a session surface without changing its permanent-answer mount.
    /// Its handlers answer only the opening their render drew, and keep that
    /// question through a host-setting write, including a failed write's
    /// busy-to-idle recovery.
    pub(crate) fn session(
        retry: Signal<Option<(String, Option<String>)>>,
        exclusions: Signal<HashSet<String>>,
        question: ConfirmSlot<PendingReplace>,
        error: Signal<Option<String>>,
    ) -> Self {
        Self {
            policy: QuestionPolicy::Session,
            ..Self::sidebar(retry, exclusions, question, error)
        }
    }

    /// The open question's token, read reactively while rendering its
    /// confirmation. The render moves it into every handler it draws.
    pub(crate) fn question(self) -> Option<ReplaceYoloQuestion> {
        self.question.current()
    }

    /// The open question's token without subscribing, for tests that name an
    /// opening no render drew. Test-only on purpose: a handler that could
    /// fetch the live token could answer whatever is open at click time;
    /// handlers test their rendered opening with [`Self::is_open`] instead.
    #[cfg(test)]
    fn current_question(self) -> Option<ReplaceYoloQuestion> {
        self.question.peek_token()
    }

    /// Whether `question`, the opening a handler's render drew, is still the
    /// open one, without subscribing. For a handler's early exit before it
    /// takes a lifecycle claim; the answer itself re-checks.
    pub(crate) fn is_open(self, question: &ReplaceYoloQuestion) -> bool {
        self.question.is_current(question)
    }

    /// Read the host-setting failure associated with the local question.
    pub(crate) fn error(self) -> Option<String> {
        self.error.read().clone()
    }

    /// Retire an ordinary cancelled prompt only when its source owns the slot.
    /// This is deliberately broader than cancelling a YOLO question's exact key.
    pub(crate) fn cancel_source(mut self, source: &str) {
        if self
            .retry
            .peek()
            .as_ref()
            .is_some_and(|(id, _)| id == source)
        {
            self.retry.set(None);
        }
    }

    /// Starting Replace on another source ends the old unresolved identity.
    /// Opening an ordinary confirmation does not yet reserve the new source.
    pub(crate) fn select_source(mut self, source: &str) {
        if self
            .retry
            .peek()
            .as_ref()
            .is_some_and(|(id, _)| id != source)
        {
            self.retry.set(None);
        }
    }

    /// Sidebar Replace-with ends the page's plain Replace, whatever its source.
    /// Header Replace-with instead uses `cancel_source` for its selected source.
    pub(crate) fn cancel_all(mut self) {
        self.retry.set(None);
    }

    /// Reserve before spawning, reusing a key only for the same source.
    /// An answer authorizes YOLO but does not substitute its old key for a newer
    /// same-source retry: identity still follows the shared page slot.
    pub(crate) fn prepare(
        mut self,
        source: Session,
        guard: DeleteGuard,
        answer: Option<ReplaceAnswer>,
    ) -> ReplaceAttempt {
        let id = source.id.raw().to_owned();
        let retained = self
            .retry
            .peek()
            .as_ref()
            .filter(|(owner, _)| owner == &id)
            .and_then(|(_, key)| key.clone());
        self.retry.set(Some((id, retained.clone())));
        ReplaceAttempt {
            source,
            guard,
            retained,
            answer,
        }
    }

    /// Publish a minted key only while this source still owns an empty reservation.
    /// A late A reply cannot overwrite B's intent, or a key already published for A.
    fn publish_key(mut self, source: &str, key: &str) {
        if self
            .retry
            .peek()
            .as_ref()
            .is_some_and(|(id, key)| id == source && key.is_none())
        {
            self.retry
                .set(Some((source.to_owned(), Some(key.to_owned()))));
        }
    }

    /// End only the submitted source/key, leaving later attempts untouched.
    /// Unknown, authentication, decode and partial failures do not prove that a
    /// replacement was never created, so they retain the identity for retry.
    fn settle(self, source: &str, key: &str, outcome: RefusalOutcome) {
        if outcome.retires_retry() {
            self.end_key(source, key);
        }
    }

    /// Success and exact-question cancellation share the same ownership check.
    fn end_key(mut self, source: &str, key: &str) {
        if self
            .retry
            .peek()
            .as_ref()
            .is_some_and(|(id, held)| id == source && held.as_deref() == Some(key))
        {
            self.retry.set(None);
        }
    }

    /// Admit a source before replacing the local opening and its failure text.
    /// A duplicate must leave both existing questions and their errors intact.
    fn open_question(mut self, pending: PendingReplace, error: Option<String>) -> bool {
        if !self
            .exclusions
            .write()
            .insert(pending.source.id.raw().to_owned())
        {
            return false;
        }
        self.release_question();
        self.error.set(error);
        // Replaces any open question; its token, and every handler still
        // carrying it, stops matching even if `pending` is equal.
        self.question.open(pending, ());
        true
    }

    /// Release only this surface's currently open question's source exclusion.
    /// Used when a newer question replaces the open one, when the session
    /// surface's host save succeeds, and on unmount; it never retires a retry
    /// identity. Answers release through `take_question` instead.
    pub(crate) fn release_question(mut self) {
        if let Some(pending) = self.question.peek_key() {
            self.exclusions.write().remove(pending.source.id.raw());
        }
    }

    /// Consume exactly one opening; stale handlers leave a newer question alone.
    fn take_question(mut self, question: &ReplaceYoloQuestion) -> bool {
        if self.question.take(question).is_none() {
            return false;
        }
        // The token matched, so the question just closed was this one and
        // its source is the exclusion to give back.
        self.exclusions
            .write()
            .remove(question.key().source.id.raw());
        self.error.set(None);
        true
    }

    /// Answer a rendered sidebar opening only after row admission succeeds.
    /// Admission refusal preserves the prompt; admitted-but-stale answers undo
    /// their row admission. A hostless permanent answer is never dispatched.
    pub(crate) fn answer_sidebar(
        self,
        question: &ReplaceYoloQuestion,
        stop_asking: bool,
        admit: impl FnOnce() -> bool,
        release: impl FnOnce(),
    ) -> Option<ReplaceAnswer> {
        let pending = question.key().clone();
        let stop_asking = if stop_asking {
            Some((pending.ask.host?, pending.ask.host_name.clone()))
        } else {
            None
        };
        if !admit() {
            return None;
        }
        if !self.take_question(question) {
            release();
            return None;
        }
        Some(ReplaceAnswer {
            pending,
            stop_asking,
        })
    }

    /// Answer the session question `question` names, after its lifecycle guard
    /// was obtained: `None`, changing nothing, unless that exact opening is
    /// still the open one. Permanent answers leave it mounted until the host
    /// save succeeds. Its hostless permanent button is not offered, but
    /// retains the old no-save continuation if invoked rather than adding a
    /// new refusal here.
    pub(crate) fn answer_session(
        mut self,
        question: &ReplaceYoloQuestion,
        stop_asking: bool,
    ) -> Option<ReplaceAnswer> {
        if !self.is_open(question) {
            return None;
        }
        let pending = question.key().clone();
        let stop_asking = if stop_asking {
            pending
                .ask
                .host
                .map(|host| (host, pending.ask.host_name.clone()))
        } else {
            self.take_question(question);
            None
        };
        self.error.set(None);
        Some(ReplaceAnswer {
            pending,
            stop_asking,
        })
    }

    /// Cancel the rendered question and only its exact retry key. Both
    /// surfaces' Cancel buttons use it; a stale Cancel changes nothing.
    pub(crate) fn cancel_question(self, question: &ReplaceYoloQuestion) {
        if self.take_question(question) {
            let pending = question.key();
            self.end_key(pending.source.id.raw(), &pending.key);
        }
    }

    /// Apply a completed permanent-answer save without moving surface timing.
    /// Sidebar has already consumed its question; session keeps it until this
    /// point. A failed sidebar save may reopen only while its local slot is empty.
    fn host_saved(mut self, pending: PendingReplace, result: Result<(), String>) -> bool {
        match result {
            Ok(()) => {
                if self.policy == QuestionPolicy::Session {
                    // Closes whatever is open rather than a token: the save
                    // carries no opening, and this is the same unconditional
                    // close the session surface has always done here. Only
                    // this surface's own Replace run opens its question, and
                    // the session view starts no second run while this one
                    // (which is doing the save) is in flight, so what is open
                    // is the question being answered.
                    self.release_question();
                    self.question.clear();
                }
                true
            }
            Err(reason) => {
                match self.policy {
                    QuestionPolicy::Sidebar if self.question.peek_key().is_none() => {
                        self.open_question(pending, Some(reason));
                    }
                    QuestionPolicy::Sidebar => {}
                    QuestionPolicy::Session => self.error.set(Some(reason)),
                }
                false
            }
        }
    }

    /// Settle one endpoint refusal and decide whether its question represents it.
    /// Duplicate-source suppression is intentionally silent in the sidebar, while
    /// the header retains its ordinary refusal text when another pane owns it.
    fn refused(
        self,
        source: Session,
        guard: DeleteGuard,
        key: String,
        error: api::ActionRefusal,
    ) -> ReplaceResult {
        self.settle(source.id.raw(), &key, error.outcome);
        if error.outcome.requires_yolo() {
            let ask = YoloAsk {
                host: source.host,
                // The confirmation escapes the raw snapshot at render time.
                host_name: source
                    .host_name
                    .as_ref()
                    .map(|name| name.raw().to_owned())
                    .unwrap_or_else(|| "this host".to_string()),
                reason: YoloReason::of_launch(source.agent_selection()),
            };
            if self.open_question(
                PendingReplace {
                    key,
                    ask,
                    source,
                    guard,
                },
                None,
            ) || self.policy == QuestionPolicy::Sidebar
            {
                return ReplaceResult::QuestionHandled;
            }
        }
        ReplaceResult::Refused(error.text)
    }

    /// Run the shared save → mint/reuse → Replace → settle sequence.
    /// The adapter owns admission throughout this future and its final success
    /// callbacks. This method neither spawns work nor releases an operation guard.
    pub(crate) async fn run(self, base: &str, attempt: ReplaceAttempt) -> ReplaceResult {
        let ReplaceAttempt {
            source,
            guard,
            retained,
            answer,
        } = attempt;
        let allow_yolo = answer.is_some();
        if let Some(ReplaceAnswer {
            pending,
            stop_asking: Some((host, name)),
        }) = answer
        {
            let result = crate::yolo_confirm::stop_asking(base, host, &name).await;
            if !self.host_saved(pending, result) {
                return ReplaceResult::QuestionHandled;
            }
        }
        let id = source.id.raw();
        let key = match retained {
            Some(key) => key,
            None => match api::mint_intent_key().await {
                Ok(key) => {
                    self.publish_key(id, &key);
                    key
                }
                Err(reason) => return ReplaceResult::Refused(reason),
            },
        };
        match api::replace_session(base, id, &key, guard, allow_yolo).await {
            Ok((session, notice)) => {
                self.end_key(id, &key);
                ReplaceResult::Replaced(Box::new(session), notice)
            }
            Err(error) => self.refused(source, guard, key, error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::ops::use_confirm_slot;

    std::thread_local! {
        static CONTROLLERS: Cell<Option<(ReplaceController, ReplaceController)>> = const { Cell::new(None) };
    }

    /// Give both surfaces their real shared identity/exclusion and local questions.
    /// The VirtualDom owns the signals throughout every assertion; no second
    /// implementation of the controller's state machine is used by these tests.
    fn with_controllers(test: impl FnOnce(ReplaceController, ReplaceController)) {
        /// Mount both real controllers without rendering a DOM or starting IO.
        /// Their signals belong to the same scope but their questions stay local.
        fn app() -> Element {
            let retry = use_signal(|| None);
            let exclusions = use_signal(HashSet::new);
            let sidebar = ReplaceController::sidebar(
                retry,
                exclusions,
                use_confirm_slot(),
                use_signal(|| None),
            );
            let session = ReplaceController::session(
                retry,
                exclusions,
                use_confirm_slot(),
                use_signal(|| None),
            );
            CONTROLLERS.with(|cell| cell.set(Some((sidebar, session))));
            rsx! {}
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let (sidebar, session) = CONTROLLERS
            .with(Cell::get)
            .expect("both controllers mounted");
        dom.in_runtime(|| test(sidebar, session));
    }

    /// A stable source snapshot isolates consent/identity decisions from listing IO.
    /// Its host deliberately differs from any later live row a test might show.
    fn source(id: &str) -> Session {
        Session {
            screen_yolo: false,
            claude_trust_prompt: false,
            workspace_trust_prompt: false,
            id: id.into(),
            title: "stable".into(),
            cwd: "/tmp".into(),
            canonical_cwd: None,
            invocation: "agent".into(),
            agent_kind: crate::SessionAgentKind::Unrecognized,
            launch: None,
            status: crate::SessionStatus::Exited { exit_code: Some(0) },
            annotation: None,
            restart_offer: crate::RestartOffer::NotCaptured,
            created_at: 0,
            last_activity_at: 0,
            tabs: Vec::new(),
            host: Some(7),
            host_identity: None,
            host_name: Some("build box".into()),
            stale: false,
            github_repo: None,
            working_copy: None,
            seen_activity_at: None,
            pinned: false,
            notifications: Vec::new(),
            notifications_read_through: 0,
        }
    }

    /// Construct an already refused operation without minting or sending anything.
    /// Real requests construct exactly this captured source, guard and host question.
    fn pending(id: &str, key: &str) -> PendingReplace {
        PendingReplace {
            key: key.to_owned(),
            ask: YoloAsk {
                host: Some(7),
                host_name: "build box".to_owned(),
                reason: YoloReason::Asserted,
            },
            source: source(id),
            guard: DeleteGuard::NothingAlive,
        }
    }

    /// Reserving and publishing happen before request completion, shared across
    /// surfaces. Late A replies must neither overwrite nor clear B, and retrying
    /// A on another surface reuses exactly its published key.
    #[farhelm_testtrace::test]
    fn retry_reservation_and_settlement_respect_exact_ownership() {
        with_controllers(|sidebar, header| {
            let first = sidebar.prepare(source("A"), DeleteGuard::NothingAlive, None);
            assert!(first.retained.is_none());
            assert_eq!(*header.retry.peek(), Some(("A".to_owned(), None)));
            sidebar.publish_key("A", "a1");
            assert_eq!(
                header
                    .prepare(source("A"), DeleteGuard::NothingAlive, None)
                    .retained
                    .as_deref(),
                Some("a1")
            );
            header.prepare(source("B"), DeleteGuard::NothingAlive, None);
            sidebar.publish_key("A", "late-a");
            sidebar.end_key("A", "a1");
            sidebar.settle("A", "a1", RefusalOutcome::DefinitelyUnaccepted);
            assert_eq!(*header.retry.peek(), Some(("B".to_owned(), None)));
            header.publish_key("B", "b1");
            sidebar.publish_key("B", "late-b");
            assert_eq!(
                header.retry.peek().as_ref().unwrap().1.as_deref(),
                Some("b1")
            );
            header.end_key("B", "b1");
            assert!(
                sidebar.retry.peek().is_none(),
                "success ends exactly its submitted key"
            );
        });
    }

    /// Only proof that creation was unaccepted retires a retry. Unknown outcomes
    /// include transport, authentication, decode and partial failures; YOLO is a
    /// continuation even when other refusal headers also carried proof.
    #[farhelm_testtrace::test]
    fn refusal_outcomes_keep_ambiguous_and_yolo_attempts() {
        with_controllers(|controller, _| {
            for (outcome, retires) in [
                (RefusalOutcome::Unresolved, false),
                (RefusalOutcome::NeedsYoloConfirmation, false),
                (RefusalOutcome::DefinitelyUnaccepted, true),
                (RefusalOutcome::StalePrecondition, true),
            ] {
                controller.prepare(source("A"), DeleteGuard::NothingAlive, None);
                controller.publish_key("A", "a1");
                controller.settle("A", "a1", outcome);
                assert_eq!(controller.retry.peek().is_none(), retires);
            }
        });
    }

    /// An old A question may remain while B then a new A owns retry identity.
    /// Cancelling it must release its question exclusion without cancelling that
    /// newer key; ordinary prompt cancellation still owns the whole source slot.
    #[farhelm_testtrace::test]
    fn old_question_cancellation_does_not_cancel_a_new_same_source_retry() {
        with_controllers(|sidebar, header| {
            assert!(sidebar.open_question(pending("A", "a1"), None));
            let old = sidebar.current_question().unwrap();
            header.prepare(source("B"), DeleteGuard::NothingAlive, None);
            header.prepare(source("A"), DeleteGuard::NothingAlive, None);
            header.publish_key("A", "a2");
            sidebar.cancel_question(&old);
            assert!(!sidebar.exclusions.peek().contains("A"));
            assert_eq!(
                header.retry.peek().as_ref().unwrap().1.as_deref(),
                Some("a2")
            );
            sidebar.cancel_source("B");
            assert!(header.retry.peek().is_some());
            sidebar.cancel_source("A");
            assert!(header.retry.peek().is_none());
        });
    }

    /// The ordinary row prompt retires another source before asking, while
    /// Replace-with retires the whole slot. Neither choice dismisses an already
    /// visible YOLO question, whose consent and exact-key cancel remain separate.
    #[farhelm_testtrace::test]
    fn selecting_another_source_and_replace_with_end_only_retry_identity() {
        with_controllers(|sidebar, header| {
            sidebar.prepare(source("A"), DeleteGuard::NothingAlive, None);
            sidebar.publish_key("A", "a1");
            assert!(sidebar.open_question(pending("A", "a1"), None));
            sidebar.select_source("A");
            assert!(header.retry.peek().is_some());
            sidebar.select_source("B");
            assert!(header.retry.peek().is_none());
            assert!(sidebar.current_question().is_some());
            header.prepare(source("B"), DeleteGuard::NothingAlive, None);
            sidebar.cancel_all();
            assert!(header.retry.peek().is_none());
            assert!(sidebar.current_question().is_some());
            assert!(sidebar.exclusions.peek().contains("A"));
        });
    }

    /// Source exclusion is independent of key and local question replacement.
    /// Rejecting a duplicate changes neither pane's question/error. A different
    /// source remains usable, and releasing one pane leaves the other's ownership.
    #[farhelm_testtrace::test]
    fn question_exclusion_preserves_existing_owners_and_failure_text() {
        with_controllers(|sidebar, header| {
            assert!(sidebar.open_question(pending("A", "a1"), Some("old reason".to_owned())));
            assert!(header.open_question(pending("B", "b1"), Some("header reason".to_owned())));
            assert!(!sidebar.open_question(pending("B", "b2"), None));
            assert_eq!(
                sidebar.current_question().unwrap().key().source.id.raw(),
                "A"
            );
            assert_eq!(sidebar.error().as_deref(), Some("old reason"));
            assert_eq!(header.error().as_deref(), Some("header reason"));
            assert!(sidebar.open_question(pending("C", "c1"), None));
            assert!(!header.exclusions.peek().contains("A"));
            assert!(header.exclusions.peek().contains("B"));
            assert!(header.exclusions.peek().contains("C"));
            sidebar.release_question();
            assert!(!header.exclusions.peek().contains("C"));
            assert!(header.exclusions.peek().contains("B"));
        });
    }

    /// Sidebar answer admission precedes consuming its rendered opening. Cancel
    /// followed by either answer in one event burst sends nothing and rolls back
    /// any admission; an old same-source opening cannot consume a later one.
    #[farhelm_testtrace::test]
    fn cancelled_or_stale_sidebar_answers_dispatch_nothing() {
        with_controllers(|sidebar, _| {
            let held = Cell::new(0);
            let admit = || {
                held.set(held.get() + 1);
                true
            };
            let release = || held.set(held.get() - 1);
            for permanent in [false, true] {
                assert!(sidebar.open_question(pending("A", "a1"), None));
                let old = sidebar.current_question().unwrap();
                sidebar.cancel_question(&old);
                assert!(
                    sidebar.current_question().is_none(),
                    "premise: Cancel consumed it"
                );
                assert!(
                    sidebar
                        .answer_sidebar(&old, permanent, admit, release)
                        .is_none()
                );
                assert_eq!(held.get(), 0);
                assert!(sidebar.open_question(pending("A", "a1"), Some("reason".to_owned())));
                assert!(
                    sidebar
                        .answer_sidebar(&old, permanent, admit, release)
                        .is_none()
                );
                let current = sidebar.current_question().unwrap();
                assert!(
                    old != current && old.key() == current.key(),
                    "premise: the same request reopened under a new opening"
                );
                assert!(
                    sidebar
                        .answer_sidebar(&current, permanent, || false, release)
                        .is_none()
                );
                assert_eq!(sidebar.error().as_deref(), Some("reason"));
                let answer = sidebar
                    .answer_sidebar(&current, permanent, admit, release)
                    .expect("current answer dispatches");
                assert!(sidebar.current_question().is_none());
                assert!(!sidebar.exclusions.peek().contains("A"));
                assert_eq!(answer.pending.guard, DeleteGuard::NothingAlive);
                assert_eq!(
                    answer.stop_asking,
                    permanent.then(|| (7, "build box".to_owned()))
                );
                assert_eq!(held.get(), 1);
                release();
            }
            let mut hostless = pending("A", "a3");
            hostless.ask.host = None;
            assert!(sidebar.open_question(hostless, None));
            let question = sidebar.current_question().unwrap();
            assert!(
                sidebar
                    .answer_sidebar(&question, true, admit, release)
                    .is_none()
            );
            assert_eq!(held.get(), 0);
            assert!(sidebar.current_question().is_some());
        });
    }

    /// Permanent answers preserve each surface's existing mount and focus timing.
    /// Sidebar consumes before the write and may reopen only an empty slot;
    /// header stays mounted/excluded through failure and hides only after success.
    #[farhelm_testtrace::test]
    fn permanent_answer_policy_preserves_the_host_save_boundary() {
        with_controllers(|sidebar, header| {
            assert!(sidebar.open_question(pending("A", "a1"), None));
            let question = sidebar.current_question().unwrap();
            let answer = sidebar
                .answer_sidebar(&question, true, || true, || {})
                .unwrap();
            assert!(
                sidebar.current_question().is_none(),
                "sidebar is down while save waits"
            );
            assert!(!sidebar.host_saved(answer.pending.clone(), Err("save failed".to_owned())));
            assert_eq!(sidebar.error().as_deref(), Some("save failed"));
            let question = sidebar.current_question().unwrap();
            let answer = sidebar
                .answer_sidebar(&question, true, || true, || {})
                .unwrap();
            assert!(sidebar.open_question(pending("C", "c1"), None));
            assert!(!sidebar.host_saved(answer.pending, Err("late failure".to_owned())));
            assert_eq!(
                sidebar.current_question().unwrap().key().source.id.raw(),
                "C"
            );
            assert!(
                sidebar.error().is_none(),
                "late failure never replaces a newer question"
            );

            assert!(header.open_question(pending("B", "b1"), None));
            let question = header.current_question().unwrap();
            let answer = header.answer_session(&question, true).unwrap();
            assert!(
                header.current_question().as_ref() == Some(&question),
                "header stays mounted while save waits"
            );
            assert!(header.exclusions.peek().contains("B"));
            assert!(!header.host_saved(answer.pending.clone(), Err("save failed".to_owned())));
            assert!(header.current_question().as_ref() == Some(&question));
            assert_eq!(header.error().as_deref(), Some("save failed"));
            let answer = header.answer_session(&question, true).unwrap();
            assert!(header.error().is_none());
            assert!(header.host_saved(answer.pending, Ok(())));
            assert!(header.current_question().is_none());
            assert!(!header.exclusions.peek().contains("B"));
            assert!(header.exclusions.peek().contains("C"));
            assert!(header.open_question(pending("B", "b2"), None));
            let question = header.current_question().unwrap();
            assert!(header.answer_session(&question, false).is_some());
            assert!(
                header.current_question().is_none(),
                "one-off consumes before dispatch"
            );
        });
    }

    /// Why: the session header's YOLO buttons used to answer whatever Replace
    /// question was open when the click ran, so a click queued behind a
    /// Cancel and a reopen could approve a question the user had not seen.
    /// The maintainer decided on 2026-10-21 that the header answers only what
    /// it showed, like the sidebar.
    ///
    /// Spec: a header answer or Cancel drawn for an earlier opening does
    /// nothing to a later opening of the same request (same source, same
    /// retry key), leaving it open with its error; an answer for the open
    /// opening proceeds, and a hostless permanent answer keeps today's
    /// no-save continuation.
    #[farhelm_testtrace::test]
    fn session_answers_only_the_rendered_opening() {
        with_controllers(|_, header| {
            assert!(header.open_question(pending("A", "a1"), None));
            let old = header.current_question().unwrap();
            header.cancel_question(&old);
            assert!(
                header.current_question().is_none(),
                "premise: Cancel closed it"
            );
            assert!(header.open_question(pending("A", "a1"), None));
            let later = header.current_question().unwrap();
            assert!(
                old != later && old.key() == later.key(),
                "premise: the same request reopened under a new opening"
            );
            // Give the reopened question an error a stale click must not clear.
            let saving = header.answer_session(&later, true).unwrap();
            assert!(!header.host_saved(saving.pending, Err("save failed".to_owned())));
            assert!(header.is_open(&later) && !header.is_open(&old));
            assert!(header.answer_session(&old, false).is_none());
            assert!(header.answer_session(&old, true).is_none());
            header.cancel_question(&old);
            assert!(
                header.current_question().as_ref() == Some(&later),
                "a stale header answer or Cancel leaves the reopened question"
            );
            assert_eq!(header.error().as_deref(), Some("save failed"));
            let answer = header.answer_session(&later, false).unwrap();
            assert_eq!(answer.pending.key, "a1");
            let mut hostless = pending("A", "a3");
            hostless.ask.host = None;
            assert!(header.open_question(hostless, None));
            let question = header.current_question().unwrap();
            let answer = header.answer_session(&question, true).unwrap();
            assert!(answer.stop_asking.is_none());
            assert!(header.current_question().is_some());
        });
    }

    /// Duplicate-source refusal has different existing presentation per surface:
    /// sidebar silently leaves the owner alone, header shows its ordinary reason.
    #[farhelm_testtrace::test]
    fn duplicate_yolo_refusal_keeps_surface_error_routing() {
        with_controllers(|sidebar, header| {
            let refusal = || api::ActionRefusal {
                outcome: RefusalOutcome::NeedsYoloConfirmation,
                text: "refused".to_owned(),
            };
            assert!(sidebar.open_question(pending("A", "a1"), Some("kept".to_owned())));
            assert!(
                matches!(header.refused(source("A"), DeleteGuard::NothingAlive, "a2".to_owned(), refusal()), ReplaceResult::Refused(text) if text == "refused")
            );
            assert!(matches!(
                sidebar.refused(
                    source("A"),
                    DeleteGuard::NothingAlive,
                    "a2".to_owned(),
                    refusal()
                ),
                ReplaceResult::QuestionHandled
            ));
            assert_eq!(sidebar.error().as_deref(), Some("kept"));
            assert!(header.current_question().is_none());
        });
    }
}
