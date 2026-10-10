# Unbounded aggregate capture replies

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Aggregate terminal capture could exceed a safe memory bound.

## Details

F328 — **possible** — `crates/farhelm-supervisor/src/tmux/control_codec.rs:368` — Unbounded aggregate capture replies

The reply collector caps individual lines but appends the complete response without an aggregate byte limit. Large
history and geometry bounds, plus coexisting replay buffers, do not establish a safe total allocation. Harmful
ordinary-use thresholds remain unverified. Assess the combined peak and enforce an explicit aggregate bound with visible
failure or bounded processing where needed; no current exhaustion or work loss is established.

## Evidence and triage context

- control_codec.rs:368–382 extends the complete reply without an aggregate byte check; lines 403–407 cap individual
  lines. tmux/stream.rs:1115–1158 retains history and visible replies and normalizes a selected snapshot. tmux.rs:58
  retains 12,000 history lines and lines 2813–2814 allow dimensions up to 10,000. BUGS.md:53–78 covers a different
  idle-input queue. BUGS.md:8–44 describes consequences of abrupt supervisor death, not acceptance of every avoidable
  allocation leading to it.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_data:p1:C5`.

- `sr_data:p1:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
