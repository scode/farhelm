# Unbounded test discovery of tmux binaries

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Test discovery could hang on a tmux executable production would never select.

## Details

F295 — **possible** — `crates/farhelm-supervisor/src/tmux.rs:5266` — Unbounded test discovery of tmux binaries

The helper scans every executable candidate in PATH and runs its version query synchronously without a local deadline. A
shadowed candidate that hangs can therefore block adoption-test setup even though the configured product dependency is
healthy. The broader filter coverage remains unresolved. Bound each discovery query and report unavailable candidates
explicitly; no security or unrelated-work loss is established.

## Evidence and triage context

- tmux.rs:5262 iterates every PATH directory; :5263 checks executable status and deduplicates path spellings; :5266
  invokes synchronous Command::output with no local deadline. The adoption test calls this at :5304 before fixture
  setup. FILTER.md:52–55 covers a configured tmux program that hangs and resulting affected-host supervisor/request
  stalls. Trigger selection and execution scope do not exactly match an arbitrary shadowed PATH candidate blocking a
  test. Under FILTER.md:18–20, unclear coverage is not a match.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_sec_02:p1:C3`.

- `gap_supervisor_runtime_sec_02:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket
  as filed: not separately tagged in candidate list.
