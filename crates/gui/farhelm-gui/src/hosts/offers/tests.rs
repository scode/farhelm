//! Tests of which actions a host offers ([`super::offers`]), ported from the old `ProvisioningMenuState::offered` tests
//! in `provisioning.rs` and the menu tests in `hosts.rs` at reference commit `24bc0580`, minus the facts the GPUI app
//! no longer has (the module docs list them). Pure: each case is a pushed host, its retained run and whether work on it
//! is in flight.

use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus};
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{HostStateView, HostView};

use super::{
    HostKind, Locality, Offers, Rerun, SetupRerun, Update, adoptable, menu_offers,
    needs_app_supervisor, offers,
};
use crate::hosts::tests::{connected, host, local_down, run, skew};
use farhelm_proto::ProviderKind;

/// An idle SSH host's offers: everything an SSH host can be offered, all of it available.
fn idle_ssh() -> Offers {
    Offers {
        settings_enabled: true,
        rerun: None,
        update: Some(Update::Available),
        remove: Some(true),
        delete: None,
        uninstall: Some(true),
        retry: Some(true),
        adopt: None,
    }
}

/// Why this matters: this is the menu's whole contract for an ordinary remote host, which every other case below
/// changes one fact of. Specified: an idle, connected SSH host with no run offers Update (live), Settings and Remove,
/// all available, and no rerun; a progress entry with no run id (the helm's idle view) counts as no run.
#[test]
fn an_idle_remote_host_offers_update_settings_and_remove() {
    let ssh = host(2, "ssh", connected());
    assert_eq!(offers(&ssh, None, false), idle_ssh());
    let mut idle = run(2, ProvisioningOperation::Update, RunStatus::Failed);
    idle.run_id = None;
    assert_eq!(offers(&ssh, Some(&idle), false), idle_ssh());
}

/// Why this matters: an Update on a host running a newer Farhelm installs the helm's older build over it (SPEC.md:
/// Update never downgrades a host), and rerunning a failed Update there is the same downgrade; the old row offered
/// Update on any remote host until this was fixed. Specified: a connected host the helm calls newer, and a host refused
/// at the handshake with the higher protocol, are offered no Update and no rerun of a failed Update, and every other
/// offer is unchanged; a failed setup on a newer host is still offered again.
#[test]
fn a_too_new_host_is_not_offered_update() {
    let newer = HostStateView::Connected {
        identity: None,
        build_version: "9.0.0".into(),
        old_version: false,
        newer_version: true,
        refresh: farhelm_proto::ui_channel::RefreshView::Pending,
    };
    for state in [newer, skew(3, 2)] {
        let too_new = host(2, "ssh", state);
        assert_eq!(
            offers(&too_new, None, false),
            Offers {
                update: None,
                ..idle_ssh()
            }
        );
        let failed_update = run(2, ProvisioningOperation::Update, RunStatus::Failed);
        assert_eq!(offers(&too_new, Some(&failed_update), false).rerun, None);
        let failed_setup = run(2, ProvisioningOperation::Add, RunStatus::Failed);
        assert_eq!(
            offers(&too_new, Some(&failed_setup), false).rerun,
            Some((Rerun::Setup, true))
        );
    }
    let older = host(2, "ssh", skew(1, 2));
    assert_eq!(
        offers(&older, None, false).update,
        Some(Update::Available),
        "a host behind the helm is offered Update"
    );
}

/// Why this matters: a suspended sandbox refuses new connections until it is resumed, so Update or a rerun of its
/// setup could only fail (the helm refuses both before dialing). Specified: suspended, or with a suspend or resume in
/// progress, a host with a failed setup is offered neither Update nor the rerun, and the rest is unchanged.
#[test]
fn a_suspended_host_is_not_offered_update_or_rerun() {
    let failed_setup = run(5, ProvisioningOperation::Add, RunStatus::Failed);
    let running = offers(
        &host(5, "tensorlake", connected()),
        Some(&failed_setup),
        false,
    );
    assert_eq!(running.update, Some(Update::Available));
    assert_eq!(running.rerun, Some((Rerun::Setup, true)));
    for state in [
        HostStateView::Suspended {
            observed_at: 0,
            suspended_by_farhelm_at: None,
            resume_failed: None,
        },
        HostStateView::TensorlakeChanging {
            starting: false,
            direction: LifecycleDirection::Resuming,
            requested_at: 0,
        },
    ] {
        let suspended = host(5, "tensorlake", state);
        assert_eq!(
            offers(&suspended, Some(&failed_setup), false),
            Offers {
                update: None,
                rerun: None,
                ..running
            }
        );
    }
}

