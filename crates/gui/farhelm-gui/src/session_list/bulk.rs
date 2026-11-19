//! Bulk Stop and Delete of the marked sessions shown ([`super::marking`]), the question they ask, and the line any
//! bulk action leaves on a row it skipped.
//!
//! SPEC.md, "Session list" (the batch and bulk question paragraphs) and "Lifecycle operations" (Skip session
//! confirmations). Ported from the old app's `list/lifecycle.rs` (`admit_batch`, `run_paced`, `DeleteSnapshot`,
//! `DialogCopy`, `row_consequence`, `reconcile`, `DeleteDialog`) and its integration in `list/view.rs` (`bulk_run`,
//! `bulk_lifecycle`, the confirm and cancel callbacks, the question's reconciling on each listing) at reference commit
//! `24bc0580`. Here's the TLDR of the rules:
//!
//! - **Targets are frozen as drawn.** The bar's Stop or Delete takes the marked rows the last frame showed, each frozen
//!   as a [`Target`]: what the question draws of it and the precondition its request carries, from the row the store
//!   holds at the press, the same row the session's own Stop or Delete reads. The request a target gets is the one its
//!   row would send ([`super::stop`], [`super::delete`]), so a session that became more alive since is refused rather
//!   than killed.
//! - **A retired containerized session is never a Stop target** ([`super::retired`]): it supports Delete only, so the
//!   bar's Stop leaves it out with no line said and its mark left alone (it was never the batch's), and the bar draws
//!   no Stop while every mark shown is one ([`super::marking`]). The bar's Delete takes it like any other mark. The old
//!   app's bar did the same.
//! - **Ask once, or not at all.** The question opens when any target would ask on its own row (a Stop with terminal
//!   tabs, a Delete of anything alive), unless Skip session confirmations is on ([`super::menu::skips_questions`]);
//!   an environment Delete always asks ("Environment sessions" below). Otherwise the batch starts at the press.
//!   Opening the question counts as a marking choice.
//! - **The question's answer authorizes only what it drew.** One row per target (environment sessions: below) with its
//!   status, title, full host, what the action does, and for a Delete the managed checkout's line. Confirming runs the
//!   frozen targets still shown and still marked, never one marked since. While the question is open, a listing that
//!   changes anything a target's row draws or its request carries (its status words too) rewords it in place
//!   ([`prompt::Open::reword`]), as the old question replaced itself: it stays open, focus stays where it was, and
//!   until the new words are on screen every answer but Cancel does nothing. A listing that proves a target gone removes it, and one that only leaves it
//!   out (filtered, cut, failed) does not. A question with no target left closes. Cancel, Close and Escape keep the
//!   marks.
//! - **Focus.** Every close of the question returns focus to the marked bar's ⋯, as the old question's Cancel did (to
//!   the bar's pressed Stop or Delete while it draws no ⋯), and Confirm then sends it to "N marked", which stays while
//!   the batch runs. A question that closes because every target is proven gone closes before the frame that drops
//!   the bar, so focus goes to the ⋯ still on screen, and once the ⋯ goes the focus coordinator's recovery sends it to
//!   the open session's terminal (SPEC.md, "Where focus rests"), whatever held focus when the question opened; the old
//!   app focused the order's title. The question's text is a keyboard scroll stop named "Sessions affected", the old body's region,
//!   so a keyboard user can bring every row into view before answering for it.
//! - **A batch paces itself per host.** Every target is admitted at acceptance: one whose commands are refused now
//!   ([`super::menu::refused`]) gets its not-started line, and every other one is held in the operation store as its
//!   Stop or Delete ([`crate::operations::Operations::hold`]), so the row shows that work pending and its own menu, the
//!   header and the shortcuts refuse it until it finishes, as the old app's reservation at acceptance did. Each host
//!   sends one request at a time, its next row's hold giving way to the request when the last one settles; hosts
//!   progress independently, and a second batch has queues of its own. Filtering, a failed listing, Clear and new marks
//!   change nothing a batch does. A refusal stays on its own row and the next row goes on. The queues belong to the UI
//!   session, and a reset drops them with the store and its holds.
//! - **A finished batch clears its own targets** (refused ones included), only if no marking choice was made since it
//!   was accepted ([`super::marking::Marks::revision`]: a gesture, Clear, the marked menu or a bulk question opening);
//!   otherwise it leaves every mark alone. When a focused control disappears with the bar, the focus coordinator
//!   recovers to the open session's terminal or the window's other resting place.
//! - **A bulk Delete holds the automatic selection back** until every admitted Delete has settled, queued ones and
//!   other hosts' included: a queued Delete is held in the operation store as a Delete, which the list's settle
//!   already counts as one in flight, so it never opens a session still waiting to be deleted. A Stop batch holds
//!   nothing back.
//!
//! **The not-started line.** A bulk action that skips a row leaves "<action>: not started, " and the operation store's
//! shared reason on the row ([`not_started`]), in the store's slot for the session, so the row, its menu, and the open
//! session's header and card draw it as any other line of the session's ([`super::outcome`]), and the session's next
//! operation clears it.
//!
//! **Environment sessions** (a session in its own Tensorlake sandbox or Podman container,
//! [`super::environment_delete`]). Ported from the old `list/lifecycle.rs` (`sends_delete`, `delete_requests`,
//! `question_targets`, `DialogCopy`, `row_consequence`) and `list/marking.rs` (`LifecycleOffer`) at the reference
//! commit:
//!
//! - **Each gets the Delete its lane allows**, picked from the hosts read at the press
//!   ([`super::environment_delete::lane`]) and frozen with the question, every reword included ([`Target`]'s
//!   `environment`). A marked session of an environment the helm can ask gets its own session-lane Delete; that
//!   environment's Farhelm takes them one at a time (they share its host's queue), and the last one seals it. The marked
//!   sessions of an environment it cannot ask share one whole-environment Delete, the first one's, since that destroys
//!   the rest with it: they send nothing of their own, the question draws only that first row, and the batch still
//!   clears their marks.
//! - **Always asked.** A batch holding an environment Delete asks whatever Skip session confirmations says, as the
//!   single environment Delete does. Each such row states its lane's consequence in the provider's words. With a
//!   whole-environment row the title, answer and help become "Delete marked sessions and whole environments?", "Delete
//!   sessions and environments" and "Delete each listed session, and each environment marked as deleted whole with
//!   everything in it"; with only session-lane rows the help says "an environment goes too with its last session". A
//!   bulk Stop's row for an environment session adds what plain Stop leaves running there (the provider's Stop help).
//! - **Settled as the single Delete settles**, with no way to ask again: a 404 counts as deleted, an "unasked" answer
//!   leaves the helm's words on the row and opens nothing, and each success hands its "stays" count or its retirement
//!   to the Delete notice band in the order the replies arrive ([`super::delete_notice::publish_environment`]).
//! - **A 504 stops the environment's queue.** The helm's 504 at its bound settles that row quietly, its Delete still
//!   running, and every row still queued for that host is not sent but left [`AFTER_CARRIES_ON`]'s line, so nothing
//!   goes into an environment whose Farhelm is still busy (decided by the GPUI migration's orchestrator on 2026-11-16;
//!   the old app waited up to nine minutes for each reply instead). This departs from SPEC.md, "Sandbox sessions" ("A
//!   refusal or a missing answer partway through leaves that row with its reason"): the row that met the 504 shows no
//!   reason, and SPEC.md does not yet say that the rest are not sent. Its wording is to be revised to match the
//!   decision.
//! - **Being deleted.** A session whose environment is being deleted is left out of the question and the batch, with
//!   the reason on its row ([`not_started`]), and the bar's Stop and Delete are drawn unavailable while every mark each
//!   would act on is such a session ([`all_being_deleted`]).

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{DeleteGuard, SessionStopRequest};
use gpui::{
    AnyWindowHandle, App, AppContext as _, Context, ElementId, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::environment::Environment;
use super::environment_delete::{self, Lane};
use super::row_marks::{self, Locality};
use super::status::{self, EndedWords, Live};
use super::{SessionList, delete, menu, outcome, retired, stop};
use crate::components::{HoverHelp, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText, Line};
use crate::hosts::identity;
use crate::layers;
use crate::operations::{NotStarted, OpKey, Work};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::theme::{self, metrics};

// ---- The not-started line --------------------------------------------------------------------------------------------

/// Leave `key`'s row the line saying a bulk action skipped it: "not started, " and the operation store's shared reason
/// ([`NotStarted::BUSY_REASON`]), recorded as `work`, the work the action would have sent (a pin, a seen toggle, a
/// Stop, a Delete), so the row's line leads with that work's ordinary name ([`super::outcome`]). A work whose line
/// shows the helm's words alone (a Delete's) gets `label` as its name instead. Nothing is recorded while the window is
/// inert.
///
/// A session whose environment is being deleted was skipped for that, not for another operation, so its line gives
/// that reason alone after the name ([`super::environment::deleting_reason`]), as the old batch's "Delete: <reason>"
/// did: "another operation was in progress. Try again." would be false there, and trying again cannot help.
///
/// The line is recorded even while the request that refused the action is still in flight, which
/// [`crate::operations::Operations::note`] otherwise warns against: here outliving that request is the point, since the
/// bulk action still did not happen once the request settles, and a refusal it settles with replaces the line, as a
/// later error replaced the old app's row error.
pub(super) fn not_started(key: &str, work: Work, label: &str, cx: &mut App) {
    let reason = match super::environment::deleting_reason(key, cx) {
        Some(reason) => reason.to_owned(),
        None => format!("not started, {}", NotStarted::BUSY_REASON),
    };
    leave_line(key, work, label, &reason, cx);
}

/// Why a queued environment Delete was not sent after an earlier one into the same environment met the helm's 504
/// (the module docs' "Environment sessions"). The app's own words for the rule the GPUI migration's orchestrator
/// decided on 2026-11-16; the old app waited instead, so it had none to port. They name no provider, as the helm's 504
/// does not.
pub(super) const AFTER_CARRIES_ON: &str = "not started, an earlier Delete in this environment was still running. Try \
                                           again once the list shows how that Delete ended.";

/// Leave `key`'s row `reason` after the name of `work` ([`not_started`] has the rules), recorded as `work`.
fn leave_line(key: &str, work: Work, label: &str, reason: &str, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let lead = if outcome::prefix(work).is_none() {
        format!("{label}: ")
    } else {
        String::new()
    };
    let words = HostText::from_host(format!("{lead}{reason}"));
    operations.update(cx, |operations, cx| {
        operations.note(&OpKey::session(key), work, words, cx)
    });
}

// ---- What a target is ------------------------------------------------------------------------------------------------

/// Which bulk lifecycle action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Lifecycle {
    Stop,
    Delete,
}

