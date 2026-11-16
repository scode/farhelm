//! Tests of what a host row says: the phase words and tones for every phase the helm reports, the Too new hover, and
//! the detail lines, ported from the old `hosts.rs` tests at reference commit `24bc0580`
//! (`old_connected_hosts_have_advisory_display_without_skew_classification`,
//! `a_newer_host_reads_too_new_with_both_versions_on_hover`, `a_connected_hosts_detail_reports_its_refresh_health`, the
//! remedy cases). Pure functions of the pushed state; what the row draws from them is tested in `hosts/tests.rs`.

use farhelm_proto::recipes::RecipeSize;
use farhelm_proto::tensorlake::recipes::Tunnel;
use farhelm_proto::tensorlake::{LifecycleDirection, TunnelState, TunnelView};
use farhelm_proto::ui_channel::{ConnectionStage, HostStateView, RefreshView};

use super::{Kind, Tone, detail_lines, host_tone, hover, tone, tunnels_failed, words};
use crate::host_text::HostText;
use crate::hosts::tests::{LOCAL_DIAL_FAILURE, connected, connecting, host, local_down, skew};
use crate::local_time::Zone;

/// The detail lines of `state` on an unaliased SSH host, each as (its kind, its accessible name), times in UTC.
fn lines_of(state: HostStateView) -> Vec<(Kind, String)> {
    lines_with_twin(state, |_| None)
}

/// [`lines_of`] with times in `zone`.
fn lines_in(state: HostStateView, zone: Zone) -> Vec<(Kind, String)> {
    detail_lines(&host(2, "ssh", state), |_| None, zone)
        .into_iter()
        .map(|(kind, line)| (kind, line.accessible_name().to_string()))
        .collect()
}

/// [`lines_of`] with `twin` naming hosts by id.
fn lines_with_twin(
    state: HostStateView,
    twin: impl Fn(i64) -> Option<HostText>,
) -> Vec<(Kind, String)> {
    detail_lines(&host(2, "ssh", state), twin, Zone::UTC)
        .into_iter()
        .map(|(kind, line)| (kind, line.accessible_name().to_string()))
        .collect()
}

/// Why this matters: the phase words and their color are the row's whole status at a glance, and SPEC.md fixes the
/// words that matter most (`Connected`, `Old version`, `Too new`, `Needs update`); the color groups phases by what the
/// user should do, so a phase drawn in the wrong group reads as the wrong urgency. Specified: every phase the helm
/// reports, with the old app's words and color group, including the environment phases the later pieces bring.
#[test]
fn every_phase_has_its_words_and_tone() {
    let connected_with = |old_version, newer_version| HostStateView::Connected {
        identity: None,
        build_version: "0.16.0".into(),
        old_version,
        newer_version,
        refresh: RefreshView::Pending,
    };
    let cases = [
        (connected_with(false, false), "Connected", Tone::Connected),
        (connected_with(true, false), "Old version", Tone::Advisory),
        (connected_with(false, true), "Too new", Tone::Advisory),
        (connecting(2, None), "Connecting", Tone::Pending),
        (
            HostStateView::Unreachable {
                cause: "transport-failure".into(),
                last_error: "refused".into(),
            },
            "Unreachable, retrying",
            Tone::Advisory,
        ),
        (local_down(""), "Unreachable, retrying", Tone::Advisory),
        (skew(1, 2), "Needs update", Tone::Attention),
        (skew(3, 2), "Too new", Tone::Attention),
        (
            HostStateView::IdentityMismatch {
                recorded: "a".into(),
                reported: "b".into(),
            },
            "Identity mismatch",
            Tone::Attention,
        ),
        (
            HostStateView::IdentityUnverified {
                recorded: "a".into(),
            },
            "Identity unverified",
            Tone::Attention,
        ),
        (
            HostStateView::Duplicate {
                twin: 3,
                identity: "a".into(),
            },
            "Duplicate",
            Tone::Attention,
        ),
        (
            HostStateView::Retired {
                reason: String::new(),
            },
            "Retired",
            Tone::Attention,
        ),
        (
            HostStateView::TensorlakeChanging {
                starting: true,
                direction: LifecycleDirection::Resuming,
                requested_at: 0,
            },
            "Starting…",
            Tone::Pending,
        ),
        (
            HostStateView::TensorlakeChanging {
                starting: false,
                direction: LifecycleDirection::Suspending,
                requested_at: 0,
            },
            "Suspending…",
            Tone::Pending,
        ),
        (
            HostStateView::TensorlakeChanging {
                starting: false,
                direction: LifecycleDirection::Resuming,
                requested_at: 0,
            },
            "Resuming…",
            Tone::Pending,
        ),
        (
            HostStateView::Suspended {
                observed_at: 0,
                suspended_by_farhelm_at: None,
                resume_failed: None,
            },
            "Suspended",
            Tone::Quiet,
        ),
        (
            HostStateView::Stopped {
                start_failed: None,
                waiting_for_place: false,
            },
            "Stopped",
            Tone::Attention,
        ),
        (
            HostStateView::Deleting { failure: None },
            "Deleting…",
            Tone::Pending,
        ),
        (
            HostStateView::Deleting {
                failure: Some("quota".into()),
            },
            "Deleting failed",
            Tone::Attention,
        ),
    ];
    for (state, expected_words, expected_tone) in cases {
        assert_eq!(
            (words(&state), tone(&state)),
            (expected_words, expected_tone),
            "{state:?}"
        );
    }
}

