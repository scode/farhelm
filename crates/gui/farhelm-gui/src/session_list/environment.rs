//! Environment sessions: a session whose host is a disposable environment Farhelm made for it (a Tensorlake sandbox or a
//! Podman container), and the Suspend, Resume and Stop and suspend its row menu and header offer.
//!
//! SPEC.md, "Sandbox sessions" and "Container sessions": such a session's host label is "<recipe> sandbox" or "<recipe>
//! container on <host>", with the provider's mark in the session's metadata; suspended rows have a pause-marked
//! **Suspended** badge; the row menu offers Suspend or Resume and Stop and suspend, and the header offers Suspend
//! immediately left of Stop, Resume as the filled primary control while suspended. Ported from the old app's
//! `sandbox_sessions.rs` (`owned_host`, `frozen`, `suspend_asks`, `Operation` and `Controls`) and the environment parts
//! of `list/row.rs` and `session_view.rs` at the reference commit. The words are the provider's, from
//! [`crate::environment_ui`]; nothing here names a provider (AGENTS.md, "Code rules"). Here's the TLDR of the rules:
//!
//! - **What makes a session an environment session** is its host alone ([`Environment::of`]): a session-owned host the
//!   helm describes an environment for, never its kind string, the session's title or a provider's binding. A retired
//!   containerized session is never one (`super::retired`): it supports Delete only.
//! - **Phases, not providers.** Which controls apply is read from the host's pushed phase: frozen (suspended, stopped,
//!   changing or being deleted) withholds Stop, Restart, Replace and Replace with ([`Environment::frozen`]); Resume
//!   replaces Suspend while the environment is suspended or stopped and not waiting for its place
//!   ([`Environment::resumable`]); and nothing is offered while it changes ([`Environment::changing`]) or is being
//!   deleted. Only Suspended draws the badge: a stopped container is never labelled Suspended.
//! - **The verb is frozen at the click** ([`Asked`]): Suspend, Resume or Stop and suspend, and Stop's two preconditions
//!   as the clicked row showed them. The activity read and the question can outlast the click, and an answer given
//!   after the phase changed (another viewer resumed it, say) still sends the captured verb, never a toggle of the new
//!   phase.
//! - **Resume never asks. Suspend asks unless the activity is known** ([`activity_asks`]), the ordinary host Suspend
//!   activity check (SPEC.md, "Topology"). Its contract, which a caller must not loosen: the listing is a fresh
//!   `GET /api/sessions?host=<id>`, never the pushed list, which the window's query filters and the listing cap cuts
//!   (a Suspend authorized from it could freeze an agent the list never showed); it counts only when read whole, its
//!   `truncated` flag clear; the host must read connected in the store after the read answers, not when it was sent;
//!   every listed status must be conclusive, so a stale, unknown or running one asks ([`needs_confirmation`]); and a
//!   read that fails asks too. Stop and suspend also asks when an ordinary Stop of the row would
//!   ([`super::stop::asks`]), so with confirmations skipped it still asks when the activity says so: SPEC.md keeps the
//!   host-wide activity confirmation for it.
//! - **One request, on the session's key**, as the old per-session claim held it: [`Work::Suspend`], [`Work::Resume`]
//!   or [`Work::StopAndSuspend`]. While it is in flight the clicked session's other lifecycle commands are refused (a
//!   pending Resume refuses Delete), as `super::menu::refused` refuses on any request of the session; other sessions on
//!   the same host are not held until the provider's change is pushed (next rule), as the old claim held only its own
//!   session. The activity read holds the clicked session too ([`reading`]), as the old claim was held through it.
//!   Settings-and-hosts' later Suspend and Resume from Hosts moves the key to [`OpKey::host`] and lifts the activity
//!   read out of [`begin`] as `pub(crate)` for its own use.
//! - **The 20-second bound is routine, and the change still holds the session.** The helm answers these routes within
//!   its host reply bound, and work still running then answers a 504 that says it carries on; a Tensorlake Resume
//!   routinely takes longer. That answer is not shown as a failure ([`settle`]): the row and header go on showing
//!   **Suspending…** or **Resuming…** from the pushed phase until it settles ([`pending`]), and the commands of every
//!   session on that host, Delete and Clone included, stay refused while that phase shows the provider's change running
//!   ([`held`]), as the old claim lasted for the whole change. Any other refusal stays on the session's key and shows on
//!   the row and the header as a lifecycle refusal does (`super::outcome`, "Sandbox: " or "Stop: " before the helm's
//!   words). The changing phase is `HostStateView::TensorlakeChanging` for every provider, despite its name: a
//!   container's pause and unpause go through the helm's shared `environment::lifecycle::perform`, which pushes it while
//!   the change runs, just as a sandbox's does.
//!
//! NOTE: A container host first checks its container over SSH, before the helm pushes the changing phase. When that
//! check alone outlasts the 20-second bound, the 504 arrives before any changing phase, and the session looks idle (its
//! commands no longer held) until the push; the old app waited on the request itself and had no such gap. The helm's
//! lifecycle lock still refuses a second Suspend or Resume meanwhile as busy.
//!
//! NOTE: Waiting on the launcher's environment work, and not drawn: refusing Suspend and Resume while this window's own
//! launch targets or is waking the same host (SPEC.md, "Creation"; the old `LAUNCH_ON_THIS_HOST`, "A launch on this host
//! is in progress; try again once it finishes"), since the launcher exposes no predicate for its pending target yet;
//! meanwhile the helm's lifecycle lock refuses a collision as busy. And Clone's environment wording ("New session in a
//! new environment, keep this one"), which would describe a Clone into a fresh environment the launcher does not make
//! yet. The environment Delete, its being-deleted badge and the bulk parts come with the environment Delete's piece.