impl Lifecycle {
    /// Its name: the marked bar's button, and the verb of the question's title and Confirm.
    pub(super) fn label(self) -> &'static str {
        match self {
            Lifecycle::Stop => "Stop",
            Lifecycle::Delete => "Delete",
        }
    }
}

/// One target as a bulk Stop or Delete drew it: what its row in the question shows and what its request carries.
///
/// Compared whole to decide a reword: the open question rewords when the same session frozen afresh differs in
/// anything, which covers every field its row draws (status words included, as the old question compared) and the
/// precondition its answer authorizes. Whether its own row would ask follows from that precondition, so it never
/// changes alone; the one field that can differ while every drawn word stays the same is the host's registry id, which
/// its row does not draw, and a reword then shows the same words, which costs nothing, since it is in place.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Target {
    pub(super) key: String,
    /// The registry id of the session's host: one request at a time goes to each.
    host: i64,
    /// The session's title, escaped from the row as read.
    title: HostText,
    /// The host's full name as the helm names it (`Local (this machine)` for the local host), `None` when it sent none.
    host_name: Option<HostText>,
    /// What the row's status said when the target was frozen.
    status: Status,
    /// What the action does to the session: the row question's own consequence.
    consequence: String,
    /// Whether a Delete row adds the managed checkout's line.
    managed: bool,
    /// The precondition the request carries, frozen from the same row.
    guard: Guard,
    /// Whether this target's own row would ask before acting, Skip session confirmations aside.
    asks: bool,
    /// For a Delete of a session in a disposable environment, the environment Delete it gets, with its lane as the
    /// hosts read at the press picked it (the module docs' "Environment sessions"); `None` for an ordinary session and
    /// for every Stop. Kept through every reword, so the answer sends the lane its row said.
    environment: Option<environment_delete::Asked>,
}