/// Why this matters: a host running a newer Farhelm used to read "Needs update" or plain "Connected", inviting an
/// Update that would install the helm's older build over it, and nothing said the helm was the side behind. SPEC.md:
/// its hover names the host's version, the helm's, and their protocols. Specified: a connected newer host's hover names
/// its build, this build and the shared protocol and says to update the helm; a skewed newer host's names both builds
/// and both protocols; a host's build is escaped; an older skewed host's hover is its words.
#[test]
fn a_newer_host_reads_too_new_with_both_versions_on_hover() {
    let connected_newer = HostStateView::Connected {
        identity: None,
        build_version: "9.0.0\u{202E}".into(),
        old_version: false,
        newer_version: true,
        refresh: RefreshView::Pending,
    };
    let text = hover(&connected_newer);
    for expected in [
        "This host runs Farhelm 9.0.0<U+202E>, which is newer than this helm's ",
        farhelm_proto::BUILD_VERSION,
        &format!("(both speak protocol {})", farhelm_proto::PROTOCOL_VERSION),
        "update the helm",
    ] {
        assert!(
            text.contains(expected),
            "{expected:?} missing from {text:?}"
        );
    }
    assert!(!text.contains('\u{202E}'));
    assert_eq!(
        hover(&skew(3, 2)).as_ref(),
        "This host runs Farhelm 9.0.0 (protocol 3), which is newer than this helm's 0.16.0 (protocol 2); update the helm"
    );
    assert_eq!(hover(&skew(1, 2)).as_ref(), "Needs update");
    assert_eq!(hover(&connected()).as_ref(), "Connected");
}

/// Why this matters: for a connected host the one detail line is the version, identity and session count SPEC.md has
/// the details reveal, and a failed refresh must read as last-known sessions on a host that is still connected, not as
/// a disconnection. Specified: the build and identity (or "no identity reported") and each refresh state's words.
#[test]
fn a_connected_hosts_detail_reports_its_version_identity_and_sessions() {
    let detail = |identity: Option<&str>, refresh| {
        lines_of(HostStateView::Connected {
            identity: identity.map(str::to_owned),
            build_version: "0.16.0".into(),
            old_version: false,
            newer_version: false,
            refresh,
        })
    };
    let evidence = |words: &str| vec![(Kind::Evidence, words.to_owned())];
    assert_eq!(
        detail(Some("id-1"), RefreshView::Ok { sessions: 3 }),
        evidence("Farhelm 0.16.0; identity id-1; 3 sessions")
    );
    assert_eq!(
        detail(Some("id-1"), RefreshView::Ok { sessions: 1 }),
        evidence("Farhelm 0.16.0; identity id-1; 1 session")
    );
    assert_eq!(
        detail(None, RefreshView::Ok { sessions: 0 }),
        evidence("Farhelm 0.16.0; no identity reported; 0 sessions")
    );
    assert_eq!(
        detail(None, RefreshView::Pending),
        evidence(
            "Farhelm 0.16.0; no identity reported; the first session refresh is still in flight"
        )
    );
    assert_eq!(
        detail(
            None,
            RefreshView::Failed {
                error: "list timed out".into()
            }
        ),
        evidence(
            "Farhelm 0.16.0; no identity reported; the last session refresh failed, so its sessions are last-known: \
             list timed out"
        )
    );
}

