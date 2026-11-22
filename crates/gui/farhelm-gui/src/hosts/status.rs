//! What the host rail and the attention list read about each host, and the attention list's Always update: the update
//! badge, the work going on, the setup or Update warning, and Always update's failed save with its Try again. This
//! module draws nothing; the sidebar draws the rail's marks and the attention list from it.
//!
//! SPEC.md, "Session list" (the rail's badge, ring and warning mark, the Hosts control's `↑`, the attention list's
//! Always update) and "Topology" (when a warning shows and what clears it). Ported from the old app's
//! `hosts/updates.rs` (`classify`), `hosts/attention.rs` (`HostAttention`) and the Always update parts of
//! `list/view.rs` and `hosts/attention_list.rs`, at reference commit `24bc0580`. Here's the TLDR of its rules:
//!
//! - **The update badge** ([`update_badge`]) is a pure function of the pushed host entry, its retained run and whether
//!   work on it is in flight: the host's Update is available ([`update::available`]: a remote host with nothing
//!   running or in flight on it that does not run a newer Farhelm) and its phase says it runs an older build (optional)
//!   or speaks a lower protocol (required) ([`update::urgency`]). An available Update alone is not a badge: SPEC.md says
//!   having one "does not itself mean the host needs an update". The inline `↑ Update` in Hosts uses the same two
//!   tests, so the badge and the button always agree.
//! - **Live work** ([`live`]) is derived, never stored: a setup, Update or uninstall run going on, with its step count
//!   and current step, or else the host's requests out in the operation store (a confirmed setup's submission, an
//!   Update's plan or confirmation) before any run shows.
//! - **The warning** ([`warned`]) is the one fact this module keeps: a setup or Update that failed, was refused, or
//!   ended without the helm reporting how, while Hosts was closed. It lasts until Hosts opens ([`opened`]) or a later
//!   run of the same action completes. Starting a retry does not acknowledge it, and a completed run of the other
//!   action does not clear it (SPEC.md, "Topology"). Uninstall keeps its own notices and never warns, as in the old
//!   app.
//! - **Always update** ([`always_update`]) asks for the hosts the attention list shows, exactly as its Update all does
//!   ([`update::start_all`]), then turns on "Update remote hosts automatically" under the preference row's key, as the
//!   Settings switch saves it. A failed save leaves the updates running; [`auto_update_failed`] then says so while the
//!   preference still reads off, until a newer choice of that preference: a later Always update, its Try again
//!   ([`retry_auto_update`], which saves only the preference), or a press of the Settings switch
//!   ([`auto_update_chosen`]). A save the operation store refuses because another preference save is in flight is a
//!   failed save too: nothing was saved.
//!
//! ## What is kept, and for how long
//!
//! The warnings, whether Hosts is open, and Always update's choice and failure live in one app global tagged with the
//! entity id of the operation store they were made under. Every read compares that id with the current UI session's
//! store, so a reset forgets all of it, as it forgets SPEC.md's update dismissal, and as the old app's reload did. A
//! global rather than a field of the Hosts panel, because the completions of refused requests ([`failed`]) and of
//! Always update's save have no handle on the panel. The panel's store subscription follows the pushed host progress
//! into it ([`follow`]), whether or not Hosts shows; the sidebar tells the panel when it shows and hides it, which is how
//! this module knows whether Hosts is open.
//!
//! A run already failed when this UI session first sees it warns too (the old app's first read did the same), so after
//! a reset a failure the helm still retains warns again.
//!
//! ## Not ported
//!
//! SPEC.md has an uncertain first Add warn the host that registered for it, if one did ("Session list"); here a new
//! host's setup refused before the helm registered it has no host to warn and shows its existing Add notice, and a
//! first Add whose run then fails warns through the pushed progress like any setup. A destination or installation
//! change has no warning of its own (SPEC.md: one "that leaves an Update outcome uncertain warns that same host
//! entry"): an Update left uncertain around such a change warns only through its own refusal, its run's failure, or
//! its run's withdrawal. The old app's revisions, which kept a late success from clearing a newer failure
//! (`HostAttention`), are not ported: the pushed progress arrives in order, so a completion clears a warning
//! raised before it, even in the one case where that run started before the refused request that raised it (a refusal
//! because the host was busy with that very run).
//!
//! The rule for a run that ends unseen is not exact. The panel's store subscription may hand several pushed changes to
//! one follow, so a run's completion and a new run on the same host arriving together read as the first run ending
//! without a result, which warns. That takes a new run started right after a completion on the same host, which the
//! user rarely does.

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use farhelm_proto::provisioning::{ProvisioningOperation, ProvisioningView, RunStatus};
use farhelm_proto::ui_channel::{Collection, HostView, PREFERENCES_KEY, Preferences};
use gpui::{App, Context, Subscription, Window};

