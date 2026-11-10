## What this was about

Update, setup recovery and uninstall on a remote Mac could fail repeatedly when the SSH command PATH selected Homebrew's
GNU `stat`. Farhelm passed it BSD arguments. Upload-size reports also stopped working, so an upload's stall timeout
could not arm.

The maintainer chose to support this configuration by always using the Mac's system `/usr/bin/stat`. The change applies
that choice to artifact inspection, upload reports and uninstall, documents it, and archives the resolved triage item.

## Things you should know

Linux retains its existing GNU `stat` selection through PATH. No new production setting or fixed PATH was introduced.
The existing unreleased remote-Mac changelog entry now mentions GNU-first PATH support; no duplicate entry was added.
The code, tests and specification change totals 91 added and removed lines, within the plan's size gate.

Tests run on Linux with owned shell and SSH stand-ins. They substitute only the absolute BSD executable while leaving
GNU `stat` first on the child's PATH. This proves command selection and expected arguments, not native BSD execution.
The metadata regression retains symlink and unchanged-file checks; upload reporting must report the received bytes;
uninstall retains its missing-path, denied-lookup and unsupported-command refusals.

## Open questions and possible follow-ups

None identified within the recorded scope. Other BSD-only commands and real-Mac verification were outside this plan.

## The PRs

- [#3298 — use the system stat on remote Macs](https://github.com/scode/farhelm/pull/3298/changes), draft, based on
  main.

## Checks run, reused and skipped

- Recorder run `cd4649bb-70b7-49e1-a233-6ebf99baae43` passed all 16 selected tests with
  `cargo nextest run -p farhelm-helm --lib -E 'test(remote_metadata_) | test(remote_upload_script_) | test(darwin_upload_ignores_path_stat) | test(/^provisioning::macos::tests::/)'`.
  The other 1,662 cases were selected out. Controlled nextest 0.9.148 used four slots, zero retries and verified pinned
  tmux. Source identity, report and output EOF were complete; no omitted or truncated output, dropped console bytes,
  forced cleanup or runtime `SKIPPED` marker occurred. No failed runtime run preceded this pass.
- That run covered the implementation working tree above `593de1e8`, fingerprint
  `7b7cb3a1509c4002f495abe4456ef048b213dbf4fc22a108f449fbd270d822f2`. Its results apply to the final PR because later
  edits only graduate the outcome and add its PR link. The upstream changes inspected before publication only moved
  queue state and added another plan's report; no executable or contract interaction required a rebase or rerun.
- Passed `cargo fmt --all -- --check`, dprint on changed Markdown, `python3 releasing/check-changelog.py format`, and
  the isolated interpreter's `python -B scripts/check-test-sleeps.py` (429 delays, zero missing rationales).
  `python3 scripts/check-agents-md.py` reported zero errors and one existing archived heading-reference warning.
- Passed `cargo clippy -p farhelm-helm --all-targets -- -D warnings`.
- Skipped full-workspace, browser, desktop, cloud and real-Mac suites: this change selects native commands in the
  provisioning scripts, covered by the focused shell and backend tests. No live installation or remote resource was
  touched.

## Review gate outcome

The required single fresh-context GPT-6.1 Sol reviewer at high effort passed the implementation, tests and specification
with no findings, then checked the final outcome graduation and queue removal with no additional findings. A separate
blind reader confirmed the commit explanation is clear and matches the change.

### Landing

Landed as #3298, second in a round with one other plan, user tmux jobs surviving scoped cleanup
(user-tmux-jobs-outlive-cleanup, #3297), which shares no code with it: Mac uninstall stops processes through its own
background-job manager and the terminal-host stop command, not through session cleanup. Since these changes were
written, main also gained the repository-cache lock repair (#3296), which does not touch Mac provisioning. Rebasing
conflicted only in the triage records and the review queue's index, where both sides had moved entries; the landing kept
both. The landing made no other change to this plan.

#### Review

The landing's fresh-context review found nothing that must change. It searched the whole repository, scripts, release
tooling, packaging and installers included, and every `stat` Farhelm sends to a Mac now uses the system one. Each new
test fails without the fix, and none changes the test process's environment.

#### Things you should know

- Mac uninstall recognises a file that is already gone by `stat`'s exact error text, which starts with `stat:`. That
  relies on the Mac's own `stat` naming itself `stat` when run as `/usr/bin/stat`. The landing expects it does, but no
  test on Linux can show it, and no real Mac was used. If it did not, uninstall would not recognise an already-missing
  file as missing and would stop with an error instead of carrying on. A run of uninstall on a real Mac would settle it.
- The changelog entry says setup, Update and uninstall now work with GNU coreutils first on the PATH. Only `stat` was
  changed; the review found no other Mac-only command syntax, so the claim is probably right, but it was not tested with
  the rest of coreutils.
- The check Farhelm runs on a Mac before setup or Update still requires a plain `stat` to exist on the PATH, although
  Farhelm no longer runs it from there. This is harmless.

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
