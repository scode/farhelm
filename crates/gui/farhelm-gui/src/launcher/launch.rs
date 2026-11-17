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
//! - **A create in a new environment** (New container, New sandbox) goes to its own route, which the helm accepts (202)
//!   once it has admitted the launch, naming the run its outcome lands on in the pushed host progress (SPEC_impl.md,
//!   "The UI channel"). The launcher stays hidden and busy, its progress pill up, until that run ends ([`RunWait`]):
//!   completed, the new host's session is read and settled exactly as an ordinary create's reply is, so success opens
//!   it under the same rules; failed, the run's reason comes back in the launcher as a refusal does. It is never
//!   opened from the 202 itself, which only says the launch began. A completed run whose session read fails or finds
//!   no session settles as done with nothing to open (logged): the launch succeeded, and its row appears in the list.
//!   A failed run the helm marks as ending without a known result, or a run that goes away, comes back with words
//!   saying so, and its notice says to check Hosts rather than that nothing launched
//!   ([`Notice::EnvironmentLaunchUnknown`]).
//! - **A quick launch being prepared** (`view::prepare_unseen`): a launcher made out of sight on New's prefill waits
//!   here while its catalog read is out, before its templates are applied. It counts as busy, as a pending launch does
//!   ([`busy`]): New, Clone, the template shortcuts and the sequence prefix wait. Then it is started through [`start`]
//!   like any other, or shown ([`reveal_prepared`]) under the same moved-on rule as a refusal, behind the notice if
//!   the user moved on. Its progress pill only arms once it is started.
//!
//! The slot's "waiting for the reply" is the hidden launcher's own lifetime, not a copy of the operation store's busy
//! state: the store still says whether the create is in flight, and the launcher reads the refusal it shows from the
//! store, by the launch's key. An accepted environment create is the one exception: the store lets go of its key at
//! the 202, and from then on the slot's wait for the run ([`RunWait`]) is the only record that the launch goes on
//! ([`environment_create_running`]). Dismissing the notice withdraws only the notice; the launcher stays in the slot,
//! and New (or Clone) brings it back.
//!
//! **Reports while hidden.** A create that waits two seconds shows the progress pill (`super::pill`) until its reply,
//! armed by a ticker deadline the held launcher keeps, so the deadline goes with it on a reply or a reset. A success
//! that does not open its session shows the "Started" report for four seconds instead; its expiry is kept in the slot,
//! and a later report replacing it in the layer manager's one launch pill makes that expiry a no-op.

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::SessionInfo;
use farhelm_proto::provisioning::{ProvisioningView, RunStatus};
use farhelm_proto::session_requests::{
    CreateSessionBody, EnvironmentCreateBody, ReplaceSessionBody, RestartSessionBody,
};
use farhelm_proto::ui_channel::{Availability, Collection, HostView, SessionList};
use gpui::{AnyWindowHandle, App, AppContext as _, Entity, Global, Window};

use super::draft::Install;
use super::pill::{self, ProgressPill, StartedReport};
use super::view::{Launcher, show};
use crate::focus::{self, Mark};
use crate::host_text::HostText;
use crate::layers::{self, PillId};
use crate::notices::{self, Notice};
use crate::operations::{self, Command, HelmMarker, OpKey, Outcome, Settled, Work};
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
    /// The run an accepted environment create's outcome lands on, from its 202 until that run ends; `None` for every
    /// other wait. While it is set the launcher still waits for its "reply" ([`Waiting::Reply`]): the launch is not
    /// over, so it stays busy and its progress pill stays up.
    run: Option<RunWait>,
}

/// A pending create's progress pill: the deadline that shows it two seconds after the press, then the pill it showed in
/// the launch's window (the one its reply's settle is handed).
struct Progress {
    _shows: ticker::Deadline,
    shown: Option<PillId>,
}

