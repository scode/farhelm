## What this was about

The screenshot goldens compare the interface against saved reference images. Reviewing a small visual change used to
redraw the entire set, and one broken scene could stop it before the rest ran. The runner can now render just the scenes
you name and report a broken scene alongside failures from the remaining scenes. The maintainer chose four processes by
default for full runs.

## Things you should know

While revising Settings, run `python3 scripts/check-gui-goldens.py --scene 'settings-*'`. Repeat `--scene` for several
shell-style patterns. Each pattern must match a scene. Use `--update` to capture those screens for inspection. Run a
full update before publishing, since a shared control can affect screens beyond the ones selected.

The runner compares pixels and keeps a new PNG only for a missing, unreadable or different reference. It reports the
capture path. Each comparison run gets its own directory. A failed run keeps its automatically fetched reference set
alongside the new PNGs for its failures. Updates save each PNG once, and updating and publishing share an exclusive lock
on the capture directory. The runner marks a full successful update complete after all the rendering processes exit and
the expected set of names is present.

Use `--jobs 1` for one process. The default is four, capped by the available CPUs and the number of selected scenes.
Cargo build arguments still come before a bare `--`. Arguments after it control output only. Use `--scene` for filtering
and `-j` for Cargo's build-job limit.

The first development or test build with the optimized image, renderer and UI dependencies recompiles those crates. In
the dedicated Linux planning trial, a single process took 668 seconds to render 328 screenshots with the original build
settings and 89 seconds with those dependencies optimized. Four processes using the same optimized build rendered the
full set in 27 seconds. All three figures exclude compilation and come from the original harness that encoded each PNG
twice. Local execution on the shared host verified byte identity.

## Open questions and possible follow-ups

The changed-screenshots review page has its own queued plan. Native macOS renderer coverage remains separate. No
decision is pending with the maintainer.

## Pull requests

