# The PID cleanup guard is inert on macOS

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The fixture's PID cleanup guard does not work on macOS.

## Details

F135 — **definite** — `crates/farhelm/tests/e2e/harness.rs:2417` — The PID cleanup guard is inert on macOS

The guard depends on Linux process-identity observation, so it cannot validate and terminate a surviving fixture process
on macOS. Its promised failure-safe cleanup is absent there; the wrapper is left to its own expiry backstop. Capture and
revalidate process identity through a platform-capable API so supported-platform cleanup retains authority over the
actual fixture instance.

## Evidence and triage context

- harness.rs:2386–2391 captures identity using proc_starttime, whose only backend at :2416–2419 is /proc. Drop at
  :2395–2398 does nothing when identity is absent. supervisor_stop.rs:389–426 creates the sleep-120 wrapper and arms
  this guard without excluding macOS.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The guard's intended contract appears at harness.rs:2364–2382. SPEC_impl.md:2175–2182 recognizes macOS's different
  observation API. No exact existing coverage or applicable rare-trigger filter found.

Caveats:

- No surviving process was reproduced. The cited wrapper self-expires after 120 seconds, limiting that caller's leak.
- No surviving process reproduced. The cited fixture self-expires after 120 seconds. Separate edit site from the other
  process-oracle findings.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_05_cor:p1:F4`.

- `cli_installation_05_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