/// An environment create the helm accepted, waiting for its run to end in the pushed host progress (SPEC_impl.md, "The
/// UI channel"), with what its outcome is settled against: the request as sent, its window, and the press's mark, the
/// same three an ordinary create's reply is settled against.
///
/// Keyed by the run, not by the host alone: the host is new, but matching the run id as well keeps any other run
/// listed under that id from passing for this launch's.
///
/// A wait has no deadline of its own. While the host progress collection is unavailable the run counts as still
/// running ([`run_end`]), so a wait that starts or stays there ends only with a reset.
struct RunWait {
    host: i64,
    run: String,
    sent: Sent,
    window: AnyWindowHandle,
    mark: Mark,
    /// Set once the run has completed and the read of the new host's sessions is out, so the store changes that follow
    /// send no second one.
    reading: bool,
}

/// What an accepted environment create's 202 names: the host the helm registered for the new environment, and the run
/// on it that carries the launch's outcome.
#[derive(serde::Deserialize)]
struct Admitted {
    host_id: i64,
    run_id: String,
}

/// How the run an environment create waits on stands in the store.
enum RunEnd {
    /// Still running, or no host progress has been pushed yet.
    Running,
    /// The launch's session exists on the run's host.
    Completed,
    /// The launch failed, with the run's reason as the helm words it ("Not launched." and the reason), and whether the
    /// helm marks its outcome as unknown (a launch task that ended without a result).
    Failed { words: HostText, unknown: bool },
    /// The run is not listed though host progress is available, so how the launch ended cannot be learned
    /// (SPEC_impl.md, "The UI channel"). The helm lists the run before it admits the launch and pushes a command's
    /// changes before its reply, so this holds from the 202 on: a run missing then was never going to appear.
    Lost,
}

/// Why the launcher cannot say how an accepted environment create ended: its run is no longer listed, or the 202 did
/// not name it. Worded as the old app's uncertain environment launch was ("… Check Hosts for the environment before
/// launching again."), since the environment may exist.
const RUN_LOST: &str = "Farhelm can no longer see how this launch ended. Check Hosts for the environment before \
                        launching again.";

/// A failed run that carries no reason of its own. The helm always words one; this only keeps the launcher from showing
/// an empty sentence if it ever did not. A run whose outcome is unknown says [`RUN_LOST`] instead, which claims no
/// failure.
const RUN_FAILED: &str = "Not launched. The launch failed.";

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
/// reset drops what it held, the notice offering that launcher has nothing left to open, and goes too. It follows the
/// store for an accepted environment create as well, whose outcome arrives as a store change ([`follow_run`]).
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
                return;
            }
            follow_run(slot, cx);
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
/// with another draft"). An environment create stays busy after its 202, until its run ends.
pub(super) fn busy(cx: &App) -> bool {
    matches!(held(cx), Some(Waiting::Reply | Waiting::Preparing))
}

/// The run an accepted environment create waits on, by host id and run id, while the slot's launcher waits on one;
/// `None` for every other wait. The progress pill reads it while it draws (`super::pill`), so the slot is its only
/// record.
pub(super) fn followed_run(cx: &App) -> Option<(i64, String)> {
    let slot = cx.try_global::<Slot>()?.0.read(cx);
    let wait = slot.held.read(cx)?.as_ref()?.run.as_ref()?;
    Some((wait.host, wait.run.clone()))
}

/// Whether an environment create the helm accepted is still running, after its 202: what the host registry's side
/// counts as a launch in flight besides the operation store's (`hosts::launch_in_flight`), since the create's
/// operation key clears on the 202 while the launch goes on for minutes. A quick launch still being prepared, which
/// has sent nothing, does not count.
pub(crate) fn environment_create_running(cx: &App) -> bool {
    followed_run(cx).is_some()
}

/// Whether the slot holds a launcher for the current UI session at all: one being prepared, one whose create is out,
/// or one a refusal left waiting for the user behind its notice. A quick launch is refused while any is
/// (`entry::quick_launch_available`).
pub(super) fn holding(cx: &App) -> bool {
    held(cx).is_some()
}