/// Why this matters: a host takes one request at a time, and while one is out (an Update's plan, a setup probe, a save
/// of its settings) a second action on it would be refused or would act on a host about to change. This is the half
/// of the old `only_confirmation_preparation_and_pending_plans_exclude_other_settings` that survives the port: work in
/// flight withdraws the new provisioning commands, the old app's "Update planning keeps only its own Settings
/// unavailable" (`provisioning.spec.ts`). Specified: with work in flight, Update and the rerun are withdrawn, and
/// Settings, Uninstall and Remove stay listed but unavailable.
#[test]
fn work_in_flight_withdraws_update_and_rerun_and_holds_settings_uninstall_and_remove() {
    let failed = run(2, ProvisioningOperation::Update, RunStatus::Failed);
    let ssh = host(2, "ssh", connected());
    assert_eq!(
        offers(&ssh, Some(&failed), false).rerun,
        Some((Rerun::Update, true)),
        "the premise: idle, the failed Update is offered again"
    );
    assert_eq!(
        offers(&ssh, Some(&failed), true),
        Offers {
            settings_enabled: false,
            rerun: None,
            update: None,
            remove: Some(false),
            delete: None,
            uninstall: Some(false),
            retry: Some(false),
            adopt: None,
        }
    );
}

/// Why this matters: while setup or an Update runs on a host, its settings must not change under the run and it must
/// not be started again (`provisioning.spec.ts`, "accepted ADD registers before execution and releases the page lock
/// while running": during the run Settings is unavailable and neither Update nor Re-run is offered; afterwards Update
/// is). Specified: with a running run, Update is withdrawn and Settings, Uninstall and Remove are unavailable; once the
/// run completes, the host is offered as an idle one.
#[test]
fn a_running_run_holds_the_host_until_it_ends() {
    let ssh = host(2, "ssh", connected());
    let running = run(2, ProvisioningOperation::Add, RunStatus::Running);
    assert_eq!(
        offers(&ssh, Some(&running), false),
        Offers {
            settings_enabled: false,
            rerun: None,
            update: None,
            remove: Some(false),
            delete: None,
            uninstall: Some(false),
            retry: Some(false),
            adopt: None,
        }
    );
    let completed = run(2, ProvisioningOperation::Add, RunStatus::Completed);
    assert_eq!(offers(&ssh, Some(&completed), false), idle_ssh());
}

/// Why this matters: a failed run is continued from the menu, and its item has to say which run it repeats; an
/// uninstall that failed part way is continued by choosing Uninstall again, never by a rerun. Specified: a failed setup
/// is offered as [`Rerun::Setup`] ("Try the failed setup again"), a failed Update as [`Rerun::Update`] ("Try the failed
/// update again"), a failed uninstall as no rerun but with Uninstall available to continue it; the failed run leaves
/// Update available.
#[test]
fn a_failed_run_is_offered_again_as_what_it_was() {
    let ssh = host(2, "ssh", connected());
    let rerun = |operation| offers(&ssh, Some(&run(2, operation, RunStatus::Failed)), false);
    let setup = rerun(ProvisioningOperation::Add);
    assert_eq!(setup.rerun, Some((Rerun::Setup, true)));
    assert_eq!(Rerun::Setup.description(), "Try the failed setup again");
    assert_eq!(setup.update, Some(Update::Available));
    assert_eq!(
        rerun(ProvisioningOperation::Update).rerun,
        Some((Rerun::Update, true))
    );
    assert_eq!(Rerun::Update.description(), "Try the failed update again");
    assert_eq!(rerun(ProvisioningOperation::Uninstall).rerun, None);
    assert_eq!(
        rerun(ProvisioningOperation::Uninstall).uninstall,
        Some(true)
    );
}