/// What a target's status says in the question: a live word, an ended status's words, or "Unknown" for one nobody
/// classified (the question names an uncertainty a row leaves undrawn).
#[derive(Clone, Debug, PartialEq)]
enum Status {
    Live(Live),
    Ended(EndedWords),
    Unknown,
}

/// The precondition a target's request carries: Stop's two refusals or Delete's guard (see [`super::stop::guard`] and
/// [`super::delete::guard`]).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Guard {
    Stop(SessionStopRequest),
    Delete(DeleteGuard),
}

impl Target {
    /// `row`, filed under `key`, as a target of `action`, with its host placed through `collections`' registry. A
    /// Delete of an environment session takes its lane from `collections`' hosts read now
    /// ([`environment_delete::asked_in`]).
    pub(super) fn of(
        action: Lifecycle,
        key: &str,
        row: &SessionRow,
        collections: &Collections,
    ) -> Target {
        let environment = match action {
            Lifecycle::Stop => None,
            Lifecycle::Delete => environment_delete::asked_in(row, collections),
        };
        Target::frozen_as(action, key, row, collections, environment)
    }

    /// `row` as a target of `action` whose environment Delete, if any, is `environment` as already frozen: what a
    /// reword freezes afresh, so a hosts read that changed since the press never changes the lane a row was asked in,
    /// nor makes an ordinary row an environment one (the old `reconcile` kept its `sandbox` and `whole` the same way).
    fn frozen_as(
        action: Lifecycle,
        key: &str,
        row: &SessionRow,
        collections: &Collections,
        environment: Option<environment_delete::Asked>,
    ) -> Target {
        let local = row_marks::locality(row, collections) == Some(Locality::Local);
        let status = match (status::live(row), status::ended_words(row, false)) {
            (Some(live), _) => Status::Live(live),
            (None, Some(ended)) => Status::Ended(ended),
            (None, None) => Status::Unknown,
        };
        let (consequence, guard, asks) = match (action, &environment) {
            // An environment session's Stop also says what plain Stop leaves running there, as the old bulk row added
            // the provider's Stop help (not the row question's Stop and suspend note: the batch offers no such answer).
            (Lifecycle::Stop, _) => (
                match Environment::of(row, collections) {
                    Some(environment) => {
                        format!("{} {}", stop::consequence(row), environment.ui.stop_help())
                    }
                    None => stop::consequence(row),
                },
                Guard::Stop(stop::guard(row)),
                stop::asks(row, false),
            ),
            // An environment Delete states its lane's consequence and always asks; its request carries no guard, so the
            // one kept here only fills the shape the question's answer hands back.
            (Lifecycle::Delete, Some(asked)) => (
                asked.lane_consequence().to_owned(),
                Guard::Delete(delete::guard(row)),
                true,
            ),
            (Lifecycle::Delete, None) => (
                delete::consequence(row),
                Guard::Delete(delete::guard(row)),
                !delete::shows_nothing_alive(row),
            ),
        };
        Target {
            key: key.to_owned(),
            host: row.host,
            title: HostText::from_host(row.info.title.as_str()),
            host_name: (!row.host_name.is_empty()).then(|| identity::named(&row.host_name, local)),
            status,
            consequence,
            // An environment's checkouts go with it or are discarded, so its rows have no managed-checkout line.
            managed: action == Lifecycle::Delete
                && environment.is_none()
                && row.info.working_copy.is_some(),
            guard,
            asks,
            environment,
        }
    }

