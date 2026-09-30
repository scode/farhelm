//! The supervisor's per-agent behavior that needs the supervisor itself:
//! one file per agent kind, each an `impl Supervisor` block.
//!
//! This is the stateful half of a kind's integration. The pure half (launch
//! argv, hook injection decisions, record and locator parsing, screen
//! reading) lives in `crate::agent_kind`, and the process-tree corridors in
//! `crate::procs`. What lives here reads and writes the store, takes the
//! capture claim, or walks the live process tree: admitting a kind's
//! conversation report, refreshing or verifying its captured identity, and
//! recording launch details its admission proof checks later.
//!
//! These are child modules of `service::core` so they can use its private
//! helpers and fields directly; being in a separate file is organization,
//! not encapsulation. Their tracing events name `target: LOG_TARGET` (the
//! core module's path, see its docs), so moving code here changes no log line;
//! keep doing that for new events. The dispatch into them stays in `core`, as exhaustive
//! matches over `AgentKind`, so a new kind is a compile error at each
//! decision until it states its answer. A kind with no entry here (Goose,
//! Pi, Generic) has no stateful supervisor behavior of its own.

mod claude;
mod codex;
mod grok;
mod omp;
