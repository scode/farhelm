//! Which actions a host offers: one pure function of what the helm pushed about the host, its retained provisioning
//! run, and whether work on it is in flight.
//!
//! Ported from the old app's `ProvisioningMenuState::offered` (`provisioning.rs`) and the parts of `hosts.rs` that
//! decided the menu's items (`host_menu_order`, `update_menu_item`) at reference commit `24bc0580`, and from the old
//! `HostKind` methods in `lib.rs`. The old function took a dozen facts, several of them about mechanisms the GPUI app
//! does not have; the settings-and-hosts goal (2026-11-14) asks for the derivation from current state alone:
//!
//! - The old "confirmation planning" (a setup probe out) and "update owned" (an Update's plan or submission out) are
//!   both work in flight under the host's key in the operation store, so they are one input, `in_flight`. Once the
//!   helm accepts a run (202) the run is already pushed as running, so the run itself covers the rest of what "update
//!   owned" used to track.
//! - The old "pending plan" (a setup confirmation on screen) is dropped: the open question's dialog holds the frozen
//!   plan and blocks the menu while it is open, so the menu never has to know about it.
//! - The automatic local setup and its `can_retry_local_setup` fact are dropped: the panel never installs a supervisor
//!   on the helm's own machine (SPEC.md, "Topology"), and the migration's orchestrator settled on 2026-11-14 that the
//!   local host without a supervisor offers no install.
//! - The `confirmation_may_open` output, which made Settings on every host wait for another host's confirmation, is
//!   dropped with the old modal queue it existed for. A host's own Settings still waits for that host's own work.
//! - Suspend, Resume and Delete sandbox come with their own pieces of the port, and are not offered here.
//!
//! The host menu ([`super::menu`]) renders only what this says; the rules are tested here, without a window. The menu
//! draws the rerun of a failed setup on an SSH host ([`HostKind::reruns_setup`]), the rerun of a failed Update, Update
//! (live, greyed on the helm's own machine, or the app's own check there in the installed desktop app), Settings,
//! Uninstall on an SSH host ([`Offers::uninstall`]), and Remove on a host whose removal only forgets it
//! ([`Offers::remove`]); the inline Update and Update all ask the panel's own offer (`super::update::available`), which
//! never includes the app's check.

use farhelm_proto::http::LOCAL_SUPERVISOR_NOT_RUNNING;
use farhelm_proto::provisioning::{ProvisioningOperation, ProvisioningView, RunStatus};
use farhelm_proto::ui_channel::{HostStateView, HostView};

#[cfg(test)]
mod tests;

/// A host's kind, from the wire word in [`HostView::kind`]. Every per-kind decision is an exhaustive method here, so a
/// kind added later has to decide each of them rather than inherit whatever a shared branch does.
///
/// A word this build does not know is [`HostKind::Unrecognized`], which gets no verbs this build cannot vouch for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostKind {
    /// The helm's own machine, the one reserved row.
    Local,
    /// A registered SSH destination.
    Ssh,
    /// A Tensorlake sandbox.
    Tensorlake,
    /// A container Farhelm created.
    Podman,
    /// A kind this build does not know.
    Unrecognized,
}

impl HostKind {
    /// The kind of `host`.
    pub(crate) fn of(host: &HostView) -> HostKind {
        match host.kind.as_str() {
            "local" => HostKind::Local,
            "ssh" => HostKind::Ssh,
            "tensorlake" => HostKind::Tensorlake,
            "podman" => HostKind::Podman,
            _ => HostKind::Unrecognized,
        }
    }

    /// Whether the host offers provisioning actions (Update, a rerun) at all. The helm's own machine does, but its
    /// Update is greyed ([`HostKind::updates_from_panel`]); only an unrecognized kind is offered nothing.
    pub(crate) fn offers_provisioning_actions(self) -> bool {
        match self {
            HostKind::Local | HostKind::Ssh | HostKind::Tensorlake | HostKind::Podman => true,
            HostKind::Unrecognized => false,
        }
    }

    /// Whether the panel's Update can update this host: the GUI's side of the helm's own rule. The helm refuses to
    /// update its own machine, which is updated by running the installer again, so that row's Update is shown greyed
    /// and never sends anything (SPEC.md, "Session list").
    pub(crate) fn updates_from_panel(self) -> bool {
        match self {
            HostKind::Ssh | HostKind::Tensorlake | HostKind::Podman => true,
            HostKind::Local | HostKind::Unrecognized => false,
        }
    }