    /// Whether this target gets the whole-environment Delete.
    fn whole(&self) -> bool {
        self.environment
            .as_ref()
            .is_some_and(|asked| asked.lane == Lane::Whole)
    }

    /// The work this target's request is, which its row draws pending while it is held and in flight.
    fn work(&self) -> Work {
        match (self.guard, &self.environment) {
            (Guard::Stop(_), _) => Work::Stop,
            (Guard::Delete(_), Some(_)) => Work::EnvironmentDelete,
            (Guard::Delete(guard), None) => Work::Delete(guard),
        }
    }

    /// The name its not-started line leads with ([`not_started`]).
    fn label(&self) -> &'static str {
        match self.guard {
            Guard::Stop(_) => Lifecycle::Stop.label(),
            Guard::Delete(_) => Lifecycle::Delete.label(),
        }
    }

    /// Send this target's request through its row's own send, running `then` once it has settled, told whether the
    /// helm answered an environment Delete with the 504 that says it carries on (never for any other request).
    fn start(
        &self,
        then: impl FnOnce(bool, &mut App) + 'static,
        cx: &mut App,
    ) -> Result<(), NotStarted> {
        match (self.guard, &self.environment) {
            (Guard::Stop(guard), _) => stop::start(&self.key, guard, |cx| then(false, cx), cx),
            // No way to ask again: in a batch an "unasked" answer leaves the helm's words on the row.
            (Guard::Delete(guard), Some(asked)) => environment_delete::start(
                delete::Frozen {
                    key: self.key.clone(),
                    guard,
                    environment: Some(asked.clone()),
                },
                None,
                then,
                cx,
            ),
            (Guard::Delete(guard), None) => {
                delete::start(&self.key, guard, |cx| then(false, cx), cx)
            }
        }
    }

    /// Hold this target's row in the operation store as its work, pending until its turn ([`Self::release`]).
    fn hold(&self, cx: &mut App) -> Result<(), NotStarted> {
        let Some(operations) = session::current_operations(cx) else {
            return Err(NotStarted::Retired);
        };
        let work = self.work();
        operations.update(cx, |operations, cx| {
            operations.hold(&OpKey::session(&self.key), work, cx)
        })
    }

    /// End this target's hold at its turn, right before its request starts in the hold's place. (Only a reset ends a
    /// hold otherwise, with the store.)
    fn release(&self, cx: &mut App) {
        if let Some(operations) = session::current_operations(cx) {
            operations.update(cx, |operations, cx| {
                operations.release(&OpKey::session(&self.key), cx)
            });
        }
    }
}

// ---- What the list keeps ---------------------------------------------------------------------------------------------

/// The bulk work of one UI session: the accepted batches and the open bulk question. Kept in the list's per-UI-session
/// state, so a reset drops the queues as it drops the operation store and the holds in it, and no completion is left
/// waiting on either.
#[derive(Default)]
pub(super) struct Bulk {
    batches: HashMap<u64, Batch>,
    /// The next batch's id: completions find their batch by it, and one that finished is never found again.
    next: u64,
    question: Option<Asked>,
}

/// One accepted batch.
struct Batch {
    /// [`super::marking::Marks::revision`] when the batch was accepted: a later choice keeps the batch from clearing.
    revision: u64,
    /// Every target the batch was accepted with, refused ones included: what it clears when it finishes.
    targets: Vec<String>,
    /// Each host still busy with the batch, with its rows not sent yet, each held in the operation store. A host stays
    /// while its last request is in flight, with nothing left queued, and goes when that request settles.
    queues: HashMap<i64, VecDeque<Target>>,
}

/// A bulk question's targets as frozen when it was asked: one shared value, which the list keeps to reconcile against,
/// the prompt hands back to Confirm, and the question's body draws, so the three cannot drift apart.
type Targets = Rc<[Target]>;

/// The open bulk question, as the list checks it with the layer manager and rewords it.
struct Asked {
    /// The prompt's handle: its layer for the layer manager, and the reword in place.
    open: prompt::Open<Targets>,
    action: Lifecycle,
    /// The question's frozen targets as last asked or reworded, shared with the prompt's value and its body
    /// ([`Targets`]). The prompt gives no way to read its value back, so the list keeps this handle to compare each
    /// listing against.
    targets: Targets,
    /// The window it is open in, which a reconciling settle (an observer, with no window) reaches it through.
    window: AnyWindowHandle,
}

