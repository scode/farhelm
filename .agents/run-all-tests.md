# Run all tests and fix

NOTE: This is not a finishing gate and not something to start on your own initiative. It is what an explicit "run all
tests and fix" request means: every check this machine can run, one deflake sweep for the runtime batteries, then the
release gate in GitHub, with straightforward fixes drafted along the way. Expect it to take several hours, most of it
the sweep.

The point is to find, before a release is cut, what the release process would otherwise find: deterministic failures
that crept in because no ordinary gate runs a suite (the browser suite runs in neither CI nor the release gate), and
failures that only happen on GitHub's runners. It was first done by hand on 2026-10-09, and that run found one of each:
a browser test whose cleanup broke when hosts started refusing more than eight concurrent session operations (#1740),
and clippy failing in CI because CI's floating Rust stable had moved to 1.99 while the local machine was on 1.98.1
(#1742).

## Ground rules

- Nothing runs twice locally. The runtime batteries run only inside the deflake sweep; the static checks below are
  exactly what the sweep does not run. Do not "warm up" by running a battery the sweep will run anyway.
- Fix what is straightforward, on one linear jjstack stack, one draft PR per fix, with the per-PR gates from "Finishing
  work" in the root `AGENTS.md` and a cold-read commit message. Straightforward means a clear cause and a small,
  low-risk fix inside the existing design: a test that assumes an old limit, a lint, a renamed symbol. Anything that
  grows scope, adds complexity, or needs a design or product decision is not fixed here: record it where the deflake
  protocol says (a `TODO.md` bucket, with the evidence) and list it in the final report for the maintainer to decide.
- Flakes are recorded, not chased: the deflake protocol's record (a `TODO.md` Deflake entry, a dated `FLAKES.md` entry
  and a `deflake/known-flakes.txt` line), one PR per flake, no narrow-test ladder.
- Never mark a PR ready and never merge; landing is a separate request.
- Keep a working log in your scratch directory (`/tmp/farhelm-tmp/<id>/`, per "Agent scratch space") from the start:
  every check with its result and recorder run id, the sweep's run id and events, every decision and why, PRs, and
  GitHub run URLs. Name its path in the final report. The log is what lets the maintainer ask afterwards what ran and
  what failed, including after a context compaction.
- The machine is shared. Do not run `rustup update` (other agents share the default toolchain), and leave the live
  install alone as the root `AGENTS.md` requires.

## 1. Start

Start on a clean working copy of the latest `main` (`jj git fetch`, then a new change on `main@origin`). Prepare the
pinned nextest and tmux once (`python3 scripts/install-pinned-nextest.py`, `scripts/build-pinned-tmux-ci.sh`); fix
verification later needs them.

Check that no other deflake sweep holds the machine: `deflake/bin/deflake start` refuses while another daemon lives, but
finding out now avoids running static checks under someone else's sweep. If another checkout's sweep is running, tell
the user. When that sweep is on the same `main` commit, offer to use its results (its run directory under
`~/.local/state/farhelm-deflake/runs/` has `status.json` and `events/`) instead of waiting for the lock; its own session
records its flakes. Coordinate with that session before fixing a failure it reported, so the two of you do not open
duplicate PRs.

## 2. Static and tooling checks

Run these before the sweep: several of them compile, and the sweep's timing-sensitive tests assume a quiet machine. This
is the "Finishing work" inventory minus every battery the sweep runs, plus three checks the inventory does not name (the
CI job's Alpine/BusyBox installer run, the newest-stable clippy, and the rehearsal generator's self-test). Generic
recorder wrapping is fine but not required for these; record each result in the log either way.

- `cargo fmt --all -- --check`
- `python -B scripts/check-test-sleeps.py`, with the isolated interpreter from `docs/test-sleep-check.md` (create its
  venv in your scratch directory)
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
- The same two clippy commands on the newest Rust stable, until the `TODO.md` entry "Pin the Rust toolchain" lands (then
  drop this item). CI installs whatever stable is current when it runs, so a machine that has not updated can pass a
  check CI fails. `rustup check` names the newest stable; install it beside the default with
  `rustup toolchain install <version> --profile minimal -c clippy` and run `cargo +<version> clippy ...` with
  `CARGO_TARGET_DIR` pointing into your scratch directory, so it does not churn the checkout's `target/`.
- `cargo check -p farhelm-ui --features desktop` and `cargo check -p farhelm-desktop`
- `scripts/check-desktop-assets.sh`
- `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh`
- The CI install-script job's Alpine/BusyBox run (the `docker run ... alpine:3` step in `.github/workflows/ci.yml`; the
  sweep runs `test-install-sh.sh` only under the host's GNU userland)
- `dprint check`
- `cd website && bun install --frozen-lockfile && bun run build`, and `cd website/feedback-tests && node --test`
- `dist generate --check`
- `python3 releasing/check-changelog.py format` and `python3 releasing/check-changelog.py --self-test`
- `python3 releasing/rehearse-release-gate.py --self-test`
- `scripts/publish-docs-shots.sh --self-test`
- `bash scripts/test-plans-watch.sh` and `python3 scripts/test-plans-queue.py`, with `shellcheck` over
  `scripts/plans-watch.sh`, `scripts/test-plans-watch.sh` and `scripts/publish-docs-shots.sh`
- `python3 scripts/test-record-test-run.py` and `python3 scripts/test-test-run-traces.py`

Deliberately not here: everything the sweep runs (workspace nextest including the tmux e2e suite, the browser suite on
both engines, desktop nextest, both doctest runs, the two JS harnesses, `test-install-sh.sh`, CentOS provisioning,
desktop smoke), `scripts/test-tmux-pinned-shutdown.sh` (the sweep's workspace nextest runs the same tests on the same
pinned tmux), and the macOS-only uninstall acceptance (the rehearsal's macOS leg runs it in step 4).

A failure here is fixed or recorded per the ground rules before going on. Static fixes can be verified at once by
rerunning the check.

## 3. Deflake sweep

Run one sweep exactly as `deflake/AGENTS.md` describes, including its rule that you take no turn until `wait` returns,
and handle every event as it says. Its PRs go on the same stack as the fixes from step 2, on top of them.

A fix for a deterministic failure needs a runtime check of its own (usually the failing test, through the recorder).
Running it in the middle of the sweep competes with the sweep for the machine; prefer to run it once the sweep is past
its timing-sensitive phases (the nextest and browser phases come first) or after it finishes, and do not count the
sweep's own pre-fix result as evidence for the fix.

## 4. Release gate and CI in GitHub

Only once the static checks pass and the sweep has finished with every failure fixed or recorded. On top of the stack
(or of `main` when there is nothing to fix):

1. `python3 releasing/rehearse-release-gate.py` writes `.github/workflows/release-gate-rehearsal.yml`. Its docstring
   says what the rehearsal runs and what it does not (the two tag assertions).
2. Commit only that file as `ci: rehearse the release gate (do not merge)`, and finish that message before pushing:
   every push to the branch starts another full rehearsal, and rewording after the first push starts a duplicate run.
3. Bookmark it as `rehearsal/release-gate/<short id>`, with a fresh id from `uuidgen` so concurrent rehearsals cannot
   collide, and push it. The push starts the rehearsal. Do not open a PR for it; nothing about it is for review.
4. `gh workflow run ci.yml --ref <that bookmark>` runs the on-demand CI baseline on the same commit.
5. Wait for both runs with one background loop over `gh run view <id> --json status,conclusion` (no `pgrep` liveness
   checks), then read every job's conclusion. The rehearsal's three legs take a while; the macOS leg is usually last.
6. A failure that did not happen locally is the point of this step. Fix it below the rehearsal commit (insert the fix
   into the stack, then let the rehearsal rebase on top), push the fix PR, and push the rehearsal bookmark again to get
   a fresh verdict.
7. When both runs are green, delete the rehearsal branch on GitHub
   (`gh api -X DELETE repos/scode/farhelm/git/refs/heads/<bookmark>`), its local bookmark, and its commit
   (`jj abandon`). The runs and their logs stay on GitHub.

## 5. Report

Report, in product terms per "Talking to the user" in the root `AGENTS.md`: the checks run in step 2 and their results;
the sweep's run id and its events; each flake recorded; the fix stack bottom-up with each PR's link and the failure it
fixes; anything recorded for the maintainer's judgment instead of fixed; the GitHub run URLs and their verdicts; what
was skipped and why; and the working log's path.

## Keeping this file honest

The step 2 list is defined against two other lists: the root `AGENTS.md` "Finishing work" inventory and the phases in
`deflake/bin/deflake`. When either changes, update the list here in the same change, so nothing falls between them and
nothing runs twice.
