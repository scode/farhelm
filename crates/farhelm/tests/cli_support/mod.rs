//! Shared execution boundary for CLI tests that do not supply standard input.

use farhelm_teststate::process::{CommandRunLimits, run_bounded};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Collect complete CLI output without letting pipe backpressure or inherited
/// descriptors defeat the test's execution deadline.
///
/// The command gets ten seconds to run and one additional second for direct-child
/// cleanup. The shared runner drains both pipes during execution; this adapter
/// rejects incomplete or capped output so callers' exact-output assertions never
/// mistake a retained prefix for the whole reply. Four MiB per stream accommodates
/// the large refusal fixtures while bounding evidence from a broken command.
/// Standard input is closed; tests whose contract requires an open input pipe
/// must keep their own fixture. Child arguments, cwd and environment are preserved.
pub fn output_with_timeout(command: Command) -> Output {
    output_before_deadline(command, Instant::now() + Duration::from_secs(10))
        .expect("CLI execution deadline expired before command execution")
}

/// Collect a command within a caller's remaining readiness budget.
///
/// The deadline is checked when execution starts, including time spent queued
/// for a blocking worker. Once it expires, the runner retains one additional
/// second for direct-child cleanup. A dropped async waiter must not abandon an
/// already-running child, so callers run this owned operation to completion on
/// a blocking worker rather than timing out the worker's join handle.
/// Returns `None` when no execution time remains before spawning, so readiness
/// callers can report their boundary and last observation without exposing the
/// command's potentially credential-bearing environment.
pub fn output_before_deadline(mut command: Command, deadline: Instant) -> Option<Output> {
    const OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return None;
    }
    let limits = CommandRunLimits::new(
        remaining + Duration::from_secs(1),
        Duration::from_secs(1),
        OUTPUT_LIMIT,
        OUTPUT_LIMIT,
        OUTPUT_LIMIT * 2 + 1,
    )
    .expect("valid CLI test execution limits");
    let outcome = run_bounded(&mut command, &limits).expect("representable CLI test deadline");
    assert!(
        !outcome.timed_out
            && outcome.errors.is_empty()
            && !outcome.errors_truncated
            && outcome.direct_child_reaped
            && !outcome.ownership_lost
            && outcome.status.is_some()
            && outcome.stdout.complete
            && outcome.stderr.complete
            && outcome.stdout.omitted_bytes == Some(0)
            && outcome.stderr.omitted_bytes == Some(0),
        "CLI execution did not yield complete output: status={:?}, timed_out={}, reaped={}, \
         ownership_lost={}, errors={:?}, stdout_complete={}, stdout_omitted={:?}, \
         stderr_complete={}, stderr_omitted={:?}; stderr prefix: {}",
        outcome.status,
        outcome.timed_out,
        outcome.direct_child_reaped,
        outcome.ownership_lost,
        outcome.errors,
        outcome.stdout.complete,
        outcome.stdout.omitted_bytes,
        outcome.stderr.complete,
        outcome.stderr.omitted_bytes,
        String::from_utf8_lossy(&outcome.stderr.prefix[..outcome.stderr.prefix.len().min(2048)]),
    );
    Some(Output {
        status: outcome.status.expect("checked child status"),
        stdout: outcome.stdout.prefix,
        stderr: outcome.stderr.prefix,
    })
}
