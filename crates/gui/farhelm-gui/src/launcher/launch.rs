//! Launching: what happens to a New or Clone from the press of Launch on a complete draft until its reply has been
//! dealt with.
//!
//! SPEC.md, "Creation" (the paragraphs after the launcher layout) and "Keyboard focus", "Who may move focus". Ported
//! from the old app's submit path in `list/create_form.rs` and the reply handling in `list/view.rs` at reference commit
//! `24bc0580`, without its intent keys, request fences and return admission: the ordered channel and the operation
//! store replaced them. Here's the TLDR:
//!
//! - **One submit path.** [`start`] sends the draft's create through the operation store, under a fresh launch key and
//!   [`Work::Launch`], and hides the launcher. One intended create yields one session or a clear error: the launcher
//!   is not drawn while the create is out, so nothing can press Launch again or edit the draft it sent.
//! - **Hidden, not closed.** The launcher's dialog layer closes, which gives the page back to the user (SPEC.md: "its
//!   hidden launcher releases that isolation"), but the launcher itself stays here in the slot, with its draft and
//!   fields. There is one slot per UI session, so a reset drops whatever it holds, the draft included (decided by the
//!   maintainer for the migration: drafts do not survive a reset).
//! - **Success** opens the new session the way the quick switcher opens one, if the user has not moved on since
//!   pressing Launch; otherwise the row appears and nothing else moves.
//! - **A refusal** brings the same launcher back with its draft and the helm's words beside Launch, "Not launched."
//!   first, if the user has not moved on; otherwise a notice offers it ([`Notice::LaunchNotStarted`]) and the launcher
//!   waits in the slot until the user opens that notice.
//! - **The host's timeout** (the helm's bounded create answering 504 without saying the create never reached the host)
//!   is handled as a refusal, except that it is shown without "Not launched." and offered by its own notice, since the
//!   create may still happen. Once that reply has come, the window no longer counts the launch as in flight, so New,
//!   another launch and host changes go ahead while the helm may still be creating; a duplicate session is then
//!   possible, an accepted risk like the one around a lost connection (SPEC.md, "Creation").
//! - **The YOLO question.** A refusal saying the host asks before YOLO launches (`HelmMarker::YoloConfirmation`) is a
//!   follow-up question, not a failure: the launcher comes back the same way, with the question asked in place
//!   (`view::yolo_question`).
//!
//! The slot's "waiting for the reply" is the hidden launcher's own lifetime, not a copy of the operation store's busy
//! state: the store still says whether the create is in flight, and the launcher reads the refusal it shows from the
//! store, by the launch's key. Dismissing the notice withdraws only the notice; the launcher stays in the slot, and New
//! (or Clone) brings it back.
//!
//! **Reports while hidden.** A create that waits two seconds shows the progress pill (`super::pill`) until its reply,
//! armed by a ticker deadline the held launcher keeps, so the deadline goes with it on a reply or a reset. A success
//! that does not open its session shows the "Started" report for four seconds instead; its expiry is kept in the slot,
//! and a later report replacing it in the layer manager's one launch pill makes that expiry a no-op.

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::SessionInfo;
use farhelm_proto::session_requests::CreateSessionBody;
use gpui::{AnyWindowHandle, App, AppContext as _, Entity, Global, Window};

use super::pill::{self, ProgressPill, StartedReport};
use super::view::{Launcher, show};
use crate::focus::{self, Mark};
use crate::host_text::HostText;
use crate::layers::{self, PillId};
use crate::notices::{self, Notice};
use crate::operations::{Command, HelmMarker, OpKey, Outcome, Settled, Work};
use crate::session::{self, ObservingCurrent, PerSession};
use crate::session_list::selection;
use crate::ticker;

/// Where a submitted launcher waits while it is not drawn: one per UI session, so a reset drops it.
pub(crate) struct LaunchSlot {
    held: PerSession<Option<Held>>,
    /// The "Started" report's expiry, which takes it down four seconds after it shows. Kept apart from `held`, since
    /// the report outlives the launcher that launched it.
    started: Option<ticker::Deadline>,
    /// Withdraws the launch notice once the UI session that owed its launcher is gone.
    _following: ObservingCurrent,
}

/// A launcher that is not drawn, and what it waits for.
struct Held {
    launcher: Entity<Launcher>,
    waiting: Waiting,
    /// The progress pill of the create it waits on: `None` once it waits for the user.
    progress: Option<Progress>,
}

/// A pending create's progress pill: the deadline that shows it two seconds after the press, then the pill it showed in
/// the launch's window (the one its reply's settle is handed).
struct Progress {
    _shows: ticker::Deadline,
    shown: Option<PillId>,
}

