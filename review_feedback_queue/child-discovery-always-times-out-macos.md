# Child discovery always times out on macOS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Child-discovery fixtures cannot reach lifecycle checks on macOS.

## Details

F133 — **definite** — `crates/farhelm/tests/e2e/harness.rs:2168`; `crates/farhelm/tests/e2e/harness.rs:2166` — Child
discovery always times out on macOS

The shared child enumeration depends on Linux's process filesystem. On ordinary macOS it cannot discover the real child
required by the fixture, so setup times out before restart or Stop behavior is tested. Provide platform-capable
parent/child enumeration and report unavailable observation explicitly, keeping substrate failures separate from the
product lifecycle result.

## Evidence and triage context

- harness.rs:2166–2170 returns an empty child list when /proc enumeration fails. :2208–2216 polls it until panic.
  restart_with_resume.rs:546–571 invokes this wait in an ungated test.
- crates/farhelm/tests/e2e/harness.rs:2048 treats failed Linux procfs reads as death.
- crates/farhelm/tests/e2e/harness.rs:2168 treats unavailable procfs as an empty child table.
- crates/farhelm/tests/e2e/restart_with_resume.rs:665 asserts these processes remain alive; :571 requires grandchild
  discovery.
- crates/farhelm/tests/e2e/session_lifecycle.rs:4453 independently requires grandchild discovery.
- crates/farhelm/tests/e2e/main.rs:39 and :52 include these modules without Linux gating.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:2175–2182 distinguishes the platform observation backends. No exact Planned, BUGS.md, queue, ledger, or
  filter coverage found.
- No exact existing coverage. birth-oracle.md is a different filesystem-capability oracle.

Caveats:

- No native macOS execution. This independently editable enumeration helper differs from the liveness helper.
- No native macOS execution. This is a separate enumeration edit site from process_is_gone.
- No macOS execution.
- Some daemon fixtures additionally require setsid; that can cause an earlier setup failure but does not repair the
  shared observer.
- This establishes test defects, not a current production cleanup failure.
- Aggregate needs two independently editable findings: process_is_gone/death observation, and children_of/child
  discovery.
- Some reparent fixtures can fail earlier due to setsid availability.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_cor:p1:F2`,
`cli_installation_07_sec:p1:F3`.

- `cli_installation_05_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_07_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
