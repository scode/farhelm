# An unavailable process scan certifies that no marked processes remain

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A missing process scan can make cleanup assertions pass without inspection.

## Details

F134 — **possible** — `crates/farhelm/tests/e2e/harness.rs:2547`; `crates/farhelm/tests/e2e/harness.rs:2544` — An
unavailable process scan certifies that no marked processes remain

When enumeration is unavailable on macOS, the marked-process helper returns an empty result rather than an observation
failure. A surviving marked process can therefore pass the cleanup assertion without any scan. No actual leak was
reproduced. Provide a supported observation backend and propagate unavailability, distinguishing inability to inspect
process environments from confirmed absence of marked processes.

## Evidence and triage context

- harness.rs:2544–2549 returns an empty vector when /proc is unavailable. tab_lifecycle_edges.rs:49–54 accepts that as
  cleanup completion. The ungated open/delete test at :851–900 invokes this assertion. MarkerCleanupGuard at
  harness.rs:2529–2533 likewise performs no cleanup.
- harness.rs:2047-2049 treats unavailable /proc as death; :2544-2548 returns no marked PIDs when /proc is absent.
  session_lifecycle.rs:5496-5497 and :5550 use the death helper without Linux gating. fake_agent.rs:2655 additionally
  invokes setsid, so some macOS setups may fail earlier; that does not validate the death oracle.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:2181–2192 accepts unreadable environments for individual production processes, not an unavailable test
  enumeration backend certifying emptiness. birth-oracle.md addresses another oracle. No exact coverage found.

Caveats:

- A macOS replacement must distinguish unavailable environment observations from absence of marked processes.
- No leak reproduced. A replacement backend must distinguish unavailable observations from absence.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_cor:p1:F3`,
`cli_installation_08_cor:p1:C7`.

- `cli_installation_05_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_08_cor:p1:C7`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