use super::progress;
use super::update;
pub use super::update::Urgency;
use crate::focus::Mark;
use crate::host_text::HostText;
use crate::operations::{NotStarted, OpKey, Outcome, Work};
use crate::session;

/// What the attention list says under its heading after Always update's save failed, while the preference still reads
/// off ([`auto_update_failed`]): the old alert's words, before its Try again.
pub const AUTO_UPDATE_FAILED: &str =
    "Could not turn on automatic updates. Later updates will still wait for you.";

// ---- The update badge ------------------------------------------------------------------------------------------------

/// The rail badge's update test for `host`, given its retained run `progress` and whether work on it is in flight
/// (under `OpKey::host` for its id): its urgency when its Update is available and its phase says it is behind, `None`
/// otherwise. The port of the old `classify`; the attention list's Update rows and the Hosts control's `↑` use it too.
pub fn update_badge(
    host: &HostView,
    progress: Option<&ProvisioningView>,
    in_flight: bool,
) -> Option<Urgency> {
    update::urgency(&host.state).filter(|_| update::available(host, progress, in_flight))
}

// ---- Live work -------------------------------------------------------------------------------------------------------

/// What work is going on on a host, which decides the rail's ring and the attention list's working row and its verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    /// A setup run, or a confirmed setup's submission out ("Setting up"). A rerun's probe is not yet a setup: nothing
    /// was confirmed, as the old app showed setup as working only once confirmed.
    Setup,
    /// An Update run, or its plan or confirming request out ("Updating").
    Update,
    /// An uninstall run ("Uninstalling").
    Uninstall,
}

impl Activity {
    /// The words for this work where they stand alone, the attention list's working rows' state: "Setting up",
    /// "Updating" or "Uninstalling" (SPEC.md, "Session list"). The rail's avatar names it in the same words, lowercased
    /// inside its sentence, so both say the same thing about one host.
    pub fn verb(self) -> &'static str {
        match self {
            Activity::Setup => "Setting up",
            Activity::Update => "Updating",
            Activity::Uninstall => "Uninstalling",
        }
    }
}

/// A run's progress as the attention list's bar and step name show it.
#[derive(Clone, Debug, PartialEq)]
pub struct Steps {
    /// Steps completed, unchanged, warned or failed.
    pub done: usize,
    /// Every step of the run; zero when the helm has not sent them yet, which the bar shows empty.
    pub total: usize,
    /// The running step's name, as host text.
    pub current: Option<HostText>,
}

/// The work going on on a host: what it is, and the run's steps once a run shows (`None` while only requests are out).
#[derive(Clone, Debug, PartialEq)]
pub struct Live {
    /// What the work is.
    pub activity: Activity,
    /// The running run's progress; `None` while only requests are out, which the attention list shows as a bar left
    /// empty and the plain verb.
    pub steps: Option<Steps>,
}

