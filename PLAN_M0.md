# Farhelm M0: bootstrap

NOTE: This is the plan for milestone 0 only — pure project bootstrap, no
product functionality. The overall motivation and milestone ladder live in
PLAN.md. M0 exists so that M1 starts from a repo where the conventions,
checks, and tooling are already standing, instead of retrofitting them
around growing code.

## Goal

A buildable, CI-green Rust workspace with the project's standing
conventions applied from the first line of code: the scode-modernize
baseline (CI job layout, runners, dprint, agent instructions, conventional
commits, stacked-PR guard) plus the workspace skeleton SPEC_impl.md names.

## Scope

### In

- **Cargo workspace** with the five crates from SPEC_impl.md
  (`farhelm`, `farhelm-supervisor`, `farhelm-helm`, `farhelm-proto`,
  `farhelm-ui`) as compilable stubs — each with at least one trivial test
  so the CI jobs exercise something real. No product code; M1 fills them.
- **CI** (`.github/workflows/ci.yml`): separate `fmt`, `clippy`, and
  `test` jobs — separate so failures report in parallel — on `runs-on:
  ubicloud`, official actions only (`actions/checkout`, `actions/cache`,
  `actions-rust-lang/setup-rust-toolchain`), cargo caching on every job
  that compiles.
- **dprint** for JSON/TOML/Markdown: `dprint.json` with plugins added via
  `dprint config add` + `dprint config update` (never hand-pinned URLs), a
  `dprint` CI job (`dprint/check`), and a one-time `dprint fmt` over the
  existing docs — the specs and plans will reflow to the configured line
  width in this milestone, deliberately.
- **AGENTS.md** as the canonical agent instruction file, with `CLAUDE.md`
  as a symlink to it. Contents: intent and preferences only (no
  architecture overview — the specs own that), the Conventional Commits
  section per the modernize template, and a finish-work section whose
  commands match the CI invocations exactly, flags included.
- **lore/ directory**: a home for historical artifacts — records of
  notable decisions and their motivations, kept as they were written.
  It gets its own `AGENTS.md` (with a `CLAUDE.md` symlink) stating the
  rules: contents are historical, not part of the codebase, never
  "maintained" as the code changes, and only changed on explicit user
  request. Entries are named `YYYY-MM-DD-foo-bar-baz.md`.
- **Conventional Commits**: commit messages and PR titles use
  `<type>: <summary>`. The pre-M0 doc PRs were retitled to conform, so the
  convention holds across the entire history.
- **Stacked-PR base guard** (`.github/workflows/pr-base.yml`): a
  `require-main-base` job failing any PR whose base is not `main`, so a
  stacked child PR can't be merged into its parent by accident. Marking
  the check required in branch protection is a manual GitHub step and part
  of M0's definition of done.

### Out (deliberately)

Release automation — cargo-dist, git-cliff, changelog, Homebrew publishing
(that is scode-dist-rust-setup territory, adopted when there is something
to release); cross-compilation CI (arrives with the artifacts that need
it); Playwright CI (lands with the harness in M1); any product code.

## Acceptance

1. `cargo build` and `cargo test` succeed on the five-crate workspace.
2. CI is green with `fmt`, `clippy`, `test`, `dprint`, and
   `require-main-base` as separate jobs, all on Ubicloud runners.
3. `dprint check` passes on the repo, including the reflowed docs.
4. `AGENTS.md` exists as a regular file, `CLAUDE.md` is a symlink to it,
   and the finish-work commands in it match CI's invocations exactly.
5. `require-main-base` is a required check in branch protection, verified
   by observing it fail on a stacked PR.
6. `lore/` exists with its `AGENTS.md` rules file and `CLAUDE.md` symlink.
