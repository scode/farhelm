## What this was about

Bulk trash cleanup can include archived checkouts whose folders Farhelm could not inspect when the trash was displayed.
The confirmation called these entries unchecked and only explained that folders might stay in place or be forgotten from
the trash. Cleanup checks them again, so a folder that became readable could instead be permanently deleted without
warning about the loss of work never committed or pushed.

The maintainer chose to fix the wording and keep cleanup behavior. The confirmation now explicitly warns about permanent
deletion for these entries too, and explains that the remaining folders stay in place and may be forgotten from the
trash.

## Things you should know

This changes the confirmation text and nearby explanatory comments. It does not change which archives cleanup deletes.
The warning uses the plan's suggested text. The fixed changelog fragment is included, and the resolved feedback item and
index entry are removed with the triage ledger completed.

The original timing-dependent failure was established by source inspection and was not reproduced during this copy-only
change.

## Open questions and possible follow-ups

None identified.

## The PRs

- [#2080: warn of permanent loss for all bulk trash cleanup](https://github.com/scode/farhelm/pull/2080/changes) — one
  draft PR containing the warning and its triage bookkeeping.

## Checks run, reused and skipped

- Ran `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `python3 releasing/check-changelog.py format`, and
  `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md releasing/changelog.d/unchecked-trash-warning.md`; all
  passed. Clippy ran offline in an owned container. Inspected added-line whitespace, searched product source,
  specifications, website and browser tests for the old sentence, and made a separate documentation pass over every
  touched file.
- Reused the source review, Rust formatting, Clippy and changelog results from change `rwpknzxq` at `d4414fba` through
  final head `37c3b89d`. The only later change adds the PR URL to the ledger; the rebase from main `d96041d5` to
  `8ee6973b` adds only another plan's queue state and report. Read that entire incoming diff; it does not affect the
  warning, cleanup implementation or validation coverage. Markdown checks passed after the final ledger update and
  rebase.
- Skipped runtime and browser tests because only copy, comments and triage records changed. No test asserts the old
  sentence; the existing browser assertion checks permanent-loss wording that remains unchanged. No runtime evidence
  from a previous plan is claimed.

## Review gate's outcome

The prescribed fresh-context gpt-6.1-sol review at high effort, through the external harness, found no actionable issues
after reading the complete diff, plan, triage decision and surrounding selection, confirmation and deletion code. Commit
and PR wording passed a separate fresh-context cold read after the title was clarified. No scope departure or unresolved
finding remains.

### Landing

Landed as #2080, the first of the round's two plans; triage-layout-filter-and-devbuild (#2076, #2079, #2081) landed
after it. Main had gained a batch of 50 triage decisions (#2082) and plan bookkeeping, which touch no trash code; the
rebase met an index conflict where both sides removed entries, and both removals were kept.

#### Review

An independent fresh-context review found the new text accurate in every case, checked against how cleanup actually
works: listed entries are either checked or unchecked, and cleanup reopens every selected entry, deleting what it can
now verify and nothing a session still uses, only forgetting missing or mismatched records, and leaving everything else
in place. With only unchecked entries selected the confirmation now warns of permanent loss; with checked entries only
it is unchanged; mixed, both paragraphs agree. The text matches SPEC.md and the docs' trash section, and no test quoted
the old sentence.

#### Fix made while landing

The changelog fragment spoke of "initially unchecked entries" that cleanup can "verify"; it now says what the user sees:
when emptying the trash includes entries Farhelm could not check when it listed them, the confirmation warns that those
can still be deleted for good, losing work never committed or pushed.

#### Left as is

No test renders the confirmation with only unchecked entries, so the fix itself has no regression test; the plan scoped
tests out. A short browser test with a fake trash listing of unchecked entries, checking the confirmation says "lost
permanently", would protect it.

#### Checks

- Run now, on the rebased change: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, the UI crate's
  clippy; the UI crate in full with the desktop feature through the recorder, run
  `098fceae-d181-47ab-be16-a12890f527ff`: 649 passed; the trash dialog spec on Chromium and WebKit, run
  `9c65574c-3aae-42d7-883a-b0f7a42a1c83`: 18 passed.
- Not run: other specs; only the confirmation's text changed.
