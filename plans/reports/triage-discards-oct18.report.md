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

### Landing

Landed as #2360, unchanged from what the report describes. It was the sixth of eight plans in this landing round. Its
one PR removes the five discarded findings from the review queue and records their completion in the triage record.

#### What landed since the stack was based, and whether it interacted

Since the PR's base, main gained link-hover-covers, workspace-trust-destination, session-header-and-row-fixes,
queued-launch-snapshot, payload-lock-reproduction and plan bookkeeping. Each of those also removes its own entries from
the review queue's index or edits its own triage entries; the only conflicts were in that index, where every removal is
kept, and the triage entries are separate. One of the five discarded findings concerns an initial launch starting in
another folder if a symlink changes during creation; launch-enters-working-dir, blocked in this round, works on a
related problem (sessions starting in the home folder) but does not reopen that decision. Resolving those index
conflicts was mechanical, keeping every plan's removals, so it is not counted as a fix; the landed text is identical to
what the landing checked.

#### Fixes made during the landing

None.

#### Checks

`dprint check` on TRIAGE_OUTCOMES.md and the review queue's index, on the combined result of the round's landing plans:
passed. No runtime tests were run for this PR, since it changes only Markdown records. It was part of the combined tree
on which the landing ran the round's Rust, JavaScript and browser checks (runs `cdabc40d-ecaa-418e-938f-01f2cbe4da64`,
`fcb2b9cf-4424-4fc2-8148-dede8454a5ea` and `1484ba9a-0aab-4e7f-a7a4-716faede7c66`), all of which passed.

The report's statements that main had not moved since the PR's base and that the PR was still an open draft were true
when it was delivered; main then moved as described above, and the PR has now merged. Nothing else in the report became
untrue at landing.