- Optimize the dependencies used for development and test screenshot rendering.
  [#4450](https://github.com/scode/farhelm/pull/4450/changes)
- Write each kept capture once and report failed scenes together.
  [#4451](https://github.com/scode/farhelm/pull/4451/changes)
- Select screenshot scenes by pattern. [#4452](https://github.com/scode/farhelm/pull/4452/changes)
- Render groups of scenes in parallel. [#4454](https://github.com/scode/farhelm/pull/4454/changes)

## Checks

- Local full Linux updates before and after the dependency optimizations produced 328 byte-identical PNGs. On
  `e97e39dc`, run `python3 scripts/check-gui-goldens.py --update` (recorder `7b65d675-97f7-4e36-83d4-27b6440f09f2`). On
  `bdf8d06a`, build with `cargo test --locked --no-run -p farhelm-gui --lib` and run
  `<test-binary> --ignored --exact goldens::component_goldens` (recorder `8b5cbf65-e840-41b6-8586-aee84ca7fef2`). The
  direct run used the script's lavapipe and Mesa EGL paths, `LIBGL_ALWAYS_SOFTWARE=true`, a private `XDG_RUNTIME_DIR`,
  an isolated `FARHELM_UPDATE_GOLDENS` destination, and cleared `DISPLAY` and `WAYLAND_DISPLAY`.
- `python3 scripts/check-gui-goldens.py --update` on the write-once change at `2643ca81` produced the same 328 PNGs and
  a completion marker containing their count. The screenshot test process inherited the update's lock through Cargo.
  Recorder `0c2fc2e8-829f-45ed-b3d0-b6c03938d952`.
- At the same revision, `FARHELM_GOLDENS_DIR=<controlled-references> python3 scripts/check-gui-goldens.py` reported
  exactly one missing and one different reference. The controlled set was a copy of the optimized baseline with one file
  removed and another replaced by a different PNG. The run kept only the two new captures associated with those
  failures. Every other comparison passed. Recorder `7e444ed5-a450-4821-9a4e-d4b9c8671d2f`.
- `python3 scripts/check-gui-goldens.py --scene 'settings-search*' --scene settings-appearance` passed comparison
  against the baseline on `776dff07` (recorder `c0b90075-6e98-4b7c-bbcf-9be6cd132160`). The same command with `--update`
  and `RUST_TEST_THREADS=1` produced six PNGs without a completion marker after the listing fix in #4452 (recorder
  `aa324818-f7bf-4e51-9898-cd52352487b4`). Replacing the second pattern with `not-a-real-scene` was refused before
  rendering (recorder `6207fae8-6d50-4861-a02e-079234ea0fd4`). The final PR 3 fixture run passed nine cases (recorder
  `2cd517b1-1e41-4986-8a06-120d1cbe7802`).
- The final default `python3 scripts/check-gui-goldens.py --update` wrote 328 PNGs and the completion marker and matched
  the one-process `--jobs 1` update and the unoptimized baseline byte for byte. Default recorder
  `e708a5e3-54c0-482e-aaf2-528b493aa1bf` used the final runner on the rebased stack. Serial recorder
  `51c1120e-bda2-4016-87be-7499dc8d33b4` used the same renderer binary, scene order and output paths on the PR 4 working
  copy above `28092290`, before cancellation handling was revised.
- `python3 -B scripts/test-check-gui-goldens.py` passed all 13 cases. This includes a killed worker with all PNGs
  present, a TERM arriving during worker launch, and a TERM during cleanup after a worker exited successfully. Recorder
  `15a6194e-59c7-4285-9945-db091cf57ed3`.
- `python3 -B scripts/gui-goldens.py --self-test` passed all 15 storage cases. Recorder
  `eefee3a1-5f00-4d8c-b5ea-c5f831f92311`.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, dprint for the edited
  Markdown, the instruction and changelog checks, the Rust test delay check, and the locked Cargo dependency license and
  source check passed. The instruction check reported an existing stale archive heading citation.
- The dependency, write-once and selection checks above were reused after rebasing onto `329f2644`. Its changes targeted
  the docs page capture script, the GUI automation driver and their instructions. The golden scripts, renderer inputs
  and Cargo settings used by those checks stayed the same. The final parallel update, fixture checks and formatting
  checks ran on the rebased stack. Native macOS, installer and full-workspace suites were outside the selected checks
  because this plan changes development screenshot tooling.

## Review gate

All four drafts passed independent Sol high source reviews and cold reads of their commit wording. Review led to the
update and publish lock, isolated reference files, recovery after a broken fixture, a catalog that works with one
libtest thread, and cancellation that reaps workers before deciding whether a run can be published.

### Landing

#### What landed with it

The plan's four PRs, #4450, #4451, #4452 and #4454, merged with a landing fix, #4455. Nothing else landed in this round,
and main had gained nothing since the executor based the plan. The plan before it, docs-shots-targeted-runs, landed an
hour earlier and changed the docs screenshot tooling, which shares no files or behavior with the golden runner. An
independent reviewer and I read the four PRs against the plan. Neither of us found a blocker.

The speed-up is real on this machine too. The full golden comparison, with the default four processes side by side,
rendered all 328 pictures in about 33 seconds of wall-clock time on the shared development machine, not counting the
build. The same comparison took between 8 and 20 minutes there before. The first build with the new settings took about
7.5 minutes in all, once.

#### Fixes made while landing

One fix, in [#4455](https://github.com/scode/farhelm/pull/4455/changes). An update run cleared the folder of pictures
waiting to be published before it checked the `--scene` patterns. A mistyped pattern was refused, but only after an
earlier full update's pictures were gone, so publishing them needed another full update first. The update now builds the
test program and checks the patterns before it clears anything. A test now shows that. The same PR rewords one stale
code comment.

#### Checks

The plan changes the development build settings in `Cargo.toml`, the GUI crate's golden test, the golden scripts and
their tests, and instructions. The landing ran the GUI crate's tests, the lint runs and the app's end-to-end smoke test
on a separate arm64 Linux test machine, and the goldens and script tests locally on x86_64.

On the plan as delivered, stacked on main:

- `cargo nextest run -p farhelm-gui`, on the test machine with the new build settings (recorder run
  `94be0830-6c80-4d94-a7a5-43000750d507`): all 3238 passed; the three skipped are the golden comparison and its scene
  listing, both run below through the golden script, and a generator that runs only by hand.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test machine (recorder run `65285cba-2f16-447d-8ba0-0e4287929250`):
  every step passed but paste, which skips on Linux as it always does.
- `cargo clippy --all-targets -- -D warnings` and the desktop app's own two clippy runs, on the test machine, not
  through the recorder: passed.
- `python3 scripts/check-gui-goldens.py`, the full comparison with the default four processes, locally (recorder run
  `e6b7d3b4-d33b-494b-ab91-3f3d59f9112a`): passed, about 33 seconds of rendering in each of the four processes, side by
  side.
- `python3 scripts/check-gui-goldens.py --scene 'no-such-scene-*'` was refused before anything rendered, naming the
  pattern.
- `python3 -B scripts/test-check-gui-goldens.py` (recorder run `80325694-a1fe-4bd5-8d56-d7d46c64e71c`, 13 passed) and
  `python3 -B scripts/gui-goldens.py --self-test` (recorder run `97f7a4cc-0a2c-48e6-a9d2-b5303759271d`): passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its isolated environment), `dprint check`,
  `python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format`: passed.

After the landing fix (#4455), which changes the update's order of steps, a test, a fragment and a comment:

- `python3 -B scripts/test-check-gui-goldens.py` (recorder run `8037e992-bfbf-42c2-93f2-6f31815ca131`): all 13 passed.
  With the fix reverted, the strengthened test fails, so it does check the fix.
- `python3 scripts/check-gui-goldens.py --scene 'settings-*' --scene 'terminal-*'`, locally (recorder run
  `b70852cf-d4e3-4297-a886-fa5a6fe23fd2`): passed, 37 scenes on four processes in about 10 seconds each.
- `cargo fmt --all -- --check`, `dprint check` and `python3 releasing/check-changelog.py format`: passed.

The other runs above still apply to the fix: it changes no code the GUI tests, the smoke or the lint runs use, and the
Rust change is a comment. Skipped: the workspace suite and the desktop app's tests, since only build optimization
settings changed for them, which changes no behavior; the dependency license notices, since no dependency changed;
macOS, which this plan does not touch; CI, which runs only on demand.

#### Things you should know that the report does not say

- The plan required the byte-for-byte check and the timings to come from one dedicated test machine, with a fallback
  only if it could not be created. The executor never created one: its working log cites the standing instruction to run
  checks locally. It checked byte-for-byte identity on the shared development machine instead, so this plan's runner was
  never timed under controlled conditions. The report's 668, 89 and 27 seconds come from the planning trial. The 33
  seconds above are the landing's own number, on a busy shared machine.
- Every development and test build of Farhelm now compiles 31 of its image, graphics and interface libraries optimized,
  as you decided. Farhelm's own code stays unoptimized. Tests of other parts of Farhelm behave the same. Each checkout's
  first build after updating main recompiles those libraries, once.
- A follow-up worth doing: each comparison run gets a fresh folder, and a run that fails keeps it, with its own full
  copy of the reference pictures (about 59 MB) next to its new pictures. Nothing removes the old folders, and the
  cleanup an executor runs when it closes a plan does not name them, so they pile up in each checkout's build folder.
  The plan asked for the run's folder to be cleared at the start of each run. The executor's tests require that a later
  comparison leaves an earlier failure's pictures in place; neither its report nor its log says why. Changing that is a
  design call, so I left it rather than making it a landing fix. A small change would remove old run folders when a new
  comparison starts, or keep only the reference pictures of failed scenes.
- The plan's PRs use `perf`, `fix` and `feat!` for tooling nobody outside development sees. The `!` marks that the
  golden script no longer passes test-name filters given after a bare `--`; only development command lines are affected.
  The four changelog notes all say so and ask for no release entry.
- The pending plan golden-changes-page builds a page of changed pictures on this runner. Its plan still says that a
  failed comparison leaves its pictures beside the test program. They are now in the run's own folder, which the failure
  message names. Its fallback covers that.
