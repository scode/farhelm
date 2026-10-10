# Sink-healing witness can consume queued output

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The healed-sink test could consume output queued before the failure.

## Details

F294 — **possible** — `crates/farhelm/tests/e2e/terminal_tabs.rs:2492` — Sink-healing witness can consume queued output

The busy-tab receiver retains queued data while only the accumulated display buffer is cleared. After sink replacement,
the same old flood prefix can satisfy the progress assertion even if that tab has stopped. A round trip on the agent
terminal proves a different stream. Establish a fresh boundary for the busy tab and require new output after healing,
excluding queued and replayed bytes.

## Evidence and triage context

- terminal_tabs.rs:2464 starts a finite flood and :2468 waits only for its opening prefix. After sink death and
  replacement at :2474-2477, :2492-2494 clears tab_seen but does not clear or establish a boundary in tab_rx, then
  accepts the same prefix. The agent-terminal round trip at :2486-2489 proves progress on a different terminal, not the
  busy tab.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_09_cor:p1:C3`.

- `cli_installation_09_cor:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
