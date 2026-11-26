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
//! - **Being deleted refuses everything** ([`Environment::deleting`]). While the helm pushes the host as Deleting (its
//!   Farhelm sealed it and the helm is destroying it, or a whole-environment Delete runs), or as Deleting failed until
//!   a Retry in Hosts, every command of each of its sessions is refused, Delete and Clone included: nothing can reach a
//!   sealed supervisor, and the destruction under way is the only way forward (SPEC.md, "Sandbox sessions": "its
//!   sessions show as being deleted with their actions unavailable"). The refusal goes through [`held`], so the row
//!   menu, the header, the Delete shortcut and bulk admission all refuse through `super::menu::refused` as for any
//!   other hold. In place of the stale cause the row and the details bar show the host's phase words ("Deleting…" or
//!   "Deleting failed", the Hosts panel's) with the provider's reason as their help, and the row menu's Clone and
//!   Delete and the header's Stop, Clone and Delete give that reason ([`Environment::deleting`]), as does the line a
//!   bulk action leaves on such a session, which it leaves out of its question and its batch ([`deleting_reason`]):
//!   the old `being_deleted` and `deleting_reason`.
//! - **The verb is frozen at the click** ([`Asked`]): Suspend, Resume or Stop and suspend, and Stop's two preconditions
//!   as the clicked row showed them. The activity read and the question can outlast the click, and an answer given
//!   after the phase changed (another viewer resumed it, say) still sends the captured verb, never a toggle of the new
//!   phase.
//! - **Resume never asks. Suspend asks unless the activity is known** ([`activity_asks`]), the ordinary host Suspend
//!   activity check (SPEC.md, "Topology"). Its contract, which a caller must not loosen: the listing is a fresh
//!   `GET /api/sessions?host=<id>`, never the pushed list, which the window's query filters and the listing cap cuts
//!   (a Suspend authorized from it could freeze an agent the list never showed); it counts only when read whole, its
//!   `truncated` flag clear; the host must read connected with a successful refresh after the read answers;
//!   every listed status must be conclusive, so a stale, unknown or running one asks ([`needs_confirmation`]); and a
//!   read that fails asks too. Stop and suspend also asks when an ordinary Stop of the row would
//!   ([`super::stop::asks`]), so with confirmations skipped it still asks when the activity says so: SPEC.md keeps the
//!   host-wide activity confirmation for it.
//! - **One request, on the session's key**, as the old per-session claim held it: [`Work::Suspend`], [`Work::Resume`]
//!   or [`Work::StopAndSuspend`]. While it is in flight the clicked session's other lifecycle commands are refused (a
//!   pending Resume refuses Delete), as `super::menu::refused` refuses on any request of the session; other sessions on
//!   the same host are not held until the provider's change is pushed (next rule), as the old claim held only its own
//!   session. The activity read holds the clicked session too ([`reading`]), as the old claim was held through it.
//!   The Hosts panel's Suspend and Resume (`crate::hosts::lifecycle`) run on the host's key instead, as the old panel's
//!   own per-host hold did, and share only the activity read ([`read_activity`]) and the commands
//!   ([`suspend_command`], [`resume_command`]) with these: neither surface waits for the other's request, which the
//!   pushed phase and the helm's lifecycle lock coordinate, as they did in the old app.
//! - **The 20-second bound and the pushed phase.** A Suspend or Resume 504 is dropped rather than shown as a failure:
//!   the helm answers it at its host reply bound while the change goes on, and the pushed phase shows how the change
//!   goes. For Stop and suspend, only the helm's marked 504 means the outcome is unknown. Its notice can show during
//!   Stop, before the host phase moves, and clears on Suspended, Deleting or another session action. A supervisor
//!   timeout remains a failure ([`settle`]). The row and header show **Suspending…** or **Resuming…** from the pushed
//!   phase until it settles ([`pending`]), and the commands of every session on that host, Delete and Clone included,
//!   stay refused while that phase shows the provider's change running
//!   ([`held`]), as the old claim lasted for the whole change. Any other refusal stays on the session's key and shows on
//!   the row and the header as a lifecycle refusal does (`super::outcome`, with the action name before the helm's
//!   words). The changing phase is `HostStateView::TensorlakeChanging` for every provider, despite its name: a
//!   container's pause and unpause go through the helm's shared `environment::lifecycle::perform`, just as a sandbox's
//!   do. The phase can arrive before the reply and hold other sessions even while the initiating session's request is
//!   still out.
//!
//! NOTE: A container host first checks its container over SSH, before the helm pushes the changing phase. When that
//! check alone outlasts the 20-second bound, the 504 arrives before any changing phase, and the session looks idle (its
//! commands no longer held) until the push; the old app waited on the request itself and had no such gap. The helm's
//! lifecycle lock still refuses a second Suspend or Resume meanwhile as busy.
//!
//! Suspend, Resume and Stop and suspend wait while this window's own launch targets the same host (SPEC.md,
//! "Creation"). The launcher can claim the target while an activity read or question is open, so the request checks
//! again when it sends. The environment Delete is
//! [`super::environment_delete`]'s, and its bulk form `super::bulk`'s.
//!
//! A failed deletion's reason ends "Retry in Hosts finishes the deletion", the old words: the Hosts panel's menu offers
//! that Retry (`crate::hosts`'s remedies). Nothing here offers Retry, as the old row did not.