/// The work going on on a host whose retained run is `progress` and whose key has `in_flight` work: the run when one is
/// running, else a confirmed setup's submission or an Update's requests out (SPEC.md: live work "takes precedence over a
/// problem"). `None` when the host is idle. SPEC.md's working rows and rings are setup, Update and uninstall only, so a
/// rerun's probe, a removal and a host settings save are not live work.
pub fn live(progress: Option<&ProvisioningView>, in_flight: Option<Work>) -> Option<Live> {
    let running = progress.filter(|run| run.run_id.is_some() && run.status == RunStatus::Running);
    if let Some((run, operation)) = running.and_then(|run| Some((run, run.operation?))) {
        let (done, current) = progress::counts(run);
        let activity = match operation {
            ProvisioningOperation::Add => Activity::Setup,
            ProvisioningOperation::Update => Activity::Update,
            ProvisioningOperation::Uninstall => Activity::Uninstall,
        };
        return Some(Live {
            activity,
            steps: Some(Steps {
                done,
                total: run.steps.len(),
                current,
            }),
        });
    }
    // Exhaustive, so a new kind of work decides here whether a host shows it as working. An uninstall's requests are
    // not: its plan request asks for a question the user may still cancel, as a rerun's probe does, and its confirming
    // request is accepted with the run already pushed, which the branch above shows. Retry and Adopt start no run.
    let activity = match in_flight? {
        Work::Setup => Activity::Setup,
        Work::UpdateHost => Activity::Update,
        Work::UninstallHost
        | Work::RetryHost
        | Work::AdoptHost
        | Work::TakeOverHost { .. }
        | Work::Probe
        | Work::RemoveHost
        | Work::Save
        | Work::Rename
        | Work::Delete(_)
        | Work::Stop
        | Work::Replace
        | Work::Seen { .. }
        | Work::Restart
        | Work::TunnelOpen
        | Work::TunnelClose
        | Work::Launch
        | Work::Answer
        | Work::SendFeedback
        | Work::TrashDelete
        | Work::TabOpen
        | Work::TabClose
        | Work::Pin
        | Work::Mute
        | Work::MovePin
        | Work::SessionGroup
        | Work::Suspend
        | Work::Resume
        | Work::StopAndSuspend
        | Work::EnvironmentDelete => return None,
    };
    Some(Live {
        activity,
        steps: None,
    })
}

// ---- One host, as the rail reads it ----------------------------------------------------------------------------------

/// Everything this module says about one host, read from the current UI session: what the rail's avatar and an
/// attention row need beside the host entry itself.
#[derive(Clone, Debug, PartialEq)]
pub struct HostStatus {
    /// The update badge ([`update_badge`]).
    pub update: Option<Urgency>,
    /// A waiting SSH question or a sign-in the user must start again.
    pub(crate) sign_in: Option<crate::sign_in::Status>,
    /// The work going on ([`live`]).
    pub live: Option<Live>,
    /// Whether the setup or Update warning shows ([`warned`]).
    pub warned: bool,
}

/// [`HostStatus`] for `host`, from the current store and operation store; everything off while the window is inert.
pub fn host_status(host: &HostView, cx: &App) -> HostStatus {
    let key = HostView::key(host.id);
    let in_flight = session::current_operations(cx)
        .and_then(|operations| operations.read(cx).in_flight(&OpKey::host(&key)));
    let store = session::current_store(cx);
    let progress = store
        .as_ref()
        .and_then(|store| store.read(cx).collections().get::<ProvisioningView>(&key));
    HostStatus {
        update: update_badge(host, progress, in_flight.is_some()),
        sign_in: crate::sign_in::status(host, cx),
        live: live(progress, in_flight),
        warned: warned(host.id, cx),
    }
}

// ---- The warning -----------------------------------------------------------------------------------------------------

/// The two actions that warn the rail when they fail while Hosts is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    /// The helm's `add` operation: a host's setup, or its rerun.
    Setup,
    Update,
}

