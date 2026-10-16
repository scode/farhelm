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

### Landing

Landed as #2110, seventh of the round's nine plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking. The printed sign-in command names exactly the state
directory written into the service units whenever it differs from the shell's default, the bare command appears only
where it works, and the quoting round-trips through both a shell parser and the real CLI parser.

#### Fix made while landing

The changelog fragment now says when this shows up (running `farhelm helm setup` with `XDG_STATE_HOME` set only for that
command) instead of "environment-selected state directory".

#### Checks

Run on all nine of this round's plans stacked on main (terminal-link-safety, ntfy-token-server-change,
remote-uninstall-fixes, update-failure-reporting, docs-accuracy-fixes, host-picker-dotted-i,
setup-advice-state-directory, copy-guidance-resize, brick-test-feedback-evidence), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`; the brick-test tooling's own test.
- Through the recorder: the protocol and helm packages in full with pinned tmux, run
  `6352fad3-a3b1-48dc-9884-3efc17ee03c6`: 1258 passed; the CLI's own unit tests, run
  `0e1855d7-6f43-4da7-8a9a-495936ae565e`: 171 passed; the UI crate with the desktop feature, run
  `513addbb-2505-4c54-bc07-2ac2ec06713b`: 655 passed.
- The JS harness (257 passed) and the website build.
- The terminal-link-clicks, terminal-links, terminal-files, ntfy-settings, provisioning, settings and tooltip-coverage
  specs on Chromium and WebKit, run `97a26936-2b7b-42a5-af59-8f0fdb0b9d78`: 208 passed, 2 failed. Both failures were the
  clipped-disclosure test pinning the refusal's old layering after the quick switcher closed (see terminal-link-safety).
  After that expectation was fixed, the terminal-link-clicks spec on both engines, run
  `a698ea79-451d-4dd4-87f5-24ef1d4b1cad`: 16 passed. An intermediate run, `6113fd52-60bf-4730-ae85-5e9dc2e90d46`, failed
  on WebKit only because the first corrected expectation assumed Chromium's behavior.
- Not run: the supervisor package and the CLI's end-to-end targets, which no PR in this round changes; other browser
  specs.