use std::collections::HashSet;

use farhelm_helm_client::{ApiPath, Collections, Method, PathError};
use farhelm_proto::EnvironmentHostView;
use farhelm_proto::ui_channel::{
    HelmMarker, HostStateView, HostView, RefreshView, SessionList, SessionRow,
};
use farhelm_proto::{SessionStatus, SessionStopRequest};
use gpui::{AnyWindowHandle, App, BorrowAppContext as _, Global, Window};
use serde_json::json;

use super::status::HostLookup;
use super::{menu, outcome};
use crate::components::{HoverHelp, Tier};
use crate::environment_ui::{ProviderUi, StopTarget, target_label};
use crate::hosts::identity::phase;
use crate::launcher::launch::host_lifecycle_refusal;
use crate::operations::{self, Command, NotStarted, OpKey, Outcome, Settled, Work};
use crate::prompt::{Answer, Question};
use crate::session::{self, PerSession};

/// The badge's and the details bar's hover help for a suspended environment, the old `.status-badge`'s tooltip.
pub(super) const SUSPENDED_HELP: &str =
    "Suspended: this session's environment is frozen; Resume continues its processes";

/// The badge's word.
pub(super) const SUSPENDED: &str = "Suspended";

/// The HTTP gateway timeout status. Suspend, Resume and environment Delete treat a 504 as work that carries on past the
/// helm's host reply bound (`within_host_reply_bound_saying` in the helm's sessions module), here, in the environment
/// Delete (`super::environment_delete`) and in the Hosts panel's Suspend and Resume (`crate::hosts::lifecycle`). Stop
/// and suspend requires the helm's reply-bound marker as well, because a supervisor's own Stop timeout also answers
/// 504 and is a failure.
pub(crate) const CARRIES_ON: u16 = 504;

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

    /// The environment's own name, the sandbox's or the container's as its provider knows it: raw host text, empty when
    /// the helm has none. The Delete questions name the resource by it apart from the session's title
    /// (`super::environment_delete`).
    pub(crate) fn name(self) -> &'a str {
        &self.described.name
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
            | HostStateView::SignInNeeded
            | HostStateView::VersionSkew { .. }
            | HostStateView::IdentityMismatch { .. }
            | HostStateView::OwnedElsewhere { .. }
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
        crate::hosts::offers::resumable(&self.host.state)
    }

    /// An edited restart or replacement can wake a resumable host before it talks to the supervisor. An environment
    /// changing state, waiting for its container host, or being deleted cannot prepare a new launch.
    pub(crate) fn permits_launch_preparation(self) -> bool {
        !self.frozen() || self.resumable()
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

    /// What its sessions show while the environment is being deleted (the module docs' "Being deleted" rule), `None`
    /// in every other phase: the old `being_deleted`, which a failed deletion keeps until a Retry succeeds.
    pub(crate) fn deleting(self) -> Option<Deleting> {
        let HostStateView::Deleting { failure } = &self.host.state else {
            return None;
        };
        Some(Deleting {
            label: phase::words(&self.host.state),
            help: self.ui.session_deleting_help(failure.is_some()),
        })
    }
}