impl Action {
    /// The action a run of `operation` is, or `None` for an uninstall, which never warns.
    fn of(operation: ProvisioningOperation) -> Option<Action> {
        match operation {
            ProvisioningOperation::Add => Some(Action::Setup),
            ProvisioningOperation::Update => Some(Action::Update),
            ProvisioningOperation::Uninstall => None,
        }
    }
}

/// A host's retained run as last followed. What a change of the pushed progress is compared with, so that each
/// failure, completion or withdrawn run counts once.
#[derive(Clone, Debug, PartialEq)]
struct Observed {
    /// The helm's id for the run.
    run: String,
    operation: ProvisioningOperation,
    status: RunStatus,
}

/// What this module keeps about one host.
#[derive(Clone, Debug, Default, PartialEq)]
struct HostWarning {
    /// The host's run as last followed; `None` when the helm retains none for it, or before the first follow.
    last: Option<Observed>,
    /// Whether a setup warning shows.
    setup: bool,
    /// Whether an Update warning shows.
    update: bool,
}

impl HostWarning {
    /// Show or clear `action`'s warning.
    fn set(&mut self, action: Action, on: bool) {
        match action {
            Action::Setup => self.setup = on,
            Action::Update => self.update = on,
        }
    }

    /// The helm now retains `now` for this host (`None`: no run), with `open` whether Hosts shows. Nothing happens when
    /// that is the run as last followed. Otherwise: a run last seen running that is no longer the retained run ended
    /// without the helm saying how, which warns its action; a run newly failed warns its action; a run newly completed
    /// clears its action's warning (a later run of the same action succeeded, or a setup recovery by discovery
    /// resolved the failed run). Warnings are raised only while Hosts is closed.
    fn follow(&mut self, now: Option<Observed>, open: bool) {
        let before = std::mem::replace(&mut self.last, now.clone());
        if before == now {
            return;
        }
        if let Some(before) = &before
            && before.status == RunStatus::Running
            && now.as_ref().is_none_or(|now| now.run != before.run)
            && let Some(action) = Action::of(before.operation)
            && !open
        {
            self.set(action, true);
        }
        if let Some(now) = &now
            && let Some(action) = Action::of(now.operation)
        {
            match now.status {
                RunStatus::Failed if !open => self.set(action, true),
                RunStatus::Completed => self.set(action, false),
                RunStatus::Failed | RunStatus::Running => {}
            }
        }
    }
}

/// What this module keeps for one UI session. Pure, so its rules are tested without a window.
#[derive(Clone, Debug, Default, PartialEq)]
struct State {
    /// Whether Hosts shows: a failure then warns nothing, since the user sees it there.
    open: bool,
    /// Each host's warnings and last followed run, by registry id.
    hosts: HashMap<i64, HostWarning>,
    /// How many choices of "Update remote hosts automatically" this UI session made: each Always update, Try again
    /// and Settings press counts one. A failed save matters only while it is the newest choice.
    choice: u64,
    /// The choice whose save failed, if it was the newest when it failed.
    failed_save: Option<u64>,
}

impl State {
    /// `host`'s `action` failed or ended uncertainly: warn, unless Hosts shows.
    fn raise(&mut self, host: i64, action: Action) {
        if !self.open {
            self.hosts.entry(host).or_default().set(action, true);
        }
    }

    /// Hosts opened: it shows from now on, and every warning is acknowledged. The runs followed so far stay, so a
    /// retained failure does not warn again.
    fn opened(&mut self) {
        self.open = true;
        for warning in self.hosts.values_mut() {
            warning.setup = false;
            warning.update = false;
        }
    }

    /// Whether `host` shows a warning of either action.
    fn warned(&self, host: i64) -> bool {
        self.hosts
            .get(&host)
            .is_some_and(|warning| warning.setup || warning.update)
    }

