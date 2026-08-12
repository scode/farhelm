# Farhelm agent instructions

Intent and preferences only — for what the system is and how it works, read SPEC.md (product behavior), SPEC_impl.md
(implementation choices and their motivations), and PLAN.md / PLAN_M*.md (build order). Those documents are
authoritative; do not restate them here and do not change behavior they specify without surfacing the conflict.

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
- `cargo test -- --show-output` — `--show-output` is what makes a loudly-skipped test's reason visible; the cgroup tests
  skip themselves where no systemd user manager exists, and libtest hides a passing test's output otherwise. CI adds
  `--test-threads=4` to match its 4-vCPU runners and keep the process-heavy tmux integration suite's aggregate load
  predictable; a beefier local machine does not need the cap.
- `scripts/test-tmux-3.7b-shutdown.sh` — builds the exact checksummed 3.7b release on cache miss, then runs every
  focused output-client teardown regression in its own test process. CI uses the same script separately from the distro
  tmux used by the full suite because silently substituting another version loses the affected-version coverage.
- `cd crates/farhelm-ui/js-tests && node --test` — the JS unit harness for the asset-JS layer's pure functions
  (PLAN_M6_5.md item 1); node is already a CI requirement for Playwright below, so this adds no dependency. Run from
  inside the directory rather than as a glob from the repo root: node's no-argument default discovery (every `*.test.js`
  in cwd) is the oldest, most version-portable form the test runner has, whereas quoted-glob CLI arguments are newer and
  CI pins no node version.
- `cargo check -p farhelm-ui --features desktop` — the desktop renderer compiles nowhere else; needs the webkit2gtk/gtk
  dev packages (see the CI job for the apt list).
- `scripts/desktop-smoke.sh` — the non-pixel Xvfb integration gate for the embedded helm, managed supervisor, desktop
  authentication, bundle-local tmux, hard-exit tether, and restart persistence. The optional coordinate-driven leg is
  not part of CI.
- `dprint check`

These commands mirror `.github/workflows/ci.yml`; if CI changes, update this list in the same change (and vice versa).

The browser end-to-end suite is deliberately NOT in that per-change list, and its CI job is disabled (`if: false` in
ci.yml): it is far too slow to pay on every PR. It gates MERGING instead — before landing a PR stack on main, run
`cd e2e && npx playwright test` (Chromium and WebKit; WebKit stands in for the desktop app's actual engine family). It
needs `cargo build` and `cd crates/farhelm-ui && dx build --platform web --release` first (it drives the built web UI
against a real helm and supervisor), plus a one-time `cd e2e && npm install && npx playwright install chromium webkit`.
This split lets changes accumulate across a stack and surface bugs once, before merge, without each PR paying the
suite's cost — but it also means NOTHING else runs it: CI green does not include e2e, so skipping it at merge time means
shipping unexercised browser paths.

# Desktop/web UI bug triage

`docs/desktop-web-triage.md` is the recipe: which engine comparison localizes a UI bug, where the unified log lives, and
what a bridge-death line looks like. Start there before investigating any "the desktop UI is broken" report.

# lore/

`lore/` holds historical artifacts — decision records written when the decision was made. It is not part of the codebase
and is never updated to track code changes. See `lore/AGENTS.md` for its rules before touching anything in it.
