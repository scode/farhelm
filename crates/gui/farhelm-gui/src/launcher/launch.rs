//! Launching: what happens to a New, Clone, Replace with or Restart with from the press of Launch on a complete draft
//! until its reply has been dealt with, and to a quick launch prepared out of sight before it is started or shown.
//!
//! SPEC.md, "Creation" (the paragraphs after the launcher layout) and "Keyboard focus", "Who may move focus". Ported
//! from the old app's submit path in `list/create_form.rs` and the reply handling in `list/view.rs` at reference commit
//! `24bc0580`, without its intent keys, request fences and return admission: the ordered channel and the operation
//! store replaced them. Here's the TLDR:
//!
//! - **One submit path.** [`start`] sends the draft's [`Request`] through the operation store and hides the launcher:
//!   a create under a fresh launch key and [`Work::Launch`], or a Replace with or Restart with under its source's own
//!   key with [`Work::Replace`] or [`Work::Restart`], the plain operation's route, key and work, so the source's row
//!   and header treat both alike. One
//!   intended launch yields one session or a clear error: the launcher is not drawn while the request is out, so nothing
//!   can press Launch again or edit the draft it sent.
//! - **Hidden, not closed.** The launcher's dialog layer closes, which gives the page back to the user (SPEC.md: "its
//!   hidden launcher releases that isolation"), but the launcher itself stays here in the slot, with its draft and
//!   fields. There is one slot per UI session, so a reset drops whatever it holds, the draft included (decided by the
//!   maintainer for the migration: drafts do not survive a reset).
//! - **Success** opens the new session the way the quick switcher opens one, if the user has not moved on since
//!   pressing Launch; otherwise the row appears, nothing else moves, and the "Started" report says so. A Replace with's
//!   success is plain Replace's: its replacement opens only while its source is still the open session.
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
//! - **A quick launch being prepared** (`view::prepare_unseen`): a launcher made out of sight on New's prefill waits
//!   here while its catalog read is out, before its templates are applied. It counts as busy, as a pending launch does
//!   ([`busy`]): New, Clone, the template shortcuts and the sequence prefix wait. Then it is started through [`start`]
//!   like any other, or shown ([`reveal_prepared`]) under the same moved-on rule as a refusal, behind the notice if
//!   the user moved on. Its progress pill only arms once it is started.
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
use farhelm_proto::session_requests::{CreateSessionBody, ReplaceSessionBody, RestartSessionBody};
use gpui::{AnyWindowHandle, App, AppContext as _, Entity, Global, Window};

use super::pill::{self, ProgressPill, StartedReport};
use super::view::{Launcher, show};
use crate::focus::{self, Mark};
use crate::host_text::HostText;
use crate::layers::{self, PillId};
use crate::notices::{self, Notice};
use crate::operations::{Command, HelmMarker, OpKey, Outcome, Settled, Work};
use crate::session::{self, ObservingCurrent, PerSession};
use crate::session_list::{outcome, replace, selection};
use crate::ticker;

/// Where a launcher that is not drawn waits (one submitted, refused behind its notice, or a quick launch's being
/// prepared): one per UI session, so a reset drops it.
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
    /// The progress pill of the create it waits on: `None` while it waits for the user, and while a quick launch is
    /// being prepared, before any create.
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
    /// A quick launch being prepared out of sight: the launcher's catalog read, before its templates are applied and
    /// it is started or shown (`view::prepare_unseen`). New, other shortcuts and the sequence prefix wait meanwhile, as
    /// for a launch in flight.
    Preparing,
    /// The helm's reply to its create.
    Reply,
    /// The user, after a refusal that arrived once they had moved on: the notice offers it.
    User,
}

/// The slot's entity, made the first time a launcher is held: a submit, or a quick launch's preparation.
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

/// Whether the launcher is busy for New and the other ways of starting a launch: a launch's create is out, or a quick
/// launch is being prepared (SPEC.md: "The same launcher remains busy and cannot accept a second launch or be replaced
/// with another draft").
pub(super) fn busy(cx: &mut App) -> bool {
    matches!(held(cx), Some(Waiting::Reply | Waiting::Preparing))
}

/// Whether the slot holds a launcher for the current UI session at all: one being prepared, one whose create is out,
/// or one a refusal left waiting for the user behind its notice. A quick launch is refused while any is
/// (`entry::quick_launch_available`).
pub(super) fn holding(cx: &mut App) -> bool {
    held(cx).is_some()
}

/// Hold `launcher`, a quick launch's that is not drawn, in the slot while it is prepared (`view::prepare_unseen`).
pub(super) fn preparing(launcher: Entity<Launcher>, cx: &mut App) {
    hold(
        Some(Held {
            launcher,
            waiting: Waiting::Preparing,
            // No create is out yet, so no progress pill: the preparation is one catalog read.
            progress: None,
        }),
        cx,
    );
}