/// Why this matters: a connecting host's detail says how far the current attempt got and why the last one failed, the
/// evidence a user reads before deciding whether to wait or fix something (the reference screen
/// `hosts-panel-host-unreachable`). Specified: the first attempt and a later one, each stage's phrase, and the last
/// failure after it; no remedy, since the host is still trying.
#[test]
fn a_connecting_hosts_detail_says_how_far_its_attempt_got() {
    assert_eq!(
        lines_of(connecting(0, None)),
        [(
            Kind::Evidence,
            "The first connection attempt: waiting for the next connection attempt…".to_owned()
        )]
    );
    assert_eq!(
        lines_of(connecting(2, Some("connection refused"))),
        [(
            Kind::Evidence,
            "Attempt 2: waiting for the next connection attempt…; the last one failed: connection refused".to_owned()
        )]
    );
    for (stage, phrase) in [
        (ConnectionStage::ConnectingToHost, "connecting to the host…"),
        (
            ConnectionStage::ConnectingToSupervisor,
            "connecting to the local supervisor…",
        ),
        (
            ConnectionStage::WaitingForHello,
            "waiting for the supervisor to answer…",
        ),
        (ConnectionStage::SettingUp, "finishing connection setup…"),
    ] {
        let lines = lines_of(HostStateView::Connecting {
            attempt: 1,
            stage,
            last_error: None,
        });
        assert_eq!(lines, [(Kind::Evidence, format!("Attempt 1: {phrase}"))]);
    }
}

/// Why this matters: the desktop app starts the supervisor of the helm's own machine whenever it opens and none
/// answers, so reopening it is what fixes a missing one there. Specified: with the helm giving no detail (which it
/// never does in practice), the local host's evidence says no supervisor is running and its remedy is the app's
/// sentence; another unreachable host shows the helm's error, or "The host did not answer", and no remedy.
#[test]
fn the_local_hosts_missing_supervisor_says_to_reopen_the_app() {
    assert_eq!(
        lines_of(local_down("  ")),
        [
            (
                Kind::Evidence,
                "No supervisor is running on this machine".to_owned()
            ),
            (
                Kind::Remedy,
                "Quit Farhelm and open it again; it starts this machine's supervisor when it opens"
                    .to_owned()
            ),
        ]
    );
    let unreachable = |error: &str| HostStateView::Unreachable {
        cause: "transport-failure".into(),
        last_error: error.into(),
    };
    assert_eq!(
        lines_of(unreachable("ssh: no route")),
        [(Kind::Evidence, "ssh: no route".to_owned())]
    );
    assert_eq!(
        lines_of(unreachable("")),
        [(Kind::Evidence, "The host did not answer".to_owned())]
    );
}

/// Why this matters: the local row's remedy is the app's sentence alone, because the desktop app starts this machine's
/// supervisor when it opens, and the helm's dial failure is shown only as evidence of what happened (the deviation for
/// the local row in `docs/gpui/parity/settings-and-hosts.md` records the decision). Specified: with the helm's real
/// dial failure ([`LOCAL_DIAL_FAILURE`]), the local host's evidence is that failure verbatim and its remedy is the
/// app's sentence alone.
#[test]
fn the_local_remedy_is_the_apps_sentence_alone_and_the_helms_detail_stays_evidence() {
    assert_eq!(
        lines_of(local_down(LOCAL_DIAL_FAILURE)),
        [
            (Kind::Evidence, LOCAL_DIAL_FAILURE.to_owned()),
            (
                Kind::Remedy,
                "Quit Farhelm and open it again; it starts this machine's supervisor when it opens"
                    .to_owned()
            ),
        ]
    );
}