/// What a session shows in place of its stale cause while its environment is being deleted ([`Environment::deleting`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Deleting {
    /// The host's phase words, "Deleting…" or "Deleting failed": the Hosts panel's (`hosts::identity::phase::words`),
    /// so the row and the panel never name the phase differently, as the old badge took the panel's label.
    pub(crate) label: &'static str,
    /// Why every action of the session is unavailable, in the provider's words
    /// ([`ProviderUi::session_deleting_help`]): the badge's help, and the reason the refused controls give.
    pub(crate) help: &'static str,
}

/// Why `key`'s commands are refused because its environment is being deleted ([`Deleting::help`]), or `None` for a
/// session whose environment is not, or that is no environment session: [`Environment::deleting`] for a caller that
/// has only the key (bulk's skipped-row line and its leaving such a session out). Read from the current UI session's
/// store now, as [`held`] reads it.
pub(crate) fn deleting_reason(key: &str, cx: &App) -> Option<&'static str> {
    of_key(key, cx)?.deleting().map(|deleting| deleting.help)
}

/// The environment of the session the current UI session's store holds under `key`, or `None` for a key with no row
/// and a session that is not an environment session ([`Environment::of`]).
pub(crate) fn of_key<'a>(key: &str, cx: &'a App) -> Option<Environment<'a>> {
    let collections = session::current_store(cx)?.read(cx).collections();
    Environment::of(collections.get::<SessionRow>(key)?, collections)
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
            Pending::Suspending => {
                phase::changing_words(farhelm_proto::tensorlake::LifecycleDirection::Suspending)
            }
            Pending::Resuming => {
                phase::changing_words(farhelm_proto::tensorlake::LifecycleDirection::Resuming)
            }
            Pending::Stopping => super::stop::PROGRESS_LABEL,
        }
    }
}

/// What `key`'s row and header show as pending for its environment `environment`: this window's request while it is in
/// flight on the session's key, then the pushed phase while the provider changes the environment, which is what a 504
/// at the bound leaves to show (the module docs). `None` while nothing is under way.
pub(crate) fn pending(key: &str, environment: Environment, cx: &App) -> Option<Pending> {
    match outcome::in_flight(key, cx) {
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

/// Whether this window has a Suspend in flight for the host that session `key` runs on: a Suspend or Stop and suspend
/// on the session's key, or the Hosts panel's Suspend on the host's key. The session pane holds back the reconnect card
/// meanwhile. No provider is compared: every provider's Suspend ends the terminal's stream.
pub(crate) fn suspend_in_flight(key: &str, cx: &App) -> bool {
    if matches!(
        outcome::in_flight(key, cx),
        Some(Work::Suspend | Work::StopAndSuspend)
    ) {
        return true;
    }
    let Some(operations) = session::current_operations(cx) else {
        return false;
    };
    let operations = operations.read(cx);
    let host = session::current_store(cx).and_then(|store| {
        store
            .read(cx)
            .collections()
            .get::<SessionRow>(key)
            .map(|row| row.host)
    });
    host.is_some_and(|host| {
        operations.in_flight(&OpKey::host(&HostView::key(host))) == Some(Work::Suspend)
    })
}

/// [`pending`] for the row the current UI session's store holds under `key`, with its environment looked up; `None`
/// for a session that is not an environment session. What the list's and the header's spinners ask.
pub(crate) fn pending_of(key: &str, cx: &App) -> Option<Pending> {
    pending(key, of_key(key, cx)?, cx)
}

/// Whether `key`'s lifecycle commands are held by its environment beyond its own request in flight (the module docs'
/// "Being deleted" rule and its rules on the request and the bound): its activity read is out, or its environment's
/// pushed phase shows the provider's change still running or the environment being deleted, either of which holds
/// every session on that host whichever session asked. `super::menu::refused` reads it beside the session's own
/// request, so every surface that refuses on a pending request refuses on this too.
pub(super) fn held(key: &str, cx: &App) -> bool {
    if reading(key, cx) {
        return true;
    }
    of_key(key, cx).is_some_and(|environment| {
        matches!(
            environment.host.state,
            HostStateView::TensorlakeChanging { .. } | HostStateView::Deleting { .. }
        )
    })
}

/// Whether `key`'s environment control is unavailable now: the session's commands are refused ([`menu::refused`]), or
/// the environment is between states or has something pending, or a launch targets its host.
pub(crate) fn unavailable(key: &str, environment: Environment, cx: &App) -> bool {
    menu::refused(key, cx)
        || environment.changing()
        || pending(key, environment, cx).is_some()
        || host_lifecycle_refusal(environment.host.id, cx).is_some()
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
    /// The Stop and suspend confirmation and its guards use the same clicked row.
    stops: Option<StopTarget>,
}

/// Keep row-menu and confirmation wording aligned with the row state that supplies Stop's guards.
pub(super) fn stop_target(row: &SessionRow) -> StopTarget {
    match (row.info.status.has_ended(), row.info.tabs.is_empty()) {
        (false, _) => StopTarget::Program(super::stop::program_word(row)),
        (true, false) => StopTarget::Tabs,
        (true, true) => StopTarget::EndedWithoutTabs,
    }
}

/// The part of an environment's state that decides which command its controls offer. Keep the provider and phase
/// with the session's action data so a late press cannot reverse the toggle or act on another environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shown {
    pub(crate) host: i64,
    pub(crate) ui: ProviderUi,
    resumable: bool,
    stopped: bool,
}

impl Environment<'_> {
    /// The command offered by the environment's phase, independent of whether a pending operation holds the control.
    pub(super) fn shown(self) -> Shown {
        Shown {
            host: self.host.id,
            ui: self.ui,
            resumable: self.resumable(),
            stopped: self.stopped(),
        }
    }
}