    /// Follow the pushed host progress, with `runs` the collection's entries (each host's retained run) once it has
    /// been read, `listed` the hosts the registry lists once it has been read, and `auto_updates_on` the preference as
    /// pushed.
    ///
    /// Each entry is followed ([`HostWarning::follow`]). The helm leaves a host with no retained run out of the
    /// collection rather than listing it idle, and removes the entry when it withdraws a run, so a host followed with a
    /// run that has no entry now is followed as having none: that is how a withdrawn running run warns, and why the
    /// host's next run does not. A host the registry no longer lists is forgotten. The preference reading on retires
    /// Always update's failed save for good: someone chose it since, so the failure is no longer the newest choice even
    /// if the preference is turned off again.
    fn observe(
        &mut self,
        runs: Option<&[(i64, Option<Observed>)]>,
        listed: Option<&HashSet<i64>>,
        auto_updates_on: bool,
    ) {
        if let Some(runs) = runs {
            let open = self.open;
            let pushed: HashSet<i64> = runs.iter().map(|(host, _)| *host).collect();
            let withdrawn: Vec<i64> = self
                .hosts
                .iter()
                .filter(|(host, warning)| warning.last.is_some() && !pushed.contains(host))
                .map(|(host, _)| *host)
                .collect();
            let changes = runs
                .iter()
                .cloned()
                .chain(withdrawn.into_iter().map(|host| (host, None)));
            for (host, now) in changes {
                self.hosts.entry(host).or_default().follow(now, open);
            }
        }
        if let Some(listed) = listed {
            self.hosts.retain(|host, _| listed.contains(host));
        }
        if auto_updates_on {
            self.failed_save = None;
        }
    }

    /// A new choice of "Update remote hosts automatically": its number, which retires any earlier failed save.
    fn choose(&mut self) -> u64 {
        self.choice += 1;
        self.choice
    }

    /// The save of choice `choice` failed: keep it while it is the newest choice.
    fn save_failed(&mut self, choice: u64) {
        if choice == self.choice {
            self.failed_save = Some(choice);
        }
    }

    /// Whether the newest choice's save failed.
    fn save_error(&self) -> bool {
        self.failed_save == Some(self.choice)
    }
}

/// The rail and attention list observe status independently of progress and uninstall notices.
type Kept = super::SessionState<State>;

/// What is kept for the current UI session; `None` while the window is inert or when nothing was kept under it yet.
fn kept(cx: &App) -> Option<&State> {
    Kept::read(cx)
}

/// Apply `change` to the current UI session's state, starting it from empty when it was made under another session,
/// and store it only when it changed, so the global's observers redraw only then. Nothing while the window is inert.
fn change(cx: &mut App, change: impl FnOnce(&mut State)) {
    Kept::change(cx, change);
}

/// Whether host `host`'s rail avatar shows the setup or Update warning (SPEC.md, "Session list": "A setup or Update
/// warning has its own mark and hover explanation"), and its attention row reads "Setup or update needs attention".
pub fn warned(host: i64, cx: &App) -> bool {
    kept(cx).is_some_and(|state| state.warned(host))
}

/// Host `host`'s `action` was refused, or its request's reply cannot be used: warn its rail avatar unless Hosts shows.
/// Called from the completions that already show the refusal under the host's row or in the refusal dialog.
pub(crate) fn failed(host: i64, action: Action, cx: &mut App) {
    change(cx, |state| state.raise(host, action));
}

/// Hosts shows now: acknowledge every warning, and warn of nothing while it shows. The Hosts panel's `shown`.
pub(super) fn opened(cx: &mut App) {
    change(cx, State::opened);
}

/// Hosts no longer shows. The Hosts panel's `hidden`.
pub(super) fn closed(cx: &mut App) {
    change(cx, |state| state.open = false);
}

