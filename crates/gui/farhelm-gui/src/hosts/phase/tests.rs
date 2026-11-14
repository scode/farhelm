//! Tests of what a host row says: the phase words and tones for every phase the helm reports, the Too new hover, and
//! the detail lines, ported from the old `hosts.rs` tests at reference commit `24bc0580`
//! (`old_connected_hosts_have_advisory_display_without_skew_classification`,
//! `a_newer_host_reads_too_new_with_both_versions_on_hover`, `a_connected_hosts_detail_reports_its_refresh_health`, the
//! remedy cases). Pure functions of the pushed state; what the row draws from them is tested in `hosts/tests.rs`.

use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{ConnectionStage, HostStateView, RefreshView};

use super::{Kind, Tone, detail_lines, hover, tone, utc_stamp, words};
use crate::host_text::HostText;
use crate::hosts::tests::{connected, connecting, host, local_down, skew};

/// The detail lines of `state` on an unaliased SSH host, each as (its kind, its accessible name).
fn lines_of(state: HostStateView) -> Vec<(Kind, String)> {
    lines_with_twin(state, |_| None)
}

/// [`lines_of`] with `twin` naming hosts by id.
fn lines_with_twin(
    state: HostStateView,
    twin: impl Fn(i64) -> Option<HostText>,
) -> Vec<(Kind, String)> {
    detail_lines(&host(2, "ssh", state), twin)
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

/// Why this matters: the helm's own machine with no supervisor is fixed by a command on that machine, and only the
/// helm knows the state directory it must name; the old app printed the helm's dial failure as the remedy (and its own
/// words only when the helm said nothing), keeping the detail line to what happened. Specified: the detail says no
/// supervisor is running; the remedy is "The helm reports: " and the helm's words, or the app's fallback when they are
/// blank; another unreachable host shows the helm's error, or "The host did not answer", and no remedy.
#[test]
fn the_local_hosts_missing_supervisor_shows_the_helms_remedy() {
    assert_eq!(
        lines_of(local_down(
            "start it with `farhelm supervisor run --state-dir /srv/state`"
        )),
        [
            (
                Kind::Evidence,
                "No supervisor is running on this machine".to_owned()
            ),
            (
                Kind::Remedy,
                "The helm reports: start it with `farhelm supervisor run --state-dir /srv/state`"
                    .to_owned()
            ),
        ]
    );
    assert_eq!(
        lines_of(local_down("  "))
            .last()
            .map(|(_, line)| line.as_str()),
        Some(
            "Start a supervisor on this machine with `farhelm supervisor run`, passing the same `--state-dir` this \
             helm was started with if it has one"
        )
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
                "Adopt the identity the host reports, or fix the destination; adopting resets this host to asking \
                 before YOLO launches"
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
    let first = detail_lines(&aliased, |_| None)
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

/// Why this matters: the environment phases' details carry the time something was asked or noticed, as an unambiguous
/// UTC stamp, since the GUI carries no time zone database. Specified: the stamp's format at a known instant, before the
/// epoch, and in a suspended host's detail.
#[test]
fn environment_times_read_as_utc_stamps() {
    assert_eq!(utc_stamp(1_700_000_000), "2023-11-14 22:13:20 UTC");
    assert_eq!(utc_stamp(-1), "1969-12-31 23:59:59 UTC");
    assert_eq!(
        lines_of(HostStateView::Suspended {
            observed_at: 1_700_000_000_000,
            suspended_by_farhelm_at: None,
            resume_failed: Some("quota".into()),
        }),
        [(
            Kind::Evidence,
            "Suspension noticed at 2023-11-14 22:13:20 UTC. Resume failed: quota".to_owned()
        )]
    );
}
