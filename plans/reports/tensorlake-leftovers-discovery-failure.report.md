## What this was about

In Settings → Tensorlake → **Sandboxes not on this helm**, a failed refresh could hide sandboxes that still needed
deletion. If some cleanups were already running, the list could show only those rows and put the refresh error on each
of them.

Settings now keeps the last successful list and shows the refresh error above it, as requested. Cleanup status continues
to update during the outage. When cleanup takes over a sandbox's row, its older discovery row is forgotten, so a
completed cleanup disappears without bringing that older row back. A successful refresh replaces the list and clears the
error.

## Things you should know

The 20-second discovery deadline covers waiting for the saved key and checking which sandboxes the helm owns, as well as
waiting for Tensorlake. A stalled local read can therefore report its own error and still return the cleanup snapshot.
If Settings has yet to discover any sandboxes, failure shows the error without claiming the account is empty.

## Open questions and possible follow-ups

None.

## PRs

- [#4675](https://github.com/scode/farhelm/pull/4675/changes) keeps known Tensorlake sandboxes visible while discovery
  is unavailable.

## Checks

- Ran `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/tensorlake-leftovers-discovery-failure.md` and
  `git diff --check` on the rebased change. Both passed.
- Reused the pinned `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib` with the Tensorlake
  discovery, provisioning and Settings filters at `1a13c5e70fbd`, run `caa9e360-c9ab-4dd7-bcc4-1b1d73bfdc98`. All 73
  selected tests passed. The helm cases used an owned SQLite store and fake Tensorlake, and Settings used a fake helm.
  The cases cover failure, cleanup starting and finishing during an outage, overlapping refreshes, deadline, key clear,
  Delete and recovery.
- Reused `python3 scripts/check-gui-goldens.py --scene 'settings-tensorlake*' -j 4`, run
  `a61bc2a1-d0f8-4eae-adab-012c4ef89bf0`, and `cargo clippy -j 4 --all-targets -- -D warnings`, run
  `c6ad0c9d-84b0-48ad-a51c-1ec3a30fc05e`, at `1a13c5e70fbd`. The selected reference renders matched and Clippy passed.
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, and
  `python3 releasing/check-changelog.py format` passed at the same revision.
- Main gained the independent Suspend and Resume routes change and bookkeeping updates. The rebase leaves the discovery
  and Settings patch intact. Resolving the adjacent TODO removal preserves main's changes and removes only this plan's
  entry, so the earlier checks still cover the rebased change.
- Skipped the full test and reference render suites because the targeted checks cover the affected workflow and its
  reference renders matched. Skipped live Tensorlake because the local fixture can reliably trigger failures and
  timeouts while cleanup runs.

## Review gate

An independent source review found that overlapping refreshes could leave a deleted sandbox listed during an outage.
Settings now remembers that cleanup began even if it has already requested another refresh with the same saved
Tensorlake API key. The review also led to coverage for cleanup starting during an outage, a direct check of the alert's
position above the rows, and the deadline for local discovery reads. All four findings were addressed and the revised
change passed the checks above.