/// What `asked`'s targets are after a listing that `collections` now holds: each one the store holds, frozen afresh
/// from its row with the environment Delete it was asked with ([`Target::frozen_as`]); each one a listing proves gone,
/// left out; and each one only missing (a filtered, cut or failed listing), kept as drawn. Never one more.
fn reconciled(asked: &Asked, collections: &Collections) -> Vec<Target> {
    asked
        .targets
        .iter()
        .filter_map(|drawn| match collections.get::<SessionRow>(&drawn.key) {
            Some(row) => Some(Target::frozen_as(
                asked.action,
                &drawn.key,
                row,
                collections,
                drawn.environment.clone(),
            )),
            None if session::listing_proves_gone(collections, &drawn.key) => None,
            None => Some(drawn.clone()),
        })
        .collect()
}

// ---- Starting ------------------------------------------------------------------------------------------------------

/// The marked bar's Stop's element id.
pub(super) const STOP_ID: &str = "session-marked-stop";

/// The marked bar's Delete's element id.
pub(super) const DELETE_ID: &str = "session-marked-delete";

/// The marked bar's Stop's or Delete's hover help while it is drawn unavailable ([`all_being_deleted`]): the old
/// `ALL_BEING_DELETED`, word for word.
pub(super) const ALL_BEING_DELETED: &str =
    "The marked sessions' environments are being deleted, so their actions are unavailable";

/// Whether the marked bar's `action` would act on nothing of `shown`, the marks the last frame showed, because every
/// one it would take is a session whose environment is being deleted, which takes no action: the bar then draws it
/// unavailable (the old `LifecycleOffer`). Stop never takes a retired session ([`super::retired`]), which is never an
/// environment session either, so Stop counts only the others; a mark with no row in the store is no target of either.
/// `false` when there is nothing to count.
pub(super) fn all_being_deleted(action: Lifecycle, shown: &[String], cx: &App) -> bool {
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    let collections = store.read(cx).collections();
    let mut takes = shown
        .iter()
        .filter(|key| {
            collections
                .get::<SessionRow>(key.as_str())
                .is_some_and(|row| action == Lifecycle::Delete || !retired::is(row))
        })
        .peekable();
    takes.peek().is_some()
        && takes.all(|key| super::environment::deleting_reason(key, cx).is_some())
}

impl SessionList {
    /// The marked bar's Stop or Delete was pressed: freeze the marked rows the last frame showed as targets of
    /// `action`, then ask about them, or, when no target would ask on its own row or Skip session confirmations is on,
    /// run them at once; a batch holding an environment Delete always asks. Nothing happens while a bulk question is
    /// open (a second press queued before the question took the window's input) or when no shown mark has a row in the
    /// store.
    pub(super) fn bulk_lifecycle(
        &mut self,
        action: Lifecycle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mark = Mark::take(cx);
        if self.bulk_question_open(window, cx) {
            return;
        }
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let shown = self.marking.shown();
        let (targets, skips) = {
            let collections = store.read(cx).collections();
            let targets: Vec<Target> = shown
                .iter()
                .filter_map(|key| {
                    let row = collections.get::<SessionRow>(key)?;
                    // A retired session supports Delete only: never a Stop target, and nothing said about it.
                    if action == Lifecycle::Stop && retired::is(row) {
                        return None;
                    }
                    Some(Target::of(action, key, row, collections))
                })
                .collect();
            (targets, menu::skips_questions(collections))
        };
        // A session whose environment is being deleted takes no action, so it is left out of the question and the
        // batch, its line said at once, as the old batch's `exclude_unavailable` did when the batch was drawn; never
        // the batch's, it keeps its mark when the batch finishes. Admission checks again (`run_batch`), for one whose
        // environment started being deleted while the question was open.
        let (unavailable, targets): (Vec<Target>, Vec<Target>) = targets
            .into_iter()
            .partition(|target| super::environment::deleting_reason(&target.key, cx).is_some());
        for target in &unavailable {
            not_started(&target.key, target.work(), target.label(), cx);
        }
        if targets.is_empty() {
            return;
        }
        // An environment Delete is never skipped, so a batch holding one always asks (the old `needs_confirmation`).
        let must_ask = targets.iter().any(|target| target.environment.is_some());
        if !must_ask && (skips || !targets.iter().any(|target| target.asks)) {
            self.run_batch(targets, window, cx);
            return;
        }
        self.ask_bulk(action, targets.into(), &mark, window, cx);
        // The question is a choice of its own: a batch accepted earlier must not clear the marks it asks about.
        let current = self.current.current(cx);
        if current.bulk.question.is_some() {
            current.marks.touch();
        }
    }

