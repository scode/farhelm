# add-check appends release-specific extra checks after window-drag on the fresh pass, so the drag check is not actually last and its failure still costs them

Reviewed commit: 7c58c410d7e381b357af8ad29b250b0d5779e602

## TLDR

The brick-test change moved the window-drag check to the end of the fresh pass so that, when the Mac VM cannot deliver a
native window drag, the release's other bonus checks still get results. But the release-specific extra checks the
maintainer adds for a candidate's new features (brick_test.py add-check) are appended after window-drag, so on the fresh
pass a drag that does not work still stops every one of those feature checks.

## Details

required_checks() in brick_test.py (lines 327-336 at the tip) now orders the fresh pass ... 'remote-terminal',
'addendum', 'window-drag', with the comment 'Keep it after the addendum too, since a missing gesture ends the pass and
those earlier checks use different input', and the test docstring test_fresh_pass_checks_window_drag_after_its_sessions
says 'Running this last gives the addendum and the session checks their own results even when the native drag is
unavailable.' add_check() (line 1408-1421) inserts entry['required'][check_id] = {...}, which, being a dict insertion,
lands after window-drag. The scheduler dispatches pending checks in dict order (line 953: 'key for key in
entry["required"] if key not in entry["results"]'), and require_no_failure() (around line 1466) refuses all further work
once any bonus result is not a pass ('stop after the first failed or unavailable check'). BRICK-TEST.md (line 387) has
the operator add every extra obligation with add-check on a bonus pass, and the fresh pass is the natural home for
new-feature checks on a fresh install. So when window-drag is could-not-run or fails, every extra-* check on fresh goes
unrun, which is the outcome the reorder was meant to prevent. Fix: have add_check insert extra checks before window-drag
(rebuild the dict), or order window-drag last at dispatch; add a test that add-check on fresh leaves window-drag last.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (maintainer release tooling; loses bonus coverage on runs where the VM cannot drag,
  no product or user impact).
- Confidence: confirmed from source (dict insertion order, dispatch order and the bonus stop rule).
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g33/f3 (finding), group g33 "Maintainer tooling: brick tests and release-notes screenshots", findings log line 4.
