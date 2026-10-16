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

### Landing

Landed on its own in round 67, with no product plan alongside it. All six PRs merged bottom-up. They are unchanged from
what the report describes, apart from one bookkeeping conflict resolved during the rebase (below).

#### What landed since the stack was based, and whether it interacted

Forty-four changes reached main between this stack's base and its landing: round 65's and round 66's product fixes, a
batch of browser-test permission and flake updates, and the agent-instruction split. I read their diffs against this
stack, and a fresh-context reviewer did the same independently. Neither review found an interaction:

- The Claude trust edits (the "preserve Claude settings numbers" fix) changed the same source file as the first PR
  (#2093, canonical trust fixture paths) and added a test to it. That new test already builds its fixture on the
  canonical temporary path, the same premise this plan applies to the older tests. The two edits merged cleanly. The
  rule for resolving a trust path did not change.
- The other code changes on main (download naming, session status after a stop, checkout error wording, terminal link
  handling, remote uninstall, update failure reporting) touch none of the functions these tests exercise. Nothing
  changed the terminal close and reap path, the downloads file-inspection path, device-token rotation, repository
  discovery, or the test fixture cleanup crate.
- Nothing on main amends what SPEC.md or SPEC_impl.md say about these areas.

#### Fixes made during the landing

The only one was the review-feedback index. The terminal-reap PR (#2101) removes its own feedback entry from that index.
Meanwhile, the docs-preview fix that landed in round 65 had removed the adjacent entry. The resolution keeps both
removals, so the index again lists exactly the feedback files in the queue. No source file changed.

#### Checks

- Run now, at the rebased stack tip: `cargo fmt --all -- --check` and
  `cargo clippy -p farhelm-supervisor -p farhelm-helm -p farhelm-teststate --all-targets -- -D warnings` (the three
  packages the stack changes), both clean.
- Run now: `dprint check` on the resolved feedback index, clean.
- Run now: the Claude trust module's tests and the launch-time trust test, since the trust source file is the one place
  the rebase merged this stack with a change from main. The run went through the test recorder with the pinned nextest
  and tmux, four slots and no retries:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(agent_kind::claude::trust::) | test(launch::tests::claude_trust_launch_is_opt_in_and_best_effort_after_ready)'`.
  It passed 7 of 7 with no runtime skips, run `0d9e0ed2-a6ae-4ed8-b0ef-a2867ea334bf`. That includes main's new
  number-preservation test next to this plan's canonical-path test.
- Reused: the executor's recorded runs for the other five fixes, listed in the report, and its workspace-wide clippy and
  test-sleep checks. Each PR's own diff is byte-identical after the rebase, apart from the index resolution above, and
  nothing on main touches what those tests depend on. The test-sleep check is source-only, and the merged trust file
  adds no delay from either side.
- Skipped: broader Rust, browser, desktop and installer runs, since nothing here changes shipped behavior. No macOS run
  was made, as the report says.

Nothing in the executor's report became untrue at landing.
