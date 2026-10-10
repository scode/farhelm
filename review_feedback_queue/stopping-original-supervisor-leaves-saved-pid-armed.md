# Stopping the original supervisor leaves its saved PID armed in stack cleanup

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Final browser-stack cleanup could signal a replacement for an exited supervisor.

## Details

F63 — **possible** — `e2e/tests/terminal-multihost.spec.ts:497; e2e/start-stack.sh:253`; `e2e/start-stack.sh:253` —
Stopping the original supervisor leaves its saved PID armed in stack cleanup

Browser tests intentionally stop the original supervisor, but the stack owner retains its numeric process identifier for
final cleanup across later asynchronous work. The first stop's identity check does not protect that later signal. If the
child is reaped and its number reused, teardown could terminate unrelated same-account work; that sequence was not
reproduced. Relinquish the cleanup target during intentional shutdown or revalidate process-instance identity
immediately before final signaling.

## Evidence and triage context

- e2e/start-stack.sh:362–364 starts the original supervisor and retains its PID.
- e2e/start-stack.sh:411–420 publishes that number to browser fixtures.
- e2e/tests/terminal-multihost.spec.ts:489–503 stops the original supervisor after immediate identity verification.
- e2e/tests/terminal-multihost.spec.ts:2470 and :2478 stop the original and restore a replacement.
- e2e/tests/terminal-multihost.spec.ts:600–604 stores the replacement only in worker-owned restartedRemote.
- e2e/start-stack.sh:253 later signals the original saved number without identity revalidation; :551 keeps the stack
  shell waiting during the suite.
- SPEC_impl.md:2194–2205 explicitly excludes numbers stored for later or carried across asynchronous work from the
  accepted short-window race.
- e2e/start-stack.sh:364 stores remote_sup_pid; :253 unconditionally signals the stored number during cleanup.
- e2e/tests/terminal-multihost.spec.ts:460-497 verifies and terminates the original supervisor.
- e2e/tests/terminal-multihost.spec.ts:2470-2479 deliberately performs that termination and later restores a
  replacement.
- e2e/start-stack.sh:551 waits for the helm, without retiring the original supervisor identifier.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:6523–6528 expressly limits the PID-reuse acceptance to short windows. Entries at :6610–6638 concern
  distinct preview-server mechanisms. TODO.md:241–253 concerns delayed parent-SIGTERM cleanup, not retaining a dead
  supervisor's number. No exact existing queue item found. FILTER.md excludes loss of user processes and wrong-target
  actions.
- SPEC_impl.md:2194-2205 expressly excludes bare PIDs retained across asynchronous work. TODO.md:241-253 concerns
  deferred parent-SIGTERM cleanup, not signalling a recycled supervisor number, and is outside Planned.

Caveats:

- The unsafe retained-number lifecycle is confirmed.
- Actual wrong-process impact requires the original child to be reaped and its PID reused before final cleanup; that was
  not demonstrated.
- The shared anchor with the immediate-race entries does not make these the same mechanism.
- No demonstrated PID reuse or terminated unrelated process.
- This is distinct from the accepted immediate check-to-signal race at terminal-multihost.spec.ts:497.
- Highest reflects possible loss of unrelated running work, not a claimed security exploit.
- Coordination with the stack owner or identity revalidation should address the retained cleanup target.
- PID reuse before final cleanup was not reproduced. Bash may reap terminated background children while retaining their
  status; an eventual shell wait is not a kernel identity reservation.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_12_sec:p1:F1`,
`test_infrastructure_05_sec:p2:F1`.

- `test_infrastructure_12_sec:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
- `test_infrastructure_05_sec:p2:F1`: confidence as filed: possible — PID reuse during the remaining test run is the
  open premise; suggested bucket as filed: highest.