/// What a hidden launcher waits for before it is drawn again or dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Waiting {
    /// The helm's reply to its create.
    Reply,
    /// The user, after a refusal that arrived once they had moved on: the notice offers it.
    User,
}

/// The slot's entity, made on the first submit.
struct Slot(Entity<LaunchSlot>);

impl Global for Slot {}

/// The slot, made on first need. It is an entity rather than plain global state so it can follow the UI session: when a
/// reset drops what it held, the notice offering that launcher has nothing left to open, and goes too.
fn slot(cx: &mut App) -> Entity<LaunchSlot> {
    if let Some(slot) = cx.try_global::<Slot>() {
        return slot.0.clone();
    }
    let slot = cx.new(|cx| LaunchSlot {
        held: PerSession::default(),
        started: None,
        _following: session::observe_current(cx, |slot: &mut LaunchSlot, cx| {
            if slot.held.current(cx).is_none() {
                withdraw_notices(cx);
            }
        }),
    });
    cx.set_global(Slot(slot.clone()));
    slot
}

/// What the slot holds for the current UI session, without making the slot.
fn waiting(launcher: &Entity<Launcher>, cx: &mut App) -> Option<Waiting> {
    let slot = cx.try_global::<Slot>()?.0.clone();
    slot.update(cx, |slot, cx| {
        slot.held
            .current(cx)
            .as_ref()
            .filter(|held| held.launcher == *launcher)
            .map(|held| held.waiting)
    })
}

/// Whether a launch's create is out in the current UI session: New and the other ways of starting a launch are refused
/// meanwhile (SPEC.md: "The same launcher remains busy and cannot accept a second launch or be replaced with another
/// draft").
pub(super) fn pending(cx: &mut App) -> bool {
    let Some(slot) = cx.try_global::<Slot>().map(|slot| slot.0.clone()) else {
        return false;
    };
    slot.update(cx, |slot, cx| {
        slot.held
            .current(cx)
            .as_ref()
            .is_some_and(|held| held.waiting == Waiting::Reply)
    })
}

/// Why Launch waits while one of this window's host-registry changes is in flight, as its hover text and after "Not
/// launched." on a press: SPEC.md, "Creation": "Launches and changes to the host registry still exclude each other".
/// The old app disabled Launch and New without a reason while its one page-wide operation token was held.
pub(super) const HOST_CHANGE_IN_FLIGHT: &str = "Wait for the host change to finish";

/// Put `held` in the slot for the current UI session, or empty it, and notify: a surface drawing New's trigger reads
/// [`super::entry::new_available`] while it draws, and redraws when the slot it read changes.
fn hold(held: Option<Held>, cx: &mut App) {
    slot(cx).update(cx, |slot, cx| {
        *slot.held.current(cx) = held;
        cx.notify();
    });
}

/// Send `body`, the draft's create, for `launcher` (the open launcher in `window`) through the operation store, and
/// hold the launcher in the slot while the create is out. `mark` is the user's answer the reply's moved-on check counts
/// from: taken in the handler of the press of Launch, or of the YOLO question's answer whose host write this create
/// waited for. `host` is the target host's name as the launcher shows it now, which the progress pill keeps for the
/// request's life. Returns whether the create went out; it does not without a UI session, or with a body that will not
/// encode, which the draft's types rule out.
///
/// The caller closes the launcher's layer once this returns `true`; the launcher stays alive here.
pub(super) fn start(
    launcher: Entity<Launcher>,
    body: &CreateSessionBody,
    host: HostText,
    mark: Mark,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(operations) = session::current_operations(cx) else {
        return false;
    };
    let title = body.title.clone();
    let (Ok(path), Ok(body)) = (ApiPath::api(&["sessions"]), serde_json::to_value(body)) else {
        tracing::warn!("a launch's create could not be put into a command");
        return false;
    };
    let command = Command::new(
        Work::Launch,
        Method::Post,
        path,
        Some(body),
        "POST /api/sessions",
    );
    let handle = window.window_handle();
    let settling = launcher.clone();
    let key = OpKey::new_launch();
    let refusal_key = key.clone();
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            settle(settling, refusal_key, handle, mark, settled, cx)
        })
    });
    if let Err(refused) = started {
        // A fresh key is never busy, so only a retired store gets here, which a reset is about to replace.
        tracing::debug!(?refused, "a launch was not sent");
        return false;
    }
    let shows = arm_progress(&launcher, handle, host, title, cx);
    hold(
        Some(Held {
            launcher,
            waiting: Waiting::Reply,
            progress: Some(Progress {
                _shows: shows,
                shown: None,
            }),
        }),
        cx,
    );
    true
}