    /// Whether removing the host only forgets its entry, which the menu's Remove does: an SSH host, whose supervisor and
    /// sessions keep running (SPEC.md, "Topology"). Removing a Tensorlake host deletes its sandbox and removing a
    /// container host deletes its container, so theirs is a different command with its own question (the Hosts panel's
    /// Delete sandbox, which a later piece of the port brings); until then their menus offer no removal at all. The
    /// helm's own machine is never removed.
    pub(crate) fn removal_only_forgets(self) -> bool {
        match self {
            HostKind::Ssh => true,
            HostKind::Local | HostKind::Tensorlake | HostKind::Podman | HostKind::Unrecognized => {
                false
            }
        }
    }

    /// Whether the desktop app's own updater updates this host: the helm's own machine, which in the desktop app is the
    /// machine the app runs on, so a newer release the app installs is that machine's update (SPEC.md, "Session list":
    /// "there the local row always offers Update, and choosing it checks for a newer release and installs it").
    pub(crate) fn updates_with_the_app(self) -> bool {
        match self {
            HostKind::Local => true,
            HostKind::Ssh | HostKind::Tensorlake | HostKind::Podman | HostKind::Unrecognized => {
                false
            }
        }
    }

    /// Whether the panel's Uninstall removes Farhelm from the host: an SSH host only (SPEC.md, "Topology"). The helm's
    /// own machine is uninstalled with `farhelm uninstall` there, and a Tensorlake sandbox or a container goes with its
    /// machine (Delete sandbox); the helm refuses an uninstall plan for each of those, and the menu offers none.
    pub(crate) fn uninstalls_from_panel(self) -> bool {
        match self {
            HostKind::Ssh => true,
            HostKind::Local | HostKind::Tensorlake | HostKind::Podman | HostKind::Unrecognized => {
                false
            }
        }
    }

    /// Whether the host's supervisor is the one `farhelm helm setup` installs on the helm's own machine, rather than
    /// one the panel provisions over SSH.
    pub(crate) fn sets_up_locally(self) -> bool {
        match self {
            HostKind::Local => true,
            HostKind::Ssh | HostKind::Tensorlake | HostKind::Podman | HostKind::Unrecognized => {
                false
            }
        }
    }

    /// Whether the menu's Re-run can run the host's failed setup again: only over SSH so far, by probing its registered
    /// destination ([`super::setup::rerun`]). The helm's own machine never runs a setup from the panel, so it has no
    /// failed one; a Tensorlake sandbox's rerun probes through the Tensorlake route, which comes with the Tensorlake
    /// pieces of the port; a container is set up through its environment's launch, as the old app told the user.
    pub(crate) fn reruns_setup(self) -> bool {
        match self {
            HostKind::Ssh => true,
            HostKind::Local | HostKind::Tensorlake | HostKind::Podman | HostKind::Unrecognized => {
                false
            }
        }
    }

    /// Whether the host's SSH destination can be changed in its settings: only a registered SSH host has one the user
    /// chose. The helm's own machine has none, and a sandbox's or a container's address is Farhelm's to manage.
    pub(crate) fn edits_destination(self) -> bool {
        match self {
            HostKind::Ssh => true,
            HostKind::Local | HostKind::Tensorlake | HostKind::Podman | HostKind::Unrecognized => {
                false
            }
        }
    }

    /// Whether the host has an identity mark the user can choose (its icon and color): every kind this build knows.
    /// A kind it does not know draws no mark, so its settings offer none to choose.
    pub(crate) fn supports_appearance(self) -> bool {
        match self {
            HostKind::Local | HostKind::Ssh | HostKind::Tensorlake | HostKind::Podman => true,
            HostKind::Unrecognized => false,
        }
    }

    /// Where the host is, as its identity mark says on hover (`Local` or `Remote`), or `None` for a kind this build
    /// cannot place, which the old row drew no mark for.
    pub(crate) fn locality(self) -> Option<Locality> {
        match self {
            HostKind::Local => Some(Locality::Local),
            HostKind::Ssh | HostKind::Tensorlake | HostKind::Podman => Some(Locality::Remote),
            HostKind::Unrecognized => None,
        }
    }
}

/// Where a host is, for its identity mark's word and hover help.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Locality {
    Local,
    Remote,
}