use std::collections::HashSet;

use farhelm_helm_client::{ApiPath, Collections, Method, PathError};
use farhelm_proto::EnvironmentHostView;
use farhelm_proto::ui_channel::{HostStateView, HostView, SessionList, SessionRow};
use farhelm_proto::{SessionStatus, SessionStopRequest};
use gpui::{AnyWindowHandle, App, BorrowAppContext as _, Global, Window};
use serde_json::json;

use super::menu;
use super::status::HostLookup;
use crate::components::{HoverHelp, Tier};
use crate::environment_ui::{ProviderUi, target_label};
use crate::operations::{self, Command, OpKey, Outcome, Settled, Work};
use crate::prompt::{Answer, Question};
use crate::session::{self, PerSession};

/// The badge's and the details bar's hover help for a suspended environment, the old `.status-badge`'s tooltip.
pub(super) const SUSPENDED_HELP: &str =
    "Suspended: this session's environment is frozen; Resume continues its processes";

/// The badge's word.
pub(super) const SUSPENDED: &str = "Suspended";

/// The Stop question's extra answer's help, and its consequence's addition after the provider's Stop help: the old row
/// and header Stop questions' words.
const STOP_AND_SUSPEND_HELP: &str =
    "Stop this session's agent and close its terminal tabs, then suspend the whole environment";
const STOP_AND_SUSPEND_NOTE: &str = "Stop and suspend freezes the remaining processes; they continue on Resume. The \
                                     stopped agent and closed tabs do not continue.";

/// The status the helm answers with when the host reply bound passed while the work goes on
/// (`within_host_reply_bound_saying` in the helm's sessions module): not a failure here (the module docs). It is not the
/// only 504: a supervisor's own timeout inside the bound (Stop and suspend's Stop half, say) answers it too, and the
/// helm marks neither, so such a refusal is dropped as well and only the session's pushed state shows what happened.
const CARRIES_ON: u16 = 504;

// ---- What a session's environment is --------------------------------------------------------------------------------

/// A session's own environment: its host as the registry lists it, the environment the helm describes on it, and its
/// provider's words.
#[derive(Clone, Copy)]
pub(crate) struct Environment<'a> {
    pub(crate) host: &'a HostView,
    described: &'a EnvironmentHostView,
    pub(crate) ui: ProviderUi,
}

