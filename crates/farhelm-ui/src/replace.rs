//! Plain Replace's retry identity, pending status and YOLO continuation, shared by both panes.
//!
//! Admission, tasks and successful selection belong to the persistent list;
//! header actions forward into those same row cores. This controller owns the
//! rules between those boundaries. A submitted source and its destructive
//! precondition are snapshots of consent, never live reads.

use std::collections::{HashMap, HashSet};

use dioxus::prelude::*;

use crate::api::{self, RefusalOutcome};
use crate::launch_progress::{LaunchAction, PendingLaunch};
use crate::ops::{ConfirmSlots, ConfirmToken};
use crate::yolo_confirm::{YoloAsk, YoloReason};
use crate::{DeleteGuard, HostId, Session};

/// The exact Replace refused by the helm, including the consent it continues.
///
/// A restarted source or a new terminal tab must not broaden an earlier
/// nothing-alive answer. Retry identity can move independently when another
/// source starts work; the question keeps the key its refusal actually named.
#[derive(Clone, PartialEq)]
pub(crate) struct PendingReplace {
    key: String,
    pub(crate) ask: YoloAsk,
    pub(crate) source: Session,
    pub(crate) guard: DeleteGuard,
    /// The page's press count when the user's Replace answer started this
    /// request (`LaunchActivity::snapshot`), or `None` when it is unknown.
    ///
    /// The question's Cancel takes focus outright only while the count is
    /// unchanged. Focus alone cannot tell: WebKit does not focus a clicked
    /// button, so after a header Replace click focus often stays in the
    /// session's terminal although the user never left the Replace. A question
    /// reopened after a failed host save keeps the first answer's count, so the
    /// later answer's own press sends it back to the ordinary guard.
    pub(crate) press_baseline: Option<u64>,
}

/// One local opening of a Replace question: the shared prompt mechanism's
/// token, whose key is the captured operation the question displays.
///
/// The opening number comes from the controller's [`ConfirmSlots`], like
/// every other prompt in the app; Replace has no counter of its own. Every
/// answer names the token its render drew, so a queued answer behind Cancel,
/// or behind a reopen of the same request, cannot dispatch: the reopen is a
/// new opening even when its [`PendingReplace`] is equal. Header requests use
/// this same persistent question rather than creating one in the keyed detail
/// view. Read its consent with [`ConfirmToken::key`].
pub(crate) type ReplaceYoloQuestion = ConfirmToken<PendingReplace>;

/// Approval to continue one captured Replace, optionally after marking its host.
/// The host comes from the question, never a newly fetched source session.
pub(crate) struct ReplaceAnswer {
    pub(crate) pending: PendingReplace,
    stop_asking: Option<(HostId, String)>,
}

/// An admitted operation whose retry reservation was published synchronously.
/// Keep this value in the persistent list task; creating its future alone must
/// not postpone reserving the source until the executor first polls it.
pub(crate) struct ReplaceAttempt {
    source: Session,
    guard: DeleteGuard,
    retained: Option<String>,
    answer: Option<ReplaceAnswer>,
    press_baseline: Option<u64>,
}

impl ReplaceAttempt {
    /// Record the press count read synchronously in the answering handler.
    /// A refusal's YOLO question carries it (`PendingReplace::press_baseline`).
    pub(crate) fn with_press_baseline(mut self, baseline: Option<u64>) -> Self {
        self.press_baseline = baseline;
        self
    }
}

/// The requests all Replace surfaces are waiting on, keyed by their source.
///
/// The page owns this signal so a sidebar request and the open session show
/// the same start time. Both plain Replace and Replace with also defer an
/// absence-driven removal of the selected source while its request is here:
/// the backend may remove that source before its successful reply reaches the
/// selection callback. Explicit user selection remains independent. This map
/// does not admit requests; existing row and lifecycle guards still do that.
#[derive(Clone, Copy)]
pub(crate) struct ReplaceProgress(pub(crate) Signal<HashMap<String, PendingLaunch>>);

