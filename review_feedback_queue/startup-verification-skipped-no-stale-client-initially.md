# Startup verification skipped when no stale client is initially listed

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An initially empty startup roster could miss a late predecessor client.

## Details

F329 — **possible** — `crates/farhelm-supervisor/src/tmux.rs:2200` — Startup verification skipped when no stale client
is initially listed

The stale-client sweep returns immediately when its first classification is empty, skipping stability verification used
after finding candidates. A predecessor sink can attach with output already enabled during that gap, potentially
surviving into new session work. Survival and harmful overlap were not reproduced. Apply a stable-roster check to the
initially empty branch as well, accounting for late arrivals before declaring startup cleanup complete.

## Evidence and triage context

- tmux.rs:2165–2166 returns on an initially empty classification; lines 2200–2235 perform stable-roster verification
  only after finding candidates. tmux/sink.rs:54–74 spawns an output-enabled sink. service/core.rs:5300–5314 runs this
  sweep before new session work. BUGS.md:34–45 accepts abrupt-death failures but does not explicitly accept this startup
  omission; shutdown-expiry.md:19–24 concerns a different timeout mechanism.
- tmux.rs:2165–2166 and 2200–2235 skip verification when no initial candidates are found. tmux/sink.rs:54–74 launches
  clients with output enabled from attachment; service/core.rs:5300–5314 relies on the sweep before new session work.
  Neither BUGS.md's abrupt-death entry nor shutdown-expiry.md's planned-stop timeout matches the complete startup
  mechanism. Survival and harmful overlap on the supported substrate remain unverified.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_data:p1:C10`, `sr_general:p1:C6`.

- `sr_data:p1:C10`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `sr_general:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
