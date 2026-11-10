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

### Landing

Landed as #3297, first in a round with one other plan, the remote Mac `stat` fix (mac-system-stat, #3298), which shares
no code with it. Since these changes were written, main gained the repository-cache lock repair (#3296). That repair is
safe only if every Git process using a host's repository cache ends with whatever holds Farhelm's lock on it, including
through session cleanup. The landing's review checked that this plan does not weaken that: a session's Git work on the
cache runs inside the launch's own systemd group, which cleanup still kills whole, and a user's tmux never runs that
Git, so the sweep this plan drops never touched it. Rebasing conflicted only in the triage records, where both sides had
moved entries; the landing kept both.

#### Fix made by the landing

SPEC.md's paragraph on rare accepted failures still listed exceptions to "never alongside them", the old wording of the
rule that Restart ends the previous run's leftover processes before starting the new one. This plan reworded that rule,
and two code comments also quoted the old wording. The landing pointed all three at the current wording; nothing
executable changed.

#### Review

The landing's fresh-context review found nothing that must change. The tmux check is gone entirely, the end-to-end test
with real tmux now expects the moved job to survive, and the changelog entry edited is not yet in any stable release, so
editing it rather than adding a second entry is right.

#### Things you should know

- One case beyond the triage decision. When systemd cannot answer whether a session's scope still exists at the moment
  cleanup checks it (an error, not a "no"), cleanup now always keeps the full machine-wide sweep. That is a third case,
  separate from the two the report lists as already keeping it (a scope already gone, and one not confirmed in the first
  place). In that rare case, jobs a user's tmux moved out of the session are still ended, as all of them were before
  this plan; the maintainer's decision that they may survive does not apply there. It errs toward cleaning up, SPEC.md
  says uncertain containment keeps the sweep, and no test covers this case.
- The executor did not run some supervisor tests whose stand-in systemd now behaves differently: units that vanish now
  take the full sweep in those tests where they used to skip it. Production behavior on those paths is unchanged. The
  landing ran them (see Checks), and they pass.

#### Checks

The checks ran once for the whole round, which landed user tmux jobs surviving scoped cleanup (#3297) and the remote Mac
`stat` fix (#3298) together; the supervisor and end-to-end runs cover #3297, the provisioning run covers #3298. Run now
on this development machine, on the round's stack (both plans on main), all through the recorder with the pinned tmux
first on `PATH`; none printed a runtime `SKIPPED` line:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and
  `cargo clippy -p farhelm-supervisor -p farhelm-helm --all-targets -- -D warnings` passed.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^scope::/) | test(/^service::sweep::/) | test(/^procs::/)'`:
  152 passed (run `fc62a350-df48-443f-bf57-6794c1c853bf`).
- `cargo nextest run -p farhelm --test e2e` with the session lifecycle, supervisor stop, tab lifecycle and GitHub
  checkout modules (`session_lifecycle::`, `supervisor_stop::`, `tab_lifecycle_edges::` and `github_checkouts::`): 121
  passed (run `628ef238-538e-4858-8417-2c48e673f998`), with the systemd user manager available. The checkout tests are
  there because the repository-cache fix that landed just before this round relies on session cleanup ending a launch's
  Git.
- `cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::/)'`: 352 passed (run
  `fe1109fc-1144-4e98-b2f2-90d7b3c376c9`), every Mac script path, including the reach checks and uninstall tests the
  changed test helper also serves.
- For #3297, after the landing's wording fix to it, on that tree: the supervisor's teardown, host-restart and
  scope-handling units (`cargo nextest run -p farhelm-supervisor --lib -E <filter>` with the filter
  `test(/^service::teardown::/) | test(/host_restart/)` or `(test(/^service::(core|handlers)::/) & test(/scope/))`): 41
  passed (run `26f6dfcd-32bb-43a7-beec-2c2f36ff73d2`); and the terminal tab close through its scope (one e2e test,
  `terminal_tabs::` closing a tab that holds an environment-scrubbed double fork): 1 passed (run
  `5aa7ba85-d817-42b4-8922-49d48b86fcb3`). These are cleanup paths that now take the full sweep where they used to skip
  it. The earlier runs predate that fix, which changed only SPEC.md and two comments of #3297.

Reused from the executors: nothing; the runs above include their selections.

Skipped: browser specs (no UI change), the rest of the workspace's Rust tests (only the supervisor's cleanup and the
helm's Mac scripts changed, and the selections above cover their callers), a real Mac (none available to the landing),
and hosted CI (manual-only, not dispatched).