impl<'a> Environment<'a> {
    /// `row`'s environment, or `None` for a session on an ordinary host, one the registry does not list, and a retired
    /// containerized session (the module docs' first rule).
    pub(crate) fn of(row: &SessionRow, collections: &'a Collections) -> Option<Environment<'a>> {
        if super::retired::is(row) {
            return None;
        }
        Environment::of_host(collections.get::<HostView>(&HostView::key(row.host))?)
    }

    /// `host`'s environment, when it is a session-owned host the helm describes one for.
    pub(crate) fn of_host(host: &'a HostView) -> Option<Environment<'a>> {
        let described = host
            .environment
            .as_ref()
            .filter(|_| host.is_session_owned)?;
        Some(Environment {
            host,
            described,
            ui: ProviderUi(described.target.provider()),
        })
    }

    /// The name the session's host goes by in its row's last line, its details bar and its menu's header: "<recipe>
    /// sandbox" or "<recipe> container on <place host>" ([`target_label`]). Raw host text, for the caller to escape.
    pub(crate) fn label(self) -> String {
        target_label(&self.described.target, self.described.place_name.as_deref())
    }

    /// Whether the environment is suspended, stopped, changing or being deleted, so that the supervisor's lifecycle
    /// verbs (Stop, Restart, Replace and Replace with) are withheld: the old `frozen`.
    pub(crate) fn frozen(self) -> bool {
        match self.host.state {
            HostStateView::Suspended { .. }
            | HostStateView::Stopped { .. }
            | HostStateView::TensorlakeChanging { .. }
            | HostStateView::Deleting { .. } => true,
            HostStateView::Connecting { .. }
            | HostStateView::Connected { .. }
            | HostStateView::Unreachable { .. }
            | HostStateView::VersionSkew { .. }
            | HostStateView::IdentityMismatch { .. }
            | HostStateView::IdentityUnverified { .. }
            | HostStateView::Duplicate { .. }
            | HostStateView::Retired { .. } => false,
        }
    }

    /// Whether the environment is stopped (a container found stopped), so its Resume starts it again rather than
    /// continuing its processes ([`ProviderUi::resume_help`]).
    pub(crate) fn stopped(self) -> bool {
        matches!(self.host.state, HostStateView::Stopped { .. })
    }

    /// Whether the row draws the Suspended badge: suspended alone, never stopped (SPEC.md: "Stopped has its own state
    /// and is never labelled Suspended").
    pub(crate) fn suspended(self) -> bool {
        matches!(self.host.state, HostStateView::Suspended { .. })
    }

    /// Whether the control reads Resume rather than Suspend: suspended, or stopped and not waiting for its place (the
    /// old `Controls`' `suspended`). Its row offers no Stop and suspend then.
    pub(crate) fn resumable(self) -> bool {
        matches!(
            self.host.state,
            HostStateView::Suspended { .. }
                | HostStateView::Stopped {
                    waiting_for_place: false,
                    ..
                }
        )
    }

    /// Whether the environment is between states, so neither Suspend nor Resume is offered: the provider is changing it
    /// (SPEC.md: "Neither transition offers a competing lifecycle action"), it is stopped and waiting for its place, or
    /// it is being deleted.
    pub(super) fn changing(self) -> bool {
        matches!(
            self.host.state,
            HostStateView::TensorlakeChanging { .. }
                | HostStateView::Stopped {
                    waiting_for_place: true,
                    ..
                }
                | HostStateView::Deleting { .. }
        )
    }
}

/// Whether one listed session's status leaves a Suspend to ask first: a stale status or an unknown or running one says
/// nothing about whether its agent is working now. The old `suspend_status_needs_confirmation`, which Hosts' own Suspend
/// shares (SPEC.md: "Reuse the ordinary host Suspend activity check"); a caller also requires a connected host and a
/// complete listing, since an empty listing of an offline host has no rows to check.
pub(super) fn needs_confirmation(status: &SessionStatus, stale: bool) -> bool {
    stale
        || match status {
            SessionStatus::Unknown | SessionStatus::Running => true,
            SessionStatus::Waiting
            | SessionStatus::Idle
            | SessionStatus::Exited { .. }
            | SessionStatus::Error { .. }
            | SessionStatus::Interrupted => false,
        }
}

// ---- What the controls show ------------------------------------------------------------------------------------------

/// What an environment's control says while its request or the change it asked for is under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    Suspending,
    Resuming,
    /// A Stop and suspend before its reply: the Stop half runs first, so it reads as a Stop.
    Stopping,
}

impl Pending {
    /// The words beside the spinner.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Pending::Suspending => "Suspending…",
            Pending::Resuming => "Resuming…",
            Pending::Stopping => super::stop::PROGRESS_LABEL,
        }
    }
}

