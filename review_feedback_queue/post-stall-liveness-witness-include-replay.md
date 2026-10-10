# Post-stall liveness witness can include replay

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The post-detach progress test could accept records from old replay.

## Details

F293 — **possible** — `crates/farhelm/tests/e2e/terminal_backpressure.rs:1853` — Post-stall liveness witness can include
replay

Its baseline comes from bytes delivered to a paused viewer. A replacement attachment can replay fifty later records that
were already produced before detach, satisfying the progress witness even if the pane subsequently remains blocked. The
full counterexample was not reproduced. Establish an action-specific post-detach boundary or round trip and require
output newly produced afterward, rather than accepting retained history.

## Evidence and triage context

- terminal_backpressure.rs:1810-1813 pauses delivery and waits for detach; :1844 derives its baseline from bytes
  delivered to that paused viewer, then :1848-1854 accepts baseline+50 in the new attachment's replay. wait_for_bytes at
  :348-357 accepts already-buffered replay immediately. fake_agent.rs:1724-1727 emits every 2 ms, allowing those 50
  records to predate detach. A pane that subsequently remains blocked can therefore satisfy the claimed progress witness
  from retained history.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_09_cor:p1:C2`.

- `cli_installation_09_cor:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
