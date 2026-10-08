# New button shortcut hint

## What this was about

The Mac desktop app already opens the session launcher with Cmd+N, but the New button did not tell users about it. The
requested change was to show the shortcut in that button’s hover text in the Mac desktop app only.

## Things you should know

Hovering New in the Mac desktop app now reads `new session: start an agent or a command on any host (⌘N)`. The web UI
and Linux desktop app retain the original text. The button’s accessible name remains `new session`, and its launch,
focus and busy behavior is unchanged.

The hint and shortcut installation share one build-time condition, so the hint cannot appear in a build that does not
install the shortcut. The specification records this behavior, the manual Mac checklist checks both the desktop hint and
its absence in a Mac browser, and the completed TODO entry is removed.

## Open questions and possible follow-ups

No product decisions remain. Native Mac compilation and the visual checklist were not run on this Linux executor; the
Linux desktop check cannot establish their result. The updated manual checklist is the remaining native Mac appearance
check.

## The PRs

- [#1714: show ⌘N in the Mac desktop New button hint](https://github.com/scode/farhelm/pull/1714/changes) — one draft
  PR, not marked ready or merged by the executor.

## Checks run, reused and skipped

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`: passed, with the pinned nextest and tmux setup on PATH.
- `cargo check -p farhelm-ui --features desktop`: passed on Linux; covers the desktop build and its unchanged non-Mac
  hint.
- `dprint check SPEC.md TODO.md docs/manual-mac-checklist.md`: passed after formatting the checklist’s line wrapping.
- `python3 releasing/check-changelog.py format`: passed.
- These checks and review covered source revision `1458d2c3` and are reused for pushed commit
  `3fd5183e0dedf890fad0e45337ac58ee0cfebe03`. Rebase onto newer main left the product diff identical.
- Runtime Rust, JavaScript and browser tests skipped: this changes compile-time hover text, and existing tooltip
  coverage checks presence rather than platform wording. The plan explicitly excludes a test that merely compares
  literal strings.
- Native Mac build and manual visual checks skipped because this executor runs Linux. Test-delay source check skipped
  because no tests, test helpers or fixtures changed. Broader validation adds no useful coverage for this change.

## Review gate’s outcome

The required fresh-context `gpt-6.1-sol` high-effort code reviewer found no issues. The executor read its artifact and
checked the shared build condition, exact tooltip strings and unchanged accessible name against the source. The
independent commit-wording cold read also passed. The executor implemented the change locally.
