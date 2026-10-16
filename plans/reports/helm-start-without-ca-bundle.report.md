### What this was about

A Linux machine without usable CA certificates could lose the entire helm, including its web UI, because startup tried
to construct the HTTPS client used for remote-host installation downloads. That also affected developer builds and
operator-staged installation files, which need no release download. The approved triage decision was to keep this
outbound failure local to provisioning, within a small change to payload selection and its existing test seam.

The helm now avoids constructing that client for non-downloading sources. When a download source's client cannot be
built, startup retains a source that refuses installation with the original contextual error, including “building the
release download HTTP client.” Other helm operations remain available.

### Things you should know

The failure is retained for that helm run. After repairing the machine's CA certificates, restart the helm to construct
the download client again. There is no new retry mechanism, TLS fallback, persisted state or change to feedback sending.
The other startup HTTP clients already contain their construction failures; static inspection found no second fatal
client on the startup path. The feedback finding is removed from the pending queue, and a fixed changelog fragment is
included.

Validation injects the client-construction failure at the production selection boundary. No CA store was removed and no
live missing-CA startup or UI smoke was performed. The tests prove that payload wiring returns successfully, local and
developer sources never attempt client construction, and download requests retain the diagnostic. Existing selection and
signed-download provisioning coverage also passed.

### Open questions and possible follow-ups

None required for this plan. A live missing-CA smoke could add system-level evidence beyond the injected regression.

### The PRs

- [#2116](https://github.com/scode/farhelm/pull/2116/changes): let the helm start without usable CA certificates and
  report download-client setup failures during remote-host installation; draft based on main.

### Checks run, reused and skipped

- Recorded `cargo nextest run -p farhelm-helm --lib` with the two new payload-selection tests, the existing complete
  source-selection test and the signed-download provisioning test selected: four passed, 1,105 unrelated tests selected
  out, zero runtime skips. Recorder `7f115a05-50c8-469d-b678-04d104c117d8`, four slots, zero retries, pinned nextest and
  tmux; complete manifest, JUnit and exit-zero result inspected. The signed-download case uses its loopback fixture.
- `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, cargo formatting, changed-Markdown dprint, changelog
  format, explicit-commit whitespace validation and the isolated `python -B scripts/check-test-sleeps.py` passed. The
  delay checker found 301 delays and zero missing reasons.
- The tested source is the implementation in `dcdaff8b`; a later comment and PR-link bookkeeping leave executable code
  and tests unchanged. Incoming main through `be164859` contains only another plan's report and queue state, with no
  interaction requiring rebase or repeated runtime checks.
- Skipped broader workspace, browser, desktop, installer and release suites: the change is confined to payload-source
  selection and two existing test injection call sites. Targeted regressions and helm all-target lint cover the changed
  behavior; the live missing-CA limitation is stated above.

### The review gate's outcome

The required fresh-context external gpt-6.1-sol high source review found no issues and confirmed the agreed complexity
gate. Its full artifact was read and checked against the implementation; this was static review, with validation run
separately by the executor. The reviewer received the full triage entry and test-authoring checklist. A separate
gpt-6.1-sol medium wording cold read passed after clarifying which downloads the failure affects. The documentation pass
covers client construction, retained failure behavior and the regression contracts. No unresolved finding remains.

### Landing

Landed as #2116, the fourth of the round's seven plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking. The download client is still built once at startup with the
unchanged certificate-verifying builder; when that fails, the release sources become a source that refuses with the full
error instead of stopping the helm. There is no insecure fallback, and no other helm client blocks startup.

#### Fixes made while landing

- The failure was silent until someone first added or updated a host; the helm now logs a warning at startup, as the
  feedback and ntfy clients already do.
- The refusal is kept for the life of the process, so after installing CA certificates the helm must be restarted. The
  changelog fragment now says so: install the `ca-certificates` package and restart the helm.

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
