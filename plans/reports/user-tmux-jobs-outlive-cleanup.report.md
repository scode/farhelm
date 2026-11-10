## What this was about

On Linux hosts with systemd containment, a tmux you start inside a Farhelm session can put its pane jobs in separate
process groups. Ending the session's group ends that tmux server, but jobs that ignore hangup can keep running. Cleanup
previously looked for a live tmux server and retained a machine-wide process sweep to end those jobs. That check missed
servers started after inspection or already gone.

The maintainer chose to accept those jobs surviving as the intent of using tmux, and to remove the check. This PR
implements that decision for both triage findings, updates the cleanup specification and the existing unreleased
changelog entry, and archives both outcomes with the same PR.

## Things you should know

Stop, Restart, Replace, Delete and tab close may leave pane jobs that your own tmux moved outside the session's groups.
The server inside the session's group still ends. This is permission for jobs to survive, not a promise that they always
will: hangup or a portable sweep retained for another reason can still end them.

The other reasons for a full sweep remain: incomplete containment or discovery, a scope already gone before cleanup, an
initially unconfirmed scope, an unusable manager, or a surviving explicit process root. Unscoped cleanup, including
macOS, is unchanged. Bounded membership snapshots remain solely for signal diagnostics. The real stock-tmux test is
retained with the new survival expectation and its existing owned-process cleanup. The change has 93 added code, test
and documentation lines under the plan's size gate; removals, outcome bookkeeping and the fragment are excluded.

## Open questions and possible follow-ups

None identified. No new detection, setting or scope-freezing mechanism was needed.

## The PRs

- [#3297 — let user tmux jobs survive scoped cleanup](https://github.com/scode/farhelm/pull/3297/changes), draft, based
  on main; head `b25610f57dc35b6bfe1858d553f35799f6e78f4a`.

## Checks run, reused and skipped

- Recorder run `bcddf931-f8d9-4ca9-a9d0-dfd601a0682e`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^scope::/) | test(/^service::sweep::/)'` passed all 60
  selected tests. This covers confirmed containment omitting the sweep, already-gone scopes retaining it, incomplete
  containment, surviving roots and unconfirmed cleanup policies. The other 1,762 cases were selected out.
- Recorder run `8db5259e-487e-4e40-8c41-049b3b85ae96`: four focused scoped-cleanup tests in `session_lifecycle` passed:
  ordinary containment without a marker sweep, migrated user-tmux job survival, recorded scopes across a supervisor
  restart, and multithreaded SIGKILL cleanup. The real stock-tmux fixture reported tmux 3.4 and established actual pane
  migration, inherited markers and ignored hangup before Stop. No runtime `SKIPPED` message occurred, so the systemd
  checks ran. The other 543 cases were selected out; JUnit also counts one successful fixture setup.
- Both runs used controlled nextest 0.9.148, four slots, zero retries and verified pinned tmux 3.7c for the harness.
  Both exited zero with complete source identities, reports and output EOF, and no omitted or truncated output. They
  started on the implementation working tree above `ecfab480`; source fingerprints are
  `94080f0289ffafcd4b37812a43797c3f75397776ca8768ea6512008247c0894e` and
  `243c39db729c5f3ca755e4f5bd4376db176a95f99f92d685eecebf9d695029ec`, respectively. Results are reused for the final PR
  head: subsequent changes were a module comment, outcome bookkeeping, PR links and their wrapping, with no executable
  change. No failed runtime run preceded these passes.
- Passed `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, the isolated
  interpreter's `python -B scripts/check-test-sleeps.py` (429 delays, zero missing rationales), dprint on changed
  Markdown, `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`. The latter has zero
  errors and one pre-existing archived heading-reference warning. Dprint initially required wrapping the two new PR URLs
  in the archive; that was corrected and rechecked.
- Skipped full-workspace, browser, desktop, installer and cloud suites because no changed behavior reaches them beyond
  the cleanup contracts exercised above. No live installation or cloud resource was used. The final upstream diff
  contained only queue bookkeeping and another plan's report, so no rebase or runtime rerun was warranted.

## Review gate outcome

The required single fresh-context GPT-6.1 Sol reviewer at high effort passed the implementation, specification, tests
and outcome graduation with no findings. No code-review finding was rejected. A separate blind commit-message reader
confirmed the final explanation matches the change; a fresh native reader also checked this report for an actionable
account and public-repository hygiene.