/// Hold `launcher`, a quick launch's that is not drawn, in the slot while it is prepared (`view::prepare_unseen`).
pub(super) fn preparing(launcher: Entity<Launcher>, cx: &mut App) {
    hold(
        Some(Held {
            launcher,
            waiting: Waiting::Preparing,
            // No create is out yet, so no progress pill: the preparation is the catalog read, and then at most the
            // host's history or a checkout preview.
            progress: None,
            run: None,
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

/// What the slot's launcher for the current UI session waits for, if it holds one, without making the slot. Reads the
/// slot rather than updating it, so it answers from a plain `&App`.
fn held(cx: &App) -> Option<Waiting> {
    let slot = cx.try_global::<Slot>()?.0.read(cx);
    slot.held.read(cx)?.as_ref().map(|held| held.waiting)
}

/// Whether the launch whose reply the window waits for is a quick launch that never showed the launcher
/// (`view::prepare_unseen`): the fact SPEC.md's naming rule turns on ("When New was never shown and success selects its
/// session, the rename dialog opens over it"). Revealing the launcher, for a refusal or the draft's own reason, clears
/// it for good, since "A launch that revealed New gets no naming dialog, even if a later submission succeeds".
///
/// For tests: the success path reads the same fact from the launcher it was handed (`settle`), not from the slot.
#[cfg(test)]
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
/// header"). Requests compare equal when they would send the same thing, which is what an open YOLO question answers for
/// (`view`'s `apply_now`).
#[derive(PartialEq, Eq)]
pub(super) enum Request {
    Create(CreateSessionBody),
    /// New or Clone into a new disposable environment (New container, New sandbox): a create like [`Request::Create`]
    /// under a fresh launch key, sent to the environment route (`POST /api/environment-sessions`), which answers 202
    /// once it has admitted the launch and lands the outcome on a run in the pushed host progress ([`RunWait`]). A
    /// route of its own rather than a target on the ordinary create, which an older helm would run on its own machine
    /// (`EnvironmentCreateBody`'s docs).
    Environment(EnvironmentCreateBody),
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
            Request::Create(_) | Request::Environment(_) => OpKey::new_launch(),
            Request::Replace { source, .. } | Request::Restart { source, .. } => {
                OpKey::session(source)
            }
        }
    }

    /// The operation store's work: the sidebar's own Replace and Restart work for the source's requests, so the
    /// source's row and header read them as they read the plain ones.
    fn work(&self) -> Work {
        match self {
            Request::Create(_) | Request::Environment(_) => Work::Launch,
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
            Request::Environment(body) => Some(Command::new(
                Work::Launch,
                Method::Post,
                ApiPath::api(&["environment-sessions"]).ok()?,
                Some(serde_json::to_value(body).ok()?),
                "POST /api/environment-sessions",
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
            Request::Environment(body) => body.request.title.clone(),
            Request::Replace { body, .. } => body.with.as_ref().and_then(|with| with.title.clone()),
            Request::Restart { .. } => None,
        }
    }

    /// The host an ordinary create is sent to; `None` for every other request, which acts on its source's host or
    /// creates a new one.
    fn created_on(&self) -> Option<i64> {
        match self {
            Request::Create(body) => body.host,
            Request::Environment(_) | Request::Replace { .. } | Request::Restart { .. } => None,
        }
    }

    /// The progress pill's verb (the old `LaunchAction::present`).
    fn verb(&self) -> &'static str {
        match self {
            Request::Create(_) | Request::Environment(_) => "Launching",
            Request::Replace { .. } => "Replacing",
            Request::Restart { .. } => "Restarting",
        }
    }

    /// The source a Replace with replaces or a Restart with restarts; `None` for a create.
    fn source(&self) -> Option<String> {
        match self {
            Request::Create(_) | Request::Environment(_) => None,
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
        installation: request.created_on().and_then(|host| install_of(host, cx)),
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
            run: None,
        }),
        cx,
    );
    true
}

/// The deadline that shows `launcher`'s progress pill in `window` two seconds from now, opening with `verb` and naming
/// `host` and `title` as the request carries them, counting from now, if the slot still holds `launcher` waiting for
/// its reply then. The pill finds an accepted environment create's run itself ([`followed_run`]).
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
    /// The installation an ordinary create was sent to, as the store held it at the press; `None` for every other
    /// request. A refusal that comes back after that host's registry row moved to another installation is not shown in
    /// its own words ([`EARLIER_SETUP_REFUSED`], [`EARLIER_SETUP_TIMED_OUT`]).
    installation: Option<Install>,
}

/// The installation `host`'s registry row names in the current store, if the store lists the host.
fn install_of(host: i64, cx: &App) -> Option<Install> {
    let store = session::current_store(cx)?;
    store
        .read(cx)
        .collections()
        .get::<HostView>(&HostView::key(host))
        .map(Install::of)
}

/// What a refused create says when its host's registry row moved to another installation (or left the registry) while
/// it was out, the old launcher's words. The refusal came from the earlier installation, and its own words could
/// describe a machine the launcher no longer targets, so they are not shown beside a draft that now names another
/// (the old app's "launcher shows a generic refusal after its target changes").
const EARLIER_SETUP_REFUSED: &str = "A launch for an earlier setup was refused";

/// What a create says instead of [`EARLIER_SETUP_REFUSED`] when the helm's host-reply bound cut it off, so it may still
/// start: "refused" would contradict the notice saying so. These are the old launcher's words too.
const EARLIER_SETUP_TIMED_OUT: &str =
    "A launch for an earlier setup timed out; its outcome is unknown";

/// `launcher`'s request (`sent`) settled. `mark` is the user's answer the launch counts from (the press of Launch, or the YOLO
/// question's answer), which also reopens the launcher: the layer manager refuses a mark the user moved on from.
///
/// A Replace with's success is plain Replace's ([`replace::settle`]): the source's delete notice goes to the session
/// list's Delete notice band, and the replacement opens only while its source is still the open session (SPEC.md,
/// "Creation": "Replace with opens its replacement only while its source is still selected"); no "Started" report. A
/// Restart with's success does nothing more ("Restart with never navigates"): the pushed list shows the session running
/// again. Their refusals come back in the launcher as a create's do, and the operation store keeps the same words on
/// the source's row.
///
/// An environment create's 202 is not its outcome: the launcher goes on waiting for the run it names
/// ([`wait_for_run`]), and comes back here with the session the run made once it completes ([`completed`]), so its
/// success opens the session under exactly the rules below.
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
    // Only an environment create is answered 202: its launch is admitted, not over, so its pill stays up and the
    // launcher stays busy until the run ends.
    if let Outcome::Accepted(body) = &settled.outcome {
        wait_for_run(launcher, body.clone(), sent, window, mark, cx);
        return;
    }
    let Sent {
        key,
        source,
        work,
        installation,
    } = sent;
    hide_progress(window, cx);
    // The store counts from when it sent the create; `mark` from the answer that asked for it, which for a create sent
    // after the YOLO question's host write is earlier, and presses made during that write count too.
    let user_moved_on = settled.user_moved_on || focus::has_moved_on(&mark, cx);
    match &settled.outcome {
        Outcome::Done(body) => {
            // Whether New was never shown, which decides naming (`opened`), read from the launcher this reply is for.
            let unseen = launcher.read(cx).quick_launch_unseen;
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
                None => opened(body.clone(), unseen, user_moved_on, window, cx),
            }
        }
        // Followed above, on its run.
        Outcome::Accepted(_) => {}
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
            let moved = installation
                .as_ref()
                .is_some_and(|sent| install_of(sent.host, cx).as_ref() != Some(sent));
            if moved && let Some(operations) = session::current_operations(cx) {
                operations.update(cx, |operations, cx| {
                    let words = if may_still_start {
                        EARLIER_SETUP_TIMED_OUT
                    } else {
                        EARLIER_SETUP_REFUSED
                    };
                    operations.note(&key, work, HostText::from_host(words), cx)
                });
            }
            let refused_launch = RefusedLaunch {
                key,
                work,
                may_still_start,
                unaccepted: refusal.has(HelmMarker::CreateUnaccepted),
                complete: false,
                outcome_unknown: false,
            };
            refused(launcher, refused_launch, window, mark, user_moved_on, cx);
        }
    }
}