    /// Ask the bulk question about `targets` under `mark`, the press of the bar's button. Every close returns focus to
    /// the ⋯, as the old question's Cancel did. While the bar draws no ⋯ (every mark shown is a retired session, which
    /// no metadata action takes), it returns to the bar's own button for `action` instead, where the old app fell back
    /// to the order's title, which the bar here replaces and so is not drawn. A close whose ⋯ goes right after it (every
    /// target proven gone) leaves focus to the coordinator's recovery, which sends it to the open session's terminal
    /// (the module docs, "Focus").
    fn ask_bulk(
        &mut self,
        action: Lifecycle,
        targets: Targets,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let returns_to = self
            .marking
            .menu_toggle_drawn()
            .unwrap_or_else(|| self.marking.button_focus(action))
            .clone();
        let list = cx.weak_entity();
        let open = prompt::ask_rewordable(
            question(action, targets.clone()).returns_to(returns_to),
            None,
            mark,
            move |targets, window, cx| {
                let _ = list.update(cx, |list, cx| list.confirm_bulk(targets, window, cx));
            },
            window,
            cx,
        );
        self.current.current(cx).bulk.question = open.map(|open| Asked {
            open,
            action,
            targets,
            window: window.window_handle(),
        });
    }

    /// The bulk question is open: checked with the layer manager, so a question closed some other way (Cancel, Escape)
    /// is forgotten here.
    fn bulk_question_open(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let bulk = &mut self.current.current(cx).bulk;
        if bulk
            .question
            .as_ref()
            .is_some_and(|asked| !layers::is_open(asked.open.layer(), window, cx))
        {
            bulk.question = None;
        }
        bulk.question.is_some()
    }

    /// The bulk question about `targets` was confirmed: move focus to "N marked", then run the targets still shown and
    /// still marked (an answer may lose a target, never gain one).
    fn confirm_bulk(&mut self, targets: Targets, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.marking.shown();
        let current = self.current.current(cx);
        current.bulk.question = None;
        let marks = &current.marks;
        let admitted: Vec<Target> = targets
            .iter()
            .filter(|target| marks.contains(&target.key) && shown.contains(&target.key))
            .cloned()
            .collect();
        // Before the batch runs: a batch that finishes at once hands focus on from the count as the bar goes.
        let count = self.marking.count_focus().clone();
        focus::focus_control(&count, &Mark::take(cx), window, cx);
        self.run_batch(admitted, window, cx);
    }

    // ---- Running -----------------------------------------------------------------------------------------------------

    /// Accept `targets` as one batch: give each refused target its not-started line, hold the rest in the operation
    /// store and queue them by host, and send each host's first. Of the targets that get one environment's
    /// whole-environment Delete, only the first held is queued; the rest send nothing of their own, since that one
    /// Delete destroys them with everything else in it, and the batch still clears their marks.
    fn run_batch(&mut self, targets: Vec<Target>, window: &mut Window, cx: &mut Context<Self>) {
        let mut hosts: Vec<i64> = Vec::new();
        let mut queues: HashMap<i64, VecDeque<Target>> = HashMap::new();
        let mut wholes: HashSet<i64> = HashSet::new();
        let keys = targets.iter().map(|target| target.key.clone()).collect();
        for target in targets {
            if target.whole() && wholes.contains(&target.host) {
                continue;
            }
            let held = !menu::refused(&target.key, cx) && target.hold(cx).is_ok();
            if !held {
                not_started(&target.key, target.work(), target.label(), cx);
                continue;
            }
            if target.whole() {
                wholes.insert(target.host);
            }
            if !queues.contains_key(&target.host) {
                hosts.push(target.host);
            }
            queues.entry(target.host).or_default().push_back(target);
        }
        let current = self.current.current(cx);
        let id = current.bulk.next;
        current.bulk.next += 1;
        current.bulk.batches.insert(
            id,
            Batch {
                revision: current.marks.revision(),
                targets: keys,
                queues,
            },
        );
        if hosts.is_empty() {
            self.finish_batch(id, cx);
        }
        for host in hosts {
            self.advance(id, host, window, cx);
        }
    }

    /// Send batch `id`'s next row on `host`: end its hold and send its request in the hold's place, or, for one refused
    /// by now, give it its not-started line and go on, until one is sent or the host has none left; the sent one's
    /// settling calls this again. The batch finishes once no host is left.
    fn advance(&mut self, id: u64, host: i64, window: &mut Window, cx: &mut Context<Self>) {
        loop {
            let Some(batch) = self.current.current(cx).bulk.batches.get_mut(&id) else {
                return;
            };
            let Some(queue) = batch.queues.get_mut(&host) else {
                return;
            };
            let Some(target) = queue.pop_front() else {
                batch.queues.remove(&host);
                if batch.queues.is_empty() {
                    self.finish_batch(id, cx);
                }
                return;
            };
            // The release and the start below (or the not-started line) happen in this one update, which no frame
            // and no observer runs inside, so nothing ever sees the row free between its hold and its request.
            target.release(cx);
            // The hold refused everything else on the row, so this finds the row as admission left it; checked all the
            // same, since a refusal here must leave a line rather than a request.
            if menu::refused(&target.key, cx) {
                not_started(&target.key, target.work(), target.label(), cx);
                continue;
            }
            let (list, handle) = (cx.weak_entity(), window.window_handle());
            let next = move |carries_on: bool, cx: &mut App| {
                // A window that has gone takes its queues with it.
                let _ = handle.update(cx, |_, window, cx| {
                    list.update(cx, |list, cx| {
                        if carries_on {
                            list.give_up_on_host(id, host, cx);
                        }
                        list.advance(id, host, window, cx)
                    })
                });
            };
            match target.start(next, cx) {
                Ok(()) => return,
                Err(NotStarted::Busy(_)) => {
                    not_started(&target.key, target.work(), target.label(), cx)
                }
                // Inert, or a key no helm serves (logged where it was built): nothing to show on a row.
                Err(NotStarted::Retired | NotStarted::Path(_)) => {}
            }
        }
    }

