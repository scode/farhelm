### What this was about

Pressing Escape while an input method was composing a template name could close “save as template,” losing its name and
checkbox choices. The launcher underneath remained, but the inline panel's draft was gone.

The small fix keeps a composing Escape inside that panel without cancelling it or preventing the input method's default
action. Ordinary Escape still closes the panel and returns to the launcher.

### Things you should know

The originally proposed guard was insufficient: skipping the panel's handler let Escape reach the launcher's own
cancellation handler. Source review identified that path, and the new regression failed in both browser engines. The
correction retains the existing propagation boundary and places the single composition condition around default
prevention and cancellation. The reviewer explicitly confirmed that this stays within the agreed complexity gate; no
parent-form or harness redesign was needed.

The test preserves a typed name and an unchecked agent choice, then verifies ordinary Escape keeps the launcher and
selected agent. It uses the existing synthetic composing-key event pattern and creates no persistent template or
session. Native operating-system input-method delivery was not tested. The resolved review finding is removed from the
pending queue, and a fixed changelog fragment is included.

### Open questions and possible follow-ups

None required for this plan.

### The PRs

- [#2115](https://github.com/scode/farhelm/pull/2115/changes): preserve template drafts when cancelling input-method
  candidates, draft based on main.

### Checks run, reused and skipped

- Recorded
  `npx playwright test 'templates\.spec\.ts' --grep 'save as template keeps the draft on composing Escape|save as template leaves composing Enter|launcher template save refuses a taken name'`:
  six passed, three each in Chromium and WebKit, zero skips, failures or flaky cases. Recorder
  `214adfec-8914-4c68-9462-39c4ed97dac9`, pinned tmux, one worker, zero retries, complete report and untruncated
  console. The disposable stack cleaned up normally.
- The first run, `8f996c26-ef24-4cbf-a571-7edd5d2810be`, had four passes and two failures: composing Escape still
  removed the panel in each engine. That failed evidence is retained separately and is not a pass. The corrected run
  repeats the exact selection.
- The corrected pinned Dioxus web build and `cargo clippy --all-targets -j 4 -- -D warnings` passed. Formatting,
  changed-Markdown dprint, changelog format and the isolated `python -B scripts/check-test-sleeps.py` passed; the
  checker inspected 301 delays with none missing a reason. The delay check remains applicable after the handler
  correction because test code and delays are unchanged.
- Reused the earlier native `cargo build -j 4` from the checkout-row working tree later recorded as `289f3473`, based on
  `3c5c4a61`. This plan changes only a UI key handler; inspected incoming numeric parsing and agent retry changes do not
  affect ordinary template reads/writes or the string fields exercised here. The UI web bundle was rebuilt for the
  correction. Final PR-link bookkeeping leaves the tested source and tests unchanged. Incoming main since base
  `c2e9ec08` contains only a download report and a separate plan claim, with no interaction requiring rebase or repeated
  runtime checks.
- Skipped broader Rust and browser suites, desktop, installer and release tests: the selected browser cases directly
  exercise composing Enter, composing Escape and ordinary cancellation through the real UI. No service lifecycle, native
  desktop integration or installation behavior changed.

### The review gate's outcome

The required fresh-context external gpt-6.1-sol high reviewer found the propagation defect in the first attempt. Its
correction review found no remaining issues and explicitly accepted the scope. Both complete artifacts were read and
checked against the diff; the review received the full test-authoring checklist and performed static inspection only. A
separate gpt-6.1-sol medium wording cold read passed. The documentation pass covers the key-ownership contract,
propagation comment and regression's purpose. No unresolved finding or scope departure remains.

### Landing

Landed as #2115, the third of the round's seven plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking for the plan as scoped: plain Escape still cancels, a
composing Escape now only stops the key from reaching the launcher, and the test checks its setup. The panel's Escape
handling was released, so a "fixed" entry is right.

#### Fix made while landing

The reviewer found the fix very likely does not work in the Mac app or Safari: WebKit ends input-method composition
before the keydown of the key that ends it, so Escape arrives with `isComposing` false (WebKit bug 165004). The usual
extra check (key code 229) is not available through Dioxus on the desktop renderer. The changelog fragment now says the
fix does not yet apply in the Mac app or Safari, instead of claiming it everywhere.

#### Left as is

- Follow-up for the maintainer: the same WebKit gap affects about 25 other input-method guards across the UI, including
  this panel's released composing-Enter fix. The new browser test fakes an Escape keydown with `isComposing: true`,
  which real WebKit does not send, so its WebKit pass does not show the Mac app is fixed.
- Noted, not widened: the launcher's own Escape handler has no composing check, so a composing Escape in its search
  field (results closed) or repository field (no suggestions) still cancels the whole launcher draft.

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
