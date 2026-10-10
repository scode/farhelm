# Orderly-shutdown assertion is vacuous on macOS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The orderly-shutdown death assertion is always satisfied on macOS.

## Details

F191 — **definite** — `crates/farhelm-supervisor/src/service/terminals.rs:3540` — Orderly-shutdown assertion is vacuous
on macOS

The test checks a Linux process-filesystem path that is absent on ordinary macOS even while the fixture is alive.
Shutdown can return before child death and still pass this central assertion. Use a portable lifetime oracle, establish
that it detects the live fixture, and then require disappearance at the shutdown completion boundary.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/terminals.rs:2437 gates the module only on test.
- crates/farhelm-supervisor/src/service/terminals.rs:3354 creates a real cat child.
- crates/farhelm-supervisor/src/service/terminals.rs:3540 checks child disappearance solely through /proc.
- terminals.rs:3354-3361 starts a real cat client; :3514-3537 awaits orderly shutdown; :3540 checks only that
  /proc/<pid> does not exist. The test module is cfg(test), not Linux-only.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The production shutdown-expiry queue item and abrupt-death BUGS entry concern different contracts.
- SPEC_impl.md:2176 recognizes macOS's lack of /proc. No matching existing coverage; birth-oracle.md concerns a
  different helper and contract.

Caveats:

- No production cleanup failure demonstrated. Linux retains a meaningful procfs observation. Verified by source
  inspection; no runtime tests performed.
- Linux retains a meaningful observation.
- No current production shutdown regression is established.
- A defective test oracle is confirmed; no current production cleanup failure is claimed. SPEC_impl.md:2176 explicitly
  recognizes that macOS has no /proc.
- No current production cleanup failure is claimed. Keep this separately editable assertion separate from the two
  following assertions.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_sec_02:p1:F1`, `sr_systems:p2:F1`.

- `gap_supervisor_runtime_sec_02:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `sr_systems:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
