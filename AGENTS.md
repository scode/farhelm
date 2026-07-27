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
- `cargo test`
- `dprint check`

These commands mirror `.github/workflows/ci.yml`; if CI changes, update this list in the same change (and vice versa).

# lore/

`lore/` holds historical artifacts — decision records written when the decision was made. It is not part of the codebase
and is never updated to track code changes. See `lore/AGENTS.md` for its rules before touching anything in it.
