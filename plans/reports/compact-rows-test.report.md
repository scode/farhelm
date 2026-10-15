### What this was about

Two browser tests still expected the session sidebar's permission descriptions from before guarded permission modes.
They failed even though the sidebar followed the specification. The tests now expect the agent name, the actual
permission mode and the stored start command, including the grey mark for an agent using its own permission setting.
Their existing ended-state and compact-layout cases remain covered. No product behavior changed.

### Things you should know

The permission mark, agent mark and screen-reader text are each checked against the same description. Agent-launch
checks pin the specific mode and stored start command without copying the long shared caveat verbatim. The OMP fixture
keeps its different invocation, so it detects using that invocation in place of the stored start command. A sweep found
no other browser assertions with the same stale-wording problem.

### Open questions and possible follow-ups

None identified within this test-only scope.

### PRs

- [#2031](https://github.com/scode/farhelm/pull/2031/changes): refresh sidebar permission expectations; draft, based on
  main.

### Checks run, reused and skipped

- Built the CLI and fixtures with `cargo build`, and the web UI with
  `dx build --package farhelm-ui --platform web --release`.
- Recorded the compact-row and OMP-row cases on Chromium and WebKit: 4/4 passed, one worker, zero retries. Run
  `4df7951a-ece5-46a0-8f28-787c77d56595`, using the pinned tmux. The local stack started and cleaned up successfully;
  these fabricated-row cases do not exercise a remote SSH host or a vendor agent.
- `python -B scripts/check-test-sleeps.py`: 299 delays inspected, zero unannotated. `dprint check TODO.md` and the diff
  whitespace check passed.
- No earlier runtime results were reused. Only queue claims changed upstream during execution; their complete diff was
  inspected and the PR was rebased without changing its tested behavior.
- Rust tests, Clippy, desktop compilation and the full browser suite were skipped: only two browser tests and the TODO
  entry changed. The two affected cases on both engines cover the changed assertions.

### Review gate

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idiomatic code and test
quality with the full test-authoring contract. No findings. The commit and PR wording passed their separate cold read.
The documentation pass covered both touched test files. The PR changes no files under `plans/`, stays a draft and has
not been merged by the executor.