/// Whether the slot still holds `launcher` as being prepared: a reset drops it, and nothing then may act for it.
pub(super) fn still_preparing(launcher: &Entity<Launcher>, cx: &mut App) -> bool {
    waiting(launcher, cx) == Some(Waiting::Preparing)
}

/// Let go of `launcher`, a quick launch's prepared out of sight whose catalog read could not be sent at all, if the slot
/// still holds it so: nothing follows to finish it, and the slot must not stay busy (New refused) until a reset.
pub(super) fn abandon_prepared(launcher: &Entity<Launcher>, cx: &mut App) {
    if still_preparing(launcher, cx) {
        hold(None, cx);
    }
}

/// Bring back `launcher`, a quick launch's prepared out of sight that did not start (its templates need more choices,
/// were refused, or its draft's own check refused it), as a refused launch comes back: shown in `window` if the user
/// has not moved on since the press (`mark`), otherwise kept behind the notice that offers it. Nothing happens for a
/// launcher the slot no longer holds as being prepared.
pub(super) fn reveal_prepared(
    launcher: Entity<Launcher>,
    window: AnyWindowHandle,
    mark: Mark,
    cx: &mut App,
) {
    if !still_preparing(&launcher, cx) {
        return;
    }
    let user_moved_on = focus::has_moved_on(&mark, cx);
    come_back(
        launcher,
        window,
        mark,
        user_moved_on,
        Notice::LaunchNotStarted,
        cx,
    );
}

/// What the slot's launcher for the current UI session waits for, if it holds one, without making the slot.
fn held(cx: &mut App) -> Option<Waiting> {
    let slot = cx.try_global::<Slot>().map(|slot| slot.0.clone())?;
    slot.update(cx, |slot, cx| {
        slot.held.current(cx).as_ref().map(|held| held.waiting)
    })
}

/// Whether the launch whose reply the window waits for is a quick launch that never showed the launcher
/// (`view::prepare_unseen`): the fact SPEC.md's naming rule turns on ("When New was never shown and success selects its
/// session, the rename dialog opens over it"). Revealing the launcher, for a refusal or the draft's own reason, clears
/// it for good, since "A launch that revealed New gets no naming dialog, even if a later submission succeeds".
///
/// Only tests read it so far; naming a quick-launched session, the templates work's next piece, reads the same fact in
/// the success path. Crate-private until then, and allowed to be unused outside tests meanwhile.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "naming a quick-launched session reads it next")
)]
pub(crate) fn pending_unseen_quick_launch(cx: &mut App) -> bool {
    let Some(slot) = cx.try_global::<Slot>().map(|slot| slot.0.clone()) else {
        return false;
    };
    slot.update(cx, |slot, cx| {
        slot.held.current(cx).as_ref().is_some_and(|held| {
            held.waiting == Waiting::Reply && held.launcher.read(cx).quick_launch_unseen
        })
    })
}

/// Why Launch waits while one of this window's host-registry changes is in flight, as its hover text and after "Not
/// launched." on a press: SPEC.md, "Creation": "Launches and changes to the host registry still exclude each other".
/// The old app disabled Launch and New without a reason while its one page-wide operation token was held. Restart and
/// Replace give it too (`crate::session_list::menu::launch_wait`).
pub(crate) const HOST_CHANGE_IN_FLIGHT: &str = "Wait for the host change to finish";

/// Put `held` in the slot for the current UI session, or empty it, and notify: a surface drawing New's trigger reads
/// [`super::entry::new_available`] while it draws, and redraws when the slot it read changes.
fn hold(held: Option<Held>, cx: &mut App) {
    slot(cx).update(cx, |slot, cx| {
        *slot.held.current(cx) = held;
        cx.notify();
    });
}

/// What a launcher's main action sends: a create (New and Clone), or Replace with's create followed by its source's
/// delete, which goes through the source's own Replace route and key so that the source's row and header show it as
/// they show a plain Replace (SPEC.md: Replace with has "exactly Replace's create-then-delete contract and failure
/// reporting"), or Restart with's restart of its source with the edited launch, through the source's own Restart route
/// and key for the same reason ("Restart with and Replace with share the sidebar's pending states on the row and
/// header").
pub(super) enum Request {
    Create(CreateSessionBody),
    Replace {
        /// The source session's key.
        source: String,
        body: ReplaceSessionBody,
    },
    Restart {
        /// The source session's key.
        source: String,
        body: RestartSessionBody,
    },
}

