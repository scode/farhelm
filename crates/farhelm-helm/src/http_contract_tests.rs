//! Golden JSON for the helm's HTTP views that the browser mirrors by hand.
//!
//! `farhelm-ui` keeps its own decoders for [`SessionListBody`] and
//! [`HostView`] rather than sharing these types, because the helm shapes them
//! for HTTP and the UI deliberately tolerates words a newer helm may add (a
//! browser tab loaded before a helm upgrade keeps running old code). Nothing
//! compiled ties the two sides together, so drift used to surface only in the
//! browser suite, which CI does not run.
//!
//! The files under `crates/farhelm-helm/http-contract/` are the contract. The
//! tests here prove that the helm serializes exactly those bytes' JSON value,
//! and `farhelm-ui`'s tests decode the same files with the UI's types. A field
//! added or renamed on the helm side therefore fails here first; updating the
//! fixture then runs it through the UI decoder, which is where a UI that
//! cannot read the new shape fails.
//!
//! The values are fully populated on purpose: an `Option` left at `None` or a
//! defaulted field would let a rename of that field slip through both sides.

use crate::aggregate::{SessionListBody, SessionRow};
use crate::hosts::{HostStateView, HostView, RefreshView};
use farhelm_proto::{
    GithubRepo, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, ProfileExistence,
    RestartOffer, SessionInfo, SessionStatus, SourceProfile, TabInfo, WorkingCopyInfo,
};

/// The one session-list fixture shared with `farhelm-ui`'s decoder tests.
const SESSION_LIST_JSON: &str = include_str!("../http-contract/session-list.json");

/// The one host-list fixture shared with `farhelm-ui`'s decoder tests.
const HOST_LIST_JSON: &str = include_str!("../http-contract/host-list.json");

/// A `GET /api/sessions` body whose single row sets every field the wire
/// carries, including the helm's own row fields beside the flattened
/// `SessionInfo`.
fn session_list_body() -> SessionListBody {
    let repo = GithubRepo {
        owner: "octo".to_string(),
        name: "widgets".to_string(),
    };
    let info = SessionInfo {
        id: "fh-0123abcd".to_string(),
        parent: Some("fh-89abcdef".to_string()),
        title: "Fix the widget".to_string(),
        created_at: 1_700_000_000,
        last_activity_at: 1_700_000_100,
        last_work_started_at: 1_700_000_050_000,
        creation_seq: Some(7),
        cwd: "~/src/widgets".to_string(),
        canonical_cwd: Some("/home/user/src/widgets".to_string()),
        invocation: "codex --model gpt-5".to_string(),
        resume_template: Some(vec![
            "codex".to_string(),
            "resume".to_string(),
            "{conversation}".to_string(),
        ]),
        launch: Some(LaunchSelection {
            harness: LaunchHarness::Codex,
            model: Some("gpt-5".to_string()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::SmartApprove),
            workspace_trust: Some(true),
        }),
        status: SessionStatus::Exited { exit_code: Some(3) },
        annotation: Some("left a note".to_string()),
        restart_offer: RestartOffer::Resume,
        tabs: vec![TabInfo {
            id: "tab-1".to_string(),
        }],
        source_profile: Some(SourceProfile {
            id: "profile-1".to_string(),
            name: "Codex".to_string(),
            existence: ProfileExistence::Renamed,
        }),
        github_repo: Some(repo.clone()),
        working_copy: Some(WorkingCopyInfo {
            id: "wc-1".to_string(),
            repo,
            canonical_path: "/home/user/src/widgets".to_string(),
            origin_session_id: "fh-0123abcd".to_string(),
        }),
    };
    SessionListBody {
        sessions: vec![SessionRow {
            info,
            host: 2,
            host_identity: Some("install-identity".to_string()),
            host_name: "buildbox".to_string(),
            stale: true,
            seen_activity_at: Some(1_700_000_090),
        }],
        total: 5,
        matching: Some(1),
        truncated: true,
    }
}