/// Why this matters: the panel cannot update the helm's own machine, so rerunning a failed Update there could only be
/// refused; the old menu showed that rerun greyed, as it showed the machine's Update. Only a run an older helm retained
/// can reach this (today's helm refuses a local Update before any run exists). Specified: on the local host a failed
/// Update's rerun is offered greyed and a failed setup's live; on an SSH host the failed Update's rerun is live.
#[test]
fn a_failed_update_on_the_local_host_is_offered_again_greyed() {
    let local = host(1, "local", connected());
    let failed = |operation| run(1, operation, RunStatus::Failed);
    assert_eq!(
        offers(&local, Some(&failed(ProvisioningOperation::Update)), false).rerun,
        Some((Rerun::Update, false))
    );
    assert_eq!(
        offers(&local, Some(&failed(ProvisioningOperation::Add)), false).rerun,
        Some((Rerun::Setup, true))
    );
    let ssh = host(2, "ssh", connected());
    assert_eq!(
        offers(
            &ssh,
            Some(&run(2, ProvisioningOperation::Update, RunStatus::Failed)),
            false
        )
        .rerun,
        Some((Rerun::Update, true))
    );
}

/// Why this matters: the helm refuses to update its own machine (it is updated by running the installer again), and
/// that refusal used to stick under the local row with no way to dismiss it; SPEC.md, "Session list", has the row show
/// Update greyed wherever it would otherwise be offered, saying to run the installer again. The old
/// `update_menu_item_is_disabled_on_the_local_row_and_points_at_the_installer` and the spec case "the local row's
/// update is greyed out, points at the installer, and sends nothing". Specified: the connected local host is offered
/// [`Update::Greyed`] with the installer description, Settings, and no Remove; an SSH host's Update is
/// [`Update::Available`] with "Install the newer Farhelm version".
#[test]
fn the_local_hosts_update_is_greyed_and_points_at_the_installer() {
    let local = host(1, "local", connected());
    assert_eq!(
        offers(&local, None, false),
        Offers {
            settings_enabled: true,
            rerun: None,
            update: Some(Update::Greyed),
            remove: None,
            delete: None,
            uninstall: None,
            retry: Some(true),
            adopt: None,
        }
    );
    assert_eq!(
        Update::Greyed.description(),
        "Run the installer again to update this machine"
    );
    assert_eq!(
        Update::Available.description(),
        "Install the newer Farhelm version"
    );
}

/// Why this matters: in the installed desktop app the helm's own machine is the machine the app runs on, and the app
/// updates itself, so SPEC.md ("Session list") has that row always offer Update, which checks for a newer release and
/// installs it; the old `update_menu_item_is_disabled_on_the_local_row_and_points_at_the_installer`, its app-updater
/// half. That check is not the panel's Update: the inline button and Update all must never send the helm's refused
/// local Update, and the rerun of a failed panel Update there stays greyed. Specified: with the app's updater running,
/// the local host's menu offers [`Update::CheckForRelease`] ("Check for a newer Farhelm and install it") when idle,
/// with no supervisor, during a run and with work in flight, every other offer as [`offers`] says; a failed Update's
/// rerun there stays greyed; the panel's own [`offers`] still greys it; without the updater the menu's offers are
/// [`offers`]'s; and an SSH host's are [`offers`]'s either way.
#[test]
fn in_the_desktop_app_the_local_hosts_update_checks_for_a_release() {
    let local = host(1, "local", connected());
    let running = run(1, ProvisioningOperation::Add, RunStatus::Running);
    for (view, progress, in_flight) in [
        (local.clone(), None, false),
        (host(1, "local", local_down("")), None, false),
        (local.clone(), Some(&running), false),
        (local.clone(), None, true),
    ] {
        assert_eq!(
            menu_offers(&view, progress, in_flight, true),
            Offers {
                update: Some(Update::CheckForRelease),
                ..offers(&view, progress, in_flight)
            },
            "{view:?}, {progress:?}, in flight {in_flight}"
        );
        assert_eq!(
            menu_offers(&view, progress, in_flight, false),
            offers(&view, progress, in_flight)
        );
    }
    assert_eq!(
        Update::CheckForRelease.description(),
        "Check for a newer Farhelm and install it"
    );
    let failed = run(1, ProvisioningOperation::Update, RunStatus::Failed);
    assert_eq!(
        menu_offers(&local, Some(&failed), false, true).rerun,
        Some((Rerun::Update, false))
    );
    assert_eq!(offers(&local, None, false).update, Some(Update::Greyed));
    let ssh = host(2, "ssh", connected());
    assert_eq!(menu_offers(&ssh, None, false, true), idle_ssh());
}