impl Request {
    /// The operation store's key: a fresh launch key for a create, the source's own key for a Replace with or Restart
    /// with, which shares that key's one-request-at-a-time rule and its failure line with the plain Replace and
    /// Restart.
    fn key(&self) -> OpKey {
        match self {
            Request::Create(_) => OpKey::new_launch(),
            Request::Replace { source, .. } | Request::Restart { source, .. } => {
                OpKey::session(source)
            }
        }
    }

    /// The operation store's work: the sidebar's own Replace and Restart work for the source's requests, so the
    /// source's row and header read them as they read the plain ones.
    fn work(&self) -> Work {
        match self {
            Request::Create(_) => Work::Launch,
            Request::Replace { .. } => Work::Replace,
            Request::Restart { .. } => Work::Restart,
        }
    }

    /// The command, or `None` for a body that will not encode or a source key that cannot be a path segment, which
    /// the draft's types and the helm's keys rule out.
    fn command(&self) -> Option<Command> {
        match self {
            Request::Create(body) => Some(Command::new(
                Work::Launch,
                Method::Post,
                ApiPath::api(&["sessions"]).ok()?,
                Some(serde_json::to_value(body).ok()?),
                "POST /api/sessions",
            )),
            Request::Replace { source, body } => Some(Command::new(
                Work::Replace,
                Method::Post,
                ApiPath::api(&["sessions", source.as_str(), "replace"]).ok()?,
                Some(serde_json::to_value(body).ok()?),
                "POST /api/sessions/[session]/replace",
            )),
            Request::Restart { source, body } => Some(Command::new(
                Work::Restart,
                Method::Post,
                ApiPath::api(&["sessions", source.as_str(), "restart"]).ok()?,
                Some(serde_json::to_value(body).ok()?),
                "POST /api/sessions/[session]/restart",
            )),
        }
    }

    /// The session name the request carries, for the progress pill. A restart keeps its session's name and names none,
    /// as the old restart's pill named only the host.
    fn title(&self) -> Option<String> {
        match self {
            Request::Create(body) => body.title.clone(),
            Request::Replace { body, .. } => body.with.as_ref().and_then(|with| with.title.clone()),
            Request::Restart { .. } => None,
        }
    }

    /// The progress pill's verb (the old `LaunchAction::present`).
    fn verb(&self) -> &'static str {
        match self {
            Request::Create(_) => "Launching",
            Request::Replace { .. } => "Replacing",
            Request::Restart { .. } => "Restarting",
        }
    }

    /// The source a Replace with replaces or a Restart with restarts; `None` for a create.
    fn source(&self) -> Option<String> {
        match self {
            Request::Create(_) => None,
            Request::Replace { source, .. } | Request::Restart { source, .. } => {
                Some(source.clone())
            }
        }
    }
}

