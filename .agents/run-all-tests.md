# Run all tests and fix

NOTE: This is not a finishing gate and not something to start on your own initiative. It is what an explicit "run all
tests and fix" request means: local checks (including live agent drift), one deflake sweep for the runtime batteries,
then the release gate in GitHub, with straightforward fixes validated and landed along the way. Expect it to take
several hours, most of it the sweep.

The point is to find, before a release is cut, what the release process would otherwise find: deterministic failures
that crept in because no ordinary gate runs a suite, and failures that only happen on GitHub's runners. It was first
done by hand on 2026-10-09, and that run found one of each: a browser test (from the since-removed browser suite) whose
cleanup broke when hosts started refusing more than eight concurrent session operations (#1740), and clippy failing in
CI because CI's floating Rust stable had moved to 1.99 while the local machine was on 1.98.1 (#1742).

## Reading list

Before starting, read these in full, and again after any compaction or resume:

- this file
- `deflake/AGENTS.md` (the sweep's protocol, which this flow runs and whose flake record it uses)
- `.agents/validation.md` (the check inventory step 2 is defined against)
- `.agents/test-evidence.md` (the recorder and retained runs)
- `.agents/machine.md` (scratch environments, `CARGO_TARGET_DIR`, what stays where it goes)
- `docs/test-sleep-check.md` (the sleep checker's isolated interpreter)
- `docs/harness-drift-tests.md` (the live agent suite and its prerequisites)

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Ground rules

- Nothing runs twice locally. The deflake sweep owns the routine runtime batteries. The tooling checks and live agent
  suite below have their own commands. Do not "warm up" by running a battery the sweep will run anyway.
- Fix what is straightforward, on one linear jjstack stack, one draft PR per fix, with the per-PR gates from
  `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, and a cold-read commit message. Straightforward
  means a clear cause and a small, low-risk fix inside the existing design: a test that assumes an old limit, a lint, a
  renamed symbol. Anything that grows scope, adds complexity, or needs a design or product decision is not fixed here:
  record it where the deflake protocol says (a `TODO.md` bucket, with the evidence) and list it in the final report for
  the maintainer to decide.
- Act on failures as they arrive: investigate and make reasonable fixes proactively, without waiting for the sweep or
  the whole flow to finish, or for another request from the maintainer. During the sweep, defer only runtime validation
  that would compete with its timing-sensitive phases, as step 3 describes; that is not a reason to postpone the fix.
- Flakes are recorded, not chased: the deflake protocol's record (a `TODO.md` Deflake entry, a dated `FLAKES.md` entry
  and a `deflake/known-flakes.txt` line), one PR per flake, no narrow-test ladder.
- Land each straightforward fix as soon as its relevant validation and review pass, using normal jjstack landing. Do not
  wait for the sweep or the hosted rehearsal to finish when the fix already has adequate evidence. This flow authorizes
  marking ready and merging its fix and record PRs by default; no separate landing request is needed. Record-only PRs
  are ready once their document checks pass. A fix that needs a design decision, adds non-trivial complexity, or expands
  scope stays unimplemented and recorded for the maintainer, as above. Do not turn an uncertain fix into a merge merely
  to clear a failure.
- Keep ready, independent fixes below unfinished fixes in the linear stack so unrelated work cannot delay their landing.
  Reorder only when the changes are independent, preserving their diffs and validation coverage; an unready prerequisite
  still blocks its dependents.
- Keep a working log from the start at `farhelm-run-all-tests-log-<checkout>.md` in the directory that contains the
  checkout, where a compacted session can find it again (a scratch directory can be lost track of). Record in it this
  flow's name, the reading list above, and your scratch directory (`/tmp/farhelm-tmp/<id>/`, per `AGENTS.md`, "Agent
  scratch space"); then every check with its result and recorder run id, the sweep's run id and events, every decision
  and why, PRs, and GitHub run URLs. Name its path in the final report. The log is what lets the maintainer ask
  afterwards what ran and what failed, including after a context compaction.
- The machine is shared. Do not run `rustup update` (other agents share the default toolchain), and leave the live
  install alone as `AGENTS.md`, "The live install is off-limits", requires.

## 1. Start

Start on a clean working copy of the latest `main` (`jj git fetch`, then a new change on `main@origin`). Prepare the
pinned nextest once (`python3 scripts/install-pinned-nextest.py`); fix verification later needs it. Make sure the
machine has a tmux on `PATH` (the distribution's is fine): the tests that start a real tmux server print `SKIPPED`
without one, which loses their coverage, so the sweep's preparation stops at its tmux check when there is none.

Check that no other deflake sweep holds the machine: `deflake/bin/deflake start` refuses while another daemon lives, but
finding out now avoids running static checks under someone else's sweep. If another checkout's sweep is running, tell
the user. When that sweep is on the same `main` commit, offer to use its results (its run directory under
`/tmp/farhelm-tmp/deflake/runs/` has `status.json` and `events/`) instead of waiting for the lock; its own session
records its flakes. Coordinate with that session before fixing a failure it reported, so the two of you do not open
duplicate PRs.

## 2. Static and tooling checks

Run these before the sweep: several of them compile, and the sweep's timing-sensitive tests assume a quiet machine. This
is the inventory in `.agents/validation.md` minus every battery the sweep runs, plus two checks the inventory does not
name (the CI job's Alpine/BusyBox installer run and the rehearsal generator's self-test). Recorder wrapping is optional
for static and tooling checks. Use the recorder for runtime test entries, including the ignored terminal emulator tests,
and record each result in the log. Rust commands use `rust-toolchain.toml`, the same pin CI and release builds use; no
second clippy run on floating stable is needed.

- `cargo fmt --all -- --check`
- `python -B scripts/check-test-sleeps.py`, with the isolated interpreter from `docs/test-sleep-check.md` (create its
  venv in your scratch directory)
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
- `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, and the same with `--features agent`
- `python3 -B scripts/check-gui-lints.py` and `python3 -B scripts/test-check-gui-lints.py` (every GUI lint ban fires,
  only its owners switch one off, and no crate switches one off as a whole)
- `python3 -B scripts/check-crate-layering.py` and `python3 -B scripts/test-check-crate-layering.py`
- `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh`
- `bash -n scripts/devbuild-mac.sh && bash -n scripts/test-provision-centos.sh`, and `shellcheck` on both scripts. The
  scripts handle an absent pin in the CentOS setup and multiline active-toolchain output in Mac devbuild. The workspace
  Rust selection covers the devbuild's remote-update startup preference.
- `scripts/build-desktop-binary.sh --self-test` and `shellcheck scripts/build-desktop-binary.sh` (the desktop archive's
  build command; the self-test builds nothing)
- `bash -n scripts/build-podman-test-payloads.sh` and `shellcheck scripts/build-podman-test-payloads.sh` (the focused
  nextest producer with a checkout-local build). Its payload/install contract runs in the sweep's real Podman selection.
- The CI install-script job's Alpine/BusyBox run (the `docker run ... alpine:3` step in `.github/workflows/ci.yml`, with
  a static musl `farhelm` from `cargo build -p farhelm --target x86_64-unknown-linux-musl` passed as `--farhelm`; the
  sweep runs `test-install-sh.sh` only under the host's GNU userland). Both runs include the installer's scenario-scoped
  request counts and diagnostics for unexpected requests in refused `/latest` cases, plus legacy upgrade-floor refusal,
  dedicated status propagation and preservation of the old app and command files.
- `dprint check`
- `python3 -B deflake/test-deflake.py` (deflake tooling, including state-root ownership and sweep base ancestry). The
  recorder's own tests are listed below.
- `python3 scripts/check-agents-md.py` and `python3 scripts/check-agents-md.py --self-test` (includes both Claude
  instruction-file names in the repository inventory and mixed-case specification pointers)
- `cd website && bun install --frozen-lockfile && bun run build`, and `cd website/feedback-tests && node --test`
- `node --test website/scripts/release-notes.test.mjs` after installing website dependencies
- `python3 releasing/generate-workflow.py --check` with pinned cargo-dist 0.32.0,
  `python3 releasing/generate-workflow.py --self-test` and `shellcheck releasing/check-artifact-handoff.sh`
- `python3 scripts/install-pinned-cargo-deny.py`, then `<printed directory>/cargo-deny --locked check licenses sources`
  (the dependency license and source policy in `deny.toml`)
- `python3 -B scripts/dependency-notices.py --check` and `python3 -B scripts/test-dependency-notices.py` (the generated
  dependency license notices are current)
- `python3 releasing/check-changelog.py format` and `python3 releasing/check-changelog.py --self-test` (release-note
  format, including the v0.40.0 policy section, and fragment and announcement checks)
- `python3 releasing/finalize-release-notes.py --self-test`
- `python3 releasing/notes-review-page.py --self-test` (legacy standalone page; includes link attribute escaping and
  HTTP(S)/relative-only targets)
- `python3 releasing/rehearse-release-gate.py --self-test`
- `scripts/publish-docs-shots.sh --self-test`
- `scripts/publish-readme-hero.sh --self-test` and `shellcheck scripts/publish-readme-hero.sh`, and
  `python3 -I scripts/test-scenario-json5.py` (the scenario loader the hero publisher reads its size through)
- `shellcheck scripts/readme-screenshot.sh scripts/docs-screenshots.sh scripts/readme-video.sh` and `bash -n` on each
  capture command
- `bash scripts/test-plans-watch.sh` and `python3 scripts/test-plans-queue.py`, with `shellcheck` over
  `scripts/plans-watch.sh`, `scripts/test-plans-watch.sh` and `scripts/publish-docs-shots.sh`
- `python3 -B scripts/plans-landing/test-landing.py`, `deno check` and `deno test -A` on
  `scripts/plans-landing/idle-watch.ts`, and `shellcheck scripts/plans-landing/notify.sh` (the landing monitor's round
  tooling, including the Python module import before checking the round safeguards)
- `python3 scripts/test-todo-picker.py` (TODO extraction and legacy standalone checklist)
- `python3 -B scripts/test-capture-agent-screens.py` (fixture URL privacy and terminal host ownership with stand-ins, no
  vendor turns)
- `python3 -B scripts/test-run-harness-drift.py` (runner stand-ins verify category selection, one upgrade per harness,
  per-case timeouts and verdict retention with zero vendor turns)
- `deno test -A scripts/serve-page.ts` (legacy standalone server; agent pages use the `interactive-artifact` skill)
- `deno test -A scripts/tensorlake/sandbox-tests.ts`, `deno test -A scripts/tensorlake/rust-tests.ts`,
  `deno check scripts/tensorlake/rust-tests.ts` and
  `shellcheck scripts/tensorlake/bootstrap.sh scripts/tensorlake/run-rust-shard.sh` (the Tensorlake harness's own
  checks, including snapshot listing, the archive configuration preserving scoped Podman payload setup, and a local CLI
  stand-in for shard verdicts and retained evidence). Run the harness only on the maintainer's request, per
  `.agents/tensorlake.md`.
- `deno check scripts/boat/boat-tests.ts`, `deno test -A scripts/boat/boat-tests.ts` and
  `shellcheck scripts/boat/sandbox/*.sh` (the boat harness's own checks, including the shard configuration preserving
  the scoped Podman payload setup while removing the local fixture build, grouping docs shot selectors by page, and
  clearing only the selected docs outputs). Run the harness only on the maintainer's request, per `.agents/boat.md`.
- `shellcheck -x scripts/test-worker/test-worker.sh scripts/test-worker/remote/*.sh` and
  `python3 -m py_compile scripts/test-worker/lock-output.py scripts/test-worker/remote/prune.py scripts/test-worker/remote/run-workload.py scripts/test-worker/test-test-worker.py`
  (the SSH test worker harness's checks, including the evidence-copy driver). On Linux, run
  `python3 -B scripts/test-worker/test-test-worker.py` through the recorder for local stand-in coverage of staging,
  bootstrap, detachment and evidence ownership. Use the worker only on the maintainer's request, per
  `.agents/test-worker.md`.
- `python3 scripts/test-record-test-run.py` and `python3 scripts/test-test-run-traces.py` (the runtime recorder,
  including nested failure evidence, and trace tooling)
- `python3 -B scripts/test-summarize-test-runs.py` and `python3 -B scripts/test-test-run-summary.py` (ledger selection,
  flake counts and partial run discovery)
- `python3 -B scripts/test-install-pinned-nextest.py`, `python3 -B scripts/test-install-pinned-cargo-deny.py` and
  `python3 -B scripts/test-install-pinned-cargo-about.py`
- The vendored terminal crates' three recorded test commands from `.agents/validation.md` (`vendor/vte`,
  `vendor/alacritty_terminal`, and both together in `vendor/pair-tests`), which the sweep's workspace batteries never
  reach
- The vendored GPUI core's recorded native-key, driver record, hitbox occlusion and motion preference tests from
  `.agents/validation.md` (`vendor/gpui-pre`), which the workspace batteries never reach either
- The GPUI app's screenshot goldens from `.agents/validation.md` (`scripts/check-gui-goldens.py`), ignored in the
  workspace batteries because they need the software renderer the script selects, and needing read access to the storage
  repository that holds their references (or a fetched copy in `FARHELM_GOLDENS_DIR`). The development profile optimizes
  their PNG, rendering and UI dependencies, so the first build with that profile can take longer. This sweep uses the
  full comparison and the runner's default of four processes (capped at available CPUs). While iterating on a particular
  view, use `--scene PATTERN` (repeatable). `--jobs 1` runs only one renderer process.
- `python3 -B scripts/test-check-gui-goldens.py` (private comparison outputs, the update lock, scene selection and
  completeness against a stand-in renderer, an interrupted worker and the golden changes page with its native review
  requests. The GUI unit tests cover the native highlight and resize calculations.)
- `python3 -B scripts/gui-goldens.py --self-test` (fetching and publishing the golden references)
- `python3 -B scripts/test-gui-agent.py` (including the bounded automatic parking report), and one end-to-end run of the
  GPUI app harness from `.agents/validation.md` (`scripts/gui-agent.py <run dir> start`, a `snapshot` and a
  `screenshot`, then `stop`), which needs `mutter` and lavapipe
- `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot batch page)
- `python3 -B scripts/test-docs-shots.py` (repeatable page and shot selection, draft output boundaries, frozen release
  shots, crop geometry and capture failures against a stand-in harness); one live `scripts/docs-screenshots.sh --local`
  run on Linux, checking the images at 2x and the full capture record
- The recorded demo-video checks from `.agents/validation.md`: recorder unit tests, the capture-build overlay test, and
  a live `scripts/readme-video.sh` run on Linux with its marked stills checked.
- `python3 -B scripts/test-gui-e2e-smoke.py` (the end-to-end smoke's check that every step is in its `Smoke` class,
  under a number and a name no other step uses)
- The GPUI app's end-to-end smoke test from `.agents/validation.md` (`scripts/gui-e2e-smoke.py`, through the recorder),
  which checks Help's open and Escape focus return alongside reordering pins and moving an unpinned session into a group
  by its grip. It needs what the harness's end-to-end run needs.
- The Linux app's lifecycle check from `.agents/validation.md` (`scripts/gui-lifecycle.py`, through the recorder), which
  needs the same
- The terminal emulator's ignored tests from `.agents/validation.md`, in release: the snapshot spike's full round-trip
  configuration, the review's short scripts and hostile blobs, and the per-terminal memory measurement (on Linux), whose
  assertions fail when an emulator exceeds the crate's stated budget; the workspace batteries skip ignored tests
- The snapshot encoder benchmark from `.agents/validation.md`, in release, recording machine load and its five-sample
  medians and spreads for typical, true-color and image-like output. It asserts fixture shape and repeated bytes, not
  speed; the workspace batteries skip this ignored measurement too.
- The recorded portable brick-test helper suite from `.agents/validation.md`, including native-runtime and Codex model
  selection, manual restart readiness and automatic remote upgrade continuity, with process-group inspection available;
  this exercises Rust/Python upgrade-floor agreement, fixed-floor baseline selection, compiled task contracts, fixed
  operations, fresh-versus-update installer entrypoints, fixed command-only execution and provenance, bounded pre-action
  provider recovery with checkpoint floors, attempt-local continuity exports on deferral, bounded read-only readiness
  deferrals and evidence-checked retries of legacy pre-action sampling failures, retained unavailable-fixture retries
  excluding product failures and feedback, structured native actions, restart handoff failures, durable update evidence,
  exact native update-channel readouts, normal Finder readiness, conditional product confirmations, remote menu
  Remove/re-add and Uninstall with independent effect receipts, old-session survival and Restart onto a new terminal
  host, calibrated window-trace and bonus gesture-readiness gates, RC and dev discovery skip continuation, stable
  discovery failure, consent-checked scope restoration, signed-announcement launch isolation and missing-feed reporting,
  and remote launcher visibility. The guest-only Swift helper separately needs SDK type-check and read-only runtime
  validation in an owned disposable guest. None establishes live release product checks. Report platform skips.
- Terminal host coverage and mutation testing from `.agents/validation.md`
  (`cargo +nightly llvm-cov -p terminal-host --branch -- --test-threads=1`, and
  `RUST_TEST_THREADS=1 cargo mutants -p terminal-host` run the contained way that entry gives; both on one test thread,
  since the crate's fixtures do not support parallel libtest threads), compared with the baseline in
  `crates/terminal-host/TESTING.md`: a drop in line coverage below 95% or a new surviving mutant is a finding.

Deliberately not here: everything the sweep runs (workspace nextest including the e2e suite, desktop nextest, both
doctest runs, the OMP reporter's JS harness, `test-install-sh.sh`, CentOS provisioning), the GPUI host text tests on a
Mac from `.agents/validation.md` (CoreText shaping, which a Linux machine cannot run; the rehearsal's macOS leg runs
them in step 4 with the rest of the GUI crates' and the desktop app's tests), and the macOS-only uninstall acceptance
(the rehearsal's macOS leg runs it in step 4 too, including legacy upgrade-floor refusal with the old installation
preserved and uninstall through an indirect Terminal link). The focused macOS CI job's terminal host step (the host
crate, its entry test in the CLI, and the supervisor's spawning, its client, its host terminal runtime, discovery, the
test-state cleanup and the stop command) runs only with
`gh workflow run ci.yml --ref <branch> -f suite=uninstall-macos`; its Linux side is part of the sweep's workspace
nextest. The same goes for that job's desktop supervisor record and stop tests and the stop's macOS process-exit check
(`managed_supervisor::`, `procs::tests::finished_exiting`).

The workspace nextest selection includes `scoped_podman` lifecycle coverage with a real systemd user manager and fake
Podman and conmon. Read any `SKIPPED` output as missing scope coverage. The ignored `scoped_podman_real` test runs when
explicitly selected on Linux with rootless Podman, systemd user scopes and static BusyBox. It uses its own Podman store
and requires no base image or payload.

The workspace nextest selection includes historical `containerized_sessions` compatibility and the real Linux
`podman_environments` managed-checkout and lifecycle fixture. It covers credential delivery and ordinary tab, file,
transfer and TCP-relay protocols with a stand-in agent. Its ignored SSH acceptance variant runs only when explicitly
selected on a preconfigured owned localhost SSH substrate. Historical coverage uses stored rows without creating a
container or image and keeps them readable with Delete only. Real coverage requires rootless Podman at least 4.9.3,
cgroup v2, the systemd cgroup manager, a usable user manager and engine events, plus the cached exact production Ubuntu
base `docker.io/library/ubuntu@sha256:fe81560990fae86dc4743202b2cb4d3df98d0bcf949173cecef15ce1127ef852`. The fixture
requires that image to be present before it starts. The production recipe then uses network apt and official agent
installers, and the managed checkout clones a public GitHub repository. Its focused nextest setup automatically builds
the matching static native-musl CLI payload once per selected suite, using `musl-tools`, `file` and binutils. It exposes
the staged path only to that module, which reserves four slots and has a 40-minute nextest limit. An explicit
`FARHELM_HELM_PAYLOAD_DIR` override is kept through the recorder by name. Automatic setup supplies the payload variable
by default. Tensorlake Rust packs stage matching payloads locally and pass the private staged path through the worker
recorder. Cargo is unavailable on clones. An intentional opt-out uses `FARHELM_TEST_SKIP_PODMAN=1` retained through the
recorder's `--keep-farhelm-env FARHELM_TEST_SKIP_PODMAN` option. Setup then skips the payload build. Read and report
explicit SKIPPED output as missing real-environment coverage and exclude it from runtime pass evidence.
`.agents/validation.md` gives focused selections.

A failure here is fixed or recorded per the ground rules before going on. Static fixes can be verified at once by
rerunning the check.

### Real agent drift

Run `python3 -B scripts/run-harness-drift.py --category all` on Linux as part of this explicitly requested flow, before
the sweep. That selects every drift category, `model_selection` included; the bare command runs only `base`. The runner
first upgrades every agent CLI to its vendor's latest release (`docs/harness-drift-tests.md`, "Upgrading the agents
first"). Each tracked agent then spends two real vendor turns to verify that Restart preserves the conversation. Record
PASSED, COULD NOT RUN and FAILED separately with the version each ran against and the recorder run IDs, and report any
UPGRADE FAILED line. Missing credentials leave coverage unknown for that agent; never sign in to get past them, and
never write an agent's real hook files (the cases set hooks up in a private copy of its configuration). A FAILED case is
handled like any other failure under the ground rules: fixed when the fix is straightforward, otherwise recorded in
`TODO.md` with its evidence (it is drift in a vendor's CLI, not a flake), and either way it does not hold back the sweep
or the release gate. Keep going through the rest of this flow and preserve the evidence for any failures.

## 3. Deflake sweep

Run one sweep with `deflake/AGENTS.md`'s waiting, classification and event-handling protocol, including its rule that
you take no turn until `wait` returns. Its outstanding PRs stay on the same linear stack as the fixes from step 2. This
parent flow's eager-landing policy overrides the standalone deflake flow's prohibition on marking ready or merging those
PRs. Handle and acknowledge an event in the same turn, including any landing for which the evidence is ready.

A fix for a deterministic failure needs a runtime check of its own (usually the failing test, through the recorder).
Running it in the middle of the sweep competes with the sweep for the machine; prefer to run it once the sweep is past
its timing-sensitive phases (the nextest phases come first) or after it finishes, and do not count the sweep's own
pre-fix result as evidence for the fix. A fix waiting for that required check is not ready to land; once the check
passes, land it without waiting for unrelated work.

## 4. Release gate and CI in GitHub

The Linux release gate includes the full farhelm integration selection as well as the other workspace targets. Its setup
prepares rootless Podman, starts the runner's systemd user manager, exports its bus environment and verifies a transient
user scope before caching the exact Ubuntu digest. Loopback SSH, the distribution's tmux, musl build tools and the GPUI
app's build packages are also ready before the workspace selection, and the desktop app's tests run after it. The GPUI
end-to-end smoke follows the desktop tests on x86_64 Linux. It builds the app with the automation bridge and runs under
headless Mutter with software rendering, recording evidence alongside the other release tests. The focused nextest setup
builds the same-source installer payload before cargo-dist has produced a release artifact. The real recipe's network
bootstrap is part of the gate.

Only once the static checks pass and the sweep has finished with every failure fixed or recorded. On top of the stack
(or of `main` when all fixes have already landed):

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
   into the stack, then let the rehearsal rebase on top), validate and land it under the same readiness rule, and push
   the rehearsal bookmark again to get a fresh verdict.
7. When both runs are green, delete the rehearsal branch on GitHub
   (`gh api -X DELETE repos/scode/farhelm/git/refs/heads/<bookmark>`), its local bookmark, and its commit
   (`jj abandon`). The runs and their logs stay on GitHub.

## 5. Report

Report, in product terms per `AGENTS.md`, "Talking to the user": the checks run in step 2 and their results, including
the live agent verdicts and recorder run IDs; the sweep's run id and its events; each flake recorded; the fix stack
bottom-up with each PR's link, landing status and the failure it fixes; anything recorded for the maintainer's judgment
instead of fixed; the GitHub run URLs and their verdicts; what was skipped and why; and the working log's path.

## Keeping this file honest

The step 2 list is defined against two other lists: the check inventory in `.agents/validation.md` and the phases in
`deflake/bin/deflake`. When either changes, update the list here in the same change, so nothing falls between them and
nothing runs twice.