/// Follow the current store's host progress, host list and preferences ([`State::observe`]): the Hosts panel calls it
/// on every change of the store, whether or not Hosts shows.
pub(super) fn follow(cx: &mut App) {
    let Some(store) = session::current_store(cx) else {
        return;
    };
    let collections = store.read(cx).collections();
    let runs: Option<Vec<(i64, Option<Observed>)>> =
        collections.meta(Collection::HostProgress).map(|_| {
            collections
                .ordered::<ProvisioningView>()
                .into_iter()
                .map(|run| {
                    let observed = run
                        .run_id
                        .clone()
                        .zip(run.operation)
                        .map(|(id, operation)| Observed {
                            run: id,
                            operation,
                            status: run.status.clone(),
                        });
                    (run.host_id, observed)
                })
                .collect()
        });
    let listed: Option<HashSet<i64>> = collections.meta(Collection::Hosts).map(|_| {
        collections
            .ordered::<HostView>()
            .into_iter()
            .map(|host| host.id)
            .collect()
    });
    let on = auto_updates_on(cx);
    change(cx, |state| {
        state.observe(runs.as_deref(), listed.as_ref(), on)
    });
}

/// Redraw the observing view whenever what this module keeps changes (a warning raised or acknowledged, Always
/// update's save failing or retired), for as long as it holds the subscription: the global notifies no view by itself.
/// For the sidebar, which draws the rail and the attention list.
pub fn observe_changes<V: 'static>(cx: &mut Context<V>) -> Subscription {
    cx.observe_global::<Kept>(|_, cx| cx.notify())
}

// ---- Always update ---------------------------------------------------------------------------------------------------

/// Whether automatic remote-host updates are on, including before the helm pushes a preference row.
pub fn auto_updates_on(cx: &App) -> bool {
    session::current_store(cx).is_none_or(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| preferences.auto_update_remote_hosts)
            != Some(false)
    })
}

/// The attention list's Always update, pressed with `mark` taken in its handler: Update every host of `hosts` (the
/// displayed Update rows) whose Update is available, as Update all does, then save the preference on
/// ([`retry_auto_update`]'s save). The save goes out even when a session launch holds the updates back, which the Hosts
/// panel's line reports as for Update all.
pub fn always_update(hosts: &[i64], mark: Mark, window: &mut Window, cx: &mut App) {
    update::start_all(hosts, mark, window, cx);
    save_auto_update(cx);
}

/// The Try again beside Always update's failure: save the preference on again, without asking for any Update.
pub fn retry_auto_update(cx: &mut App) {
    save_auto_update(cx);
}

/// The user made a newer choice of "Update remote hosts automatically" than any earlier Always update, from the
/// Settings switch: an earlier failed save stops showing.
pub(crate) fn auto_update_chosen(cx: &mut App) {
    change(cx, |state| {
        state.choose();
    });
}

/// Whether the attention list shows Always update's failure ([`AUTO_UPDATE_FAILED`]) with its Try again: the newest
/// choice of the preference was an Always update or Try again whose save failed, and the preference still reads off. It
/// survives opening and closing Hosts; a reset forgets it.
pub fn auto_update_failed(cx: &App) -> bool {
    !auto_updates_on(cx) && kept(cx).is_some_and(State::save_error)
}

/// Save "Update remote hosts automatically" on, as a new choice, under the preference row's key: as the Settings switch
/// saves it, and not retried. A refusal, or a start refused because another preference save holds the key, records the
/// failure for [`auto_update_failed`].
fn save_auto_update(cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        return;
    };
    let command = match crate::settings::auto_update_command() {
        Ok(command) => command,
        Err(error) => {
            tracing::debug!(?error, "automatic updates' preference not built");
            return;
        }
    };
    let mut choice = 0;
    change(cx, |state| choice = state.choose());
    let started = operations.update(cx, |operations, cx| {
        operations.start(&OpKey::preferences(), command, cx, move |settled, cx| {
            if let Outcome::Refused(_) = settled.outcome {
                change(cx, |state| state.save_failed(choice));
            }
        })
    });
    match started {
        Ok(()) => {}
        Err(NotStarted::Busy(_)) => change(cx, |state| state.save_failed(choice)),
        Err(not_started) => tracing::debug!(?not_started, "automatic updates' preference not sent"),
    }
}
