# Review-queue discards

## What this was about

This plan carries out the maintainer's decision to discard 21 review findings. They covered speculative test failures,
tests whose strengthening would require new product hooks, product cases without a demonstrated shipped trigger, release
tooling that fails safe, and documented limitations outside Linux. The findings remained in the review queue after that
decision. All 21 have now been removed, with their execution recorded in the triage ledger.

## Things you should know

Nothing changes for someone running Farhelm. Discard means the finding is not worth the maintainer's time now; it does
not mean the finding was disproved or the underlying behavior was fixed. The ledger retains every assessment and
decision. Each completed entry points to the same change and PR.

All 21 entries still carried the expected discard decision and this plan's reference when execution began. None was
skipped, revised, or already removed. The remaining queue index matches all 97 feedback files exactly once.

## Open questions and possible follow-ups

None from this execution. The previously recorded decisions remain available in the ledger if priorities change.

## The PRs

- [#1921](https://github.com/scode/farhelm/pull/1921/changes): removes the 21 discarded findings and records execution;
  one draft PR based on main, with no code or specification changes.

## Checks run, reused and skipped

- Ran `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` after the removal and after adding the PR link;
  both passed. A separate directory/index comparison passed: 97 remaining feedback files, each indexed exactly once,
  with no index entry naming an absent file.
- Reused the initial Markdown and index evidence across a rebase that changed only other executors' queue claims. The
  final URL update was then checked separately.
- Skipped Rust, JavaScript, browser, desktop, installer and provisioning tests: this change only removes review records
  and updates their ledger execution fields. There are no executable examples or generated artifacts to validate.

## The review gate's outcome

The required independent review on `gpt-6.1-sol` at high effort found no defects in the final PR head. It verified all
21 removals against their discard decisions, the unchanged surrounding ledger fields, the completed execution links, the
remaining queue index, and the PR's draft state. The commit/title cold read also conveyed the intended cleanup, with no
claim contradicted by the diff or repository convention violation.

### Landing

Landed on 2026-10-12 (UTC) as #1921. The rebase met no conflicts.

#### Review before merging

No findings. The PR removes exactly the 21 feedback files the plan lists and their index entries; each of the 21 ledger
entries keeps its `discard` outcome and now records completion with this PR; no other ledger entry or field changed; and
the review queue's index lists exactly the files present.

#### Checks

- Run now: `dprint check`, and a comparison of the review queue's index with its directory (no entry without a file, no
  file without an entry).
- Not run: any test suite, since only review-queue files and the triage ledger changed.

Nothing in the report above was made untrue by the landing.
