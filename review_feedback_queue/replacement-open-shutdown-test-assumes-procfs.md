# Replacement-open shutdown test also assumes procfs

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Returned replacement-client cleanup is unverified on macOS.

## Details

F193 — **definite** — `crates/farhelm-supervisor/src/service/terminals.rs:3710` — Replacement-open shutdown test also
assumes procfs

The preceding assertions establish that opening continues despite cancellation, but the final Linux process-filesystem
check is already true on macOS while the returned child lives. It can therefore pass without enforcing cleanup of that
child. Replace this assertion with a portable lifetime observation and first verify that it detects the live fixture.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/terminals.rs:3694 and :3698 check opener noncancellation.
- crates/farhelm-supervisor/src/service/terminals.rs:3703 releases the opener and awaits completion; :3710 checks
  returned-child death solely through procfs.
- terminals.rs:3668-3670 returns a real replacement client; :3703-3706 completes shutdown; :3708 verifies a nonzero PID;
  :3710 treats absence of /proc/<pid> as process death.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified. Prior decisions against more production teardown complexity do not cover this existing
  false assertion.
- SPEC_impl.md:2176 recognizes macOS's missing procfs. No exact existing coverage.

Caveats:

- The noncancellation assertions remain useful. No current production leak demonstrated; no runtime tests performed.
- The noncancellation portion remains useful.
- Preserve this independent return-from-opener location separately from F1 and F2.
- The preceding assertions about keeping the in-flight opener pending remain meaningful. No production cleanup
  regression was established.
- No production cleanup regression demonstrated. Retain this independently editable returned-client assertion.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_sec_02:p1:F3`, `sr_systems:p2:F3`.

- `gap_supervisor_runtime_sec_02:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `sr_systems:p2:F3`: confidence as filed: definite; suggested bucket as filed: other.
