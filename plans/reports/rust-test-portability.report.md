### What this was about

Several Rust tests could fail for reasons unrelated to Farhelm: macOS resolves temporary paths through symlinks and
rejects an invalid UTF-8 directory name; inherited Git configuration could redirect fixture writes; and two terminal
cleanup tests could finish before the cleanup they were supposed to check. Separately, cleanup of abandoned test runs
could hang while connecting to a private tmux socket whose listen queue was full.

The maintainer delegated six small fixes. Each is a separate draft PR. Fixture expectations now use canonical paths, the
impossible filename fixture is excluded on macOS, fixture Git children discard the inherited config-file selector, and
the terminal tests wait for the existing close-completion signal. Abandoned-run cleanup now probes sockets without
blocking and retains the directory when its existing bounded kill cannot confirm that the server stopped.

### Things you should know

All six fixes stayed within their complexity gates. The first five change tests or their fixtures only. The last changes
the internal test-state cleanup crate, which is not shipped in the product. No product behavior, schema, cleanup policy,
or cleanup budget changed, and no changelog fragment is required.

Native macOS validation was unavailable. The path fixes and platform exclusion were checked by source review; Linux ran
the affected tests. The new full-backlog regression is Linux-only and establishes the full-queue premise before running
the sweep. It also closes its listener before joining the worker on a timeout, so a blocking-connect regression can fail
without stranding that worker.

One extra hostile-environment experiment intentionally inherited an owned `GIT_CONFIG` file. The file remained
unchanged, but three discovery assertions failed because Git rejects the production scanner's explicit `--local` read
when that selector is inherited. Production discovery was deliberately left unchanged, as the plan directs; the normal
discovery module passed all 15 tests. The failed experiment remains retained as evidence, not counted as a passing run.

### Open questions and possible follow-ups

Production discovery could separately discard bare `GIT_CONFIG` to tolerate that inherited environment. This plan fixes
fixture-write isolation only; it does not claim production discovery succeeds under the hostile experiment. Nothing in
this follow-up blocks the six agreed outcomes.

### The PRs

- [#2093](https://github.com/scode/farhelm/pull/2093/changes): canonical trust fixture paths.
- [#2094](https://github.com/scode/farhelm/pull/2094/changes): canonical file-stat expectation.
- [#2097](https://github.com/scode/farhelm/pull/2097/changes): exclude the impossible token fixture on macOS.
- [#2099](https://github.com/scode/farhelm/pull/2099/changes): isolate fixture Git configuration.
- [#2101](https://github.com/scode/farhelm/pull/2101/changes): wait for terminal close completion.
- [#2103](https://github.com/scode/farhelm/pull/2103/changes): prevent full socket backlogs from hanging fixture
  cleanup.

All six are drafts in one linear stack, in that order. Each includes its own completed triage record and feedback
removal.

### Checks run, reused and skipped

All runtime selections used recorded nextest runs with the pinned runner and tmux, four global slots and zero retries.
They passed 25 selected cases in total, with complete JUnit reports and no runtime early-return skips. Other cases were
intentionally selected out.

- `cargo nextest run -p farhelm-supervisor --lib` with the two named trust tests: 2 passed, run
  `af1d4b25-60d2-4345-9bff-1ae80a195cfc`.
- The same package/target with the named file-stat test: 1 passed, run `cd453cc0-192e-4bb1-9eca-5db69cf1ed8e`.
- `cargo nextest run -p farhelm-helm --lib` with the non-UTF-8 rotation test: 1 passed, run
  `15b0db9b-1265-43ae-a823-d0bb662864fa`.
- The supervisor library's `repository_discovery::tests::` selection: 15 passed, run
  `823edc14-5284-4d6d-807d-f0a854f3852b`. The additional inherited-selector experiment was 12 passed and 3 failed, run
  `3f79d3c4-cd66-4f62-af14-3eac78dd90e7`, with the limitation described above.
- The supervisor library's two changed terminal-reap tests: 2 passed, run `ce67d3ec-a354-4496-bdf7-ef64f7422329`.
- `cargo nextest run -p farhelm-teststate --lib` selecting dead-socket removal, unresponsive-listener retention, real
  tmux shutdown, and full-backlog retention: 4 passed, run `8d87a585-4887-46c4-a0b2-23d97100ab3e`.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, changed-Markdown `dprint check`, and the
  isolated `python -B scripts/check-test-sleeps.py`: passed. The checker inspected 301 delays, none missing a reason.
- Reused those checks after PR-link amendments and the careful rebase from `405175b4` onto `c6833c3a`. All six
  per-change interdiffs were empty. Incoming executable changes only affected installation-identity display, which these
  tests and the socket probe do not call; the remaining incoming changes were queue/report bookkeeping.
- Skipped broader Rust, browser, desktop, installer and release runs: the focused tests cover the affected fixture and
  socket boundaries, and the stack adds no product integration behavior. No macOS execution was claimed.

### The review gate's outcome

Each PR passed its required fresh-context external gpt-6.1-sol reviewer at high effort. All six complete findings
artifacts were read and reported no issues. Each changed-test prompt included the full test-authoring checklist. The
executor implemented and validated the work locally; the reviewers performed static inspection only.

Separate wording cold reads passed, with two titles/bodies clarified after the first reader. A documentation pass
covered every touched source file. No unresolved source finding, scope departure, or complexity-gate failure remains.
