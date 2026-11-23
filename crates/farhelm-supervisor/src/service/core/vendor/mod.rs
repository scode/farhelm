//! The supervisor's per-agent behavior that needs the supervisor itself:
//! one file per agent kind, with positional admission shared by Claude, Muse, Antigravity
//! and custom agents, and one process-pin judge shared by every pinned kind.
//!
//! This is the stateful half of a kind's integration. The pure half (launch
//! argv, hook injection decisions, record and locator parsing, screen
//! reading) lives in `crate::agent_kind`, and the process-tree corridors in
//! `crate::procs`. What lives here reads and writes the store, takes the
//! capture claim, or reads a report's process evidence: admitting a kind's
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

use crate::agent_kind::report_pin::{PinDecision, ReportPin};
use crate::procs::{self, ChainLink, ProcessIdentity, ProcessState};

mod codex;
mod grok;
mod omp;
pub(super) mod positional;

/// Resolve process authority under the report's capture claim.
///
/// Stable identity and recorded ancestors can settle a report without a
/// process lookup. Only a reliable liveness read can release a pin that
/// would otherwise refuse the report. The caller interprets unavailable
/// evidence according to the vendor's event policy.
fn judge_report_pin(
    pin: Option<&ReportPin>,
    emitter: ProcessIdentity,
    chain: &[ChainLink],
    process_fault: impl FnOnce(u32) -> Option<String>,
) -> Result<PinDecision, String> {
    let Some(pin) = pin else {
        return Ok(PinDecision::Accept);
    };
    let initial = pin.judge(emitter, true, chain);
    if initial != PinDecision::Refuse {
        return Ok(initial);
    }
    if let Some(reason) = process_fault(pin.process.pid) {
        return Err(reason);
    }
    let alive = procs::read_process(pin.process.pid)?.is_some_and(|(_, start, state)| {
        start == pin.process.start && state == ProcessState::Running
    });
    Ok(pin.judge(emitter, alive, chain))
}