/// What `key`'s row and header show as pending for its environment `environment`: this window's request while it is in
/// flight on the session's key, then the pushed phase while the provider changes the environment, which is what a 504
/// at the bound leaves to show (the module docs). `None` while nothing is under way.
pub(crate) fn pending(key: &str, environment: Environment, cx: &App) -> Option<Pending> {
    let work = session::current_operations(cx)
        .and_then(|operations| operations.read(cx).in_flight(&OpKey::session(key)));
    match work {
        Some(Work::Suspend) => return Some(Pending::Suspending),
        Some(Work::Resume) => return Some(Pending::Resuming),
        Some(Work::StopAndSuspend) => return Some(Pending::Stopping),
        _ => {}
    }
    match &environment.host.state {
        HostStateView::TensorlakeChanging { direction, .. } => Some(match direction {
            farhelm_proto::tensorlake::LifecycleDirection::Suspending => Pending::Suspending,
            farhelm_proto::tensorlake::LifecycleDirection::Resuming => Pending::Resuming,
        }),
        _ => None,
    }
}

/// [`pending`] for the row the current UI session's store holds under `key`, with its environment looked up; `None`
/// for a session that is not an environment session. What the list's and the header's spinners ask.
pub(crate) fn pending_of(key: &str, cx: &App) -> Option<Pending> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    let row = collections.get::<SessionRow>(key)?;
    pending(key, Environment::of(row, collections)?, cx)
}

/// Whether `key`'s lifecycle commands are held by its environment beyond its own request in flight (the module docs'
/// fourth and fifth rules): its activity read is out, or its environment's pushed phase shows the provider's change
/// still running, which holds every session on that host whichever session asked. `super::menu::refused` reads it
/// beside the session's own request, so every surface that refuses on a pending request refuses on this too.
pub(super) fn held(key: &str, cx: &App) -> bool {
    if reading(key, cx) {
        return true;
    }
    let Some(store) = session::current_store(cx) else {
        return false;
    };
    let collections = store.read(cx).collections();
    collections
        .get::<SessionRow>(key)
        .and_then(|row| Environment::of(row, collections))
        .is_some_and(|environment| {
            matches!(
                environment.host.state,
                HostStateView::TensorlakeChanging { .. }
            )
        })
}

/// Whether `key`'s environment control is unavailable now: the session's commands are refused ([`menu::refused`]), or
/// the environment is between states or has something pending.
pub(crate) fn unavailable(key: &str, environment: Environment, cx: &App) -> bool {
    menu::refused(key, cx) || environment.changing() || pending(key, environment, cx).is_some()
}

// ---- Asking and sending ---------------------------------------------------------------------------------------------

/// What the user asked for, frozen at the click (the module docs' third rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verb {
    Suspend,
    Resume,
    StopAndSuspend,
}

/// One environment request as the click froze it: the session it was asked from, its host, the verb, and Stop's
/// preconditions from the clicked row (used by Stop and suspend alone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Asked {
    pub(crate) key: String,
    pub(crate) host: i64,
    pub(crate) verb: Verb,
    pub(crate) guard: SessionStopRequest,
}