// ---- An accepted environment create, waiting for its run ------------------------------------------------------------

/// `launcher`'s environment create was accepted (202) with `body`, which names the new environment's host and the run
/// on it that carries the launch's outcome: keep the launcher waiting, now for that run ([`RunWait`], which its
/// progress pill reads), and look at the run at once, since the channel pushes what a command changed before its
/// reply (SPEC_impl.md, "The UI channel"). A 202 that names no run cannot be followed, so the launcher comes back
/// saying that how the launch ended is unknown ([`RUN_LOST`]), as for a run that goes away.
fn wait_for_run(
    launcher: Entity<Launcher>,
    body: Option<serde_json::Value>,
    sent: Sent,
    window: AnyWindowHandle,
    mark: Mark,
    cx: &mut App,
) {
    let Some(Admitted { host_id, run_id }) =
        body.and_then(|body| serde_json::from_value::<Admitted>(body).ok())
    else {
        tracing::warn!("an environment create was accepted without naming its run");
        run_failed(
            launcher,
            sent,
            window,
            mark,
            HostText::from_host(RUN_LOST),
            true,
            cx,
        );
        return;
    };
    slot(cx).update(cx, |slot, cx| {
        if let Some(held) = slot.held.current(cx).as_mut() {
            held.run = Some(RunWait {
                host: host_id,
                run: run_id,
                sent,
                window,
                mark,
                reading: false,
            });
        }
        // The pill reads the run as it draws, which its seconds clock makes at least once a second.
        cx.notify();
    });
    slot(cx).update(cx, follow_run);
}

