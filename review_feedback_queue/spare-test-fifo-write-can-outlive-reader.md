# Spare completion test can block after its FIFO reader exits

Reviewed commit: 8f5d016f04ab13c284530e759e828826790f2843

## TLDR

The prepared-checkout completion test can hang internally if its guarded shell exits before the test releases a FIFO.
The standard runner eventually terminates it; a direct test run has no internal bound at that point.

## Details

Suggested bucket: **other**, for a test-harness hang. Confidence: **confirmed conditional source mechanism**; adverse
scheduling and occurrence frequency are unmeasured. No runtime reproduction was performed.

`crates/farhelm/tests/spare_builder.rs:731–735` starts a shell waiting on a FIFO under a separate 60-second guard.
Readiness assertions at `747–765` establish a PID file, a live background sleep and absence of Git invocation, not that
the FIFO reader remains alive. If that reader dies before `tokio::fs::write` at `766`, the write-only FIFO open blocks
waiting for another reader. The bounded `GroupChild::finish` at `769–770` is never reached. The guard has an independent
OS deadline thread (`crates/farhelm-supervisor/src/spare_build.rs:123–141`), so descheduling the test does not extend
the reader's lifetime.

`.config/nextest.toml:18–20` provides an outer termination after about five minutes. That limits the standard-runner
consequence; it does not bound this release operation inside the test. A healthy FIFO with no reader is not a hung
filesystem. No exact acceptance, Planned, BUGS, queue, ledger or FILTER coverage was found. Possible cover: none.
Proposed drop: none. Use a bounded release that checks the owned reader's lifetime without waiting indefinitely in a
blocking FIFO open.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g016/f1`, independently corroborated by `g048`.