/// The user asked `key`'s environment for Stop and suspend (`stop`) or for its toggle, Suspend or Resume as the phase
/// reads now. Resume goes at once; Suspend and Stop and suspend read the host's activity first and then either go
/// ahead or hand the frozen request to `ask` inside `window`, which opens the question [`question`] builds, beside
/// whatever the user pressed. Nothing happens for a session that is not an environment session, while its commands are
/// refused, or while its control is unavailable.
pub(crate) fn begin(
    key: &str,
    stop: bool,
    window: AnyWindowHandle,
    cx: &mut App,
    ask: impl FnOnce(Asked, ProviderUi, &mut Window, &mut App) + 'static,
) {
    let Some(target) = menu::actionable(key, cx) else {
        return;
    };
    let Some(store) = session::current_store(cx) else {
        return;
    };
    let collections = store.read(cx).collections();
    let Some(environment) = Environment::of(&target.row, collections) else {
        return;
    };
    if unavailable(key, environment, cx) || (stop && environment.resumable()) {
        return;
    }
    let verb = if stop {
        Verb::StopAndSuspend
    } else if environment.resumable() {
        Verb::Resume
    } else {
        Verb::Suspend
    };
    let (ui, host) = (environment.ui, environment.host.id);
    let asked = Asked {
        key: key.to_owned(),
        host,
        verb,
        guard: super::stop::guard(&target.row),
    };
    if verb == Verb::Resume {
        send(asked, cx);
        return;
    }
    // Stop and suspend also asks wherever an ordinary Stop of the clicked row would.
    let stop_asks = stop && super::stop::asks(&target.row, target.skips_questions);
    let Ok(path) = ApiPath::api(&["sessions"])
        .and_then(|path| path.with_query(&[("host", host.to_string().as_str())]))
    else {
        return;
    };
    set_reading(key, true, cx);
    let reading = key.to_owned();
    let sent =
        operations::read::<SessionList>(path, "GET /api/sessions", cx, move |listing, cx| {
            // Cleared here, outside the window, so a window closed meanwhile cannot leave the session's commands
            // refused.
            set_reading(&reading, false, cx);
            let asks = stop_asks || activity_asks(listing.ok().as_ref(), host, cx);
            let _ = window.update(cx, move |_, window, cx| {
                // Nothing else may redraw if the question does not open (the user moved on), and the control must not
                // stay drawn as reading.
                window.refresh();
                if asks {
                    ask(asked, ui, window, cx);
                } else {
                    send(asked, cx);
                }
            });
        });
    if !sent {
        set_reading(key, false, cx);
    }
}

/// Whether the activity `listing` of host `host` leaves Suspend to ask first (the module docs' contract): unless the
/// host reads connected in the store now, after the read, and the listing was read whole with every status conclusive
/// (the old `suspend_asks`). A failed read (`None`) asks too: a retained snapshot is no evidence.
fn activity_asks(listing: Option<&SessionList>, host: i64, cx: &App) -> bool {
    let connected = session::current_store(cx).is_some_and(|store| {
        matches!(
            HostLookup::of(store.read(cx).collections(), host),
            HostLookup::Known(host) if matches!(host.state, HostStateView::Connected { .. })
        )
    });
    !connected
        || !listing.is_some_and(|listing| {
            !listing.truncated
                && listing
                    .sessions
                    .iter()
                    .all(|row| !needs_confirmation(&row.info.status, row.stale))
        })
}

/// The Suspend or Stop and suspend question for `asked`, in `ui`'s words: titled "Suspend sandbox?" or "Stop and
/// suspend sandbox?", its line the question and the provider's Suspend help, its answer in the danger tier, and Cancel
/// saying what it keeps running (the old `Controls`' question).
pub(crate) fn question(asked: &Asked, ui: ProviderUi) -> Question<Asked> {
    let stop = asked.verb == Verb::StopAndSuspend;
    Question::new(
        ui.suspend_title(stop),
        Answer::new(
            if stop { "Stop and suspend" } else { "Suspend" },
            HoverHelp::tooltip(ui.suspend_action(stop)),
            Tier::Danger,
        ),
        asked.clone(),
    )
    .line(format!(
        "{}{}",
        ui.suspend_question(stop),
        ui.suspend_help()
    ))
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip(ui.suspend_cancel(stop)),
        Tier::Neutral,
    ))
    .keeps_focus_on_empty_click()
}

/// An environment question was confirmed: send the request it froze.
pub(crate) fn confirm(asked: Asked, _: &mut Window, cx: &mut App) {
    send(asked, cx);
}

/// What an environment session's Stop question adds after the ordinary consequence, in `ui`'s words: what plain Stop
/// leaves running, and what Stop and suspend would do instead (the old row and header Stop questions).
pub(super) fn stop_note(ui: ProviderUi) -> String {
    format!("{} {STOP_AND_SUSPEND_NOTE}", ui.stop_help())
}

/// The Stop question's extra answer for an environment session: "Stop and suspend", with the old answer's help
/// (`super::stop::question` sends [`stop_and_suspend`] from it).
pub(super) fn stop_and_suspend_answer() -> Answer {
    Answer::new(
        "Stop and suspend",
        HoverHelp::tooltip(STOP_AND_SUSPEND_HELP),
        Tier::Danger,
    )
}