/// The deadline that shows `launcher`'s progress pill in `window` two seconds from now, naming `host` and `title` as
/// the request carries them and counting from now, if the slot still holds `launcher` waiting for its reply then.
fn arm_progress(
    launcher: &Entity<Launcher>,
    window: AnyWindowHandle,
    host: HostText,
    title: Option<String>,
    cx: &mut App,
) -> ticker::Deadline {
    let sent = ticker::wall_now(cx);
    let launcher = launcher.downgrade();
    ticker::after(pill::PROGRESS_SHOWS_AFTER, cx, move |cx| {
        let Some(launcher) = launcher.upgrade() else {
            return;
        };
        if waiting(&launcher, cx) != Some(Waiting::Reply) {
            return;
        }
        let shown = cx
            .update_window(window, |_, window, cx| {
                let view = cx.new(|cx| ProgressPill::new(host, title, sent, cx));
                layers::show_launch_pill(view.into(), window, cx)
            })
            .ok()
            .flatten();
        slot(cx).update(cx, |slot, cx| {
            if let Some(progress) = slot
                .held
                .current(cx)
                .as_mut()
                .and_then(|held| held.progress.as_mut())
            {
                progress.shown = shown;
            }
        });
    })
}

/// Take down the progress pill of the create the slot waits on, if it shows in `window`: its reply has come, whatever
/// it says (SPEC.md: the pill "goes when the launcher returns or closes").
fn hide_progress(window: AnyWindowHandle, cx: &mut App) {
    let shown = slot(cx).update(cx, |slot, cx| {
        slot.held
            .current(cx)
            .as_mut()
            .and_then(|held| held.progress.take())
            .and_then(|progress| progress.shown)
    });
    if let Some(id) = shown {
        let _ = cx.update_window(window, |_, window, cx| layers::hide_pill(id, window, cx));
    }
}

/// `launcher`'s create, sent under `key`, settled. `mark` is the user's answer the launch counts from (the press of
/// Launch, or the YOLO question's answer), which also reopens the launcher: the layer manager refuses a mark the user
/// moved on from.
///
/// A launcher the slot no longer holds is left alone: nothing else lets go of a waiting launcher today, but its reply
/// must not act for a launcher something else has since taken over.
fn settle(
    launcher: Entity<Launcher>,
    key: OpKey,
    window: AnyWindowHandle,
    mark: Mark,
    settled: &Settled,
    cx: &mut App,
) {
    if waiting(&launcher, cx) != Some(Waiting::Reply) {
        return;
    }
    hide_progress(window, cx);
    // The store counts from when it sent the create; `mark` from the answer that asked for it, which for a create sent
    // after the YOLO question's host write is earlier, and presses made during that write count too.
    let user_moved_on = settled.user_moved_on || focus::has_moved_on(&mark, cx);
    match &settled.outcome {
        Outcome::Done(body) => {
            hold(None, cx);
            opened(body.clone(), user_moved_on, window, cx);
        }
        // A create is never accepted as long work, so this cannot arrive. If it did, nothing failed, and the pushed
        // list shows what the helm made of it.
        Outcome::Accepted(_) => hold(None, cx),
        // The host asks before YOLO launches: not a failure but a follow-up question, which comes back the way a
        // refusal does (SPEC.md: "A failure or follow-up question restores the same launcher").
        Outcome::Refused(refusal) if refusal.has(HelmMarker::YoloConfirmation) => {
            launcher.update(cx, |launcher, cx| launcher.yolo_asked(cx));
            come_back(
                launcher,
                window,
                mark,
                user_moved_on,
                Notice::LaunchNotStarted,
                cx,
            );
        }
        Outcome::Refused(refusal) => {
            // The bounded create's timeout: the host did not answer within the helm's bound, and the create goes on
            // behind the reply, so it may still happen. A create that provably never reached the host says so with
            // its marker, and is refused like any other.
            let may_still_start =
                refusal.status == 504 && !refusal.has(HelmMarker::CreateUnaccepted);
            let refused_launch = RefusedLaunch {
                key,
                may_still_start,
            };
            refused(launcher, refused_launch, window, mark, user_moved_on, cx);
        }
    }
}

