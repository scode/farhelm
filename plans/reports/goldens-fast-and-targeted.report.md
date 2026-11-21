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