/// Send `key`'s Stop and suspend of its environment on host `host` with `guard`, the preconditions its Stop question
/// froze: at once, with no activity read, since the user has just answered a question that said what it suspends, as
/// the old answer went.
pub(super) fn stop_and_suspend(key: &str, host: i64, guard: SessionStopRequest, cx: &mut App) {
    send(
        Asked {
            key: key.to_owned(),
            host,
            verb: Verb::StopAndSuspend,
            guard,
        },
        cx,
    );
}

/// Resume `host` from `key`'s suspended card, with no question, as Resume always goes: for an environment session's own
/// host and for an ordinary host the card offers to resume alike.
pub(crate) fn resume(key: &str, host: i64, cx: &mut App) {
    send(
        Asked {
            key: key.to_owned(),
            host,
            verb: Verb::Resume,
            guard: SessionStopRequest::default(),
        },
        cx,
    );
}

/// The command `asked` sends: `POST /api/hosts/{id}/suspend` or `/resume`, or `POST /api/sessions/{id}/stop-and-suspend`
/// with both of Stop's preconditions spelled out, as an ordinary Stop sends them.
pub(super) fn command(asked: &Asked) -> Result<Command, PathError> {
    let host = asked.host.to_string();
    Ok(match asked.verb {
        Verb::Suspend => Command::new(
            Work::Suspend,
            Method::Post,
            ApiPath::api(&["hosts", &host, "suspend"])?,
            None,
            "POST /api/hosts/[host]/suspend",
        ),
        Verb::Resume => Command::new(
            Work::Resume,
            Method::Post,
            ApiPath::api(&["hosts", &host, "resume"])?,
            None,
            "POST /api/hosts/[host]/resume",
        ),
        Verb::StopAndSuspend => Command::new(
            Work::StopAndSuspend,
            Method::Post,
            ApiPath::api(&["sessions", &asked.key, "stop-and-suspend"])?,
            // Two booleans always encode, so `json!` cannot fail here.
            Some(json!(asked.guard)),
            "POST /api/sessions/[session]/stop-and-suspend",
        ),
    })
}

/// Send `asked` through the operation store on its session's key. A session with a request in flight sends nothing.
fn send(asked: Asked, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let Ok(command) = command(&asked) else {
        tracing::warn!("an environment request whose ids cannot be path segments was not sent");
        return;
    };
    let key = asked.key;
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::session(&key), command, cx, move |settled, cx| {
            settle(&key, settled, cx)
        })
    });
    if let Err(refused) = started {
        tracing::debug!(?refused, "an environment request was not sent");
    }
}

/// `key`'s environment request settled. A 504 at the helm's bound says the work carries on, so its record is dropped
/// rather than shown as the session's refusal: the pushed phase shows how it goes on ([`pending`]). Every other refusal
/// stays the operation store's, on the session's key, which the row, the header and the card show.
fn settle(key: &str, settled: &Settled, cx: &mut App) {
    let Outcome::Refused(refusal) = &settled.outcome else {
        return;
    };
    if refusal.status != CARRIES_ON {
        return;
    }
    if let Some(operations) = session::current_operations(cx) {
        operations.update(cx, |operations, cx| {
            operations.forget(&OpKey::session(key), cx)
        });
    }
}

// ---- The activity read in flight -------------------------------------------------------------------------------------

/// The sessions whose activity read before a Suspend is out, under the current UI session: the old session claim was
/// held through the read, so while it is out the session's other commands are refused too ([`menu::refused`]) and a
/// second press reads nothing more. Kept app-wide like [`super::delete`]'s record of this window's Deletes.
#[derive(Default)]
struct Reading(PerSession<HashSet<String>>);

impl Global for Reading {}

/// Whether `key`'s activity read is out (see [`Reading`]).
fn reading(key: &str, cx: &App) -> bool {
    cx.try_global::<Reading>()
        .and_then(|reading| reading.0.read(cx))
        .is_some_and(|keys| keys.contains(key))
}

/// Record that `key`'s activity read went out (`on`) or answered.
fn set_reading(key: &str, on: bool, cx: &mut App) {
    cx.update_default_global::<Reading, _>(|reading, cx| {
        let keys = reading.0.current(cx);
        if on {
            keys.insert(key.to_owned());
        } else {
            keys.remove(key);
        }
    });
}