/// Look at the run the slot's launcher waits on, if it waits on one, as the store shows it now: the slot calls this on
/// every store change, and [`wait_for_run`] once as the wait starts. An ended run is settled after this update, since
/// settling updates the slot this runs inside: a failed or vanished run comes back in the launcher ([`run_failed`]),
/// and a completed one has its session read first ([`read_session`]). Either way it is acted on once: a failed run's
/// wait leaves the slot here, and a completed one is marked as read.
fn follow_run(slot: &mut LaunchSlot, cx: &mut gpui::Context<LaunchSlot>) {
    let Some(held) = slot.held.current(cx).as_mut() else {
        return;
    };
    let Some(wait) = held.run.as_mut().filter(|wait| !wait.reading) else {
        return;
    };
    let launcher = held.launcher.clone();
    let (words, unknown) = match run_end(wait, cx) {
        RunEnd::Running => return,
        RunEnd::Completed => {
            wait.reading = true;
            let (host, run) = (wait.host, wait.run.clone());
            cx.defer(move |cx| read_session(launcher, host, run, cx));
            return;
        }
        RunEnd::Failed { words, unknown } => (words, unknown),
        RunEnd::Lost => (HostText::from_host(RUN_LOST), true),
    };
    // A failed run and a lost one come back alike, the lost one as an unknown outcome.
    if let Some(wait) = held.run.take() {
        cx.defer(move |cx| {
            run_failed(
                launcher,
                wait.sent,
                wait.window,
                wait.mark,
                words,
                unknown,
                cx,
            )
        });
    }
}

/// Where `wait`'s run stands in the current store. Before host progress is first pushed, or while its read is failing
/// (the store then keeps the last entries it had, and an absent one proves nothing), the run counts as still running;
/// a run missing from an available collection is gone.
fn run_end(wait: &RunWait, cx: &App) -> RunEnd {
    let Some(store) = session::current_store(cx) else {
        return RunEnd::Running;
    };
    let collections = store.read(cx).collections();
    let available = collections
        .meta(Collection::HostProgress)
        .is_some_and(|meta| meta.availability == Availability::Available);
    let run = collections
        .get::<ProvisioningView>(&HostView::key(wait.host))
        .filter(|run| run.run_id.as_deref() == Some(wait.run.as_str()));
    match run {
        None if available => RunEnd::Lost,
        None => RunEnd::Running,
        Some(run) => match run.status {
            RunStatus::Running => RunEnd::Running,
            RunStatus::Completed => RunEnd::Completed,
            RunStatus::Failed => RunEnd::Failed {
                words: failed_words(run),
                unknown: run.outcome_unknown,
            },
        },
    }
}

