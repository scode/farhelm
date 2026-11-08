# Tensorlake state-path refusal

## What this was about

On supported older OpenSSH clients, a percent sign in the helm's state-folder path could leave the Tensorlake gateway's
trusted key unrecorded, allowing later connections to accept a different key. Double quotes and backslashes could
prevent connections. The maintainer chose to refuse these unsupported paths with a clear message, rather than change the
supported OpenSSH versions or introduce different encodings for different clients.

Tensorlake host connections and snapshot builders now refuse paths containing `%`, `"` or `\` before invoking SSH. The
error names the path and tells the user to move the helm's state folder. The implementation specification reflects that
rule. Both feedback items have their execution recorded and have been removed from the queue.

## Things you should know

The refusal applies on every client version, including newer OpenSSH clients that might carry some of these paths
successfully. Spaces and apostrophes remain supported, and the existing `${` refusal remains. Ordinary user-configured
SSH access keeps its existing behavior. The code, tests and specification change totals exactly 100 added and removed
lines, within the agreed gate; the ledger, queue removal and changelog are excluded by the triage rule.

The tests check refusal and argument construction without making a network connection. This run did not reproduce the
original parser defects with an old OpenSSH executable or contact Tensorlake. The installed Farhelm was not changed.

## Open questions and possible follow-ups

None required to complete this plan. No version detection or encoding mechanism was added.

## The PRs

- [#3168](https://github.com/scode/farhelm/pull/3168/changes): refuse unsupported Tensorlake paths, update the spec,
  cover each character, and close the host-key-persistence feedback item. Draft head `c3152917`.
- [#3170](https://github.com/scode/farhelm/pull/3170/changes): close the sibling quote/backslash feedback item, citing
  the first PR. Draft head `b165ff72`, based on #3168.

## Checks run, reused and skipped

- Passed: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, and the isolated
  `python -B scripts/check-test-sleeps.py` check (423 delays inspected, none missing a rationale).
- Passed: `dprint check` on the changed specification, outcome ledger, feedback index and changelog fragment;
  `python3 releasing/check-changelog.py format`; and `python3 scripts/check-agents-md.py` for the deleted feedback
  files. The latter reported zero errors and three existing citation warnings outside this change.
- Passed: recorder-backed `cargo nextest run -p farhelm-helm --lib` with filter
  `test(/^ssh::tests::/) | test(/^tensorlake::ssh::tests::/)`, four slots, zero retries, pinned nextest 0.9.148 and
  `--tmux none` because the selected tests construct options and arguments rather than use terminals. Run
  `f637cb14-fc65-4d97-97df-a4158c5b12b8`: 25 passed, 1,542 selected out, no failures or flaky outcomes. Source capture
  and runner report are complete; retained output is complete with no runtime `SKIPPED` marker or cleanup limitation.
- Reused for the final draft heads: these source checks and the focused test run covered base `082bde33` plus the
  implementation working diff. Subsequent changes only set commit descriptions, added PR URLs to the ledger and closed
  the sibling feedback item. No executable, test or specification content changed. Main advanced through separate
  Credentials UI work and plan/TODO bookkeeping; their full diffs were inspected and do not interact with this refusal.
- Skipped: other Rust suites, browser, desktop, installer and provisioning tests. The change is confined to the shared
  Tensorlake path-option refusal and its argument tests; those suites add no identified coverage for it. The second PR
  changes only feedback records and needs no runtime test of its own. No cloud or live-install check was needed.

## Review gate's outcome

Each PR received its required fresh-context gpt-6.1-sol review at high effort. Both reported no findings. The first
review also verified the 100-line gate and coverage of host and builder options for every character and both managed
path keywords. Separate cold readers checked both commit/PR messages. No review finding was left unresolved.
