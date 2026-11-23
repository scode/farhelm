## What this was about

With automatic Claude Code folder trust on, starting a session in a parent of your home directory could grant Claude
trust for your entire home. Claude extends trust to everything inside a trusted directory, and those entries can persist
after the switch is off. An invalid home directory could also let Farhelm edit Claude's settings without being able to
check whether the launch directory covered your home.

The maintainer chose to leave Claude's own trust prompt in charge at home and above it, including on disposable hosts.
Farhelm now skips automatic trust for the home directory and every parent, including the filesystem root. It also skips
the edit when the launch shell supplies a home directory that is missing, relative or cannot be resolved. Farhelm
continues the launch and gives the reason in the terminal. Claude can then ask for trust. Settings and the Claude Code
and Start a session docs explain the directory rule.

## Things you should know

The guard applies both to directories you choose and to new managed checkouts, including disposable environments.
Previously recorded trust entries stay in Claude's settings. The change governs future trust edits.

## Open questions and possible follow-ups

None.

## PRs

- [#4664](https://github.com/scode/farhelm/pull/4664/changes) — Skip automatic Claude trust at and above home and
  explain the rule in Settings and the docs.

## Checks

- `cargo nextest run -p farhelm-supervisor --lib`, launch, Claude trust and retirement selection, passed all 107 tests
  on the draft (run `8ce6992e-df63-4189-86d6-eb9ff055d9c4`). The initial run failed one new fixture when it reused a
  launch file that Farhelm consumes on first use (run `d58f54e4-d585-462b-9b4a-9a9c3a5f0aab`). With a fresh file for
  each case, the exact regression passed (run `eaa19606-b1f6-4180-8ef1-4435ef841370`). Farhelm launched a test stand-in
  for Claude with four unusable home settings, explained each refusal and left the Claude settings file intact.
- `cargo nextest run -p farhelm-gui --lib`, Settings search and agent trust selection, passed 13 tests (run
  `7e5a5594-ce83-4704-827d-a3387c004021`).
- `python3 scripts/check-gui-goldens.py --update --scene settings-claude` showed two changed Linux references (run
  `b892e666-cde1-4cb0-9ebe-560d1a83ce93`). The full `--update` of 406 pictures found the same two changes (run
  `7e6b5871-6d56-4529-bce7-d19fcfbe492b`). I reviewed and published both themes.
- `cargo clippy --all-targets -- -D warnings` passed on the exact draft. `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check` on changed Markdown and
  `python3 releasing/check-changelog.py format` passed. The website's `bun run build` built 39 pages and checked its
  links.
- Settings tests and the focused render covered snapshot `9f96bb2b`, the full render covered `b0b7203e`, and the website
  build covered `df44cbb8`. Their GUI and docs sources are identical in the draft apart from the two published
  screenshot references. The rebase picked up queue and TODO additions. Linux fixture tests cover the affected behavior,
  so broader runtime suites and live Claude runs were skipped. The Claude Settings reference set is captured on Linux,
  so a macOS capture was unnecessary.

## Review gate

The independent source review of the first candidate found one compile error in the new test and no production findings.
The fixture corrections were verified by the exact regression. The commit wording passed its separate cold read.