/// A create succeeded with the reply `body`, the new session as the helm lists it. Open it, as the quick switcher opens
/// a session, unless the user moved on since pressing Launch (SPEC.md: "A successful New or Clone opens its session
/// only while the submitted launcher opening is still current and the user has pressed no key or pointer button
/// anywhere in the page since their last answer to it"). The opening is current by construction: nothing replaces a
/// launcher while its create is out. Otherwise the row appears, nothing else moves, and the "Started" report says so in
/// `window` for four seconds.
fn opened(
    body: Option<serde_json::Value>,
    user_moved_on: bool,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    let Some(Ok(session)) = body.map(serde_json::from_value::<SessionInfo>) else {
        tracing::warn!("a launch succeeded with a reply that does not read as its session");
        return;
    };
    if !user_moved_on {
        selection::choose(&session.id, cx);
        return;
    }
    let shown = cx
        .update_window(window, |_, window, cx| {
            let view = cx.new(|_| StartedReport::new(&session.title));
            layers::show_launch_pill(view.into(), window, cx)
        })
        .ok()
        .flatten();
    let Some(id) = shown else {
        return;
    };
    let expiry = ticker::after(pill::STARTED_SHOWS_FOR, cx, move |cx| {
        let _ = cx.update_window(window, |_, window, cx| layers::hide_pill(id, window, cx));
    });
    slot(cx).update(cx, |slot, _| slot.started = Some(expiry));
}

/// A create was refused ([`RefusedLaunch`]); the operation store keeps the helm's words under its key. Bring `launcher`
/// back showing them if the user has not moved on; otherwise keep it, and post the notice that offers it, the one
/// that does not say "Not launched." when the create may still happen.
fn refused(
    launcher: Entity<Launcher>,
    refused_launch: RefusedLaunch,
    window: AnyWindowHandle,
    mark: Mark,
    user_moved_on: bool,
    cx: &mut App,
) {
    let notice = if refused_launch.may_still_start {
        Notice::LaunchMayStillStart
    } else {
        Notice::LaunchNotStarted
    };
    launcher.update(cx, |launcher, cx| launcher.refused(refused_launch, cx));
    come_back(launcher, window, mark, user_moved_on, notice, cx);
}

/// Bring `launcher` back for a reply the user must see (a refusal, a follow-up question): shown again in `window` if
/// the user has not moved on since the press (`mark`), otherwise kept in the slot with `notice` offering it.
fn come_back(
    launcher: Entity<Launcher>,
    window: AnyWindowHandle,
    mark: Mark,
    user_moved_on: bool,
    notice: Notice,
    cx: &mut App,
) {
    let shown = !user_moved_on
        && cx
            .update_window(window, |_, window, cx| {
                show(&launcher, &mark, window, cx).is_some()
            })
            .unwrap_or(false);
    if shown {
        hold(None, cx);
    } else {
        hold(
            Some(Held {
                launcher,
                waiting: Waiting::User,
                progress: None,
            }),
            cx,
        );
        notices::post(notice, cx);
    }
}

/// A refused launch as the launcher shows it: the key the operation store keeps the helm's words under, and whether the
/// reply was the helm's timeout, which says the create may still happen (SPEC_impl.md, "The UI channel": a bounded
/// command answers "the host timed out" while the work carries on). The launcher drops the "Not launched." lead for
/// that one: saying the launch failed would invite launching the same draft again while the first create may still
/// start a session.
#[derive(Clone, Debug)]
pub(super) struct RefusedLaunch {
    pub(super) key: OpKey,
    pub(super) may_still_start: bool,
}

/// Withdraw both notices that offer a waiting launcher; at most one is up, and either has nothing left to open once it
/// comes back or goes.
fn withdraw_notices(cx: &mut App) {
    notices::withdraw(Notice::LaunchNotStarted, cx);
    notices::withdraw(Notice::LaunchMayStillStart, cx);
}

/// Bring back the launcher a refusal left waiting for the user, in `window`, and withdraw the notice offering it.
/// Call it from the handler of the user's action (the notice's Open launcher). Returns whether one came back; the
/// notice goes either way, since it has nothing else to open. Without a UI session nothing can show, and the launcher
/// goes too, as a reset drops it.
pub(crate) fn open_waiting(window: &mut Window, cx: &mut App) -> bool {
    withdraw_notices(cx);
    let Some(slot) = cx.try_global::<Slot>().map(|slot| slot.0.clone()) else {
        return false;
    };
    let launcher = slot.update(cx, |slot, cx| {
        let held = slot.held.current(cx);
        if held.as_ref().map(|held| held.waiting) != Some(Waiting::User) {
            return None;
        }
        held.take().map(|held| held.launcher)
    });
    let Some(launcher) = launcher else {
        return false;
    };
    let mark = Mark::take(cx);
    show(&launcher, &mark, window, cx).is_some()
}