/// Why this matters: on the helm's own machine the panel never installs a supervisor; the desktop app starts it when it
/// opens (SPEC.md, "Topology"), and the migration's orchestrator settled on 2026-11-14 that this row offers no install.
/// The old menu's "Set up automatically" is not ported. Specified: the local host with no supervisor running needs the
/// app's supervisor and is offered Settings alone (no Update, greyed or not, no rerun, no Remove); a local host unreachable for
/// another reason, and an SSH host with the local cause, do not need it.
#[test]
fn the_local_host_without_a_supervisor_offers_no_install() {
    let down = host(1, "local", local_down(""));
    assert!(needs_app_supervisor(&down));
    assert_eq!(
        offers(&down, None, false),
        Offers {
            settings_enabled: true,
            rerun: None,
            update: None,
            remove: None,
            delete: None,
            uninstall: None,
            retry: Some(true),
            adopt: None,
        }
    );
    let transport = HostStateView::Unreachable {
        cause: "transport-failure".into(),
        last_error: "socket closed".into(),
    };
    assert!(!needs_app_supervisor(&host(1, "local", transport)));
    assert!(!needs_app_supervisor(&host(2, "ssh", local_down(""))));
}

/// Why this matters: a host kind a newer helm adds must not be offered verbs this build cannot vouch for, and each
/// known kind's decisions are made in one exhaustive place. Specified: an unrecognized kind is offered Settings and
/// Retry alone (the helm's retry does not depend on the kind, and the old menu offered it on every row),
/// even with a failed run, draws no locality and has no appearance to choose; SSH, Tensorlake and Podman hosts are
/// remote and the local host is local; only an SSH host is offered Remove, since only its removal merely forgets it (a
/// sandbox's or a container's deletes the machine, a different command, and the helm's own machine is never removed);
/// only an SSH host is offered Uninstall (SPEC.md: the local host is uninstalled with `farhelm uninstall`, and the
/// panel "offers no Uninstall" for a sandbox); only an SSH host's settings edit its destination; and every known kind's
/// settings choose its appearance.
#[test]
fn each_kind_decides_its_own_offers() {
    let failed = run(9, ProvisioningOperation::Add, RunStatus::Failed);
    let future = host(9, "future-kind", connected());
    assert_eq!(HostKind::of(&future), HostKind::Unrecognized);
    assert_eq!(
        offers(&future, Some(&failed), false),
        Offers {
            settings_enabled: true,
            rerun: None,
            update: None,
            remove: None,
            delete: None,
            uninstall: None,
            retry: Some(true),
            adopt: None,
        }
    );
    assert_eq!(HostKind::Unrecognized.locality(), None);
    assert!(!HostKind::Unrecognized.supports_appearance());
    assert!(!HostKind::Unrecognized.edits_destination());
    assert!(!HostKind::Unrecognized.removal_only_forgets());
    assert_eq!(HostKind::Unrecognized.provider(), None);
    assert_eq!(HostKind::Unrecognized.setup_rerun(), None);
    for (kind, forgets, provider, rerun, locality, edits_destination) in [
        ("local", false, None, None, Locality::Local, false),
        (
            "ssh",
            true,
            None,
            Some(SetupRerun::Ssh),
            Locality::Remote,
            true,
        ),
        (
            "tensorlake",
            false,
            Some(ProviderKind::Tensorlake),
            Some(SetupRerun::Tensorlake),
            Locality::Remote,
            false,
        ),
        (
            "podman",
            false,
            Some(ProviderKind::Podman),
            None,
            Locality::Remote,
            false,
        ),
    ] {
        let view: HostView = host(3, kind, connected());
        assert_eq!(
            HostKind::of(&view).removal_only_forgets(),
            forgets,
            "{kind}"
        );
        assert_eq!(
            offers(&view, None, false).remove.is_some(),
            forgets,
            "{kind}"
        );
        assert_eq!(HostKind::of(&view).provider(), provider, "{kind}");
        assert_eq!(
            offers(&view, None, false).delete,
            provider.map(|provider| (provider, true)),
            "{kind}: the provider's Delete in Remove's place"
        );
        assert_eq!(HostKind::of(&view).setup_rerun(), rerun, "{kind}");
        assert_eq!(
            offers(&view, None, false).uninstall.is_some(),
            kind == "ssh",
            "{kind}"
        );
        assert_eq!(HostKind::of(&view).locality(), Some(locality), "{kind}");
        assert_eq!(
            HostKind::of(&view).edits_destination(),
            edits_destination,
            "{kind}"
        );
        assert!(HostKind::of(&view).supports_appearance(), "{kind}");
    }
}