/// Why this matters: the phases that wait for a person must say what to do, and the duplicate's names the entry that
/// already holds the machine by the name the user knows it by (SPEC.md). Specified: each phase's evidence and remedy,
/// the skew's remedy verbatim from the helm (none when blank), and the duplicate with and without the twin's name.
#[test]
fn phases_that_wait_for_a_person_say_what_to_do() {
    let mut skewed = skew(1, 2);
    assert_eq!(
        lines_of(skewed.clone()),
        [
            (
                Kind::Evidence,
                "The host speaks protocol 1 (Farhelm 9.0.0); this helm speaks 2 (Farhelm 0.16.0)"
                    .to_owned()
            ),
            (Kind::Remedy, "update the host".to_owned()),
        ]
    );
    if let HostStateView::VersionSkew { remediation, .. } = &mut skewed {
        *remediation = " ".into();
    }
    assert_eq!(lines_of(skewed).len(), 1, "a blank remedy is no remedy");
    assert_eq!(
        lines_of(HostStateView::IdentityMismatch {
            recorded: "old-id".into(),
            reported: "new-id".into(),
        }),
        [
            (
                Kind::Evidence,
                "Recorded as install old-id; the destination now reports new-id, so nothing is connected until \
                 this is decided"
                    .to_owned()
            ),
            (
                Kind::Remedy,
                "Adopt the identity the host reports, or fix the destination; adopting resets all five of this host's \
                 permissions to asking"
                    .to_owned()
            ),
        ]
    );
    assert_eq!(
        lines_of(HostStateView::IdentityUnverified {
            recorded: "old-id".into(),
        }),
        [
            (
                Kind::Evidence,
                "The host answered without an identity, so this helm cannot confirm it is still the install \
                 recorded as old-id"
                    .to_owned()
            ),
            (
                Kind::Remedy,
                "Fix the host so it reports its identity, or retarget or remove this entry — it is re-probed \
                 meanwhile, so a host that starts identifying itself again recovers on its own"
                    .to_owned()
            ),
        ]
    );
    let duplicate = HostStateView::Duplicate {
        twin: 3,
        identity: "id-9".into(),
    };
    assert_eq!(
        lines_with_twin(duplicate.clone(), |id| (id == 3)
            .then(|| HostText::from_host("gpu-1"))),
        [
            (
                Kind::Evidence,
                "This entry reaches install id-9, which the entry gpu-1 already holds, so nothing is connected \
                 through this one"
                    .to_owned()
            ),
            (
                Kind::Remedy,
                "Remove gpu-1 or change this entry's destination, then press Retry".to_owned()
            ),
        ]
    );
    assert_eq!(
        lines_of(duplicate),
        [
            (
                Kind::Evidence,
                "This entry reaches install id-9, which another entry (host 3) already holds, so nothing is \
                 connected through this one"
                    .to_owned()
            ),
            (
                Kind::Remedy,
                "Remove the other entry or change this entry's destination, then press Retry".to_owned()
            ),
        ]
    );
    assert_eq!(
        lines_of(HostStateView::Retired {
            reason: String::new()
        }),
        [
            (
                Kind::Evidence,
                "No connection actor is running for this entry".to_owned()
            ),
            (
                Kind::Remedy,
                "Retry to start a fresh connection actor for this entry".to_owned()
            ),
        ]
    );
}