/// Whether `host` is the helm's own machine with no supervisor running there: the state SPEC.md ("Topology") answers
/// with the `farhelm helm setup` instruction rather than an install, and in which the row offers no Update.
pub(crate) fn needs_local_setup(host: &HostView) -> bool {
    HostKind::of(host).sets_up_locally()
        && matches!(
            &host.state,
            HostStateView::Unreachable { cause, .. } if cause == LOCAL_SUPERVISOR_NOT_RUNNING
        )
}

/// Whether the host runs a newer Farhelm than the helm: connected with a build the helm calls newer, or refused at the
/// handshake with the higher protocol. Such a host reads "Too new" and is never offered Update, which would install
/// the helm's older build over it (SPEC.md: Update never downgrades a host).
pub(crate) fn runs_newer_version(state: &HostStateView) -> bool {
    match state {
        HostStateView::Connected { newer_version, .. } => *newer_version,
        HostStateView::VersionSkew {
            peer_protocol,
            our_protocol,
            ..
        } => peer_protocol > our_protocol,
        HostStateView::Connecting { .. }
        | HostStateView::Unreachable { .. }
        | HostStateView::IdentityMismatch { .. }
        | HostStateView::IdentityUnverified { .. }
        | HostStateView::Duplicate { .. }
        | HostStateView::TensorlakeChanging { .. }
        | HostStateView::Suspended { .. }
        | HostStateView::Stopped { .. }
        | HostStateView::Retired { .. }
        | HostStateView::Deleting { .. } => false,
    }
}

/// The operation of the host's retained run when that run failed: what a rerun would repeat.
pub(crate) fn failed_operation(
    progress: Option<&ProvisioningView>,
) -> Option<ProvisioningOperation> {
    progress
        .filter(|run| run.run_id.is_some() && run.status == RunStatus::Failed)
        .and_then(|run| run.operation)
}

/// Whether the host has a provisioning run going on now.
pub(crate) fn run_active(progress: Option<&ProvisioningView>) -> bool {
    progress.is_some_and(|run| run.run_id.is_some() && run.status == RunStatus::Running)
}

/// What the host's actions menu offers, and which of its offers can be chosen now. The menu lists them in the old
/// order (a rerun, Update, Settings, then Remove behind a separator); an absent offer leaves no gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Offers {
    /// Settings is always offered: every host has its permissions, whatever else its dialog lets the user change.
    /// `false` while the host has work in flight or a run going on, so a host's settings never change under its own
    /// running setup or Update.
    pub(crate) settings_enabled: bool,
    /// The rerun of the host's failed setup or Update, if one is offered, and whether it can be chosen now: a failed
    /// Update on the helm's own machine is offered greyed, like that machine's Update, since the panel cannot update it.
    pub(crate) rerun: Option<(Rerun, bool)>,
    /// Update, if offered: live, or greyed with the reason on the helm's own machine.
    pub(crate) update: Option<Update>,
    /// Remove, which forgets the host, offered only where removing it does nothing more
    /// ([`HostKind::removal_only_forgets`]: a sandbox's or a container's removal deletes the machine and will be an
    /// offer of its own): `Some(true)` when it can be chosen now, `Some(false)` while the host is
    /// busy with work in flight or a run.
    pub(crate) remove: Option<bool>,
    /// Uninstall, which removes Farhelm from the host, offered only where the panel uninstalls
    /// ([`HostKind::uninstalls_from_panel`]): `Some(true)` when it can be chosen now, `Some(false)` while the host is
    /// busy. A failed uninstall is continued by choosing it again, so a failed run leaves it available.
    pub(crate) uninstall: Option<bool>,
}

/// Which failed run a rerun repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rerun {
    /// The host's setup (the helm's `add` operation).
    Setup,
    /// The host's Update.
    Update,
}

impl Rerun {
    /// The menu item's description line, as the old menu worded it.
    pub(crate) fn description(self) -> &'static str {
        match self {
            Rerun::Setup => "Try the failed setup again",
            Rerun::Update => "Try the failed update again",
        }
    }
}

/// How the Update item is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Update {
    /// The panel can update the host.
    Available,
    /// The host is the helm's own machine: the item shows, greyed, and choosing it sends nothing (SPEC.md, "Session
    /// list").
    Greyed,
    /// The host is the helm's own machine in a desktop app whose updater runs: choosing it asks that updater to check
    /// for a newer release and install it, and sends nothing to the helm ([`menu_offers`]).
    CheckForRelease,
}