/// Why this matters: Retry and Adopt are the remedies the phase details name ("... then press Retry", "Adopt the
/// identity the host reports, or fix the destination"), so the menu must offer them exactly where those remedies are
/// shown, and never Adopt where there is no identity to adopt (the old "identity-unverified-offers-no-adopt": the helm
/// refuses that verb), and a failed deletion's remedy is "Retry to delete it again". Specified: Retry is offered
/// available on an unreachable, a duplicate and a retired host, on the local host, and on an environment whose deletion
/// failed, never while a deletion is under way, when Delete sandbox is withheld too and offered again once the deletion
/// failed; Adopt is offered only for an identity mismatch, whose `adoptable` identity is the reported one, raw, and
/// never for an unverified identity; with work in flight both stay offered but unavailable.
#[test]
fn retry_is_offered_in_every_phase_but_a_deletion_under_way_and_adopt_only_for_a_mismatch() {
    let unreachable = HostStateView::Unreachable {
        cause: "transport-failure".into(),
        last_error: "socket closed".into(),
    };
    let duplicate = HostStateView::Duplicate {
        twin: 3,
        identity: "id-1".into(),
    };
    let retired = HostStateView::Retired {
        reason: String::new(),
    };
    for state in [unreachable, duplicate, retired] {
        let offered = offers(&host(2, "ssh", state.clone()), None, false);
        assert_eq!(
            (offered.retry, offered.adopt),
            (Some(true), None),
            "{state:?}"
        );
    }
    assert_eq!(
        offers(&host(1, "local", connected()), None, false).retry,
        Some(true)
    );
    let deleting = offers(
        &host(4, "tensorlake", HostStateView::Deleting { failure: None }),
        None,
        false,
    );
    assert_eq!((deleting.retry, deleting.delete), (None, None));
    let failed = offers(
        &host(
            4,
            "tensorlake",
            HostStateView::Deleting {
                failure: Some("the provider refused".to_owned()),
            },
        ),
        None,
        false,
    );
    assert_eq!(
        (failed.retry, failed.delete),
        (Some(true), Some((ProviderKind::Tensorlake, true)))
    );

    let mismatch = HostStateView::IdentityMismatch {
        recorded: "identity-before".into(),
        reported: "identity after\u{200b}".into(),
    };
    assert_eq!(adoptable(&mismatch), Some("identity after\u{200b}"));
    let mismatched = host(2, "ssh", mismatch);
    assert_eq!(offers(&mismatched, None, false).adopt, Some(true));
    let busy = offers(&mismatched, None, true);
    assert_eq!((busy.retry, busy.adopt), (Some(false), Some(false)));
    let unverified = HostStateView::IdentityUnverified {
        recorded: "identity-before".into(),
    };
    assert_eq!(adoptable(&unverified), None);
    assert_eq!(offers(&host(2, "ssh", unverified), None, false).adopt, None);
}
