//! End-to-end integration tests: real supervisor, real private tmux,
//! real launch path through the user's login shell, fake agent as the
//! process under supervision.
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

mod harness;
mod session_lifecycle;

mod terminal_backpressure;

mod boot_id_durable_outcome;
mod launch_sentinel_error_status;

mod conversation_identity_capture;
mod create_idempotency;
mod real_agent_capture;
mod restart_under_concurrency;
mod restart_with_resume;

mod marker_model;
mod tab_lifecycle_edges;
mod terminal_tabs;

mod attachment_uploads;
