# restart_replace_bar only hands back its mark-clearing cleanup after every step succeeded, so a failure after the first mark leaks marked sessions into the following screenshots

Reviewed commit: 50f3c38b7415997f4962501fd57e6f97bf857f26

## TLDR

When the release-notes screenshot run fails partway through staging the bulk Restart/Replace picture (one of the two
sessions marked, or the '2 marked' bar slow to appear), the marks stay on. The engine then carries on with the next
shots in the same app, so the right-click, Replace with and terminal-tab pictures can be captured with a stray 'N
marked' bar in the session list instead of failing, and that can reach published release-notes images unless someone
notices.

## Details

restart_replace_bar (scripts/docs-shots/release-notes.py, around lines 487-499 at the tip) marks each of
STOPPED_SESSIONS with mark_stopped_session, then waits for '2 marked' and id:session-marked-start, and only then returns
clear_marks. capture_pages in scripts/docs_shots.py (lines 415-451) runs cleanup only if shot.steps returned it; an
exception from the steps goes straight to the per-shot except, records the failure and moves on to the next shot.
reset() (docs_shots.py lines 305-324) only presses Escape while a layer is open and hides the Hosts panel; marks are
list state, not a layer, so nothing else clears them. The same file's terminal_tab_close (a few lines later) shows the
intended pattern: it wraps its post-mutation steps in try/except and calls its cleanup before re-raising. Fix: wrap the
marking and waits in try/except that clicks id:session-marked-clear (or clears via the bridge) before re-raising,
matching terminal_tab_close; or have reset() clear marks.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (maintainer docs-screenshot tooling; worst case a wrong published screenshot that
  review would likely catch).
- Confidence: likely; the cleanup gap is confirmed from source, the unverified premise is that no other mechanism (an
  Escape from reset or a later click) clears marks before the next capture.
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g33/f4 (finding), group g33 "Maintainer tooling: brick tests and release-notes screenshots", findings log line 5.
