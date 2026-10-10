# Pipe-scan assertion failures leave fixture processes running

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Failed pipe-scan tests leave children and descriptors open.

## Details

F179 — **definite** — `crates/farhelm/tests/e2e/terminal_tabs.rs:3302` — Pipe-scan assertion failures leave fixture
processes running

The sleep and cat children have no unwind cleanup. A duplicated pipe writer keeps cat from receiving EOF even when the
parent's original writer is dropped, so an assertion failure can leave both processes and inherited stderr alive. This
can add a runner leak failure to the original failure. Install terminate-and-reap guards immediately after each spawn
while retaining the duplicated writer for measurement.

## Evidence and triage context

- crates/farhelm/tests/e2e/terminal_tabs.rs:3302 spawns std::process::Child cat with piped stdin; :3308 duplicates its
  writer.
- crates/farhelm/tests/e2e/terminal_tabs.rs:3312-3317 spawns sleep 60 holding that writer.
- crates/farhelm/tests/e2e/terminal_tabs.rs:3329-3339 can panic before cleanup at :3341-3344.
- Neither child overrides stderr, so both inherit the test's stderr descriptor; .config/nextest.toml:21 treats leaked
  output handles after one second as failure.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Normally bounded by sleep's 60-second expiry; runner cleanup may shorten it.
- No induced assertion failure.
- No unrelated user process is shown to be affected.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_09_sec:p1:F2`.

- `cli_installation_09_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
