## What this was about

Permanently deleting a checkout from Trash could take many seconds without any supervisor log showing where the time
went. The requested change adds a diagnostic trail for each trash deletion and for spare-checkout cleanup, which uses
the same removal code.

## Things you should know

Each distinct selected checkout now produces one info completion event with its id, archive path, outcome, total time,
separate admission/database/check/open/removal/update times, and counts of files and directories actually removed.
Partial failures retain their counts. Spare cleanup logs its repository, original spare name, location, outcome,
total/removal times and counts.

These are diagnostic logs; this change does not establish the cause of the reported slowness or speed up deletion.
Replies and deletion safety checks are preserved. Counts include the removed root directory, and non-directory unlinks
include symlinks. Request-wide timings repeat on each checkout event; total time for a batch runs from request start
through that checkout's completion. The app sends one checkout per request. If a shared database read fails before an
archive path can be recovered, the failure event carries an empty path.

The TODO entry is removed, the implementation specification describes the logs, and a changelog fragment is included.

## Open questions and possible follow-ups

No decision is needed to land this work. Actual slow-deletion logs can guide a later performance investigation; no
benchmark result or performance improvement is claimed here.

## The PRs

- [#2839 — log checkout deletion timings](https://github.com/scode/farhelm/pull/2839/changes), draft.

## Checks run, reused and skipped

- Focused supervisor unit tests:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(checkout_trash::) | test(spare_checkouts::)'`, recorder run
  `ed5c7565-ea1f-4b85-89de-f95ec39cd4a4`: 41 passed, 1,345 outside the selection. Pinned nextest and tmux, four slots,
  zero retries. Retained output contains no runtime SKIPPED notices. This covers the new log assertions and existing
  deletion confinement, refusal and spare lifecycle tests.
- The first attempt, run `e10f4e12-231a-4701-976e-38591111fba7`, failed compilation because a new timer reference lacked
  its namespace. That was corrected before the successful run; the failed record is retained.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, dprint on changed
  Markdown, and changelog format validation passed.
- The isolated test-delay checker passed: 327 delays inspected, zero lacking a rationale. Its first attempt exceeded the
  checker's time budget; the retry completed successfully.
- Runtime evidence covers the uncommitted implementation on base `20aba520`, later committed as `93dd1fd0` with the same
  product and test changes. It was reused after rebasing onto main's later queue-claim commit: the only upstream diff
  changed a different plan's claim in the queue index, with no product or test interaction. No older session's evidence
  was reused.
- Browser, desktop, installer and whole-workspace runtime suites were skipped because the change is limited to
  supervisor diagnostics and counters in the already tested traversal. No protocol or UI behavior changes.

## Review gate's outcome

A fresh-context gpt-6.1-sol reviewer at high effort reviewed the single-PR diff against the plan and test-authoring
checklist, with no actionable findings. The commit and PR wording passed a separate cold read. The separate
documentation pass is complete.
