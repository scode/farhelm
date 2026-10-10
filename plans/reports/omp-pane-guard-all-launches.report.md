## What this was about

For OMP sessions launched through a custom Bun or npm command, Farhelm could offer a nested conversation as the
session's Resume target when it could not read the terminal pane's command arguments (for example, because they exceeded
the argument-read budget). The ownership check already refused that shape for the installed `omp` command, but custom
launches had a pane exemption. During triage the maintainer chose a small code fix, with a complexity gate and one
gpt-6.1-sol high reviewer.

## Things you should know

The same launcher evidence now applies at an interpreted terminal pane for every supported OMP launch type. Missing
arguments refuse attribution, and exact supported Bun/npm package launchers remain accepted. Installed `omp` launches
keep their existing behavior. The change stayed within the agreed shape: about ten runtime lines, three unit tests, and
the implementation specification correction, plus required bookkeeping and changelog.

Version-pinned package selections or unsupported launcher flags at the terminal pane can now lose Resume. They already
refused elsewhere in the process chain; the pane now follows that same rule. This affects Resume attribution, not
whether the command can run.

The original live trigger remains unverified: no live OMP reproduction established that a nested interactive child can
produce this shape. The tests establish the supplied process-chain boundary directly, including unreadable Bun and Node
panes, an accepted npm launcher at the pane, and refused version-pinned launchers. The existing Bun positive control
remains.

## Open questions and possible follow-ups

None required to complete this plan. A live reproduction would provide additional evidence about the rare trigger, but
was outside the small fix agreed during triage.

## PRs

- [PR #1779](https://github.com/scode/farhelm/pull/1779/changes): uniform OMP pane ownership check; one commit on
  bookmark `plan/omp-pane-guard-all-launches/01-omp-pane-guard`, change `uouxtvtp`. Delivered as a draft; the executor
  did not mark it ready or merge it.

## Checks run, reused and skipped

- Ran the focused supervisor process tests through the recorder:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(procs::tests::)'`, four slots, zero retries, tmux mode `none`.
  All 79 selected tests passed; run `54bf35c0-2989-43e4-968f-1c670af5d510`. This covers the synthetic ownership boundary
  and existing launch compatibility controls.
- Ran `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`, changed-Markdown
  `dprint check`, and `python3 releasing/check-changelog.py format`; all passed. Ran the isolated test-delay checker:
  276 files inspected, zero missing rationales.
- Reused the process-test and Clippy results after the review correction, which changed comments only, and the final
  ledger URL amendment. Formatting and Markdown checks were repeated for the affected text. A fresh fetch found no main
  changes since the claim base `c39871e9`, so no rebase was needed.
- Skipped broad Rust, browser, desktop, installer and live vendor tests: the runtime change is a pure process-chain
  predicate, with focused positive and refusal coverage; it changes no UI, transport, reporter asset or lifecycle
  mechanism.

## Review gate outcome

The prescribed fresh-context gpt-6.1-sol high reviewer found no correctness or design issue, verified unchanged
installed-`omp` behavior and the complexity bound, and identified one stale helper comment describing the removed
exemption. That comment was corrected and a separate documentation pass completed. The wording cold reader recovered the
motivation and custom-launch caveat without contradictions. Native actual-model identity and usage counters were not
exposed; the requested selections and review artifacts are retained privately.