/// Name the host captured by consent, never a refreshed or selected row.
/// Both the pending label and a later unknown-outcome explanation use this
/// snapshot so a host refresh cannot change which machine the request names.
fn pending_progress(source: &Session) -> PendingLaunch {
    PendingLaunch::new(
        LaunchAction::Replace,
        source
            .host_name
            .as_ref()
            .map(|name| name.raw().to_owned())
            .unwrap_or_else(|| "this host".to_string()),
        None,
    )
}

/// Clear only this request's status when its future ends, including cancellation.
///
/// Navigation leaves the persistent request running, but dropping its future
/// must still clear the page-owned status. The captured value also prevents
/// an old task clearing a later run.
struct ReplaceProgressGuard {
    progress: ReplaceProgress,
    source: String,
    pending: PendingLaunch,
}

impl Drop for ReplaceProgressGuard {
    fn drop(&mut self) {
        if let Ok(mut requests) = self.progress.0.try_write()
            && requests.get(&self.source) == Some(&self.pending)
        {
            requests.remove(&self.source);
        }
    }
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

/// Persistent-list Replace state over page-owned retry and question-exclusion slots.
///
/// Copying this handle does not create another state owner. The list creates
/// the question lanes and their errors, and releases their exclusions on
/// unmount; neither unmount nor navigation discards the page's unresolved
/// retry identity.
///
/// Each source has an existing [`ConfirmSlots`] lane keyed by the
/// [`PendingReplace`] it displays. Independent replies cannot replace a
/// question the user has not answered. The lane mints opening identity, while
/// the shared exclusion set makes that question visible to all session action
/// handlers. Errors stay source-local too. Retry identity remains the existing
/// single page slot; adding question lanes does not introduce a request queue.
#[derive(Clone, Copy)]
pub(crate) struct ReplaceController {
    retry: Signal<Option<(String, Option<String>)>>,
    exclusions: Signal<HashSet<String>>,
    progress: ReplaceProgress,
    question: ConfirmSlots<String, PendingReplace>,
    errors: Signal<HashMap<String, String>>,
}

impl ReplaceController {
    /// Bind the persistent list's question lanes to the existing page state.
    /// Token-bound answers consume their lane before either continuation.
    pub(crate) fn sidebar(
        retry: Signal<Option<(String, Option<String>)>>,
        exclusions: Signal<HashSet<String>>,
        progress: ReplaceProgress,
        question: ConfirmSlots<String, PendingReplace>,
        errors: Signal<HashMap<String, String>>,
    ) -> Self {
        Self {
            retry,
            exclusions,
            progress,
            question,
            errors,
        }
    }

    /// Read the same pending request from a row, header or interrupted card.
    /// No entry means no Replace POST is pending, even during key minting or
    /// a permanent YOLO answer's host-setting save.
    pub(crate) fn progress(self, source: &str) -> Option<PendingLaunch> {
        self.progress.0.read().get(source).cloned()
    }

    /// Publish request-local timing after preparation and own its cleanup.
    /// The source snapshot supplies the host; a later listing cannot relabel
    /// a request already dispatched to a different host.
    fn start_progress(mut self, source: &Session) -> ReplaceProgressGuard {
        let pending = pending_progress(source);
        let id = source.id.raw().to_owned();
        self.progress.0.write().insert(id.clone(), pending.clone());
        ReplaceProgressGuard {
            progress: self.progress,
            source: id,
            pending,
        }
    }

    /// Render unanswered sources in stable order, with each lane's drawn token.
    /// Reply arrival order cannot replace another source's unanswered question.
    /// Every handler captures the returned token rather than rereading it later.
    pub(crate) fn questions(self) -> Vec<ReplaceYoloQuestion> {
        let mut sources = self.question.open_lanes();
        sources.sort();
        sources
            .into_iter()
            .filter_map(|source| self.question.current(&source))
            .collect()
    }