/// A `GET /api/hosts` body with one host per connection phase, so every
/// `HostStateView` variant's payload is pinned.
fn host_list_body() -> serde_json::Value {
    let host = |id: i64, kind: &'static str, state: HostStateView| HostView {
        id,
        kind,
        destination: (kind == "ssh").then(|| format!("user@host{id}")),
        alias: (kind == "ssh").then(|| format!("alias{id}")),
        name: format!("host{id}"),
        identity: Some(format!("identity{id}")),
        remote_farhelm: (kind == "ssh").then(|| "~/.local/bin/farhelm".to_string()),
        remote_state_dir: (kind == "ssh").then(|| "~/.local/state/farhelm".to_string()),
        state,
        incarnation: 4,
    };
    let hosts = vec![
        host(
            1,
            "local",
            HostStateView::Connected {
                identity: Some("identity1".to_string()),
                build_version: "0.16.0".to_string(),
                old_version: false,
                refresh: RefreshView::Ok { sessions: 3 },
            },
        ),
        host(
            2,
            "ssh",
            HostStateView::Connecting {
                attempt: 2,
                last_error: Some("connection refused".to_string()),
            },
        ),
        host(
            3,
            "ssh",
            HostStateView::Unreachable {
                cause: "transport-failure",
                last_error: "timed out".to_string(),
            },
        ),
        host(
            4,
            "ssh",
            HostStateView::VersionSkew {
                peer_protocol: 28,
                peer_build: "0.15.0".to_string(),
                our_protocol: 29,
                our_build: "0.16.0".to_string(),
                remediation: "Update the host.".to_string(),
            },
        ),
        host(
            5,
            "ssh",
            HostStateView::IdentityMismatch {
                recorded: "old".to_string(),
                reported: "new".to_string(),
            },
        ),
        host(
            6,
            "ssh",
            HostStateView::IdentityUnverified {
                recorded: "old".to_string(),
            },
        ),
        host(
            7,
            "ssh",
            HostStateView::Duplicate {
                twin: 2,
                identity: "identity2".to_string(),
            },
        ),
        host(
            8,
            "ssh",
            HostStateView::Retired {
                reason: "removed".to_string(),
            },
        ),
        host(
            9,
            "ssh",
            HostStateView::Connected {
                identity: None,
                build_version: "0.15.0".to_string(),
                old_version: true,
                refresh: RefreshView::Failed {
                    error: "list failed".to_string(),
                },
            },
        ),
        host(
            10,
            "ssh",
            HostStateView::Connected {
                identity: None,
                build_version: "0.16.0".to_string(),
                old_version: false,
                refresh: RefreshView::Pending,
            },
        ),
    ];
    // The route builds its envelope with `json!` rather than a struct
    // (`hosts::list_hosts`); this is the same envelope.
    serde_json::json!({ "hosts": hosts })
}

/// Parse a fixture, naming the file on failure so a hand edit that broke the
/// JSON is obvious.
fn fixture(name: &str, text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{name} is not valid JSON: {e}"))
}

/// Why this matters: the UI decodes `GET /api/sessions` with a hand-kept
/// mirror, and a helm-side rename would otherwise reach users as rows that
/// silently lose a field.
///
/// Specification: the helm's session-list body, with every field set,
/// serializes to exactly the shared fixture's JSON value.
#[farhelm_testtrace::test]
fn session_list_body_serializes_to_the_shared_fixture() {
    let actual = serde_json::to_value(session_list_body()).unwrap();
    assert_eq!(
        actual,
        fixture("session-list.json", SESSION_LIST_JSON),
        "the helm's session-list JSON changed; update \
         crates/farhelm-helm/http-contract/session-list.json to:\n{}",
        serde_json::to_string_pretty(&actual).unwrap()
    );
}

/// Why this matters: the host panel keys every chip and remedy off the
/// `phase` tag and its payload, and the UI's decoder is hand-kept.
///
/// Specification: a host list with one host per connection phase serializes
/// to exactly the shared fixture's JSON value.
#[farhelm_testtrace::test]
fn host_list_serializes_to_the_shared_fixture() {
    let actual = host_list_body();
    assert_eq!(
        actual,
        fixture("host-list.json", HOST_LIST_JSON),
        "the helm's host-list JSON changed; update \
         crates/farhelm-helm/http-contract/host-list.json to:\n{}",
        serde_json::to_string_pretty(&actual).unwrap()
    );
}