/// Send `request` for `launcher` (the open launcher in `window`, or a quick launch's prepared out of sight,
/// `view::prepare_unseen`, whose request is always a create) through the operation store, and hold the launcher in the
/// slot while it is out. `mark` is the user's answer the reply's moved-on check counts from: taken in the handler of
/// the press of Launch (a quick launch's shortcut press), or of the YOLO question's answer whose host write this request
/// waited for. `host` is the target host's name as the launcher shows it now, which the progress pill keeps for the
/// request's life. Returns whether the request went out; it does not without a UI session, with a request that will
/// not encode (see [`Request::command`]), or while the source of a Replace with or Restart with has another request in
/// flight, which the launcher refuses before pressing gets here (`SOURCE_BUSY` in the view).
///
/// A Replace with or Restart with also starts the source's waiting line on its row and header
/// (`outcome::start_clock`), as the plain Replace and Restart do.
///
/// The caller closes the launcher's layer once this returns `true`; the launcher stays alive here.
pub(super) fn start(
    launcher: Entity<Launcher>,
    request: Request,
    host: HostText,
    mark: Mark,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(operations) = session::current_operations(cx) else {
        return false;
    };
    let Some(command) = request.command() else {
        tracing::warn!("a launch's request could not be put into a command");
        return false;
    };
    let handle = window.window_handle();
    let settling = launcher.clone();
    let key = request.key();
    let sent = Sent {
        key: key.clone(),
        source: request.source(),
        work: request.work(),
    };
    let started = operations.update(cx, |operations, cx| {
        operations.start(&key, command, cx, move |settled, cx| {
            settle(settling, sent, handle, mark, settled, cx)
        })
    });
    if let Err(refused) = started {
        // A fresh launch key is never busy and the view refuses a busy source, so only a retired store gets here,
        // which a reset is about to replace.
        tracing::debug!(?refused, "a launch was not sent");
        return false;
    }
    if let Some(source) = request.source() {
        outcome::start_clock(&source, request.work(), cx);
    }
    let shows = arm_progress(&launcher, handle, request.verb(), host, request.title(), cx);
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

/// The deadline that shows `launcher`'s progress pill in `window` two seconds from now, opening with `verb` and naming
/// `host` and `title` as the request carries them, counting from now, if the slot still holds `launcher` waiting for
/// its reply then.
fn arm_progress(
    launcher: &Entity<Launcher>,
    window: AnyWindowHandle,
    verb: &'static str,
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
                let view = cx.new(|cx| ProgressPill::new(verb, host, title, sent, cx));
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

/// What a sent request's reply is settled against: the key it went under, the session a Replace with or Restart with
/// acts on, and its work.
struct Sent {
    key: OpKey,
    source: Option<String>,
    work: Work,
}

/// `launcher`'s request (`sent`) settled. `mark` is the user's answer the launch counts from (the press of Launch, or the YOLO
/// question's answer), which also reopens the launcher: the layer manager refuses a mark the user moved on from.
///
/// A Replace with's success is plain Replace's ([`replace::settle`]): the source's delete notice goes onto the
/// replacement, which opens only while its source is still the open session (SPEC.md, "Creation": "Replace with opens
/// its replacement only while its source is still selected"); no "Started" report. A Restart with's success does
/// nothing more ("Restart with never navigates"): the pushed list shows the session running again. Their refusals come
/// back in the launcher as a create's do, and the operation store keeps the same words on the source's row.
///
/// A launcher the slot no longer holds is left alone: nothing else lets go of a waiting launcher today, but its reply
/// must not act for a launcher something else has since taken over.
fn settle(
    launcher: Entity<Launcher>,
    sent: Sent,
    window: AnyWindowHandle,
    mark: Mark,
    settled: &Settled,
    cx: &mut App,
) {
    if waiting(&launcher, cx) != Some(Waiting::Reply) {
        return;
    }
    let Sent { key, source, work } = sent;
    hide_progress(window, cx);
    // The store counts from when it sent the create; `mark` from the answer that asked for it, which for a create sent
    // after the YOLO question's host write is earlier, and presses made during that write count too.
    let user_moved_on = settled.user_moved_on || focus::has_moved_on(&mark, cx);
    match &settled.outcome {
        Outcome::Done(body) => {
            hold(None, cx);
            match source {
                Some(source) if work == Work::Replace => {
                    let settled = Settled {
                        outcome: settled.outcome.clone(),
                        user_moved_on,
                    };
                    replace::settle(&source, &settled, cx);
                }
                Some(_) => {}
                None => opened(body.clone(), user_moved_on, window, cx),
            }
        }
        // None of these requests is accepted as long work, so this cannot arrive. If it did, nothing failed, and
        // the pushed list shows what the helm made of it.
        Outcome::Accepted(_) => hold(None, cx),
        // The host asks before YOLO launches: not a failure but a follow-up question, which comes back the way a
        // refusal does (SPEC.md: "A failure or follow-up question restores the same launcher").
        Outcome::Refused(refusal) if refusal.has(HelmMarker::YoloConfirmation) => {
            // The operation store recorded the question's refusal under the request's key, which for a Replace with or
            // Restart with is the source's own: left there, the source's row and header would show it as a failed
            // Replace or Restart, and keep it after a Cancel. The question is the launcher's to show (the old app's
            // "with YOLO cancellation leaves no source failure" cases). A create's key is its own, and nothing else
            // reads it.
            if source.is_some()
                && let Some(operations) = session::current_operations(cx)
            {
                operations.update(cx, |operations, cx| operations.forget(&key, cx));
            }
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
                work,
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

/// Bring `launcher` back for a reply the user must see (a refusal, a follow-up question), or a quick launch prepared
/// out of sight that did not start ([`reveal_prepared`]): shown in `window` if the user has not moved on since the
/// press (`mark`), otherwise kept in the slot with `notice` offering it.
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

/// A refused launch as the launcher shows it: the key the operation store keeps the helm's words under and the work
/// they must be for (a Replace with's key is its source's, where a later Stop or Rename of the source records its own
/// words, which are not this launch's), and whether the reply was the helm's timeout, which says the create may still
/// happen (SPEC_impl.md, "The UI channel": a bounded command answers "the host timed out" while the work carries on).
/// The launcher drops the "Not launched." lead for that one: saying the launch failed would invite launching the same
/// draft again while the first create may still start a session.
#[derive(Clone, Debug)]
pub(super) struct RefusedLaunch {
    pub(super) key: OpKey,
    pub(super) work: Work,
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
