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
//!
//! The slot's "waiting for the reply" is the hidden launcher's own lifetime, not a copy of the operation store's busy
//! state: the store still says whether the create is in flight, and the launcher reads the refusal it shows from the
//! store, by the launch's key. Dismissing the notice withdraws only the notice; the launcher stays in the slot, and New
//! brings it back.
//!
//! NOTE: Not built yet, each with the piece that brings it: the pending pill and the "Started" pill that report a
//! launch while the launcher is hidden; the YOLO question a refusal can carry; and showing the host's timeout reply without "Not
//! launched.", since the create may still happen. The last two wait on the operation store telling such replies apart
//! from other refusals.

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::SessionInfo;
use farhelm_proto::session_requests::CreateSessionBody;
use farhelm_proto::ui_channel::HostView;
use gpui::{AnyWindowHandle, App, AppContext as _, Entity, Global, Window};

use super::view::{Launcher, show};
use crate::focus::Mark;
use crate::notices::{self, Notice};
use crate::operations::{Command, OpKey, Outcome, Settled, Work};
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
                notices::withdraw(Notice::LaunchNotStarted, cx);
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
/// Two kinds count. A new host's probe and setup, and a host's removal, by their kind of work ([`changes_host_registry`]);
/// the orchestrator's decision counts the probe too, which the old app left outside its token. And any work on a
/// registered host's own key: an edit of its destination or alias, a re-run of its setup, any of its settings, as the
/// old app's token covered every host settings write. Suspend and Resume are not registry changes (SPEC.md), and
/// nothing done to a session is.
pub(super) fn host_change_in_flight(cx: &App) -> bool {
    let (Some(operations), Some(store)) =
        (session::current_operations(cx), session::current_store(cx))
    else {
        return false;
    };
    let operations = operations.read(cx);
    operations.any_in_flight(changes_host_registry)
        || store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .iter()
            .any(|host| {
                operations
                    .in_flight(&OpKey::host(&HostView::key(host.id)))
                    .is_some()
            })
}

/// Whether `work` is a registry change counted by its kind: a host probe registers a host that answers, a setup
/// registers the host it sets up, and a removal takes a host out of the registry. Exhaustive, so a new kind of work decides. Work on a registered host's own key is counted by
/// its key instead ([`host_change_in_flight`]).
fn changes_host_registry(work: Work) -> bool {
    match work {
        Work::Probe | Work::Setup | Work::RemoveHost => true,
        Work::Rename
        | Work::Delete(_)
        | Work::Save
        | Work::Stop
        | Work::Replace
        | Work::Seen
        | Work::Restart
        | Work::TunnelOpen
        | Work::TunnelClose
        | Work::Launch
        // An answer to a host add's card only opens the way: the add's own probe and setup are what register a host.
        | Work::Answer => false,
    }
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
        Outcome::Refused(_) => refused(launcher, key, window, mark, settled.user_moved_on, cx),
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

/// A create sent under `key` was refused; the operation store keeps the helm's words under that key. Bring `launcher`
/// back showing them if the user has not moved on; otherwise keep it, and post the notice that offers it.
fn refused(
    launcher: Entity<Launcher>,
    key: OpKey,
    window: AnyWindowHandle,
    mark: Mark,
    user_moved_on: bool,
    cx: &mut App,
) {
    launcher.update(cx, |launcher, cx| launcher.refused(key, cx));
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
        notices::post(Notice::LaunchNotStarted, cx);
    }
}

/// Bring back the launcher a refusal left waiting for the user, in `window`, and withdraw the notice offering it.
/// Call it from the handler of the user's action (the notice's Open launcher). Returns whether one came back; the
/// notice goes either way, since it has nothing else to open. Without a UI session nothing can show, and the launcher
/// goes too, as a reset drops it.
pub(crate) fn open_waiting(window: &mut Window, cx: &mut App) -> bool {
    notices::withdraw(Notice::LaunchNotStarted, cx);
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
