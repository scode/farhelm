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

### Landing

Landed alone in its round as #2031. While it was landing, two other plans delivered their reports
(settings-lockout-scope and goose-named-restart-test); they belong to the next round. Nothing that reached main touched
these tests.

#### Fix made while landing

Running both changed spec files in full, not only the changed cases, found a second stale test in
`e2e/tests/omp-composer.spec.ts`: "the OMP composer offers OMP's own vocabulary and the row shows the effective
permission". It expected the OMP launcher to offer exactly two permission buttons. Since the guarded permission mode
(#1992) and custom flags (#1995), OMP offers guarded, YOLO, custom and Approve, and the section also holds the help
button. It had failed on main on both engines since then. It now checks each of the four choices by name, that YOLO
starts selected, and that Goose-only modes stay absent.

#### Review

An independent fresh-context review found nothing blocking. It checked every new expected string against what the
product renders, and confirmed the stricter exact assertions still tell apart every state, YOLO against not-YOLO, own
setting, Approve and unclassified, and command against agent launches. Notes left as they are:

- The two anchored patterns allow any text between the heading and the shared caveat, so they would also accept an
  unexpected extra paragraph.
- The new OMP test docstring says the screen-reader text is checked for the older no-permission row, but only its mark
  is.
- In the compact test's Codex row, the invocation and the stored command are the same; the OMP row is the one that shows
  which is rendered.

#### Checks

- Run now: `dprint check TODO.md`, the sleep checker (299 delays, none missing a reason), the CLI and web builds, and
  both changed spec files in full on Chromium and WebKit. That gave 118 passed and 2 failed, run
  `844a1fd6-5513-4dd7-8f34-605936399851`, kept; the failures were the stale OMP test above.
- After the fix, the OMP spec on both engines: 4 passed, run `5820bcd8-291a-4775-9f0f-b551b29ac0aa`.
- Not run: Rust tests and other browser specs; only browser tests and TODO.md changed.
