# Farhelm agent instructions

Intent and preferences only — for what the system is and how it works, read SPEC.md (product behavior) and SPEC_impl.md
(implementation choices and their motivations). Those documents are authoritative; do not restate them here and do not
change behavior they specify without surfacing the conflict. The historical build-order docs (PLAN.md / PLAN_M*.md,
cited by name throughout code comments) are archived under lore/ and are not maintained.

This project is either public now, or may become public in the future. No content in this project should contain
personal information such as personal usernames, hostnames, details about the local environments, etc.

# Conventional Commits

All commit messages and PR titles must use Conventional Commit format: `<type>: <short summary>`

Allowed types: `feat`, `fix`, `docs`, `perf`, `refactor`, `style`, `test`, `chore`, `ci`, `revert`.

Append `!` after the type for breaking changes (e.g. `feat!: remove legacy
endpoint`). Scope is optional.

Rules:

- Type reflects the user-visible effect, not the implementation activity. A bug fix that requires heavy refactoring is
  `fix`, not `refactor`. A new CLI flag is `feat`, not `chore`.
- The summary after the colon is lowercase, imperative mood, no trailing period.
- Keep the first line under 72 characters.

# Finishing work

Before creating or updating a PR, or claiming work is done, run exactly what CI runs and make it pass:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" cargo test -- --show-output` — the suite drives the pinned tmux from
  `.github/release/source-pins.env`, which the build script makes available under `.ci-tmux/` (one-time build, then
  cached); a run against whatever tmux is on your PATH exercises a different substrate than CI does. `--show-output` is
  what makes a loudly-skipped test's reason visible; the cgroup tests skip themselves where no systemd user manager
  exists, and libtest hides a passing test's output otherwise. CI adds `--test-threads=4` to match its 4-vCPU runners
  and keep the process-heavy tmux integration suite's aggregate load predictable; a beefier local machine does not need
  the cap.
- `scripts/test-tmux-pinned-shutdown.sh` — builds the exact checksummed release named by
  `.github/release/source-pins.env` on cache miss, then runs every focused output-client teardown regression in its own
  test process. CI uses the same script separately from the distro tmux used by the full suite because silently
  substituting another version loses the affected-version coverage.
- `cd crates/farhelm-ui/js-tests && node --test` — the JS unit harness for the asset-JS layer's pure functions
  (PLAN_M6_5.md item 1); node is already a CI requirement for Playwright below, so this adds no dependency. Run from
  inside the directory rather than as a glob from the repo root: node's no-argument default discovery (every `*.test.js`
  in cwd) is the oldest, most version-portable form the test runner has, whereas quoted-glob CLI arguments are newer and
  CI pins no node version.
- `cargo check -p farhelm-ui --features desktop` — the desktop renderer compiles nowhere else; needs the webkit2gtk/gtk
  dev packages (see the CI job for the apt list).
- `cargo test -p farhelm-ui --features desktop` — exercises the desktop-only persistence and IPC seams; needs the same
  webkit2gtk/gtk dev packages as the desktop compile check.
- `cargo check -p farhelm-desktop` — the shipped desktop binary sits outside `default-members` (so ordinary builds never
  compile WebKit), which means `-p` is the only thing that ever compiles it.
- `scripts/check-desktop-assets.sh` — holds the desktop build's `asset!()` set and the web bundle's files to the same
  set, in both directions. Needs `dx` and the wasm32 target; takes a few minutes, since it wipes `target/dx` and
  rebuilds both bundles.
- `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH" scripts/desktop-smoke.sh` — the non-pixel Xvfb integration gate for
  the embedded helm, managed supervisor, desktop authentication, the tmux override reaching the managed supervisor,
  hard-exit tether, restart persistence, and that every asset the window requested went through the desktop asset
  handler. The optional coordinate-driven leg is not part of CI.
