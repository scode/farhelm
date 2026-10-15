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
