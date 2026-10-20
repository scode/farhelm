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

### Landing

Landed as #2424, with the same content the report describes, rebased onto this round's main (see the end of these
notes). It was the fifteenth of sixteen plans in this landing round, after the fourteen listed in the earlier plans'
notes, most recently triage-discards-oct19.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the fourteen plans earlier in this round, and documentation
and plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. Two
touch the same dialogs:

- app-dialog-focus, earlier in this round, also changes SSH help: it decides where focus goes when SSH help closes, and
  it holds back host setup and uninstall questions, and host-action refusals, while another dialog is open. This plan
  moves focus to Close when SSH help's connection commands finish loading while its text holds focus. The two act at
  different moments, so they compose.
- reachable-at-small-sizes, earlier in this round, lets host action dialogs scroll as a whole in small windows. Host
  Settings does not use that whole-card scrolling, so this plan's focus recovery and its tests are unaffected.

This plan had no conflicts of its own when rebased. Its removal of its TODO.md entry later conflicted with
launch-sequences' removals; see that plan's landing notes.

#### Fixes made during the landing

None.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

The report was written on a main from before round 107's session-identity change, so its statement that no rebase was
needed and its focused checks no longer describe what landed; the combined runs above supersede them. Nothing else in
the executor's report became untrue at landing.
