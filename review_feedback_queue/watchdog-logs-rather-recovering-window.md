# Watchdog logs rather than recovering the window

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A persistent desktop bridge outage could accumulate native query entries.

## Details

F302 — **possible** — `crates/farhelm-ui/src/webview_watchdog.rs:176` — Watchdog logs rather than recovering the window

Each missed fifteen-second watchdog probe creates another small native query allocation, and timeout does not remove it
while the bridge remains unable to complete cleanup. Practical memory growth was not measured and recovery may later
release entries. Surface this allocation-lifetime remainder for an explicit remedy decision, preserving the specified
log-only watchdog and its ability to detect recovery. Automatic reload or exit is not requested.

## Evidence and triage context

- SPEC_impl.md:2951-2955 explicitly requires a 15-second one-shot heartbeat and log-only response, never reload or exit.
  This covers recovery behavior only.
- crates/farhelm-ui/src/lib.rs:1284 starts the watchdog for the desktop application.
- webview_watchdog.rs:77 and 174-182 create a fresh eval every 15 seconds and drop the waiting future after its timeout;
  webview_watchdog.rs:216-231 continues probing even in Dead state.
- Cargo.lock:1145-1146 identifies dioxus-desktop 0.7.10.
- dioxus-desktop-0.7.10/src/query.rs:50-56 allocates channels and inserts a slab entry for every query; query.rs:133-134
  removes an entry on the incoming Drop message.
- dioxus-desktop-0.7.10/src/document.rs:90-99 stores the evaluator owner inside that query entry. The returned handle
  does not own its lifetime.
- dioxus-desktop-0.7.10/src/js/native_eval.js:1 sends Drop through FinalizationRegistry; an eval that never executes
  cannot complete this cleanup.
- webview_watchdog.rs:40-58 acknowledges accumulating entries during an outage. No matching acceptance was found in
  TODO.md Planned, BUGS.md, the queue index or the searched ledger.
- SPEC_impl.md:64-69 concerns a malicious or buggy supervisor selectively withholding replies, not a dead desktop eval
  bridge. The hung-tmux/systemd filter likewise does not cover this dependency or scope.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Only one small native query allocation is added per missed 15-second probe.
- No runtime memory measurement was performed.
- Recovery may permit eventual cleanup; the demonstrated accumulation requires the bridge to remain unable to complete
  cleanup.
- This is not a request to introduce automatic reload or exit, which the specification forbids.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_15_cor:p1:C3`.

- `ui_desktop_15_cor:p1:C3`: confidence as filed: possible; suggested bucket as filed: not separately tagged in
  candidate list.