/// A failed run's reason: the run's message, which is the whole text to show (SPEC_impl.md, "The UI channel").
fn failed_words(run: &ProvisioningView) -> HostText {
    let fallback = if run.outcome_unknown {
        RUN_LOST
    } else {
        RUN_FAILED
    };
    let words = run
        .message
        .as_deref()
        .filter(|words| !words.trim().is_empty())
        .unwrap_or(fallback);
    HostText::from_host(words)
}

/// The run of `launcher`'s environment create on `host` completed: read the new host's sessions
/// (`GET /api/sessions?host=<host>`, which the helm answers from its cache) and settle the create with the session it
/// made, exactly as an ordinary create's reply is settled ([`settle`]). The pushed session list is not where to look:
/// its filter and cap can leave a session on a new host out, or bring it a push after the run's end (SPEC_impl.md,
/// "The UI channel").
///
/// The new host is the launch's own, so its oldest session is the launch's: at completion it is the only one, and any
/// session made there later (an agent in the environment starting another) is newer. A read that fails, or finds none,
/// settles the create as done with no session, logged: the launch succeeded, so nothing comes back, and the row appears
/// when the list shows it. A launcher the slot no longer holds waiting on this run (a reset) is left alone.
fn read_session(launcher: Entity<Launcher>, host: i64, run: String, cx: &mut App) {
    let path = ApiPath::api(&["sessions"])
        .and_then(|path| path.with_query(&[("host", host.to_string().as_str())]));
    let path = match path {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "the new environment's session read could not be built");
            completed(launcher, host, &run, None, cx);
            return;
        }
    };
    let sent = operations::read::<SessionList>(path, "GET /api/sessions", cx, move |answer, cx| {
        let session = match answer {
            Ok(list) => list
                .sessions
                .into_iter()
                .map(|row| row.info)
                .min_by_key(|session| session.created_at),
            Err(reason) => {
                tracing::warn!(reason = %reason.raw(), "the new environment's session could not be read");
                None
            }
        };
        if session.is_none() {
            tracing::warn!(
                host,
                "a completed environment launch's host lists no session"
            );
        }
        completed(launcher, host, &run, session, cx);
    });
    if !sent {
        // No UI session, so a reset is dropping the slot anyway.
        tracing::debug!("the new environment's session read was not sent");
    }
}

/// Settle `launcher`'s environment create on `host`'s run `run`, which completed, with `session`, the session it made,
/// if it was read: through [`settle`], as an ordinary create's reply, so it opens under the same moved-on rule, hands a
/// quick launch never shown to naming, or shows the "Started" report. Nothing happens unless the slot still holds
/// `launcher` waiting on that run.
fn completed(
    launcher: Entity<Launcher>,
    host: i64,
    run: &str,
    session: Option<SessionInfo>,
    cx: &mut App,
) {
    let wait = slot(cx).update(cx, |slot, cx| {
        let held = slot.held.current(cx).as_mut()?;
        let ours = held.launcher == launcher
            && held
                .run
                .as_ref()
                .is_some_and(|wait| wait.host == host && wait.run == run);
        if ours { held.run.take() } else { None }
    });
    let Some(RunWait {
        sent, window, mark, ..
    }) = wait
    else {
        return;
    };
    let body = session.and_then(|session| serde_json::to_value(session).ok());
    let settled = Settled {
        outcome: Outcome::Done(body),
        // The store's own count ended with the 202; the press's mark carries the moved-on check from here.
        user_moved_on: false,
    };
    settle(launcher, sent, window, mark, &settled, cx);
}

