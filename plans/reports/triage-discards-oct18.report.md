# Five recorded review discards

## What this was about

The October 18 triage discarded five findings. Four involved rare timing or deliberately constructed conditions with no
demonstrated damage: Git surviving interrupted checkout preparation after its cache lock is released; a failed Restart
selecting a nested Codex conversation for Resume; terminal text imitating capture delimiters and disrupting replay or
entering diagnostics; and an initial launch starting in a different folder if a symlink changes during creation. The
fifth concerned a tunnel following a replacement installation after opening before its host reported an identity; triage
found no reachable shipped trigger for that starting state.

The maintainer chose one combined PR to execute these decisions. It removes the five findings from the open review queue
and records their completion, with the same change, bookmark and PR in each ledger entry.

## Things you should know

Discard means the findings were not worth the maintainer's time now. Execution does not establish that they are fixed or
disproved. Their assessments and the maintainer's original decisions remain in the triage ledger. There are no product,
specification, TODO or changelog changes, and no new reproductions or evidence about the alleged failures.

Final fetch left main unchanged at `0127bafb0047beec8940ce9429fb9e90844092fa`, so there was no intervening main diff to
reconcile. The PR remains open and draft for the landing monitor.

## Open questions and possible follow-ups

None required by this plan. Any future reconsideration would need a new decision; this execution preserves the recorded
reasons, including the tunnel finding's distinct lack of a reachable trigger.

## PRs

- [#2360: close five discarded review findings](https://github.com/scode/farhelm/pull/2360/changes) — all five outcomes;
  bookmark `plan/triage-discards-oct18/01-discards`, change `wmpvqkzkkukrmrqynwykssnxkwzoytwn`, head
  `b02009b12ae46d38805669cbbab44fd0bfe34e0f`.

## Checks run, reused and skipped

- Ran `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md`: passed before PR creation and again after the
  five PR URLs were inserted, covering the final head's Markdown.
- Ran `python3 scripts/check-agents-md.py`: passed with zero errors and warnings at
  `c1ae3a3e5aa77bd9b434cd6be970a152c00eb579`. Reused for the final head because subsequent changes only set commit prose
  and replace five PR placeholders; no additional paths or headings changed.
- Ran `diff -u` between sorted index link targets and sorted feedback filenames, excluding queue infrastructure: passed
  with no differences at `c1ae3a3e5aa77bd9b434cd6be970a152c00eb579`. The independent reviewer also checked this. Reused
  for the final head because the index and feedback files did not change afterwards.
- Skipped Rust, JavaScript, browser, desktop, installer and provisioning tests: this is Markdown queue bookkeeping with
  no executable examples, generated artifacts or runtime behavior. No runtime recorder runs were needed.

## Review gate

The single required fresh-context gpt-6.1-sol reviewer at high effort reviewed the complete change against the full
goal, decisions, all five ledger entries and queue execution rules. No findings. The subsequent amend replaced only the
five temporary PR placeholders with this PR's URL; that diff was inspected separately. The independent resume check
found no discrepancy, and the commit/PR message cold reader found no unclear or contradicted claims or convention
violations.
