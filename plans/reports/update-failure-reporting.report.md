### What this was about

When the desktop app failed to install an update, its version hover sent users to the app log instead of explaining the
cause. The installation guide also said automatic failures were only logged and omitted checking from the version
number. The maintainer chose small fixes to the existing updater and guide.

### Things you should know

The hover now includes the installation stage and error chain. If the installer reports success but the installed
version does not change to the target, it says the installer finished without installing that version. The installed
record still determines whether an update succeeded. The message is one line, capped at 512 Unicode characters including
the ellipsis; the full error remains in the log.

The guide now explains immediate install-failure notices, the three-day grace for automatic latest-release checks that
keep failing, ending that streak when the latest release is found, the next-check retry, and selecting the version to
check when no update waits.

### Open questions and possible follow-ups

None. The checks exercise the real updater engine with injected installers and compile the desktop configuration on
Linux; no actual Mac installation or live update was run.

### The PRs

- [#2105](https://github.com/scode/farhelm/pull/2105/changes): show the failed installation stage and cause.
- [#2108](https://github.com/scode/farhelm/pull/2108/changes): correct the installation guide's update advice.

Both are drafts in one linear stack. The code PR carries its changelog fragment; each PR completes its triage record and
removes its feedback item.

### Checks run, reused and skipped

- Recorded desktop updater nextest selection: 77 passed; 575 unrelated tests selected out. Run
  `48822666-db0f-4907-b86f-455203bac968`. This includes the new automatic/on-demand failure, unchanged-record and
  multibyte truncation cases, plus existing verification, installed-record and scheduling coverage.
- `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings` passed in the owned build sandbox. The
  latter compiles the desktop-only updater that a normal workspace lint cannot reach.
- Formatting, changed-Markdown checks, changelog lint and the isolated sleep checker passed; 301 delays have reasons.
- The first recorder invocation refused before running because the sandbox user did not own the evidence root. The
  corrected invocation used the owning user and passed. This was a setup refusal, not a test failure.
- No browser or installer suite: this changes the existing desktop engine's reason text and ordinary Markdown, with no
  new browser behavior or installer operation. The guide needed only `dprint check`.
- Later changes only add the guide and PR tracking; the tested updater and tests remain unchanged. Incoming main changes
  since the stack's base were plan tracking and reports, with no product or validation input changed, so no rebase or
  repeated runtime check was needed.

### The review gate's outcome

Both PRs passed the required fresh-context gpt-6.1-sol high source review with no findings, and separate wording cold
reads. Both remained inside their complexity gates; no scope expansion was needed.

### Landing

Landed as #2105 and #2108, bottom-up, fourth of the round's nine plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking. The installed version still decides success before the
message is used, an installer that exits without installing gets its own message, the message is one line capped at 512
characters without splitting a character, nothing secret reaches it, and the hover is escaped for display. The guide
matches SPEC.md's update behavior.

#### Fixes made while landing

- The new message replaced "(the app's log says why)" with the error alone, but the installer's own output still goes
  only to the app's log, so the most common cause showed as just an exit status. Failures now end with "(the app's log
  has the details)", appended after the length cap so a long cause can never cut it off; the tests pin the exact text
  and the cap.
- The changelog fragment now says what users see: hovering over **check failed** beside the version number says what
  went wrong as well as pointing to the log.

#### Left as is

Control characters other than whitespace pass through from app-controlled sources; "failed" appears twice in the hover.

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
