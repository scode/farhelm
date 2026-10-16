### What this was about

An unusable checkout folder could make preview or session creation fail with advice to change the folder in Settings.
That cannot fix a host using its own command-line override: the override keeps winning over the all-hosts setting.

The error now names both Settings and the per-host command-line remedy, `farhelm helm checkout-config`, as approved in
triage.

### Things you should know

The supervisor receives the resolved folder without knowing which setting supplied it. The advice therefore names both
possible remedies. This changes one unusable-folder message, with no new configuration tracking or protocol and no
change to folder resolution or other errors. The resolved finding is removed from the pending queue, and a fixed
changelog fragment is included.

### Open questions and possible follow-ups

None required for this plan.

### The PRs

- [#2119](https://github.com/scode/farhelm/pull/2119/changes): name the host-override remedy in unusable checkout-folder
  errors; draft based on main.

### Checks run, reused and skipped

- Recorded `cargo nextest run -p farhelm-supervisor --lib` with the missing-checkout-root and root/preview refusal cases
  selected: three passed, 1,130 unrelated tests selected out, zero failures or flaky results. Recorder
  `38e80911-ad6f-4790-800f-16bee00b9ed3`, pinned nextest and tmux, four slots, zero retries. Complete manifest/JUnit and
  exit-zero result agree; retained output has EOF, no truncation and no runtime skip marker.
- The existing dangling-symlink fixture now checks both remedies. The selected cases also preserve coverage of future
  roots, symlink ancestors, reserved names, changed bindings, invalid paths and preview-field refusals. No process
  environment is changed by the tests.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` passed. Cargo formatting, changed-Markdown dprint,
  changelog format, explicit-commit whitespace and the isolated delay check passed; 301 delays inspected, zero missing
  reasons.
- No earlier runtime evidence was needed. Tests cover source `61121c98`; the later PR-link update changes bookkeeping
  only. Main remains at base `c79844e6`, so no rebase or interaction check was needed beyond confirming that.
- Skipped broader Rust, browser, desktop, installer and release suites. This is one error-text change with assertions in
  an existing root-resolution fixture; no UI flow, configuration semantics or installation behavior changes.

### The review gate's outcome

The required fresh-context external gpt-6.1-sol high review found no issues and confirmed the complexity gate. Its full
artifact was read and checked against the implementation, CLI grammar and acceptance criteria. The reviewer received the
full outcome and test-authoring checklist; its review was static, with runtime validation performed separately. A
separate native gpt-6.1-sol medium wording cold read passed. The documentation pass explains why the advice cannot infer
the setting's source and why the regression needs both remedies. No unresolved finding remains.

### Landing

Landed as #2119, the last of the round's seven plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking: the named command exists and runs on the helm's machine,
per-host folders are command-line only as SPEC.md says, and the only interpolated value is the local resolution error,
shown as text.

#### Fix made while landing

The message assumed the host had its own folder ("change this host's folder override"). It now says "or, if this host
has its own folder, with farhelm helm checkout-config set-root or clear-root --host", which also names the subcommands;
the test checks the new wording. The changelog fragment says what the error now tells the user, without "remedy" and
"per-host overrides".

#### Checks

Run on all seven of this round's plans stacked on main (managed-checkout-row-accessibility, download-long-name-copies,
save-template-ime-escape, helm-start-without-ca-bundle, stopped-session-launch-error, trash-totals-unchecked,
checkout-override-error-wording), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`.
- Through the recorder, after rebuilding the CLI: the protocol, helm and supervisor packages in full with pinned tmux,
  run `4256456b-4340-4e64-86a5-85aa71206a7a`: 2395 passed; the UI crate with the desktop feature, run
  `5d0c7c3a-4d29-4a92-a8e2-a1eba6893786`: 656 passed.
- The JS harness (257 passed) and the website build.
- The github-checkouts, templates, trash-dialog, sidebar, provisioning and tooltip-coverage specs on Chromium and
  WebKit, run `7f4cef76-eea8-4b05-9a31-1260bb14498a`: 456 passed.
- Not run: the CLI's end-to-end targets and other browser specs; no PR in this round changes them.
