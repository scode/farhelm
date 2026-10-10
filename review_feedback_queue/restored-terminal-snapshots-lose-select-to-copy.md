# Restored terminal snapshots lose select-to-copy

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Restored terminal snapshots no longer copy selected text.

## Details

F196 — **definite** — `crates/farhelm-ui/assets/terminal.js:1736`; `crates/farhelm-ui/assets/terminal.js:5579` —
Restored terminal snapshots lose select-to-copy

Unmount removes the Farhelm selection-copy handlers and invalidates their liveness token while retaining the terminal
object for recovery. Restoration reveals that object but does not restore the handlers, so selectable snapshot text
never triggers the promised clipboard write. Give retained snapshots a read-only selection-copy lifecycle without
restoring terminal input or transport callbacks.

## Evidence and triage context

- crates/farhelm-ui/assets/terminal.js:1675 retains the terminal and unmounts its island.
- crates/farhelm-ui/assets/terminal.js:5579 removes automatic-copy listeners; line 5590 invalidates deferred callbacks;
  line 5618 retains only the hidden terminal.
- crates/farhelm-ui/assets/terminal.js:1730 restores the held terminal without reinstalling copy handling.
- crates/farhelm-ui/assets/terminal.js:5305 installs the missing listeners during ordinary mounting.
- crates/farhelm-ui/assets/terminal.js:1672–1679 saves island.term, sets keepTerm, and unmounts before attempting
  recovery.
- crates/farhelm-ui/assets/terminal.js:5261–5303 implements selection-to-copy, including the alive check at :5273 and
  clipboard enqueue at :5302. :5305–5321 registers the handlers and defines their removal.
- crates/farhelm-ui/assets/terminal.js:5461–5477 stores stopCopyOnSelect and disposeDeferred on the island. :5579
  removes the handlers unconditionally; :5590 calls disposeDeferred, whose :4571–4572 sets alive=false.
- crates/farhelm-ui/assets/terminal.js:5618–5623 preserves the terminal DOM when keepTerm is set but removes the island.
- crates/farhelm-ui/assets/terminal.js:4675–4680 restores a held screen when recovery receives a decision detach.
  :1730–1743 unmounts the replacement, reveals the held terminal, and stores only term/path/gen in a tombstone. No
  copy-handler installation occurs.
- crates/farhelm-ui/assets/terminal.js:2962–2968 skips tombstoned terminals during reconciliation, so ordinary sync does
  not repair the missing handlers.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:1253 promises selection copying. The best-effort clipboard exception concerns clipboard failures, not absent
  handling. TRIAGE_OUTCOMES.md:5376 concerns disposing departed snapshots, a different lifecycle defect.
- {"basis": "TRIAGE_OUTCOMES.md:5376–5394, terminal-tombstone-never-buried.md", "comparison": "That decision concerns
  disposal when a snapshot departs the desired set and blank panes when it returns. This finding concerns
  selection-to-copy while the snapshot remains displayed; neither trigger nor consequence matches."}
- {"basis": "SPEC.md:1265–1270 and SPEC_impl.md:859–875", "comparison": "The accepted exception concerns clipboard
  permissions, engine behavior, and broken or slow system clipboards. Here no clipboard write is attempted because
  Farhelm removed its handlers, even with a working clipboard."}
- {"basis": "TRIAGE_OUTCOMES.md:5993–6003, desktop-copy-fallback-never-runs.md", "comparison": "The discarded finding
  concerns header copying during brief desktop reauthentication and a fallback that never runs. It does not cover
  terminal snapshot handler lifetime."}

Caveats:

- No interactive reproduction was performed.
- This concerns restored reconnect snapshots; a directly detached terminal can remain mounted with its listeners.
- Manual platform copy may still work.
- Applies to restored reconnect snapshots, not every detached terminal.
- No browser reproduction.
- Confirmed by source inspection, without browser reproduction.
- The defect applies to screens restored after a recovery attempt; a direct takeover that retains the original island
  need not have this failure.
- Explicit platform Copy commands may still work. Terminal contents are retained; data loss is not established.
- Repeating selection does not repair the defect. FILTER.md:24–42 does not establish coverage for a copy operation that
  remains broken throughout the snapshot's lifetime.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_02_cor:p1:F2`, `ui_desktop_03_cor:p2:F1`.

- `ui_desktop_02_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `ui_desktop_03_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