/// Suspend or Resume the environment as the activated control showed it. A changed phase can refuse the operation.
/// Resume goes at once. Suspend and Stop and suspend still read live host activity, which can require an additional
/// warning and leaves the original Stop guards intact.
pub(crate) fn begin_shown(
    target: &menu::Actionable,
    stop: bool,
    window: AnyWindowHandle,
    cx: &mut App,
    ask: impl FnOnce(Asked, ProviderUi, &mut Window, &mut App) + 'static,
) {
    let Some(shown) = target.environment else {
        return;
    };
    let key = &target.key;
    if !target.allows(cx, |_| true)
        || (stop && shown.resumable)
        || !of_key(key, cx).is_some_and(|now| now.shown() == shown && !unavailable(key, now, cx))
    {
        return;
    }
    let verb = if stop {
        Verb::StopAndSuspend
    } else if shown.resumable {
        Verb::Resume
    } else {
        Verb::Suspend
    };
    let (ui, host) = (shown.ui, shown.host);
    let asked = Asked {
        key: key.clone(),
        host,
        verb,
        guard: super::stop::guard(&target.row),
        stops: stop.then(|| stop_target(&target.row)),
    };
    if verb == Verb::Resume {
        send(asked, cx);
        return;
    }
    // Stop and suspend also asks wherever an ordinary Stop of the clicked row would.
    let stop_asks = stop && super::stop::asks(&target.row, target.skips_questions);
    set_reading(key, true, cx);
    let reading = key.clone();
    let sent = read_activity(host, cx, move |activity_says_ask, cx| {
        // Cleared here, outside the window, so a window closed meanwhile cannot leave the session's commands
        // refused.
        set_reading(&reading, false, cx);
        let asks = stop_asks || activity_says_ask;
        let _ = window.update(cx, move |_, window, cx| {
            // Nothing else may redraw if the question does not open (the user moved on), and the control must not
            // stay drawn as reading.
            window.refresh();
            if host_lifecycle_refusal(asked.host, cx).is_some() {
                return;
            }
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

/// Read host `host`'s activity for a Suspend and tell `then` whether the Suspend asks first: the module docs' activity
/// rule, whose contract a caller must not loosen. The listing is a fresh `GET /api/sessions?host=<id>`, never the
/// pushed list. It counts only when read whole, with conclusive statuses and a host still connected with a successful
/// refresh when the read answers. Returns whether the read was sent. When it was not (no
/// UI session, an id that cannot be a query), `then` is never called, and the caller asks nothing. The Hosts panel's
/// Suspend reads through this too (`crate::hosts::lifecycle`), so the session's and the host's Suspend ask alike.
/// This helm-local read can answer while an ordered command waits on another host.
pub(crate) fn read_activity(
    host: i64,
    cx: &mut App,
    then: impl FnOnce(bool, &mut App) + 'static,
) -> bool {
    let Ok(path) = ApiPath::api(&["sessions"])
        .and_then(|path| path.with_query(&[("host", host.to_string().as_str())]))
    else {
        return false;
    };
    operations::read_unordered::<SessionList>(path, "GET /api/sessions", cx, move |listing, cx| {
        let asks = activity_asks(listing.ok().as_ref(), host, cx);
        then(asks, cx);
    })
}

/// Whether the activity `listing` of host `host` leaves Suspend to ask first (the module docs' contract): unless the
/// host reads connected with a successful refresh after the read, and the listing was read whole with every status
/// conclusive. A failed refresh can leave a complete but old listing, so it must still ask.
fn activity_asks(listing: Option<&SessionList>, host: i64, cx: &App) -> bool {
    let refreshed = session::current_store(cx).is_some_and(|store| {
        matches!(
            HostLookup::of(store.read(cx).collections(), host),
            HostLookup::Known(host) if matches!(
                host.state,
                HostStateView::Connected { refresh: RefreshView::Ok { .. }, .. }
            )
        )
    });
    !refreshed
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
    let host = asked.host;
    Question::new(
        ui.suspend_title(stop),
        Answer::new(
            if stop { "Stop and suspend" } else { "Suspend" },
            HoverHelp::tooltip(ui.suspend_action(asked.stops)),
            Tier::Danger,
        )
        .disabled_while(move |cx| host_lifecycle_refusal(host, cx)),
        asked.clone(),
    )
    .line(format!(
        "{}{}",
        ui.suspend_question(asked.stops),
        ui.suspend_help()
    ))
    .cancel_answer(Answer::new(
        "Cancel",
        HoverHelp::tooltip(ui.suspend_cancel(asked.stops)),
        Tier::Neutral,
    ))
    .keeps_focus_on_empty_click()
}

/// An environment question was confirmed: send the request it froze.
pub(crate) fn confirm(asked: Asked, _: &mut Window, cx: &mut App) {
    send(asked, cx);
}

/// What an environment session's Stop question adds after the ordinary consequence, in `ui`'s words: what plain Stop
/// leaves running, and what Stop and suspend would do instead. An absent program means the question only closes tabs.
pub(super) fn stop_note(ui: ProviderUi, program: Option<&str>) -> String {
    let closed = match program {
        Some(program) => format!("The stopped {program} and closed tabs do not continue."),
        None => "The closed tabs stay closed.".into(),
    };
    format!(
        "{} Stop and suspend freezes the remaining processes; they continue on Resume. {closed}",
        ui.stop_help()
    )
}

/// The Stop question's extra answer uses the same program snapshot as the primary answer. When the program has
/// already exited, the help describes closing the tabs and suspending the environment.
pub(super) fn stop_and_suspend_answer(program: Option<&str>, host: i64) -> Answer {
    let action = match program {
        Some(program) => format!("Stop this session's {program} and close its terminal tabs"),
        None => "Close this session's terminal tabs".into(),
    };
    Answer::new(
        "Stop and suspend",
        HoverHelp::tooltip(format!("{action}, then suspend the whole environment")),
        Tier::Danger,
    )
    .disabled_while(move |cx| host_lifecycle_refusal(host, cx))
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
            stops: None,
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
            stops: None,
        },
        cx,
    );
}

/// The command `asked` sends: `POST /api/hosts/{id}/suspend` or `/resume` ([`suspend_command`], [`resume_command`]),
/// or `POST /api/sessions/{id}/stop-and-suspend` with both of Stop's preconditions spelled out, as an ordinary Stop
/// sends them.
pub(super) fn command(asked: &Asked) -> Result<Command, PathError> {
    Ok(match asked.verb {
        Verb::Suspend => suspend_command(asked.host)?,
        Verb::Resume => resume_command(asked.host)?,
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

/// Host `host`'s Suspend, `POST /api/hosts/{id}/suspend` with no body, as [`Work::Suspend`]: what a session's Suspend
/// sends, and the Hosts panel's too (`crate::hosts::lifecycle`), so the two cannot drift apart.
pub(crate) fn suspend_command(host: i64) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Suspend,
        Method::Post,
        ApiPath::api(&["hosts", &host.to_string(), "suspend"])?,
        None,
        "POST /api/hosts/[host]/suspend",
    ))
}

/// Host `host`'s Resume, `POST /api/hosts/{id}/resume` with no body, as [`Work::Resume`]: what a session's Resume
/// sends, and the Hosts panel's too (`crate::hosts::lifecycle`).
pub(crate) fn resume_command(host: i64) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Resume,
        Method::Post,
        ApiPath::api(&["hosts", &host.to_string(), "resume"])?,
        None,
        "POST /api/hosts/[host]/resume",
    ))
}