    /// Batch `id`'s request on `host` met the helm's 504 at its bound, an environment Delete still running there: end
    /// the hold of every row the host still has queued and leave each the line that says why it was not sent
    /// ([`AFTER_CARRIES_ON`]), so nothing more goes into an environment whose Farhelm is still busy with the first
    /// (decided by the GPUI migration's orchestrator on 2026-11-16, in place of the old app's waiting). The host's queue
    /// is left empty, for [`Self::advance`] to finish.
    fn give_up_on_host(&mut self, id: u64, host: i64, cx: &mut Context<Self>) {
        let Some(queue) = self
            .current
            .current(cx)
            .bulk
            .batches
            .get_mut(&id)
            .and_then(|batch| batch.queues.get_mut(&host))
        else {
            return;
        };
        let queued = std::mem::take(queue);
        for target in queued {
            target.release(cx);
            leave_line(
                &target.key,
                target.work(),
                target.label(),
                AFTER_CARRIES_ON,
                cx,
            );
        }
    }

    /// Batch `id` has nothing left in flight or queued: forget it, clear its own targets unless a marking choice came
    /// since it was accepted, and settle, since its Deletes may have been what held the automatic selection back.
    fn finish_batch(&mut self, id: u64, cx: &mut Context<Self>) {
        let current = self.current.current(cx);
        let Some(batch) = current.bulk.batches.remove(&id) else {
            return;
        };
        if current.marks.revision() == batch.revision {
            current
                .marks
                .prune(|key| batch.targets.iter().any(|target| target == key));
            cx.notify();
        }
        self.settle(cx);
    }

    // ---- Rewording -----------------------------------------------------------------------------------------------------

    /// After a change of the store, see whether the open bulk question still reads as drawn ([`reconciled`]), and if
    /// not, reword it once the observer has returned (a reword needs the window). Called from the list's settle, which
    /// every change of the store reaches.
    pub(super) fn reconcile_bulk_question(&mut self, cx: &mut Context<Self>) {
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let Some(asked) = &self.current.current(cx).bulk.question else {
            return;
        };
        if *asked.targets == *reconciled(asked, store.read(cx).collections()) {
            return;
        }
        let (list, window) = (cx.weak_entity(), asked.window);
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                list.update(cx, |list, cx| list.reword_bulk_question(window, cx))
            });
        });
    }

    /// Reword the open bulk question in place to the targets the store gives now, or close it when no target is left.
    /// Done afresh from the store as it is when it runs, so several changes before it ran reword once; a question
    /// closed meanwhile is only forgotten.
    ///
    /// The prompt refuses a reword once the question has closed, and while an answer that can fail has an attempt in
    /// flight. The bulk question has no such answer, so today a refusal means it closed; one refused while still open
    /// stays tracked with the targets it last drew, so the bar's Stop and Delete keep refusing to ask a second question
    /// over it, and a later listing tries the reword again.
    fn reword_bulk_question(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.bulk_question_open(window, cx) {
            return;
        }
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let Some(asked) = self.current.current(cx).bulk.question.take() else {
            return;
        };
        let targets = reconciled(&asked, store.read(cx).collections());
        if *asked.targets == *targets {
            self.current.current(cx).bulk.question = Some(asked);
            return;
        }
        if targets.is_empty() {
            layers::close(asked.open.layer(), window, cx);
            return;
        }
        let targets: Targets = targets.into();
        let reworded = asked
            .open
            .reword(question(asked.action, targets.clone()), window, cx);
        let kept = if reworded {
            Some(Asked { targets, ..asked })
        } else {
            layers::is_open(asked.open.layer(), window, cx).then_some(asked)
        };
        self.current.current(cx).bulk.question = kept;
    }
}

// ---- The question --------------------------------------------------------------------------------------------------

/// The bulk question's width: the old `.bulk-delete-dialog`'s 620 pixels, so a long host name or consequence wraps
/// less.
const QUESTION_WIDTH: f32 = 620.;

/// The bulk question's words besides its rows: the old `DialogCopy`.
struct Words {
    /// The question's title.
    title: String,
    /// The confirming answer's label.
    answer: String,
    /// The confirming answer's hover help.
    help: String,
}

