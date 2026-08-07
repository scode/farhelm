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
  skip themselves where no systemd user manager exists, and libtest hides a passing test's output otherwise.
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
- `cd e2e && npx playwright test` — the browser end-to-end suite, run against both Chromium and WebKit (the latter
  stands in for the desktop app's actual engine family). It needs `cargo build` and
  `cd crates/farhelm-ui && dx build --platform web --release` first (it drives the built web UI against a real helm and
  supervisor), plus a one-time `cd e2e && npm install && npx playwright install chromium webkit`.

These commands mirror `.github/workflows/ci.yml`; if CI changes, update this list in the same change (and vice versa).

# lore/

`lore/` holds historical artifacts — decision records written when the decision was made. It is not part of the codebase
and is never updated to track code changes. See `lore/AGENTS.md` for its rules before touching anything in it.
