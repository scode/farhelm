//! Decode the helm's golden HTTP fixtures with this crate's hand-kept mirrors.
//!
//! The helm-shaped views ([`Session`], [`Host`] and their reply envelopes)
//! stay local mirrors on purpose: they tolerate words a newer helm may send,
//! because a browser tab loaded before a helm upgrade keeps running old code.
//! That leaves nothing compiled tying them to the helm, so
//! `farhelm-helm`'s `http_contract_tests` pins what the helm serializes into
//! the shared files under `crates/farhelm-helm/http-contract/`, and these tests
//! decode the same files here. A helm-side change fails there first; the
//! updated fixture then fails here if this crate cannot read the new shape.
//!
//! `Profile` and `SourceProfile` are different: the helm passes farhelm-proto's
//! types through unchanged, so their contract test encodes with the proto type
//! directly instead of going through a fixture.

use super::{HostListing, SessionListBody};
use crate::{
    HostKind, HostPhase, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection,
    ProfileExistence, RefreshHealth, RestartOffer, SessionStatus,
};

const SESSION_LIST_JSON: &str =
    include_str!("../../../farhelm-helm/http-contract/session-list.json");
const HOST_LIST_JSON: &str = include_str!("../../../farhelm-helm/http-contract/host-list.json");

/// Why this matters: a row field the UI silently fails to read (a renamed
/// key decodes to its default) shows wrong status, ages, or host data with no
/// error anywhere.
///
/// Specification: the helm's fully populated session-list fixture decodes
/// through the UI's envelope, and every field the mirror declares carries the
/// fixture's value rather than a default.
#[farhelm_testtrace::test]
fn the_helm_session_list_fixture_decodes_with_every_mirrored_field() {
    let body: SessionListBody = serde_json::from_str(SESSION_LIST_JSON).unwrap();
    assert_eq!(body.total, 5);
    assert_eq!(body.matching, Some(1));
    assert!(body.truncated);
    let [row] = body.sessions.as_slice() else {
        panic!("the fixture holds exactly one row");
    };
    assert_eq!(row.id, "fh-0123abcd");
    assert_eq!(row.title, "Fix the widget");
    assert_eq!(row.cwd, "~/src/widgets");
    assert_eq!(row.canonical_cwd.as_deref(), Some("/home/user/src/widgets"));
    assert_eq!(row.invocation, "codex --model gpt-5");
    assert_eq!(
        row.launch,
        Some(LaunchSelection {
            harness: LaunchHarness::Codex,
            model: Some("gpt-5".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::SmartApprove),
            workspace_trust: Some(true),
        })
    );
    assert_eq!(row.status, SessionStatus::Exited { exit_code: Some(3) });
    assert_eq!(row.annotation.as_deref(), Some("left a note"));
    assert_eq!(row.restart_offer, RestartOffer::Resume);
    assert_eq!(row.created_at, 1_700_000_000);
    assert_eq!(row.last_activity_at, 1_700_000_100);
    assert_eq!(row.tabs.len(), 1);
    assert_eq!(row.tabs[0].id, "tab-1");
    assert_eq!(row.host, Some(2));
    assert_eq!(
        row.host_identity,
        Some(Some("install-identity".to_string()))
    );
    assert_eq!(row.host_name.as_deref(), Some("buildbox"));
    assert!(row.stale);
    let profile = row.source_profile.as_ref().expect("source_profile decodes");
    assert_eq!(
        (
            profile.id.as_str(),
            profile.name.as_str(),
            profile.existence
        ),
        ("profile-1", "Codex", ProfileExistence::Renamed)
    );
    assert_eq!(row.seen_activity_at, Some(Some(1_700_000_090)));
    let repo = row.github_repo.as_ref().expect("github_repo decodes");
    assert_eq!(
        (repo.owner.as_str(), repo.name.as_str()),
        ("octo", "widgets")
    );
    let working_copy = row.working_copy.as_ref().expect("working_copy decodes");
    assert_eq!(working_copy.id, "wc-1");
    assert_eq!(
        (
            working_copy.repo.owner.as_str(),
            working_copy.repo.name.as_str()
        ),
        ("octo", "widgets")
    );
    assert_eq!(working_copy.canonical_path, "/home/user/src/widgets");
    assert_eq!(working_copy.origin_session_id, "fh-0123abcd");
}

