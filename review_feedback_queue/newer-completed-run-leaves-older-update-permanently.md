# a newer completed run leaves an older update permanently busy

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An old update can remain busy after a newer run has completed.

## Details

F105 — **definite** — `crates/farhelm-ui/src/provisioning.rs:1655` — a newer completed run leaves an older update
permanently busy

A successful snapshot showing a different, completed provisioning run does not enter either the successor-adoption
branch or the no-run clearing branch. The window therefore keeps the old nonterminal run's busy ownership, while the
helm retains only the latest run. Repeated reads cannot resolve that lockout. Release superseded busy ownership on an
authoritative successor snapshot, preserving the diagnostic that the old run's exact outcome remains unknown.

## Evidence and triage context

- crates/farhelm-helm/src/provisioning/service.rs:1528–1535 replaces the host's retained run when a new run starts;
  2303–2317 returns only that latest retained run.
- crates/farhelm-ui/src/provisioning.rs:1847–1904 delivers applicable progress reads to reconciliation after read
  fencing and outstanding-submission checks.
- crates/farhelm-ui/src/provisioning.rs:1606–1617 settles matching IDs; 1622–1635 records an unknown-outcome warning for
  a different run.
- crates/farhelm-ui/src/provisioning.rs:1640–1664 adopts running or failed-update successors, but a completed successor
  with a run ID neither replaces nor clears the old run.
- crates/farhelm-ui/src/provisioning.rs:694–715 keeps the old nonterminal run live; 1556–1558 republishes busy; 652–660
  continues displaying awaiting-progress update status.
- crates/farhelm-ui/src/provisioning.rs:2453–2477 folds retained ownership into plan_in_flight; 143–155 disables
  provisioning offers while that flag remains set.
- SPEC.md:107–115 distinguishes running progress from success and uncertain-outcome diagnostics; SPEC_impl.md:457–460
  requires exact-run success for clearing disclosure, not perpetual operation ownership.
- crates/farhelm-helm/src/provisioning/service.rs:1532 replaces the host's retained view when a new run starts. Lines
  1984–1995 mark that run completed and release backend busy ownership. Lines 2303–2317 return only the retained latest
  view.
- crates/farhelm-ui/src/provisioning.rs:1606–1617 settles or updates a matching tracked run. For a different run, lines
  1622–1635 retain an unknown-outcome warning; lines 1640–1664 neither adopt a completed run nor clear the old tracker
  when the response still has a run ID.
- crates/farhelm-ui/src/provisioning.rs:694–715 treats the retained nonterminal tracker as live ownership. Line 1556
  republishes busy despite the received completed view; lines 652–659 publish awaiting-progress status.
- crates/farhelm-ui/src/hosts.rs:1649–1651 includes that ownership in row busy state. Lines 1707–1711 refuse settings
  and lines 1744–1746 refuse provisioning actions while it remains set.
- crates/farhelm-ui/src/provisioning.rs:323–328 excludes connection-incarnation churn from target changes, so ordinary
  reconnection need not retire the stale tracker.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- BUGS.md:80–101 concerns an actually hung provisioning run caused by an SSH helper keeping output open. Here the
  backend is idle and the client retains obsolete busy ownership.
- SPEC_impl.md:459 preserves diagnostics until correlated success; it does not explicitly accept this action lockout.
- {"basis": "SPEC_impl.md:455–464", "comparison": "Correlated success governs clearing automatic disclosure and
  diagnostics. It does not explicitly accept indefinitely retaining active ownership after a subsequent run has
  completed."}
- {"basis": "BUGS.md:80–101", "comparison": "The recorded hang requires a lingering SSH helper holding output open and
  leaves a backend run active. This finding requires completed backend work and obsolete client ownership; trigger and
  scope differ."}
- {"basis": "review_feedback_queue/FILTER.md:24–42", "comparison": "Missing transitions may be uncommon, but subsequent
  progress reads do not repair this state. The persistent-state exclusion prevents dropping it as a transient display
  glitch."}

Caveats:

- Requires missing A's terminal observation and B's running transition, then observing B completed.
- Reloading the client recovers; ordinary repeated reads of the retained completed run do not.
- No runtime reproduction was performed.
- Requires the client to miss A's completion and B's running state while retaining its mounted component.
- The state is not irrevocable: remounting, removing or retargeting the row, a later applicable running or failed-update
  view, or loss of retained backend history can release it.
- Run IDs are opaque. A correction must preserve protection against genuinely stale completed responses rather than
  treating arbitrary different IDs as chronological evidence.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_13_sec:p1:F2`, `ui_desktop_13_cor:p1:F1`.

- `ui_desktop_13_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: high.
- `ui_desktop_13_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