/// Why this matters: an aliased host's row shows the alias, so the details are the one place its real destination
/// shows (SPEC.md: "the host details keep the real destination"); an unaliased host's name already is its destination,
/// and repeating it would be noise. Specified: an aliased host's details start with "Destination: " and its
/// destination; an unaliased one's do not.
#[test]
fn the_destination_shows_only_behind_an_alias() {
    let mut aliased = host(2, "ssh", connected());
    aliased.alias = Some("build-box".into());
    aliased.name = "build-box".into();
    let first = detail_lines(&aliased, |_| None, Zone::UTC)
        .first()
        .map(|(_, line)| line.accessible_name().to_string());
    assert_eq!(first.as_deref(), Some("Destination: dev@build-box.invalid"));
    assert!(
        !lines_of(connected())
            .iter()
            .any(|(_, line)| line.starts_with("Destination")),
        "no destination line without an alias"
    );
}

/// Why this matters: the environment phases' details carry the time something was asked or noticed, and the maintainer
/// decided on 2026-11-15 that every time the app shows is the viewer's local time; the helm pushes milliseconds, so a
/// detail that forgot to divide would read as a date tens of thousands of years away. Specified: five hours west of
/// UTC, a suspended host's detail carries the local time the helm noticed the suspension, and a host whose provider was
/// asked to suspend carries the local time it was asked, each to the second with no zone named (the stamp's own format
/// is pinned in `local_time_tests.rs`).
#[test]
fn environment_times_read_in_the_viewers_zone() {
    let west = Zone::fixed(-5 * 3_600);
    let suspended = HostStateView::Suspended {
        observed_at: 1_700_000_000_000,
        suspended_by_farhelm_at: None,
        resume_failed: Some("quota".into()),
    };
    assert_eq!(
        lines_in(suspended, west),
        [(
            Kind::Evidence,
            "Suspension noticed at 2023-11-14 17:13:20. Resume failed: quota".to_owned()
        )]
    );
    let asked = HostStateView::TensorlakeChanging {
        starting: false,
        direction: LifecycleDirection::Suspending,
        requested_at: 1_700_000_000_000,
    };
    assert_eq!(
        lines_in(asked, west)[0].1,
        "Asked the environment provider to suspend at 2023-11-14 17:13:20; waiting for confirmation"
    );
}

/// Why this matters: SPEC.md has identities shown so that two different ones never look alike, and the detail lines are
/// where the user compares a recorded install id with the one a host now reports, or with its twin's; the old app
/// spelled them with `display_identity`. A space or a look-alike letter drawn as itself would make "id 1" and "id 1 "
/// read the same. Specified: in the connected line, the mismatch, the unverified and the duplicate lines, an install id
/// holding a space, a trailing space and a Cyrillic look-alike letter is spelled with `<U+XXXX>` for each, while an
/// ordinary id reads as itself.
#[test]
fn install_ids_are_spelled_so_two_ids_never_look_alike() {
    let spelled = "id<U+0020>1<U+0020>";
    let tricky = "id 1 ";
    let connected = lines_of(HostStateView::Connected {
        identity: Some(tricky.into()),
        build_version: "0.16.0".into(),
        old_version: false,
        newer_version: false,
        refresh: RefreshView::Ok { sessions: 0 },
    });
    assert!(
        connected[0].1.contains(&format!("identity {spelled};")),
        "{connected:?}"
    );
    let mismatch = lines_of(HostStateView::IdentityMismatch {
        recorded: "id-1".into(),
        reported: "\u{0456}d-1".into(),
    });
    assert!(
        mismatch[0]
            .1
            .starts_with("Recorded as install id-1; the destination now reports <U+0456>d-1,"),
        "{mismatch:?}"
    );
    let unverified = lines_of(HostStateView::IdentityUnverified {
        recorded: tricky.into(),
    });
    assert!(unverified[0].1.ends_with(spelled), "{unverified:?}");
    let duplicate = lines_of(HostStateView::Duplicate {
        twin: 3,
        identity: tricky.into(),
    });
    assert!(
        duplicate[0]
            .1
            .starts_with(&format!("This entry reaches install {spelled},")),
        "{duplicate:?}"
    );
}

// ---- Sandboxes and containers --------------------------------------------------------------------------------------

/// `host`'s detail lines, each as (its kind, its accessible name), times in UTC.
fn lines_for(host: &farhelm_proto::ui_channel::HostView) -> Vec<(Kind, String)> {
    detail_lines(host, |_| None, Zone::UTC)
        .into_iter()
        .map(|(kind, line)| (kind, line.accessible_name().to_string()))
        .collect()
}

