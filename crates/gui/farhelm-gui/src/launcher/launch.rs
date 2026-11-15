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
//!   create may still happen. Once that reply has come, the window no longer counts the launch as in flight, so New, another
//!   launch and host changes go ahead while the helm may still be creating; a duplicate session is then possible, an
//!   accepted risk like the one around a lost connection (SPEC.md, "Creation").
//!
//! The slot's "waiting for the reply" is the hidden launcher's own lifetime, not a copy of the operation store's busy
//! state: the store still says whether the create is in flight, and the launcher reads the refusal it shows from the
//! store, by the launch's key. Dismissing the notice withdraws only the notice; the launcher stays in the slot, and New
//! brings it back.
//!
//! NOTE: Not built yet, each with the piece that brings it: the pending pill and the "Started" pill that report a
//! launch while the launcher is hidden, and the YOLO question a refusal can carry (`HelmMarker::YoloConfirmation`).

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::SessionInfo;
use farhelm_proto::session_requests::CreateSessionBody;
use gpui::{AnyWindowHandle, App, AppContext as _, Entity, Global, Window};

use super::view::{Launcher, show};
use crate::focus::Mark;
use crate::notices::{self, Notice};
use crate::operations::{Command, HelmMarker, OpKey, Outcome, Settled, Work};
use crate::session::{self, ObservingCurrent, PerSession};
use crate::session_list::selection;

/// Where a submitted launcher waits while it is not drawn: one per UI session, so a reset drops it.
pub(crate) struct LaunchSlot {
    held: PerSession<Option<Held>>,
    /// Withdraws the launch notice once the UI session that owed its launcher is gone.
    _following: ObservingCurrent,
}

/// A launcher that is not drawn, and what it waits for.
struct Held {
    launcher: Entity<Launcher>,
    waiting: Waiting,
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

/// Whether one of this window's host-registry changes is in flight in the operation store, which refuses Launch and a
/// fresh New: the launcher's half of the exclusion, which the migration's orchestrator decided on 2026-11-14 is kept by
/// each side refusing its own action while the other's work is in flight, rather than by the old app's shared token.
/// (Add host's half, refusing itself while a launch is in flight, is the hosts work's.)
///
/// What counts is the operation store's own classification ([`Work::changes_host_registry`]): a host's probe and setup
/// (a new host's, or a registered one's re-run), its removal and its Update. The orchestrator decided on 2026-11-14
/// that the probe counts, which the old app left outside its token, and that a save of a registered host's settings
/// does not, its destination and alias included: that window is one short save, so a launch does not wait for it (the
/// hosts side is to refuse a destination change during a launch instead, in its own change). Suspend and Resume are
/// not registry changes (SPEC.md), and nothing done to a session is.
pub(super) fn host_change_in_flight(cx: &App) -> bool {
    session::current_operations(cx).is_some_and(|operations| {
        operations
            .read(cx)
            .any_in_flight(Work::changes_host_registry)
    })
}

/// Put `held` in the slot for the current UI session, or empty it, and notify: a surface drawing New's trigger reads
/// [`super::entry::new_available`] while it draws, and redraws when the slot it read changes.
fn hold(held: Option<Held>, cx: &mut App) {
    slot(cx).update(cx, |slot, cx| {
        *slot.held.current(cx) = held;
        cx.notify();
    });
}

/// Send `body`, the draft's create, for `launcher` (the open launcher in `window`) through the operation store, and
/// hold the launcher in the slot while the create is out. Call it from the handler of the user's press of Launch: the
/// store takes the [`Mark`] that decides what the reply may do then, and so does this. Returns whether the create went
/// out; it does not without a UI session, or with a body that will not encode, which the draft's types rule out.
///
/// The caller closes the launcher's layer once this returns `true`; the launcher stays alive here.
pub(super) fn start(
    launcher: Entity<Launcher>,
    body: &CreateSessionBody,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(operations) = session::current_operations(cx) else {
        return false;
    };
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
    let mark = Mark::take(cx);
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
    hold(
        Some(Held {
            launcher,
            waiting: Waiting::Reply,
        }),
        cx,
    );
    true
}

/// `launcher`'s create, sent under `key`, settled. `mark` is the one taken at the press, which the store's [`Settled::user_moved_on`]
/// also answers for; it is kept here to reopen the launcher, which the layer manager refuses for a mark the user moved
/// on from.
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
    match &settled.outcome {
        Outcome::Done(body) => {
            hold(None, cx);
            opened(body.clone(), settled.user_moved_on, cx);
        }
        // A create is never accepted as long work, so this cannot arrive. If it did, nothing failed, and the pushed
        // list shows what the helm made of it.
        Outcome::Accepted(_) => hold(None, cx),
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
            refused(
                launcher,
                refused_launch,
                window,
                mark,
                settled.user_moved_on,
                cx,
            );
        }
    }
}

/// A create succeeded with the reply `body`, the new session as the helm lists it. Open it, as the quick switcher opens
/// a session, unless the user moved on since pressing Launch (SPEC.md: "A successful New or Clone opens its session
/// only while the submitted launcher opening is still current and the user has pressed no key or pointer button
/// anywhere in the page since their last answer to it"). The opening is current by construction: nothing replaces a
/// launcher while its create is out.
fn opened(body: Option<serde_json::Value>, user_moved_on: bool, cx: &mut App) {
    if user_moved_on {
        return;
    }
    let Some(Ok(session)) = body.map(serde_json::from_value::<SessionInfo>) else {
        tracing::warn!("a launch succeeded with a reply that does not read as its session");
        return;
    };
    selection::choose(&session.id, cx);
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
    let may_still_start = refused_launch.may_still_start;
    launcher.update(cx, |launcher, cx| launcher.refused(refused_launch, cx));
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
            }),
            cx,
        );
        let notice = if may_still_start {
            Notice::LaunchMayStillStart
        } else {
            Notice::LaunchNotStarted
        };
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
