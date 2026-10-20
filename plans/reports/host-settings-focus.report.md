## What this was about

While a host setting saved, the dialog's text became a temporary keyboard stop so it could still be scrolled. If it held
focus when the save ended, the next keystroke could be swallowed while the dialog recovered focus. SSH help had the same
problem when its connection commands finished loading. The maintainer chose to cover every host save, successful or
refused, and to return SSH help to Close when its text could no longer take keyboard focus.

## Things you should know

Host settings now keeps focus on a usable control after a save. If the previous control is no longer usable, it returns
to the destination field while editing, or the first checkbox otherwise. Successful field saves still return to their
edit button. SSH help moves focus to Close when commands arrive while its text held focus, and preserves a control the
user was already using. This also covers someone clicking the text while waiting before using a key.

The change uses the existing focus handoffs, with a stricter check for both current and remembered focus. It does not
introduce a general focus manager. The TODO entry was removed and a fixed changelog fragment added. No scope departure,
size gate, blocked work or discarded attempt was needed.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [#2424: retain focus in host settings and SSH help](https://github.com/scode/farhelm/pull/2424/changes), draft, head
  `fb6718f2c5d56d9a6dff12ae6869cd8ffd04b56b`.

## Checks run, reused and skipped

Run on the implemented source captured as working revision `c65220a2`, now published unchanged in the PR above:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`: passed.
- Dioxus 0.7.10 release web build for `farhelm-ui`: passed.
- Recorded Playwright selection `keyboard-scroll`, `modal-layout` and `ssh-help`: 78 passed, 39 on Chromium and 39 on
  WebKit, one worker and zero retries, with pinned tmux 3.7c. Run `e9c6bb40-3e86-48d3-8ad4-d76583a8e9e8` has complete
  source/report/output evidence and cleanup, with no failed, skipped or unstarted cases. The four new scenarios cover
  refused destination saves, successful checkbox saves, command arrival from text focus, and preservation of a focused
  SSH help control. Each checks the next native keystroke.
- The isolated test-delay source checker passed: 304 delays, none missing a reason. Changed Markdown and changelog
  format checks passed.

Reused the backend build from `7355efcf`; the complete intervening path audit through plan base `7e8fcde3` contains only
TODO and planning bookkeeping, with no backend changes. The web bundle was rebuilt for this change. Main's changes
through `6a2c292d` were two other claim lines only, so no rebase or repeated runtime check was needed.

Skipped unrelated Rust runtime, desktop, installer and full browser suites: this is a narrow shared dialog-focus change
covered by the selected browser specs on both engines. No Templates behavior changed, so no additional Templates spec
run was needed. No runtime attempt failed or required a retry.

## Review gate outcome

The required single fresh gpt-6.1-sol reviewer at high effort found no actionable issues after reading the full goal,
diff, surrounding focus code and test-authoring rules. Commit and PR wording passed its cold read after replacing an
unclear description of the temporary text focus with the lost-keystroke problem. The separate resume check passed.