/// A Tensorlake sandbox host `farhelm-host-k7q2` from `rust-dev` at 2 CPUs and 4 GB, connected, with `tunnels`.
fn sandbox(tunnels: Option<TunnelView>) -> farhelm_proto::ui_channel::HostView {
    let mut view = host(4, "tensorlake", connected());
    view.tensorlake = Some(farhelm_proto::ui_channel::TensorlakeHostView {
        sandbox: "farhelm-host-k7q2".into(),
        recipe: "rust-dev".into(),
        size: Some(RecipeSize {
            cpus: 2,
            memory_gb: 4,
            disk_gb: 20,
        }),
        tunnels,
        credential_warning: None,
    });
    view
}

/// Forwards of one port in `state`.
fn forwards(state: TunnelState) -> TunnelView {
    TunnelView {
        tunnels: vec![Tunnel {
            sandbox_port: 8080,
            host: "127.0.0.1".into(),
            port: 9090,
        }],
        state,
    }
}

/// Why this matters: the details are where a sandbox's or container's row says which machine it is, what it runs and
/// how its forwards are doing (the old `host-tensorlake-detail`, `host-environment-detail` and `TunnelDetails`), since
/// its destination is Farhelm's own and says nothing to the user.
///
/// Specified: a sandbox's details start "Sandbox: farhelm-host-k7q2 · Recipe: rust-dev · 2 CPUs · 4 GB memory", then
/// its forwards' health and one line per forward ("127.0.0.1:8080 → 127.0.0.1:9090"), in each health: up, failed with
/// the reason, retrying after a failure ("retrying in 4s") and connecting; a container placed on a container host
/// starts "Container: crate-1 · Recipe: dev · Container host: build-box".
#[test]
fn a_machines_details_name_it_and_its_forwards() {
    let up = lines_for(&sandbox(Some(forwards(TunnelState::Up))));
    assert_eq!(
        up[..3],
        [
            (
                Kind::Evidence,
                "Sandbox: farhelm-host-k7q2 · Recipe: rust-dev · 2 CPUs · 4 GB memory".to_owned()
            ),
            (Kind::Evidence, "Tunnels up".to_owned()),
            (Kind::Evidence, "127.0.0.1:8080 → 127.0.0.1:9090".to_owned()),
        ]
    );
    for (state, health) in [
        (
            TunnelState::Failed {
                reason: "listen port taken".into(),
            },
            "Tunnels failed: listen port taken",
        ),
        (
            TunnelState::Retrying {
                reason: Some("remote port forwarding failed".into()),
                delay_secs: 4,
            },
            "Tunnels failed; retrying in 4s: remote port forwarding failed",
        ),
        (
            TunnelState::Retrying {
                reason: None,
                delay_secs: 1,
            },
            "Tunnels connecting…",
        ),
    ] {
        assert_eq!(lines_for(&sandbox(Some(forwards(state))))[1].1, health);
    }
    let mut container = host(5, "podman", connected());
    container.environment = Some(farhelm_proto::EnvironmentHostView {
        name: "crate-1".into(),
        place_name: Some("build-box".into()),
        target: farhelm_proto::LaunchTarget::Environment {
            recipe: "dev".into(),
            place: farhelm_proto::Place::PodmanHost(2),
        },
        tunnels: None,
        credential_warning: None,
    });
    assert_eq!(
        lines_for(&container)[0].1,
        "Container: crate-1 · Recipe: dev · Container host: build-box"
    );
}