/// The bulk question's [`Words`] for `action` on `targets`. A whole-environment Delete changes the title and the
/// answer, so the destructive scope is never only in the rows; environment Deletes that ask their Farhelm keep the
/// ordinary title, since each still deletes just its session unless it is the last, and say so in the help.
fn words(action: Lifecycle, targets: &[Target]) -> Words {
    if action == Lifecycle::Delete && targets.iter().any(Target::whole) {
        return Words {
            title: "Delete marked sessions and whole environments?".to_owned(),
            answer: "Delete sessions and environments".to_owned(),
            help: "Delete each listed session, and each environment marked as deleted whole with everything in it"
                .to_owned(),
        };
    }
    let verb = action.label();
    let noun = if targets.len() == 1 {
        "session"
    } else {
        "sessions"
    };
    let help = if targets.iter().any(|target| target.environment.is_some()) {
        "Delete the sessions listed in this question; an environment goes too with its last session"
            .to_owned()
    } else {
        format!("{verb} the sessions listed in this question")
    };
    Words {
        title: format!("{verb} {} {noun}?", targets.len()),
        answer: format!("{verb} {noun}"),
        help,
    }
}

/// The targets the question draws a row for, in order: every target but the second and later of one environment's
/// whole-environment Delete, which that first row's warning covers, as the old `question_targets` drew them. The
/// answer still carries every target, so the batch clears all their marks.
fn drawn(targets: &[Target]) -> Vec<&Target> {
    let mut wholes = HashSet::new();
    targets
        .iter()
        .filter(|target| !target.whole() || wholes.insert(target.host))
        .collect()
}

/// The bulk question about `targets`: "Stop 2 sessions?" or "Delete 1 session?" ([`words`]), with one row per drawn
/// target in its body ([`drawn`]), and the old dialog's answers and their help. Its answer acts on `targets` as frozen
/// here. A bulk Delete starts on Cancel if any target destroys an entire environment, since that can also destroy work
/// in unmarked sessions. Rewording keeps the open question's focus. The asker sets where closing returns focus
/// ([`SessionList`]'s `ask_bulk`).
fn question(action: Lifecycle, targets: Targets) -> Question<Targets> {
    let whole_delete = action == Lifecycle::Delete && targets.iter().any(Target::whole);
    let Words {
        title,
        answer,
        help,
    } = words(action, &targets);
    let question = Question::new(
        title,
        Answer::new(answer, HoverHelp::tooltip(help), Tier::Danger),
        targets,
    )
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip("Cancel: keep the marked sessions"),
        Tier::Neutral,
    ))
    .keeps_focus_on_empty_click()
    .width(px(QUESTION_WIDTH))
    .scroll_stop("Sessions affected")
    .body(move |targets: &Targets, _, cx| {
        let targets = targets.clone();
        cx.new(|_| Rows { action, targets })
    });
    if whole_delete {
        question.starts_on_cancel()
    } else {
        question
    }
}

/// The bulk question's body: one row per target, as the question froze them.
struct Rows {
    action: Lifecycle,
    targets: Targets,
}

impl Render for Rows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let rows = drawn(&self.targets);
        div()
            .flex()
            .flex_col()
            .children(rows.into_iter().enumerate().map(|(index, target)| {
                let key = &target.key;
                let id = |part: &str| SharedString::from(format!("bulk-target-{part}-{key}"));
                let status: Line = match &target.status {
                    Status::Live(live) => live.word().into(),
                    Status::Ended(ended) => ended.line(id("status")),
                    Status::Unknown => "Unknown".into(),
                };
                let host: Line = match &target.host_name {
                    Some(name) => HostSentence::new(id("host")).host(name).into(),
                    None => "Host identity unavailable".into(),
                };
                let selector = format!("bulk-target-{key}");
                // `.bulk-delete-row`: 14 pixels above and below, ruled off from the row before it; the identity line
                // (status, then the title wrapping in what is left), the host under it, then what the action does.
                div()
                    .id(ElementId::Name(selector.clone().into()))
                    .debug_selector(move || selector.clone())
                    .py(px(14.))
                    .when(index > 0, |row| {
                        row.border_t_1()
                            .border_color(theme::color(palette.border_dim))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(8.))
                            .min_w_0()
                            .child({
                                // The status cell's own id, for tests that read its words: the ended status's host
                                // sentence inside it already takes `bulk-target-status-<key>`.
                                let selector = format!("bulk-target-word-{key}");
                                div()
                                    .id(ElementId::Name(selector.clone().into()))
                                    .debug_selector(move || selector)
                                    .flex_none()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(status)
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .child(HostSentence::new(id("title")).host(&target.title)),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(4.))
                            .text_color(theme::color(palette.fg_2))
                            .child(host),
                    )
                    // A sentence of the app's own words, so a test can read a row's whole consequence by its id: an
                    // environment row's says which of its two Deletes it gets.
                    .child(div().mt(px(8.)).child(
                        HostSentence::new(id("consequence")).text(target.consequence.clone()),
                    ))
                    .when(self.action == Lifecycle::Delete && target.managed, |row| {
                        row.child(div().mt(px(8.)).child(delete::MANAGED_CHECKOUT))
                    })
                    .text_size(metrics::TEXT_MD)
            }))
    }
}