    /// The open question's token without subscribing, for tests that name an
    /// opening no render drew. Test-only on purpose: a handler that could
    /// fetch the live token could answer whatever is open at click time;
    /// handlers pass their rendered opening to [`Self::answer_sidebar`] instead.
    #[cfg(test)]
    fn current_question(self, source: &str) -> Option<ReplaceYoloQuestion> {
        self.question.peek_token(source)
    }

    /// Read only this source's host-setting failure beside its question.
    pub(crate) fn error(self, source: &str) -> Option<String> {
        self.errors.read().get(source).cloned()
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
            press_baseline: None,
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

    /// Open an unoccupied source lane without changing any existing answer.
    /// A duplicate source must preserve its token and failure text; independent
    /// sources may ask concurrently and retain their own admission exclusion.
    fn open_question(mut self, pending: PendingReplace, error: Option<String>) -> bool {
        let source = pending.source.id.raw().to_owned();
        if self.question.peek_key(source.as_str()).is_some() {
            return false;
        }
        if !self.exclusions.write().insert(source.clone()) {
            return false;
        }
        if let Some(error) = error {
            self.errors.write().insert(source.clone(), error);
        } else {
            self.errors.write().remove(&source);
        }
        self.question.open(source, pending, ());
        true
    }

    /// Release this controller's question lanes when its owner unmounts.
    /// Other controllers' source exclusions and the page's retry identity stay
    /// intact. Individual answers use `take_question` instead.
    pub(crate) fn release_question(mut self) {
        for source in self.question.open_lanes() {
            self.exclusions.write().remove(&source);
            self.errors.write().remove(&source);
        }
        self.question.retain(|_| false);
    }

    /// Consume exactly one opening; stale handlers leave a newer question alone.
    fn take_question(mut self, question: &ReplaceYoloQuestion) -> bool {
        let source = question.key().source.id.raw();
        if self.question.take(source, question).is_none() {
            return false;
        }
        // The token matched, so the question just closed was this one and
        // its source is the exclusion to give back.
        self.exclusions.write().remove(source);
        self.errors.write().remove(source);
        true
    }

    /// Transfer only the rendered question's exclusion into synchronous admission.
    /// Taking the matching token releases its source before `admit` checks the
    /// common session guard, without an await or another event between them.
    /// Refusal restores the captured question and error under a fresh opening;
    /// stale answers and hostless permanent answers change nothing.
    pub(crate) fn answer_sidebar(
        self,
        question: &ReplaceYoloQuestion,
        stop_asking: bool,
        admit: impl FnOnce() -> bool,
    ) -> Option<ReplaceAnswer> {
        let pending = question.key().clone();
        let stop_asking = if stop_asking {
            Some((pending.ask.host?, pending.ask.host_name.clone()))
        } else {
            None
        };
        let previous_error = self.errors.peek().get(pending.source.id.raw()).cloned();
        if !self.take_question(question) {
            return None;
        }
        if !admit() {
            self.open_question(pending, previous_error);
            return None;
        }
        Some(ReplaceAnswer {
            pending,
            stop_asking,
        })
    }

    /// Cancel the rendered question and only its exact retry key.
    /// A stale Cancel leaves a later opening and its retry identity untouched.
    pub(crate) fn cancel_question(self, question: &ReplaceYoloQuestion) {
        if self.take_question(question) {
            let pending = question.key();
            self.end_key(pending.source.id.raw(), &pending.key);
        }
    }

    /// Return failed permanent consent through whichever owner can still show it.
    /// Answers consume their question before saving the host preference. A failed
    /// save reopens it only if its source lane is empty and exclusion permits it;
    /// otherwise its ordinary source outcome must carry the reason. Neither a
    /// newer question nor its error may be replaced by this late failure.
    fn host_saved(
        self,
        pending: PendingReplace,
        result: Result<(), String>,
    ) -> Result<(), ReplaceResult> {
        match result {
            Ok(()) => Ok(()),
            Err(reason) => {
                if self.question.peek_key(pending.source.id.raw()).is_none()
                    && self.open_question(pending, Some(reason.clone()))
                {
                    Err(ReplaceResult::QuestionHandled)
                } else {
                    Err(ReplaceResult::Refused(reason))
                }
            }
        }
    }

    /// Settle one endpoint refusal and decide whether its question represents it.
    /// A source's existing question and error survive a duplicate refusal.
    /// If no new question can be represented, the caller still receives its
    /// ordinary row/header refusal rather than silently losing that result.
    fn refused(
        self,
        source: Session,
        guard: DeleteGuard,
        key: String,
        error: api::ActionRefusal,
        press_baseline: Option<u64>,
    ) -> ReplaceResult {
        self.settle(source.id.raw(), &key, error.outcome);
        let deadline_text = error.deadline_expired.then(|| {
            // Sidebar rows may run independently, but only one plain Replace
            // identity is retained. Starting another source can have ended
            // this key while its POST was still waiting for a reply.
            let retry_safe = self.retry.peek().as_ref().is_some_and(|(id, held)| {
                id == source.id.raw() && held.as_deref() == Some(key.as_str())
            });
            pending_progress(&source).unknown_outcome(retry_safe)
        });
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
                    press_baseline,
                },
                None,
            ) {
                return ReplaceResult::QuestionHandled;
            }
        }
        ReplaceResult::Refused(deadline_text.unwrap_or(error.text))
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
            press_baseline,
        } = attempt;
        let allow_yolo = answer.is_some();
        if let Some(ReplaceAnswer {
            pending,
            stop_asking: Some((host, name)),
        }) = answer
        {
            let result = crate::yolo_confirm::stop_asking(base, host, &name).await;
            if let Err(outcome) = self.host_saved(pending, result) {
                return outcome;
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
        let progress = self.start_progress(&source);
        let result = api::replace_session(base, id, &key, guard, allow_yolo).await;
        // End the status immediately before returning to the selection callback.
        // No await separates this removal from that callback: a fleet read must
        // not retire the selected source in the middle of this handoff.
        drop(progress);
        match result {
            Ok((session, notice)) => {
                self.end_key(id, &key);
                ReplaceResult::Replaced(Box::new(session), notice)
            }
            Err(error) => self.refused(source, guard, key, error, press_baseline),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::ops::use_confirm_slots;
    use crate::session_ops::SessionOps;

    std::thread_local! {
        static CONTROLLERS: Cell<Option<(ReplaceController, ReplaceController)>> = const { Cell::new(None) };
        static OPERATIONS: Cell<Option<SessionOps>> = const { Cell::new(None) };
    }

    /// Give two list controllers shared identity/exclusion and independent questions.
    /// Production has one persistent list; the second controller lets tests
    /// exercise late replies and competing ownership without copying its rules.
    /// The VirtualDom owns the signals throughout every assertion; no second
    /// implementation of the controller's state machine is used by these tests.
    fn with_controllers(test: impl FnOnce(ReplaceController, ReplaceController)) {
        /// Mount both real controllers without rendering a DOM or starting IO.
        /// Their signals belong to the same scope but their questions stay local.
        fn app() -> Element {
            let retry = use_signal(|| None);
            let operations = use_hook(SessionOps::new);
            let exclusions = operations.questions;
            let progress = ReplaceProgress(use_signal(HashMap::new));
            let sidebar = ReplaceController::sidebar(
                retry,
                exclusions,
                progress,
                use_confirm_slots(),
                use_signal(HashMap::new),
            );
            let other = ReplaceController::sidebar(
                retry,
                exclusions,
                progress,
                use_confirm_slots(),
                use_signal(HashMap::new),
            );
            CONTROLLERS.with(|cell| cell.set(Some((sidebar, other))));
            OPERATIONS.with(|cell| cell.set(Some(operations)));
            rsx! {}
        }
        let mut dom = VirtualDom::new(app);
        dom.rebuild_in_place();
        let (sidebar, other) = CONTROLLERS
            .with(Cell::get)
            .expect("both controllers mounted");
        dom.in_runtime(|| test(sidebar, other));
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
            pin_position: None,
            notifications: Vec::new(),
            notifications_read_through: 0,
            agent_terminal: None,
            supervisor_creates_host_terminals: false,
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
            press_baseline: None,
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

    /// Exact-key retirement is independent of the action admission boundary.
    /// Inject a later reservation directly: cancelling the captured question
    /// must preserve it, even though common UI admission refuses a new same-source
    /// action while that question is open. Ordinary cancellation owns the source.
    #[farhelm_testtrace::test]
    fn old_question_cancellation_does_not_cancel_a_new_same_source_retry() {
        with_controllers(|sidebar, header| {
            assert!(sidebar.open_question(pending("A", "a1"), None));
            let old = sidebar.current_question("A").unwrap();
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
            assert!(sidebar.current_question("A").is_some());
            header.prepare(source("B"), DeleteGuard::NothingAlive, None);
            sidebar.cancel_all();
            assert!(header.retry.peek().is_none());
            assert!(sidebar.current_question("A").is_some());
            assert!(sidebar.exclusions.peek().contains("A"));
        });
    }

    /// Every source keeps its own unanswered question and failure text. Opening
    /// another lane cannot discard its result; a duplicate source is refused
    /// across owners, and unmount releases only this controller's lanes.
    #[farhelm_testtrace::test]
    fn question_exclusion_preserves_existing_owners_and_failure_text() {
        with_controllers(|sidebar, header| {
            assert!(sidebar.open_question(pending("A", "a1"), Some("old reason".to_owned())));
            assert!(header.open_question(pending("B", "b1"), Some("header reason".to_owned())));
            assert!(!sidebar.open_question(pending("B", "b2"), None));
            assert_eq!(
                sidebar.current_question("A").unwrap().key().source.id.raw(),
                "A"
            );
            assert_eq!(sidebar.error("A").as_deref(), Some("old reason"));
            assert_eq!(header.error("B").as_deref(), Some("header reason"));
            assert!(sidebar.open_question(pending("C", "c1"), None));
            assert!(header.exclusions.peek().contains("A"));
            assert!(header.exclusions.peek().contains("B"));
            assert!(header.exclusions.peek().contains("C"));
            assert_eq!(
                sidebar
                    .questions()
                    .iter()
                    .map(|question| question.key().source.id.raw().to_owned())
                    .collect::<Vec<_>>(),
                ["A", "C"]
            );
            assert_eq!(sidebar.error("A").as_deref(), Some("old reason"));
            assert!(sidebar.error("C").is_none());
            sidebar.release_question();
            assert!(!header.exclusions.peek().contains("A"));
            assert!(!header.exclusions.peek().contains("C"));
            assert!(header.exclusions.peek().contains("B"));
        });
    }

    /// Only a matching token can release a question for synchronous admission.
    /// Cancel and stale answers never enter admission; a refused current answer
    /// restores its exact consent and error under a new, independently fenced token.
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
                let old = sidebar.current_question("A").unwrap();
                sidebar.cancel_question(&old);
                assert!(
                    sidebar.current_question("A").is_none(),
                    "premise: Cancel consumed it"
                );
                assert!(sidebar.answer_sidebar(&old, permanent, admit).is_none());
                assert_eq!(held.get(), 0);
                assert!(sidebar.open_question(pending("A", "a1"), Some("reason".to_owned())));
                assert!(sidebar.answer_sidebar(&old, permanent, admit).is_none());
                let current = sidebar.current_question("A").unwrap();
                assert!(
                    old != current && old.key() == current.key(),
                    "premise: the same request reopened under a new opening"
                );
                sidebar.cancel_question(&old);
                assert!(sidebar.current_question("A").as_ref() == Some(&current));
                assert!(sidebar.exclusions.peek().contains("A"));
                assert_eq!(sidebar.error("A").as_deref(), Some("reason"));
                assert!(
                    sidebar
                        .answer_sidebar(&current, permanent, || false)
                        .is_none()
                );
                assert_eq!(sidebar.error("A").as_deref(), Some("reason"));
                let restored = sidebar.current_question("A").unwrap();
                assert!(restored != current && restored.key() == current.key());
                sidebar.cancel_question(&current);
                assert!(sidebar.current_question("A").as_ref() == Some(&restored));
                let answer = sidebar
                    .answer_sidebar(&restored, permanent, admit)
                    .expect("current answer dispatches");
                assert!(sidebar.current_question("A").is_none());
                assert!(!sidebar.exclusions.peek().contains("A"));
                assert_eq!(answer.pending.guard, DeleteGuard::NothingAlive);
                assert_eq!(answer.pending.key, "a1");
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
            let question = sidebar.current_question("A").unwrap();
            assert!(sidebar.answer_sidebar(&question, true, admit).is_none());
            assert_eq!(held.get(), 0);
            assert!(sidebar.current_question("A").is_some());
        });
    }

    /// A rendered answer hands its actual question exclusion to the common
    /// session guard before dispatch. A stale answer must never call admission
    /// or release the continuing request, and B's question stays untouched.
    #[farhelm_testtrace::test]
    fn own_answer_transfers_question_to_common_session_claim() {
        with_controllers(|sidebar, _| {
            let operations = OPERATIONS.with(Cell::get).unwrap();
            assert!(sidebar.open_question(pending("A", "a1"), None));
            assert!(sidebar.open_question(pending("B", "b1"), Some("B reason".to_owned())));
            let question = sidebar.current_question("A").unwrap();
            let other = sidebar.current_question("B").unwrap();
            assert!(operations.claim("A").is_none());
            assert!(operations.claim("B").is_none());
            let bystander = operations.claim("C").expect("unrelated action can proceed");
            let held = RefCell::new(None);
            let answer = sidebar.answer_sidebar(&question, false, || {
                *held.borrow_mut() = operations.claim("A");
                held.borrow().is_some()
            });
            assert!(
                answer.is_some(),
                "the answer must not refuse its own question"
            );
            assert!(sidebar.current_question("A").is_none());
            assert!(!operations.questions.peek().contains("A"));
            assert!(operations.busy_now("A"), "the request now owns admission");
            assert!(operations.claim("A").is_none());
            assert!(
                sidebar
                    .answer_sidebar(&question, false, || panic!("stale answer admitted"))
                    .is_none()
            );
            sidebar.cancel_question(&question);
            assert!(
                operations.busy_now("A"),
                "stale Cancel cannot release a request"
            );
            assert!(sidebar.current_question("B").as_ref() == Some(&other));
            assert_eq!(sidebar.error("B").as_deref(), Some("B reason"));
            drop(held.borrow_mut().take());
            assert!(!operations.busy_now("A"));
            assert!(operations.busy_now("B"));
            assert!(operations.busy_now("C"));
            drop(bystander);
        });
    }

    /// A host-save failure restores its source lane even while another source
    /// asks its own question. It preserves the other lane's token and error;
    /// a successful save leaves only the answered lane closed.
    #[farhelm_testtrace::test]
    fn permanent_answer_failure_reopens_its_source_beside_another_question() {
        with_controllers(|sidebar, _| {
            assert!(sidebar.open_question(pending("A", "a1"), None));
            let question = sidebar.current_question("A").unwrap();
            assert!(sidebar.exclusions.peek().contains("A"));
            let answer = sidebar.answer_sidebar(&question, true, || true).unwrap();
            assert!(
                sidebar.current_question("A").is_none(),
                "sidebar is down while save waits"
            );
            assert!(matches!(
                sidebar.host_saved(answer.pending.clone(), Err("save failed".to_owned())),
                Err(ReplaceResult::QuestionHandled)
            ));
            assert_eq!(sidebar.error("A").as_deref(), Some("save failed"));
            let question = sidebar.current_question("A").unwrap();
            assert_eq!(question.key().source.id.raw(), "A");
            assert_eq!(question.key().key, "a1");
            assert_eq!(question.key().guard, DeleteGuard::NothingAlive);
            assert!(sidebar.exclusions.peek().contains("A"));
            let answer = sidebar.answer_sidebar(&question, true, || true).unwrap();
            assert!(sidebar.open_question(pending("C", "c1"), Some("newer failure".to_owned())));
            let newer = sidebar.current_question("C").unwrap();
            assert!(matches!(
                sidebar.host_saved(answer.pending, Err("late failure".to_owned())),
                Err(ReplaceResult::QuestionHandled)
            ));
            assert!(sidebar.current_question("C").as_ref() == Some(&newer));
            assert_eq!(sidebar.error("C").as_deref(), Some("newer failure"));
            assert_eq!(sidebar.error("A").as_deref(), Some("late failure"));
            assert!(sidebar.exclusions.peek().contains("C"));

            let question = sidebar.current_question("C").unwrap();
            let answer = sidebar.answer_sidebar(&question, true, || true).unwrap();
            assert!(sidebar.current_question("C").is_none());
            assert!(sidebar.host_saved(answer.pending, Ok(())).is_ok());
            assert!(sidebar.current_question("C").is_none());
            assert!(!sidebar.exclusions.peek().contains("C"));
            assert!(sidebar.current_question("A").is_some());
        });
    }

    /// An empty local slot is insufficient if another controller owns the
    /// failed source's question exclusion. The late save must return a visible
    /// refusal rather than claim a question reopened when admission rejected it.
    #[farhelm_testtrace::test]
    fn permanent_answer_failure_reports_a_refused_question_reopen() {
        with_controllers(|sidebar, other| {
            assert!(sidebar.open_question(pending("A", "a1"), None));
            let old = sidebar.current_question("A").unwrap();
            let answer = sidebar
                .answer_sidebar(&old, true, || true)
                .expect("premise: permanent answer consumed the old question");
            assert!(sidebar.current_question("A").is_none());
            assert!(other.open_question(pending("A", "a2"), Some("newer failure".to_owned())));
            let newer = other.current_question("A").unwrap();
            assert!(matches!(
                sidebar.host_saved(answer.pending, Err("late failure".to_owned())),
                Err(ReplaceResult::Refused(reason)) if reason == "late failure"
            ));
            assert!(sidebar.current_question("A").is_none());
            assert!(sidebar.error("A").is_none());
            assert!(other.current_question("A").as_ref() == Some(&newer));
            assert_eq!(other.error("A").as_deref(), Some("newer failure"));
            assert!(other.exclusions.peek().contains("A"));
        });
    }

    /// A duplicate refusal preserves the owning question and its failure text,
    /// but still returns a visible source outcome when no new lane can be opened.
    #[farhelm_testtrace::test]
    fn duplicate_yolo_refusal_preserves_the_existing_question() {
        with_controllers(|sidebar, header| {
            let refusal = || api::ActionRefusal {
                outcome: RefusalOutcome::NeedsYoloConfirmation,
                text: "refused".to_owned(),
                deadline_expired: false,
            };
            assert!(sidebar.open_question(pending("A", "a1"), Some("kept".to_owned())));
            assert!(matches!(
                header.refused(
                    source("A"),
                    DeleteGuard::NothingAlive,
                    "a2".to_owned(),
                    refusal(),
                    None,
                ),
                ReplaceResult::Refused(text) if text == "refused"
            ));
            assert!(matches!(
                sidebar.refused(
                    source("A"),
                    DeleteGuard::NothingAlive,
                    "a2".to_owned(),
                    refusal(),
                    None,
                ),
                ReplaceResult::Refused(text) if text == "refused"
            ));
            assert_eq!(sidebar.error("A").as_deref(), Some("kept"));
            assert!(header.current_question("A").is_none());
        });
    }

    /// A pending Replace has one clock across surfaces and no status after its
    /// future is dropped. Unrelated rows retain their own request meanwhile.
    #[farhelm_testtrace::test]
    fn replace_progress_is_shared_and_cleared_by_request_ownership() {
        with_controllers(|sidebar, header| {
            assert!(sidebar.progress("A").is_none());
            let first = sidebar.start_progress(&source("A"));
            assert!(header.progress("A") == Some(first.pending.clone()));
            let other = header.start_progress(&source("B"));
            drop(first);
            assert!(header.progress("A").is_none());
            assert!(sidebar.progress("B") == Some(other.pending.clone()));
            drop(other);
            assert!(sidebar.progress("B").is_none());
        });
    }

    /// A timeout changes the explanation but cannot retire an unresolved
    /// replacement key. The same outcome on a real refusal keeps its own text.
    #[farhelm_testtrace::test]
    fn replace_timeout_preserves_retry_without_relabeling_real_refusals() {
        with_controllers(|sidebar, header| {
            sidebar.prepare(source("A"), DeleteGuard::NothingAlive, None);
            sidebar.publish_key("A", "same-request");
            let result = header.refused(
                source("A"),
                DeleteGuard::NothingAlive,
                "same-request".to_string(),
                api::ActionRefusal {
                    outcome: RefusalOutcome::Unresolved,
                    text: "request timed out".into(),
                    deadline_expired: true,
                },
                None,
            );
            assert!(
                matches!(result, ReplaceResult::Refused(text) if text.contains("Still waiting on build box"))
            );
            assert_eq!(
                sidebar
                    .prepare(source("A"), DeleteGuard::NothingAlive, None)
                    .retained,
                Some("same-request".to_string())
            );
            let result = header.refused(
                source("A"),
                DeleteGuard::NothingAlive,
                "same-request".to_string(),
                api::ActionRefusal {
                    outcome: RefusalOutcome::Unresolved,
                    text: "host refused the replacement".into(),
                    deadline_expired: false,
                },
                None,
            );
            assert!(
                matches!(result, ReplaceResult::Refused(text) if text == "host refused the replacement")
            );
        });
    }

    /// Independent sidebar operations can supersede the one retained identity.
    /// A late timeout must not promise deduplication after another source took
    /// that slot, and it must leave the newer source's retry untouched.
    #[farhelm_testtrace::test]
    fn a_superseded_replace_timeout_does_not_promise_a_safe_retry() {
        with_controllers(|sidebar, header| {
            sidebar.prepare(source("A"), DeleteGuard::NothingAlive, None);
            sidebar.publish_key("A", "old-request");
            header.prepare(source("B"), DeleteGuard::NothingAlive, None);
            header.publish_key("B", "new-request");
            let result = sidebar.refused(
                source("A"),
                DeleteGuard::NothingAlive,
                "old-request".into(),
                api::ActionRefusal {
                    outcome: RefusalOutcome::Unresolved,
                    text: "deadline".into(),
                    deadline_expired: true,
                },
                None,
            );
            let ReplaceResult::Refused(text) = result else {
                panic!("a timeout needs an ordinary unknown-outcome explanation");
            };
            assert!(text.contains("Still waiting on build box"));
            assert!(text.contains("before replacing again"));
            assert!(!text.contains("second copy"));
            assert_eq!(
                header
                    .prepare(source("B"), DeleteGuard::NothingAlive, None)
                    .retained,
                Some("new-request".into())
            );
        });
    }
}