/// Why this matters: a failed deletion may leave a sandbox and its snapshots billing, which the old row said in the
/// provider's words, and those words come from the one per-provider module.
///
/// Specified: a sandbox being deleted says "Farhelm is deleting this sandbox, its files and its snapshots"; failed, it
/// says Farhelm could not finish, that some may remain and bill, and that Retry finishes it, then the reason; a
/// container's say "this container"; the remedy stays "Retry to delete it again".
#[test]
fn a_deletion_is_worded_for_the_hosts_provider() {
    let mut deleting = sandbox(None);
    deleting.state = HostStateView::Deleting { failure: None };
    assert!(
        lines_for(&deleting).iter().any(
            |(_, line)| line == "Farhelm is deleting this sandbox, its files and its snapshots"
        )
    );
    deleting.state = HostStateView::Deleting {
        failure: Some("Tensorlake refused".into()),
    };
    let failed = lines_for(&deleting);
    assert!(failed.iter().any(|(_, line)| line
        == "Farhelm could not finish deleting this sandbox and its snapshots; some of them may remain and bill. \
            Retry finishes the deletion: Tensorlake refused"));
    assert_eq!(
        failed.last(),
        Some(&(Kind::Remedy, "Retry to delete it again".to_owned()))
    );
    let mut container = host(5, "podman", HostStateView::Deleting { failure: None });
    container.name = "crate-1".into();
    assert!(
        lines_for(&container)
            .iter()
            .any(|(_, line)| line == "Farhelm is deleting this container and its files")
    );
}

/// Why this matters: agents may work while their inference endpoint is unreachable, so a connected host whose forwards
/// failed must read as needing attention in its row and on the rail (the old "tunnel failures make a connected host
/// amber"), while forwards still starting claim nothing and a host that is not connected keeps its own phase's color.
///
/// Specified: connected with failed forwards, or retrying after a failure, the host's tone is advisory and
/// `tunnels_failed` is true, while the phase's own tone stays connected; with forwards up or connecting it is
/// connected; an unreachable host with failed forwards keeps its phase's tone and does not count as failed forwards.
#[test]
fn failed_forwards_make_a_connected_host_advisory() {
    for state in [
        TunnelState::Failed {
            reason: "listen port taken".into(),
        },
        TunnelState::Retrying {
            reason: Some("refused".into()),
            delay_secs: 4,
        },
    ] {
        let failed = sandbox(Some(forwards(state)));
        assert!(tunnels_failed(&failed));
        assert_eq!(host_tone(&failed), Tone::Advisory);
        assert_eq!(
            tone(&failed.state),
            Tone::Connected,
            "the phase is unchanged"
        );
    }
    for state in [
        TunnelState::Up,
        TunnelState::Retrying {
            reason: None,
            delay_secs: 1,
        },
    ] {
        let fine = sandbox(Some(forwards(state)));
        assert!(!tunnels_failed(&fine));
        assert_eq!(host_tone(&fine), Tone::Connected);
    }
    let mut away = sandbox(Some(forwards(TunnelState::Failed {
        reason: "gone".into(),
    })));
    away.state = HostStateView::Unreachable {
        cause: "transport-failure".into(),
        last_error: "socket closed".into(),
    };
    assert!(!tunnels_failed(&away));
    assert_eq!(host_tone(&away), tone(&away.state));
}

/// Why this matters: an agent in a sandbox or container whose credentials the helm could not deliver may run without
/// the keys it needs, and delivery can fail during setup too, so the old row said why and turned amber whatever its
/// phase ("connected credential warnings remain readable").
///
/// Specified: a sandbox whose credentials were not delivered has the details line "Credentials not delivered: the
/// vault refused" after its machine's line, and its host tone is advisory both connected and connecting, while the
/// phase's own tone is unchanged.
#[test]
fn credentials_not_delivered_are_said_and_turn_the_host_amber() {
    let mut undelivered = sandbox(None);
    if let Some(binding) = undelivered.tensorlake.as_mut() {
        binding.credential_warning = Some("the vault refused".into());
    }
    assert_eq!(
        lines_for(&undelivered)[1],
        (
            Kind::Evidence,
            "Credentials not delivered: the vault refused".to_owned()
        )
    );
    assert_eq!(host_tone(&undelivered), Tone::Advisory);
    undelivered.state = connecting(1, None);
    assert_eq!(host_tone(&undelivered), Tone::Advisory);
    assert_eq!(tone(&undelivered.state), Tone::Pending);
}
