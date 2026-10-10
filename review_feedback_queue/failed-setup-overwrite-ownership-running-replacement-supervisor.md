# Failed setup can overwrite ownership of the running replacement supervisor

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed supervisor setup can overwrite ownership of the still-serving child.

## Details

F248 — **definite** — `e2e/tests/terminal-multihost.spec.ts:2478` — Failed setup can overwrite ownership of the running
replacement supervisor

Setup failure before shutdown leaves the tracked replacement alive. Unconditional restoration overwrites its only child
handle, and the connected-host poll can still succeed through that surviving supervisor. Cleanup and later lifecycle
actions then target the wrong child. Restore only when the fixture supervisor is absent, retaining ownership of every
live tracked instance.

## Evidence and triage context

- e2e/tests/terminal-multihost.spec.ts:654–667 can start a replacement in outer setup.
- e2e/tests/terminal-multihost.spec.ts:2420–2463 performs fallible creation/status checks before :2470 stops the
  supervisor; :2478 restores unconditionally.
- e2e/tests/terminal-multihost.spec.ts:600 overwrites restartedRemote; :608–620 checks host connection state without
  proving the new child serves it.
- e2e/tests/terminal-multihost.spec.ts:515–525 stops only the stored child; :3951 invokes that final reaper.
- crates/farhelm-supervisor/src/service/core.rs:1264–1266 refuses an occupied ownership lock; :6511–6520 refuses serving
  without ownership.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- The affected process belongs to the test fixture; no user-owned process loss is established.
- The lost handle is definite; eventual cleanup by an enclosing runner is separate from this file's broken ownership.
- The guarded sibling teardown at :2702–2711 corroborates the constraint but does not cover this independently editable
  site.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_12_sec:p2:F1`.

- `test_infrastructure_12_sec:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
