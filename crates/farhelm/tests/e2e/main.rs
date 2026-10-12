//! End-to-end integration tests: real supervisor, real private tmux,
//! real launch path through the user's login shell, fake agent as the
//! process under supervision.
//! The suite is Linux-only because its process-lifetime oracles read
//! `/proc`; gating keeps other platforms from passing on blind observations.
//!
//! These tests exercise the exact production code path — the same
//! `handle_connection` the unix socket serves and the same
//! `SupervisorClient` the helm uses — over an in-process duplex pipe.
//! What they deliberately do NOT fake: tmux, control-mode streaming,
//! `send-keys -H` input, `capture-pane` replay, and pane-mode
//! restoration.
//! If tmux behavior drifts from the audited assumptions in SPEC_impl.md,
//! it fails here first.
//!
//! They live in the `farhelm` bin crate because `CARGO_BIN_EXE_farhelm`
//! (the built multi-call binary, which carries the fake agent and the
//! launch shim) is only available to the defining crate's tests.
//!
//! This directory mirrors the file's own `// -----` section banners, one
//! module per banner, declared below grouped by milestone era (rustfmt
//! owns the order within each group). `harness` and `session_lifecycle`
//! are the exception: together they are everything that came before the
//! first banner in the original single file. `harness` holds the shared
//! fixtures, drivers, and poll helpers every module (including
//! `session_lifecycle` itself) builds on; `session_lifecycle` holds the
//! large body of unbannered lifecycle coverage the banner convention
//! never organized, kept as one unit because the source gives it no
//! finer seams.
//! Cargo still builds this whole directory as the one `e2e`
//! integration-test binary; splitting it into modules changes nothing
//! about what runs. It DOES change what the tests are called: libtest
//! now reports `module::test_name`, so substring filters (the default,
//! and what CI and the flake watchlist rely on) keep working while an
//! `--exact` filter needs the qualified name — a deliberate trade for
//! real modules with real per-module imports.

#[cfg(target_os = "linux")]
#[path = "../cli_support/mod.rs"]
pub mod cli_support;
#[cfg(target_os = "linux")]
mod harness;
#[cfg(target_os = "linux")]
mod session_lifecycle;

#[cfg(target_os = "linux")]
mod terminal_backpressure;

#[cfg(target_os = "linux")]
mod boot_id_durable_outcome;
#[cfg(target_os = "linux")]
mod launch_sentinel_error_status;

#[cfg(target_os = "linux")]
mod codex_identity;
#[cfg(target_os = "linux")]
mod create_idempotency;
#[cfg(target_os = "linux")]
mod github_checkouts;
#[cfg(target_os = "linux")]
mod hook_identity;
#[cfg(target_os = "linux")]
mod real_agent_capture;
#[cfg(target_os = "linux")]
mod restart_under_concurrency;
#[cfg(target_os = "linux")]
mod restart_with_resume;
#[cfg(target_os = "linux")]
mod structured_launches;
#[cfg(target_os = "linux")]
mod wrapper_launch;

#[cfg(target_os = "linux")]
mod marker_model;
#[cfg(target_os = "linux")]
mod tab_lifecycle_edges;
#[cfg(target_os = "linux")]
mod terminal_tabs;

#[cfg(target_os = "linux")]
mod attachment_uploads;
#[cfg(target_os = "linux")]
mod file_downloads;

#[cfg(target_os = "linux")]
mod replay_marker;
#[cfg(target_os = "linux")]
mod session_rename;
#[cfg(target_os = "linux")]
mod supervisor_stop;

#[cfg(target_os = "linux")]
mod host_connection;
#[cfg(target_os = "linux")]
mod merged_hosts;

#[cfg(target_os = "linux")]
mod agent_listing_real_stack;
#[cfg(target_os = "linux")]
mod agent_relay;
#[cfg(target_os = "linux")]
mod change_hints;

/// Report the unsupported substrate instead of certifying unobserved behavior.
///
/// The shared process oracles require Linux's `/proc`. A visible skip keeps
/// an empty or vacuously passing suite from looking like execution evidence.
#[cfg(not(target_os = "linux"))]
#[test]
fn e2e_requires_linux_process_oracles() {
    eprintln!("SKIPPED: e2e requires Linux /proc process-lifetime oracles");
}