/// `launcher`'s environment create ended without a session: its run failed, went away, or was never named. Bring the
/// launcher back with `words` as a refused create comes back ([`refused`]), keeping its draft, recipe and place
/// included. The words are shown as they are, with no lead of the launcher's: the helm's failed run already starts with
/// "Not launched.", and the launcher's own words for a lost run say what is known. When the outcome is `unknown`, the
/// notice offering the launcher says to check Hosts ([`Notice::EnvironmentLaunchUnknown`]). The words go on the
/// create's key as the store's partial result ([`operations::Operations::note`]), the place a refused create's words
/// are kept, so the launcher reads them back as it reads any refusal.
fn run_failed(
    launcher: Entity<Launcher>,
    sent: Sent,
    window: AnyWindowHandle,
    mark: Mark,
    words: HostText,
    unknown: bool,
    cx: &mut App,
) {
    if waiting(&launcher, cx) != Some(Waiting::Reply) {
        return;
    }
    hide_progress(window, cx);
    if let Some(operations) = session::current_operations(cx) {
        operations.update(cx, |operations, cx| {
            operations.note(&sent.key, sent.work, words, cx)
        });
    }
    let user_moved_on = focus::has_moved_on(&mark, cx);
    let refused_launch = RefusedLaunch {
        key: sent.key,
        work: sent.work,
        may_still_start: false,
        unaccepted: false,
        complete: true,
        outcome_unknown: unknown,
    };
    refused(launcher, refused_launch, window, mark, user_moved_on, cx);
}

/// A create succeeded with the reply `body`, the new session as the helm lists it. Open it, as the quick switcher opens
/// a session, unless the user moved on since pressing Launch (SPEC.md: "A successful New or Clone opens its session
/// only while the submitted launcher opening is still current and the user has pressed no key or pointer button
/// anywhere in the page since their last answer to it"). The opening is current by construction: nothing replaces a
/// launcher while its create is out. Otherwise the row appears, nothing else moves, and the "Started" report says so in
/// `window` for four seconds.
///
/// A quick launch whose launcher was never shown (`unseen`) and whose session this opened goes on to naming
/// (`crate::templates::name_quick_launched`), the one hand-off to it: SPEC.md, "When New was never shown and success
/// selects its session, the rename dialog opens over it". One the user moved on from keeps its generated name.
fn opened(
    body: Option<serde_json::Value>,
    unseen: bool,
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
        if unseen {
            crate::templates::name_quick_launched(&session.id, &session.title, window, cx);
        }
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
    let notice = if refused_launch.outcome_unknown {
        Notice::EnvironmentLaunchUnknown
    } else if refused_launch.may_still_start {
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
                run: None,
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
    /// The helm proved this create was never accepted ([`HelmMarker::CreateUnaccepted`]), whatever it was for: nothing
    /// was created. For a managed checkout that means the folder it previewed was taken by another create meanwhile, or
    /// its preview went stale; the launcher then takes a new preview and says nothing was accepted, so the user reviews
    /// the new folder and presses Launch again (SPEC.md: "If another create wins the displayed path, Launch reports the
    /// conflict and obtains a new preview; it never submits automatically").
    pub(super) unaccepted: bool,
    /// The words are complete as they stand, and the launcher draws them without its "Not launched." lead: an
    /// environment create's failed run, whose message the helm already starts with that lead, or the launcher's own
    /// words for a run whose end it cannot learn.
    pub(super) complete: bool,
    /// How the launch ended is unknown, so the environment it made may exist: an environment create's run the helm
    /// marks as ending without a known result, or one that went away. The notice offering the launcher then says to
    /// check Hosts ([`Notice::EnvironmentLaunchUnknown`]).
    pub(super) outcome_unknown: bool,
}

/// Withdraw every notice that offers a waiting launcher; at most one is up, and any has nothing left to open once it
/// comes back or goes.
fn withdraw_notices(cx: &mut App) {
    notices::withdraw(Notice::LaunchNotStarted, cx);
    notices::withdraw(Notice::LaunchMayStillStart, cx);
    notices::withdraw(Notice::EnvironmentLaunchUnknown, cx);
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