- `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh` — the curl-installer is POSIX
  `sh` run unread on a fresh machine (`curl | sh`), so a syntax slip or a shellcheck-catchable bug in it gets caught
  before the slower Rust-side asset-table parity test in `assets.rs` would notice. `test-install-sh.sh` is bash, not
  POSIX `sh` (it is never piped into a stranger's shell, so nothing forces the same portability constraint), which is
  why only `install.sh` goes through `sh -n`.
- `bash scripts/test-install-sh.sh` — drives `install.sh` as a real child process against a fixture HTTP server: fresh
  install, update, a forced-failure rollback (macOS-shaped, via a `uname` shim), 404, checksum mismatch, two
  malformed-archive shapes, version normalization (including a `-rc.N` prerelease), invalid versions, missing
  prerequisites (via an isolated `PATH`), the exact closing-message contract across five tmux fixtures, and that nothing
  outside `FARHELM_INSTALL_DIR` — no `systemctl`/`launchctl` call, nothing else under `$HOME` — ever changes. Every
  invocation goes through `env -i` with an explicit environment, never this process's own.
- `dprint check`

These commands mirror `.github/workflows/ci.yml`; if CI changes, update this list in the same change (and vice versa).

The RELEASE workflow is not on that list and has nothing to run locally. `.github/workflows/release.yml` is generated by
cargo-dist from `dist-workspace.toml` — never hand-edit it; change the config and run `dist init --yes && dprint fmt` to
regenerate. It runs on tag pushes only (`pr-run-mode = "skip"`, D19), so a PR exercises none of it: the release path is
validated when a tag is cut, by the gate `.github/dist-build-setup.yml` puts at the top of every build job — this
repository's full test suite on the x86_64 Linux one, an Apple compile on the macOS one. That gate lives inside the
build jobs rather than in a `plan-jobs` workflow because dist 0.32 lets a failed plan job SKIP the build jobs, and its
`host` job accepts a skip; a failure inside a build job is the only kind it refuses. What a change to release plumbing
CAN be checked locally is `dist plan` (the config parses and the asset list is what you expect), the release scripts'
own `--self-test` modes (`scripts/check-release-archive.py`, `scripts/check-static-elf.sh`,
`scripts/check-desktop-assets.sh`), and `shellcheck` over the scripts the workflow calls.

When a tag produces a public release that never got its `SHA256SUMS`, the recovery procedure is in
`dist-workspace.toml`'s header ("RECOVERY: a release that exists but was never signed"). It is maintainer-run: delete
the release, never the tag, then re-run the workflow.

The browser end-to-end suite is deliberately NOT in that per-change list, and its CI job is disabled (`if: false` in
ci.yml): it is far too slow to pay on every PR. It gates MERGING instead — before landing a PR stack on main, run
`cd e2e && npx playwright test` (Chromium and WebKit; WebKit stands in for the desktop app's actual engine family). It
needs `cargo build` and `cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release` first (it
drives the built web UI against a real helm and supervisor), plus a one-time
`cd e2e && npm install && npx playwright install chromium webkit`. This split lets changes accumulate across a stack and
surface bugs once, before merge, without each PR paying the suite's cost — but it also means NOTHING else runs it: CI
green does not include e2e, so skipping it at merge time means shipping unexercised browser paths.

# TODO.md

`TODO.md` is the maintainer's running list of wanted fixes and features. When a PR addresses an entry, remove that entry
in the same PR — the file only ever describes what is still wanted. Do not add entries on your own initiative; they are
the maintainer's.

# Desktop/web UI bug triage

`docs/desktop-web-triage.md` is the recipe: which engine comparison localizes a UI bug, where the unified log lives, and
what a bridge-death line looks like. Start there before investigating any "the desktop UI is broken" report.

# The live install is off-limits

This machine runs the maintainer's production Farhelm: the released `farhelm` binary at `~/.local/bin/farhelm` (put
there by `scripts/install.sh`) with its state under `~/.local/state/farhelm/`, the `farhelm-helm.service` and
`farhelm-supervisor.service` units that `farhelm helm setup` wrote to run it (plus the machine-local tmux pin under
`~/.local/lib/farhelm-local/` and the `*.service.d/` drop-ins that wire it in), and a separate dev-loop deployment under
`/home/scode/farhelm-live/` (its own source, binaries, and state) behind `farhelm-live-supervisor.service`. Take no
state-changing action against any of that: no running `install.sh` or `farhelm helm setup` here, no rebuilding,
redeploying, or modifying files in those trees, no start/stop/restart/reload/enable/disable/mask/kill of those units,
and no edits to their unit definitions or drop-ins — the verbs are examples, not an exhaustive list, and "it is not
literally named above" is not a loophole. Not to verify a fix, not as a finishing step, and not because the README or
release docs describe how: install documentation is addressed to the human operator, and reading it is not authorization
to run it. `farhelm helm setup --dry-run` is read-only and allowed. The only exception is the user explicitly asking, in
the current session, for a specific one of these things to be done.

This rule is a fence with a history: twice (2026-08-12 and 2026-08-15), sessions asked only to merge PR stacks went on
to rebuild and restart the live helm, the second time deploying a build that locked the operator's browser out the next
morning. Nothing in normal development needs the live install — the test suites create their own supervisors and helms
from temporary state directories, and stopping a transient `farhelm-<uuid>-*.scope` unit that belongs to such a test is
fine.

# lore/

`lore/` holds historical artifacts — decision records written when the decision was made. It is not part of the codebase
and is never updated to track code changes. See `lore/AGENTS.md` for its rules before touching anything in it.
