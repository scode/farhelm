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
//!   tabs, a Delete of anything alive), unless Skip session confirmations is on ([`super::menu::skips_questions`]).
//!   Otherwise the batch starts at the press. Opening the question counts as a marking choice.
//! - **The question's answer authorizes only what it drew.** One row per target (status, title, full host, what the
//!   action does, and for a Delete the managed checkout's line). Confirming runs the frozen targets still shown and
//!   still marked, never one marked since. While the question is open, a listing that changes what a target's answer
//!   would authorize (its title, host, consequence or precondition) rewords it by closing it and asking the
//!   replacement, which starts on Cancel as a question that arrived later does; a listing that proves a target gone
//!   removes it, and one that only leaves it out (filtered, cut, failed) does not. A question with no target left
//!   closes. Cancel, Close and Escape keep the marks. The old question replaced itself in place on any change it drew,
//!   its status words included; here a reword moves focus (the prompt cannot replace a question in place), so only what
//!   the answer authorizes rewords it, and a status word may go stale while the question is open.
//! - **Focus.** Cancel returns focus to the marked bar's ⋯, as the old question did (to the bar's pressed Stop or
//!   Delete while it draws no ⋯), and Confirm sends it to "N marked", which stays while the batch runs. The bulk
//!   question's opener is that control for that reason: the prompt returns focus to what held it when it opened, so
//!   the bar moves focus there before asking. A reword asks under the mark the
//!   question first opened with, so once the user has pressed a key or clicked since (in the question, or in an
//!   approval card beside it), it leaves the question as drawn rather than move focus out from under them.
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
//!   otherwise it leaves every mark alone. When that empties the bar while focus was on it, focus goes to the order's
//!   title, as Clear's does.
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
//! NOTE: Sandbox and recipe container rows get nothing of their own yet: which Delete each gets, one whole-resource
//! Delete per resource, continued billing for Stop and a batch with an environment Delete always asking come with the
//! environments work.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::SessionRow;
use farhelm_proto::{DeleteGuard, SessionStopRequest};
use gpui::{
    AnyWindowHandle, App, AppContext as _, Context, ElementId, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::row_marks::{self, Locality};
use super::status::{self, EndedWords, Live};
use super::{SessionList, delete, menu, outcome, retired, stop};
use crate::components::{HoverHelp, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText, Line};
use crate::hosts::identity;
use crate::layers::{self, LayerId};
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
/// The line is recorded even while the request that refused the action is still in flight, which
/// [`crate::operations::Operations::note`] otherwise warns against: here outliving that request is the point, since the
/// bulk action still did not happen once the request settles, and a refusal it settles with replaces the line, as a
/// later error replaced the old app's row error.
pub(super) fn not_started(key: &str, work: Work, label: &str, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let lead = if outcome::prefix(work).is_none() {
        format!("{label}: ")
    } else {
        String::new()
    };
    let words = HostText::from_host(format!("{lead}not started, {}", NotStarted::BUSY_REASON));
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
#[derive(Clone, Debug)]
pub(super) struct Target {
    pub(super) key: String,
    /// The registry id of the session's host: one request at a time goes to each.
    host: i64,
    /// The session's title, escaped from the row as read.
    title: HostText,
    /// The host's full name as the helm names it (`Local (this machine)` for the local host), `None` when it sent none.
    host_name: Option<HostText>,
    /// What the row's status said when the target was frozen. Drawn, but not part of what the answer authorizes
    /// ([`Target::rewords`]).
    status: Status,
    /// What the action does to the session: the row question's own consequence.
    consequence: String,
    /// Whether a Delete row adds the managed checkout's line.
    managed: bool,
    /// The precondition the request carries, frozen from the same row.
    guard: Guard,
    /// Whether this target's own row would ask before acting, Skip session confirmations aside.
    asks: bool,
}

/// What a target's status says in the question: a live word, an ended status's words, or "Unknown" for one nobody
/// classified (the question names an uncertainty a row leaves undrawn).
#[derive(Clone, Debug)]
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
    /// `row`, filed under `key`, as a target of `action`, with its host placed through `collections`' registry.
    pub(super) fn of(
        action: Lifecycle,
        key: &str,
        row: &SessionRow,
        collections: &Collections,
    ) -> Target {
        let local = row_marks::locality(row, collections) == Some(Locality::Local);
        let status = match (status::live(row), status::ended_words(row, false)) {
            (Some(live), _) => Status::Live(live),
            (None, Some(ended)) => Status::Ended(ended),
            (None, None) => Status::Unknown,
        };
        let (consequence, guard, asks) = match action {
            Lifecycle::Stop => (
                stop::consequence(row),
                Guard::Stop(stop::guard(row)),
                stop::asks(row, false),
            ),
            Lifecycle::Delete => (
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
            managed: action == Lifecycle::Delete && row.info.working_copy.is_some(),
            guard,
            asks,
        }
    }

    /// Whether `now`, the same session frozen afresh, would have the question say or authorize something else: another
    /// title, host, consequence, checkout line or precondition. The status word alone does not count. The old question
    /// compared everything it drew, status included, and replaced itself in place at no cost; here a reword closes the
    /// question and asks again, which moves focus, and an agent at work moves between Running, Waiting and Idle every
    /// turn, so a full comparison would reopen a question over working agents while the user reads it. Narrowed until
    /// the prompt can replace a question in place.
    fn rewords(&self, now: &Target) -> bool {
        self.title != now.title
            || self.host_name != now.host_name
            || self.consequence != now.consequence
            || self.managed != now.managed
            || self.guard != now.guard
    }

    /// The work this target's request is, which its row draws pending while it is held and in flight.
    fn work(&self) -> Work {
        match self.guard {
            Guard::Stop(_) => Work::Stop,
            Guard::Delete(guard) => Work::Delete(guard),
        }
    }

    /// The name its not-started line leads with ([`not_started`]).
    fn label(&self) -> &'static str {
        match self.guard {
            Guard::Stop(_) => Lifecycle::Stop.label(),
            Guard::Delete(_) => Lifecycle::Delete.label(),
        }
    }

    /// Send this target's request through its row's own send, running `then` once it has settled.
    fn start(&self, then: impl FnOnce(&mut App) + 'static, cx: &mut App) -> Result<(), NotStarted> {
        match self.guard {
            Guard::Stop(guard) => stop::start(&self.key, guard, then, cx),
            Guard::Delete(guard) => delete::start(&self.key, guard, then, cx),
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

/// Whether `drawn` would be reworded as `now`: a target gone, or one whose answer would authorize something else
/// ([`Target::rewords`]). The two lists are in the same order, `now` never longer.
fn rewords(drawn: &[Target], now: &[Target]) -> bool {
    drawn.len() != now.len() || drawn.iter().zip(now).any(|(drawn, now)| drawn.rewords(now))
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
    layer: LayerId,
    action: Lifecycle,
    /// The question's frozen targets, shared with the prompt's value and its body ([`Targets`]). The prompt gives no
    /// way to read its value back once asked, so the list keeps this handle to compare each listing against.
    targets: Targets,
    /// The user's input when the question first opened, from the press of the bar's button: a reword asks under it, so
    /// it never opens or moves focus once the user has moved on.
    mark: Mark,
    /// The window it is open in, which a reconciling settle (an observer, with no window) reaches it through.
    window: AnyWindowHandle,
}

/// What `asked`'s targets are after a listing that `collections` now holds: each one the store holds, frozen afresh
/// from its row; each one a listing proves gone, left out; and each one only missing (a filtered, cut or failed
/// listing), kept as drawn. Never one more.
fn reconciled(asked: &Asked, collections: &Collections) -> Vec<Target> {
    asked
        .targets
        .iter()
        .filter_map(|drawn| match collections.get::<SessionRow>(&drawn.key) {
            Some(row) => Some(Target::of(asked.action, &drawn.key, row, collections)),
            None if session::listing_proves_gone(collections, &drawn.key) => None,
            None => Some(drawn.clone()),
        })
        .collect()
}

/// Whether the open question stays as drawn although the store now reads `now`: the user has pressed a key or clicked
/// since it opened, so a replacement could not open or take focus under its mark, and while a target is left its answer
/// still authorizes only what it drew. The one statement of that rule, for the reconcile that schedules a reword and
/// the reword itself.
fn stays_as_drawn(asked: &Asked, now: &[Target], cx: &App) -> bool {
    focus::has_moved_on(&asked.mark, cx) && !now.is_empty()
}

// ---- Starting ------------------------------------------------------------------------------------------------------

/// The marked bar's Stop's element id.
pub(super) const STOP_ID: &str = "session-marked-stop";

/// The marked bar's Delete's element id.
pub(super) const DELETE_ID: &str = "session-marked-delete";

impl SessionList {
    /// The marked bar's Stop or Delete was pressed: freeze the marked rows the last frame showed as targets of
    /// `action`, then ask about them, or, when no target would ask on its own row or Skip session confirmations is on,
    /// run them at once. Nothing happens while a bulk question is open (a second press queued before the question took
    /// the window's input) or when no shown mark has a row in the store.
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
        if targets.is_empty() {
            return;
        }
        if skips || !targets.iter().any(|target| target.asks) {
            self.run_batch(targets, window, cx);
            return;
        }
        self.ask_bulk(action, targets.into(), mark, false, window, cx);
        // The question is a choice of its own: a batch accepted earlier must not clear the marks it asks about.
        let current = self.current.current(cx);
        if current.bulk.question.is_some() {
            current.marks.touch();
        }
    }

    /// Ask the bulk question about `targets` under `mark`, the press that first asked it, as a replacement of an
    /// earlier one when `later` (it then starts on Cancel). Focus first goes to the ⋯, which the prompt returns it to
    /// on Cancel, as the old question's Cancel did. While the bar draws no ⋯ (every mark shown is a retired session,
    /// which no metadata action takes), it goes to the bar's own button for `action` instead, where the old app fell
    /// back to the order's title, which the bar here replaces and so is not drawn.
    fn ask_bulk(
        &mut self,
        action: Lifecycle,
        targets: Targets,
        mark: Mark,
        later: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let opener = self
            .marking
            .menu_toggle_drawn()
            .unwrap_or_else(|| self.marking.button_focus(action))
            .clone();
        focus::focus_control(&opener, &mark, window, cx);
        let mut question = question(action, targets.clone());
        if later {
            question = question.starts_on_cancel();
        }
        let list = cx.weak_entity();
        let asked = prompt::ask(
            question,
            None,
            &mark,
            move |targets, window, cx| {
                let _ = list.update(cx, |list, cx| list.confirm_bulk(targets, window, cx));
            },
            window,
            cx,
        );
        self.current.current(cx).bulk.question = asked.map(|layer| Asked {
            layer,
            action,
            targets,
            mark,
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
            .is_some_and(|asked| !layers::is_open(asked.layer, window, cx))
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
    /// store and queue them by host, and send each host's first.
    fn run_batch(&mut self, targets: Vec<Target>, window: &mut Window, cx: &mut Context<Self>) {
        let mut hosts: Vec<i64> = Vec::new();
        let mut queues: HashMap<i64, VecDeque<Target>> = HashMap::new();
        let keys = targets.iter().map(|target| target.key.clone()).collect();
        for target in targets {
            let held = !menu::refused(&target.key, cx) && target.hold(cx).is_ok();
            if !held {
                not_started(&target.key, target.work(), target.label(), cx);
                continue;
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
            self.finish_batch(id, window, cx);
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
                    self.finish_batch(id, window, cx);
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
            let next = move |cx: &mut App| {
                // A window that has gone takes its queues with it.
                let _ = handle.update(cx, |_, window, cx| {
                    list.update(cx, |list, cx| list.advance(id, host, window, cx))
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

    /// Batch `id` has nothing left in flight or queued: forget it, clear its own targets unless a marking choice came
    /// since it was accepted, and settle, since its Deletes may have been what held the automatic selection back.
    fn finish_batch(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let shown = self.marking.shown();
        let in_bar = self.marking.holds_focus(window);
        let current = self.current.current(cx);
        let Some(batch) = current.bulk.batches.remove(&id) else {
            return;
        };
        if current.marks.revision() == batch.revision {
            current
                .marks
                .prune(|key| batch.targets.iter().any(|target| target == key));
            // The bar goes when every mark it showed was the batch's; focus on it goes to the title, as Clear's does.
            if in_bar && shown.iter().all(|key| batch.targets.contains(key)) {
                focus::focus_control(&self.order_focus, &Mark::take(cx), window, cx);
            }
            cx.notify();
        }
        self.settle(cx);
    }

    // ---- Rewording -----------------------------------------------------------------------------------------------------

    /// After a change of the store, see whether the open bulk question still reads as drawn ([`reconciled`],
    /// [`rewords`]), and if not, reword it once the observer has returned (rewording asks, which needs the window).
    /// Called from the list's settle, which every change of the store reaches.
    pub(super) fn reconcile_bulk_question(&mut self, cx: &mut Context<Self>) {
        let Some(store) = session::current_store(cx) else {
            return;
        };
        let Some(asked) = &self.current.current(cx).bulk.question else {
            return;
        };
        let now = reconciled(asked, store.read(cx).collections());
        if stays_as_drawn(asked, &now, cx) || !rewords(&asked.targets, &now) {
            return;
        }
        let (list, window) = (cx.weak_entity(), asked.window);
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                list.update(cx, |list, cx| list.reword_bulk_question(window, cx))
            });
        });
    }

    /// Reword the open bulk question to the targets the store gives now: close it, then ask the replacement under the
    /// question's own opening mark, or leave it closed when no target is left. Done afresh from the store as it is when
    /// it runs, so several changes before it ran reword once; a question closed meanwhile is only forgotten. Once the
    /// user has moved on since the question opened, it stays as drawn: the replacement could not open under that mark,
    /// and its answer still authorizes only what it drew (admission drops a target no longer shown).
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
        if stays_as_drawn(&asked, &targets, cx) || !rewords(&asked.targets, &targets) {
            self.current.current(cx).bulk.question = Some(asked);
            return;
        }
        layers::close(asked.layer, window, cx);
        if !targets.is_empty() {
            self.ask_bulk(asked.action, targets.into(), asked.mark, true, window, cx);
        }
    }
}

// ---- The question --------------------------------------------------------------------------------------------------

/// The bulk question's width: the old `.bulk-delete-dialog`'s 620 pixels, so a long host name or consequence wraps
/// less.
const QUESTION_WIDTH: f32 = 620.;

/// The bulk question about `targets`: "Stop 2 sessions?" or "Delete 1 session?", with one row per target in its body,
/// and the old dialog's answers and their help. Its answer acts on `targets` as frozen here.
fn question(action: Lifecycle, targets: Targets) -> Question<Targets> {
    let verb = action.label();
    let noun = if targets.len() == 1 {
        "session"
    } else {
        "sessions"
    };
    Question::new(
        format!("{verb} {} {noun}?", targets.len()),
        Answer::new(
            format!("{verb} {noun}"),
            HoverHelp::tooltip(format!("{verb} the sessions listed in this question")),
            Tier::Danger,
        ),
        targets,
    )
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip("Cancel: keep the marked sessions"),
        Tier::Neutral,
    ))
    .keeps_focus_on_empty_click()
    .width(px(QUESTION_WIDTH))
    .body(move |targets: &Targets, _, cx| {
        let targets = targets.clone();
        cx.new(|_| Rows { action, targets })
    })
}

/// The bulk question's body: one row per target, as the question froze them.
struct Rows {
    action: Lifecycle,
    targets: Targets,
}

impl Render for Rows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        div()
            .flex()
            .flex_col()
            .children(self.targets.iter().enumerate().map(|(index, target)| {
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
                            .child(
                                div()
                                    .flex_none()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(status),
                            )
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
                    .child(div().mt(px(8.)).child(target.consequence.clone()))
                    .when(self.action == Lifecycle::Delete && target.managed, |row| {
                        row.child(div().mt(px(8.)).child(delete::MANAGED_CHECKOUT))
                    })
                    .text_size(metrics::TEXT_MD)
            }))
    }
}