/// Send `asked` through the operation store on its session's key. A session with a request in flight sends nothing.
fn send(asked: Asked, cx: &mut App) {
    if host_lifecycle_refusal(asked.host, cx).is_some() {
        return;
    }
    let command = command(&asked);
    match super::start_request(
        &OpKey::session(&asked.key),
        command,
        cx,
        move |settled, cx| {
            settle(&asked, settled, cx);
        },
    ) {
        Ok(()) | Err(NotStarted::Retired) => {}
        Err(NotStarted::Path(_)) => {
            tracing::warn!("an environment request whose ids cannot be path segments was not sent");
        }
        Err(refused) => tracing::debug!(?refused, "an environment request was not sent"),
    }
}

/// The operation store already has the default failure. Preserve an unmarked Stop timeout, and replace a marked
/// Stop and suspend deadline with a notice if the pushed host state has yet to settle it.
fn settle(asked: &Asked, settled: &Settled, cx: &mut App) {
    let Outcome::Refused(refusal) = &settled.outcome else {
        return;
    };
    if refusal.status != CARRIES_ON {
        return;
    }
    let key = OpKey::session(&asked.key);
    let stop_and_suspend = asked.verb == Verb::StopAndSuspend;
    if stop_and_suspend && !refusal.has(HelmMarker::StopAndSuspendReplyBound) {
        return;
    }
    // A host already suspended or deleting has settled the outcome, so no notice. The session list's own pass
    // (`forget_settled_notices`) would clear one recorded here too; checking first keeps it from ever being recorded.
    let show_notice = stop_and_suspend
        && !session::current_store(cx)
            .is_some_and(|store| notice_settled(store.read(cx).collections(), asked.host));
    if let Some(operations) = session::current_operations(cx) {
        operations.update(cx, |operations, cx| {
            if show_notice {
                operations.notice(&key, asked.host, refusal.words.clone(), cx);
            } else {
                operations.forget(&key, cx);
            }
        });
    }
}

/// A suspended or deleting host supersedes the deadline notice. A missing host entry is inconclusive.
fn notice_settled(collections: &Collections, host: i64) -> bool {
    collections
        .get::<HostView>(&HostView::key(host))
        .is_some_and(|host| {
            matches!(
                host.state,
                HostStateView::Suspended { .. } | HostStateView::Deleting { .. }
            )
        })
}

/// Clear the record on pushed host updates so Resume from Hosts cannot reveal an old notice again. The host id
/// kept in the record lets this work even when the list's current query omits the affected session.
pub(super) fn forget_settled_notices(cx: &mut App) {
    let (Some(store), Some(operations)) =
        (session::current_store(cx), session::current_operations(cx))
    else {
        return;
    };
    let collections = store.read(cx).collections();
    let settled: Vec<_> = operations
        .read(cx)
        .host_notices()
        .filter(|(_, host)| notice_settled(collections, *host))
        .map(|(key, _)| key.clone())
        .collect();
    if !settled.is_empty() {
        operations.update(cx, |operations, cx| {
            for key in settled {
                operations.forget(&key, cx);
            }
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
