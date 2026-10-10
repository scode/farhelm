# Ignored transcript-feeder write errors conceal missing input

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A failed transcript feeder could make query-stripping tests pass with no input.

## Details

F286 — **possible** — `crates/farhelm-supervisor/src/tmux/stream.rs:2074` — Ignored transcript-feeder write errors
conceal missing input

The feeder ignores write errors, and the bare-query consumer accepts immediate EOF as successful absence of output. An
empty transcript can therefore pass even when query stripping is removed. Feeder failure was not reproduced. Propagate
delivery errors and add a positive output sentinel after the query, distinguishing successful filtering from input that
never reached the parser.

## Evidence and triage context

- crates/farhelm-supervisor/src/tmux/stream.rs:1894-1905 spawns the transcript feeder and ignores write_all's result.
- crates/farhelm-supervisor/src/tmux/stream.rs:2074-2081 feeds a bare terminal query but asserts only that next_output
  returns None. It establishes neither successful transcript delivery nor a following output sentinel.
- crates/farhelm-supervisor/src/tmux/stream.rs:1258-1277 returns None on EOF when no prefix is pending. An empty feeder
  output therefore satisfies this consumer without exercising query stripping.
- The exact-event assertions at :2026-2040 and notification-count assertion at :2535-2538 protect other consumers; they
  do not establish this independently editable consumer's premise.
- No matching acceptance or recorded disposition was found. A positive delivery sentinel following the stripped query
  would distinguish successful filtering from absent fixture input.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_cor_02:p1:C6`.

- `gap_supervisor_runtime_cor_02:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket
  as filed: not separately tagged in candidate list.