/// Why this matters: the host panel picks its chip and its remedy from the
/// phase, and an unrecognized phase deliberately degrades to a vaguer row, so
/// a helm-side rename would pass silently without this pin.
///
/// Specification: every host in the helm's host-list fixture decodes to a
/// recognized kind, and every phase decodes to exactly the fixture's variant
/// and payload, optional fields included.
#[farhelm_testtrace::test]
fn the_helm_host_list_fixture_decodes_every_phase_as_recognized() {
    let listing: HostListing = serde_json::from_str(HOST_LIST_JSON).unwrap();
    let phases: Vec<&HostPhase> = listing.hosts.iter().map(|h| &h.state).collect();
    assert_eq!(phases.len(), 10);
    assert!(
        listing
            .hosts
            .iter()
            .all(|h| !matches!(h.kind, HostKind::Unrecognized)),
        "every fixture host has a known kind"
    );
    assert!(
        phases.iter().all(|p| !matches!(p, HostPhase::Unrecognized)),
        "every fixture phase is one this build recognizes: {phases:?}"
    );
    assert_eq!(
        *phases[1],
        HostPhase::Connecting {
            attempt: 2,
            last_error: Some("connection refused".to_string()),
        }
    );
    assert_eq!(
        *phases[2],
        HostPhase::Unreachable {
            cause: "transport-failure".to_string(),
            last_error: "timed out".to_string(),
        }
    );
    assert_eq!(
        *phases[4],
        HostPhase::IdentityMismatch {
            recorded: "old".to_string(),
            reported: "new".to_string(),
        }
    );
    assert_eq!(
        *phases[5],
        HostPhase::IdentityUnverified {
            recorded: "old".to_string(),
        }
    );
    assert_eq!(
        *phases[6],
        HostPhase::Duplicate {
            twin: 2,
            identity: "identity2".to_string(),
        }
    );
    assert_eq!(
        *phases[7],
        HostPhase::Retired {
            reason: "removed".to_string(),
        }
    );
    let local = &listing.hosts[0];
    assert_eq!(local.kind, HostKind::Local);
    assert_eq!(local.destination, None);
    assert_eq!(local.alias, Some(None));
    let ssh = &listing.hosts[1];
    assert_eq!(ssh.kind, HostKind::Ssh);
    assert_eq!(ssh.destination.as_deref(), Some("user@host2"));
    assert_eq!(ssh.alias, Some(Some("alias2".to_string())));
    assert_eq!(ssh.name, "host2");
    assert_eq!(ssh.identity.as_deref(), Some("identity2"));
    assert_eq!(ssh.remote_farhelm.as_deref(), Some("~/.local/bin/farhelm"));
    assert_eq!(
        ssh.remote_state_dir.as_deref(),
        Some("~/.local/state/farhelm")
    );
    assert_eq!(ssh.incarnation, 4);
    assert_eq!(
        *phases[0],
        HostPhase::Connected {
            identity: Some("identity1".to_string()),
            build_version: "0.16.0".to_string(),
            old_version: false,
            refresh: RefreshHealth::Ok { sessions: 3 },
        }
    );
    assert_eq!(
        *phases[3],
        HostPhase::VersionSkew {
            peer_protocol: 28,
            peer_build: "0.15.0".to_string(),
            our_protocol: 29,
            our_build: "0.16.0".to_string(),
            remediation: "Update the host.".to_string(),
        }
    );
    assert_eq!(
        *phases[8],
        HostPhase::Connected {
            identity: None,
            build_version: "0.15.0".to_string(),
            old_version: true,
            refresh: RefreshHealth::Failed {
                error: "list failed".to_string(),
            },
        }
    );
    assert_eq!(
        *phases[9],
        HostPhase::Connected {
            identity: None,
            build_version: "0.16.0".to_string(),
            old_version: false,
            refresh: RefreshHealth::Pending,
        }
    );
}

/// Why this matters: the profile catalog and each row's source profile are
/// farhelm-proto types the helm forwards unchanged, while this crate keeps its
/// own tolerant copies (a kind stays a string so an edit never rewrites a word
/// a newer helm introduced).
///
/// Specification: a proto `Profile` and `SourceProfile`, serialized as the
/// helm serializes them, decode into this crate's mirrors with the same
/// values, including the kind's wire spelling.
#[farhelm_testtrace::test]
fn proto_profile_types_decode_into_the_ui_mirrors() {
    let profile = farhelm_proto::Profile {
        id: "profile-1".to_string(),
        builtin: true,
        name: "Codex".to_string(),
        invocation: "codex".to_string(),
        agent_kind: farhelm_proto::AgentKind::Codex,
        resume_template: Some(vec!["codex".to_string(), "resume".to_string()]),
    };
    let decoded: crate::Profile =
        serde_json::from_value(serde_json::to_value(&profile).unwrap()).unwrap();
    assert_eq!(decoded.id, "profile-1");
    assert!(decoded.builtin);
    assert_eq!(decoded.name, "Codex");
    assert_eq!(decoded.invocation, "codex");
    assert_eq!(decoded.agent_kind, "codex");
    assert_eq!(decoded.resume_template, profile.resume_template);

    for (existence, expected) in [
        (
            farhelm_proto::ProfileExistence::Present,
            ProfileExistence::Present,
        ),
        (
            farhelm_proto::ProfileExistence::Renamed,
            ProfileExistence::Renamed,
        ),
        (
            farhelm_proto::ProfileExistence::Deleted,
            ProfileExistence::Deleted,
        ),
    ] {
        let source = farhelm_proto::SourceProfile {
            id: "profile-1".to_string(),
            name: "Codex".to_string(),
            existence,
        };
        let decoded: crate::SourceProfile =
            serde_json::from_value(serde_json::to_value(&source).unwrap()).unwrap();
        assert_eq!(decoded.existence, expected);
        assert_eq!(
            (decoded.id.as_str(), decoded.name.as_str()),
            ("profile-1", "Codex")
        );
    }
}
