# Stale deflake PID can terminate unrelated work

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Stopping an old test sweep can signal an unrelated process.

## Details

F60 — **definite** — `deflake/bin/deflake:1463` — Stale deflake PID can terminate unrelated work

The stop command trusts a numeric process identifier persisted by the sweep daemon. Abnormal daemon death leaves that
file behind indefinitely, and a later process can reuse the number. With permission to signal that replacement, stopping
the old sweep terminates unrelated same-account work. The source establishes the stale authority; no live reuse
experiment was performed. Use a per-run control channel or validate process birth identity before signaling, and reject
stale identities.

## Evidence and triage context

- deflake/bin/deflake:1333 stores the spawned daemon PID; :822 rewrites it on daemon entry.
- deflake/bin/deflake:835-889 removes the PID file through orderly finish; SIGKILL cannot execute this path.
- deflake/bin/deflake:1286-1294 resolves the persisted current-run pointer without checking daemon identity.
- deflake/bin/deflake:1456-1463 reads the stored integer and immediately sends SIGTERM. There is no birth-time,
  ownership, control-channel, or terminal-status check.
- deflake/bin/deflake:611-618 treats any process answering kill(pid, 0) as the daemon; :1472 uses that same predicate
  after signaling.
- deflake/bin/deflake:1509-1513 exposes this path through the ordinary stop subcommand.
- deflake/bin/deflake:822 publishes the daemon PID; :889 removes it only during orderly finalization.
- deflake/bin/deflake:1286-1294 resolves the retained run without validating daemon identity.
- deflake/bin/deflake:1459-1463 reads that PID and immediately sends SIGTERM.
- deflake/bin/deflake:611-618 establishes only that some signalable process occupies the number.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:2194-2205 accepts short process-number reuse windows, but :2201-2202 explicitly excludes bare numbers
  stored for later. Trigger and duration therefore do not match.
- TRIAGE_OUTCOMES.md:6505-6533, heading probe-pgid.md, preserves the same exclusion.
- TODO.md:33-43 Planned concerns supervisor session creation; BUGS.md and the queue contain no matching deflake stop
  mechanism.
- SPEC_impl.md:2194-2205 accepts short process-number reuse windows but explicitly excludes storing numbers for later.
  SPEC.md:2152-2164 excludes deliberate interference, whereas ordinary PID reuse requires none. Neither covers this
  finding.

Caveats:

- Requires daemon exit without PID-file removal, subsequent PID reuse, and permission to signal the replacement.
- Confirmed by source tracing; no runtime reproduction was performed.
- This is accidental wrong-target process termination, not an established privilege-escalation vulnerability.
- Conditional on abnormal death and subsequent PID reuse; no live-process experiment performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_04_sec:p1:F1`,
`test_infrastructure_04_cor:p2:F1`.

- `test_infrastructure_04_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `test_infrastructure_04_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: highest.
