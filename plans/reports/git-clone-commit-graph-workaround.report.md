## What this was about

Creating a session in a managed checkout could transfer all the Git objects and then fail with "unable to parse commit"
and "Clone succeeded, but checkout failed", leaving an empty working tree. Git before 2.55 has this defect on any
platform when the repository cache has a commit-graph covering the checked-out commit. It was reported routinely with
macOS's built-in Git 2.54; the older Linux Git normally left no graph after Farhelm's cache fetch.

The agreed workaround disables commit-graph reads for the clone command alone. Cache preparation, locking, credentials
and failure handling keep their existing behavior; neither repository's configuration changes. Git 2.55.0 contains the
upstream fix, so this workaround can go once that version is Farhelm's minimum.

## Things you should know

The regression drives real Git through managed-checkout preparation with a graph-backed cache and file:// transport.
Both are required to expose the bug; a plain local clone would hide it. On Git 2.43, removing the workaround reproduced
the exact clone failure. The workaround and the final regression pass. This verifies the mechanism on Linux, without
assuming the defect is macOS-only.

Existing failed checkouts are not repaired. Delete the failed session and create it again after the fix ships.

## Open questions and possible follow-ups

No implementation decision remains open. Once the fix ships, creating a managed checkout of this repository on the Mac
would confirm it against the originally affected Apple Git. No automated check in this run used macOS's Git.

## The PRs

[PR #1968](https://github.com/scode/farhelm/pull/1968/changes) — avoid managed checkout clone failures on older Git; one
draft review unit.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(launch::tests::d)'` through the recorder: 18 preparation tests
  passed, run `50f1beea-89be-4842-8df6-66ce977c6d8b`.
- The expanded preparation selection covering all callers of the changed fake Git passed 23 tests, run
  `ee01a577-2317-451d-a913-0691705894da`, with pinned nextest/tmux, four slots and no retries. Reused for unchanged
  clone argv, failure paths, cache locking and environment behavior after the test-only review corrections.
- Mutation run `e980ee3e-a90a-47da-8467-a1cdffcaccbb` removed only the production flag and failed as intended: real Git
  2.43 clone exited 128 with "unable to parse commit". The flag was restored; this failure evidence is retained.
- Final regression after review corrections passed, run `f0c0c3ea-e75a-45b8-9b48-672a2de07a31`, through the recorder
  with the same pinned substrate and four-slot/no-retry policy.
- `cargo fmt --all -- --check`, workspace all-target Clippy with warnings denied, then supervisor all-target Clippy for
  the final test changes; changed-Markdown dprint and changelog format validation passed. The isolated test-delay
  checker inspected 298 delays with no missing rationale; its coverage remains valid because no delay code changed
  afterwards.
- Latest-main inspection found no intervening commits at the final fetch; no rebase was needed. Browser, desktop,
  installer, full Rust runtime and hosted CI runs were skipped because the clone command and its preparation fixture are
  the affected behavior, covered by the focused checks.

## Review gate outcome

Opus 5.5 at high effort reviewed correctness, design, idiomaticity and the full test-authoring checklist. It found no
blocking product issue; two low test-proof findings were addressed by clearing inherited Git setup configuration and
checking the cache graph again after preparation. Comment clarifications were also applied. The same reviewer rechecked
these corrections and found every finding resolved, with no new concrete findings. Commit and PR wording passed the
required fresh-context cold read after one rewrite.