impl Update {
    /// Whether choosing the item does something: the panel's Update and the app's check do; the greyed item, which
    /// takes no click, does not. The menu draws an item that cannot act greyed and never starts focus on it.
    pub(crate) fn acts(self) -> bool {
        match self {
            Update::Available | Update::CheckForRelease => true,
            Update::Greyed => false,
        }
    }

    /// The item's description line: what Update does, or for the greyed item how this machine is updated instead.
    pub(crate) fn description(self) -> &'static str {
        match self {
            Update::Available => "Install the newer Farhelm version",
            Update::Greyed => "Run the installer again to update this machine",
            Update::CheckForRelease => "Check for a newer Farhelm and install it",
        }
    }
}

/// What `host`'s menu offers, given its retained run `progress` (`None` when the helm retains no run for it) and
/// whether work on it is in flight in the operation store (`in_flight`, under `OpKey::host` for its id).
///
/// The rules, as the old menu had them:
///
/// - **Rerun** of the failed run's operation, unless work is in flight, the host is suspended, the kind offers no
///   provisioning, or it would rerun an Update on a host running a newer Farhelm (the same downgrade Update is
///   refused for). A failed uninstall is continued by its own item, never rerun. The rerun of a failed Update is greyed
///   where the panel cannot update the host (the helm's own machine), as the old menu greyed it; the helm refuses that
///   Update before any run exists, so only a run an older helm retained can get here.
/// - **Update** when nothing runs or is in flight, the kind offers provisioning, the host is not the helm's own machine
///   waiting for `farhelm helm setup`, it does not run a newer Farhelm, and it is not suspended. On the helm's own
///   machine it is [`Update::Greyed`] wherever it would otherwise be offered.
/// - **Settings** always, chosen only while the host is idle.
/// - **Uninstall** for a kind the panel uninstalls, chosen only while the host is idle; after a failed uninstall too,
///   since choosing it again is how that run is continued.
/// - **Remove** for a kind whose removal only forgets the host, chosen only while the host is idle.
pub(crate) fn offers(
    host: &HostView,
    progress: Option<&ProvisioningView>,
    in_flight: bool,
) -> Offers {
    let kind = HostKind::of(host);
    let provisions = kind.offers_provisioning_actions();
    let too_new = runs_newer_version(&host.state);
    let suspended = matches!(
        host.state,
        HostStateView::Suspended { .. } | HostStateView::TensorlakeChanging { .. }
    );
    let running = run_active(progress);
    let busy = in_flight || running;
    let rerun = failed_operation(progress)
        .filter(|_| provisions && !in_flight && !suspended)
        .and_then(|operation| match operation {
            ProvisioningOperation::Add => Some((Rerun::Setup, true)),
            ProvisioningOperation::Update if !too_new => {
                Some((Rerun::Update, kind.updates_from_panel()))
            }
            ProvisioningOperation::Update | ProvisioningOperation::Uninstall => None,
        });
    let update_offered =
        !running && !in_flight && provisions && !needs_local_setup(host) && !too_new && !suspended;
    let update = update_offered.then(|| {
        if kind.updates_from_panel() {
            Update::Available
        } else {
            Update::Greyed
        }
    });
    Offers {
        settings_enabled: !busy,
        rerun,
        update,
        remove: kind.removal_only_forgets().then_some(!busy),
        uninstall: kind.uninstalls_from_panel().then_some(!busy),
    }
}

/// What `host`'s actions menu offers: [`offers`], except that in a desktop app whose updater runs (`app_updater`, the
/// updater's own answer, `desktop::updates::runs`) the helm's own machine's Update is [`Update::CheckForRelease`],
/// offered and available whatever the panel's rules say about that host (SPEC.md, "Session list": "there the local row
/// always offers Update"), as the old app offered it while setup was missing, a run was going on or work was in flight:
/// the check plans nothing on the host, and the updater serializes checks itself.
///
/// Kept apart from [`offers`] because the inline Update and Update all ask that one: they are the panel's Update of
/// remote hosts, which the app's check never is. A failed Update's rerun on the helm's own machine stays greyed here
/// too, as in the old app: the app's check never makes the panel able to update that machine.
pub(crate) fn menu_offers(
    host: &HostView,
    progress: Option<&ProvisioningView>,
    in_flight: bool,
    app_updater: bool,
) -> Offers {
    let offered = offers(host, progress, in_flight);
    if app_updater && HostKind::of(host).updates_with_the_app() {
        Offers {
            update: Some(Update::CheckForRelease),
            ..offered
        }
    } else {
        offered
    }
}
