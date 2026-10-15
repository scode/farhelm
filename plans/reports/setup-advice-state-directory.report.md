### What this was about

After `farhelm helm setup` with a shell-only `XDG_STATE_HOME`, the services used the selected state directory but the
printed sign-in command omitted it. Copying that command into another shell could show another helm's token or create
credentials for that other helm, leaving sign-in unsuccessful.

The delegated fix makes the printed command retain the resolved directory with a shell-quoted `--state-dir=` when it
differs from the ordinary default. Explicit directory flags keep their existing behavior, and ordinary default advice
keeps its familiar bare command.

### Things you should know

The fix stayed within its complexity gate: the advice block and a unit regression only. State-directory resolution,
service rendering and CLI arguments are unchanged. A fixed changelog fragment is included.

The regression injects the environment choice through the existing setup context. It uses a path containing spaces and
an apostrophe, then verifies both shell splitting and the real CLI parser recover the selected directory. Tests use an
owned filesystem fixture and a fake service manager; no live machine setup was performed.

### Open questions and possible follow-ups

None required for this plan.

### The PRs

- [#2110](https://github.com/scode/farhelm/pull/2110/changes): retain the selected helm in setup's sign-in advice
  (draft). The matching review finding is marked complete and removed from the pending review queue.

### Checks run, reused and skipped

- Recorded `cargo nextest run -p farhelm --bin farhelm -E 'test(setup::tests::)'`: 63 passed, 108 unrelated cases
  selected out, no runtime early-return skips or flaky cases, complete JUnit. Recorder
  `07f6e287-b0d7-4d1a-a06c-f18da88abf52`, using the pinned nextest and tmux, four global slots and zero retries. This
  covers the new environment case and existing default, explicit-directory and service-setup contracts.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, changed-Markdown `dprint check`,
  `python3 releasing/check-changelog.py format`, and the isolated `python -B scripts/check-test-sleeps.py`: passed. The
  sleep checker inspected 301 delays, none missing a reason.
- Reused those checks after the same-change PR-link amendment. The source and tests are unchanged. Read all incoming
  changes from the base `46cc4f11` through `717f3702`: only plan claims and an unrelated update-report delivery. No
  rebase or repeated runtime validation was needed for that bookkeeping.
- Skipped broader Rust, browser, desktop, installer and release checks: this changes CLI advice only, and the focused
  setup suite exercises the output and parser contract directly. No browser or installation operation changed.

### The review gate's outcome

The required fresh-context external gpt-6.1-sol reviewer at high effort found no issues. The full findings artifact was
read and checked against the diff. Its prompt included the complete test-authoring checklist; it performed static review
only. Implementation and validation were local. A separate wording cold read passed, and a documentation pass covered
the changed advice block and the new test. No unresolved finding or scope departure remains.
