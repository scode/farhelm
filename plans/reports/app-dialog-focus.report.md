## What this was about

Three dialog problems were approved for fixing within separate 100-line gates: installing an update dropped keyboard
focus when Restart replaced the check button; closing SSH help could lose focus when a setup offer replaced its opener;
and a delayed host question or refusal could open over another dialog and leave both ignoring input until Escape. All
three fixes fit their gates: 58, 44 and 100 changed code, test and documentation lines, excluding the agreed bookkeeping
and changelog fragments. None was discarded.

## Things you should know

The update handoff moves focus to Restart only when the disappearing check button still owns it. Moving to another
control or deliberately blurring the check button leaves that choice alone. Closing SSH help still returns to its
original Add host control when it survives; otherwise it focuses the setup offer's Cancel.

Host setup and uninstall questions, and host-action refusals, now wait for the current modal dialog to close. If two
hosts finish planning together, their questions take turns. This keeps the accepted result pending without holding the
page's operation lock. Ordinary one-click Update submission remains unchanged. A fresh scope review supported guarding
these result surfaces rather than changing every dialog opener; no modal stack or general queue was added.

Review corrected a deliberate-blur ownership gap and an incorrect test expectation about uninstall's initial focus. A
trial blur handler also failed one Chromium case because removal dispatches blur before disconnecting the button; the
final handler waits until that DOM operation finishes before distinguishing removal from deliberate blur.

## Open questions and possible follow-ups

No decision is required for these outcomes. The desktop-only update transition was tested with the shipped handoff
script in Chromium and WebKit and checked by desktop compilation. An installed desktop update and the native eval bridge
were not exercised; the browser fixture does not establish that additional runtime coverage.

## The PRs

- [#2405](https://github.com/scode/farhelm/pull/2405/changes): preserve update-control focus without reclaiming another
  control's focus.
- [#2407](https://github.com/scode/farhelm/pull/2407/changes): return SSH-help focus after a setup offer replaces its
  opener; based on #2405.
- [#2408](https://github.com/scode/farhelm/pull/2408/changes): defer host questions and refusals until their modal turn;
  based on #2407.

## Checks run, reused and skipped

Browser commands ran from `e2e` through the recorder on Chromium and WebKit, one worker, zero retries and pinned tmux.
Source identities and reports are complete, output is untruncated, and no runtime skip markers were found.

- `npx playwright test tests/update-readout-css.spec.ts`: eight passed in `10b2f8dc-92ec-4ad2-9753-edb1347de5a6`. The
  four unchanged stylesheet cases are reused. The focus cases were replaced with stronger ownership histories after
  review.
- `npx playwright test tests/update-readout-css.spec.ts -g 'installed update'`: six final cases passed in
  `6641fe50-b57e-4f71-b6d7-082b93a32359`, covering retained focus, focus moved elsewhere and deliberate blur in each
  engine. The preceding attempt `86d3fde4-ff67-4a88-bcd0-24b318b72b5f` had one failed Chromium removal case and five
  passes; it is retained as the failed trial described above, not counted as final success or a latent flake.
- `npx playwright test tests/ssh-help.spec.ts`: eight passed in `f67e8b41-17b9-43ec-b552-d74055c3fca3`, including the
  held probe that replaces the hidden parent's opener while help owns focus.
- `npx playwright test tests/provisioning.spec.ts -g 'delayed host dialogs wait|Settings waits while an uninstall|Update planning keeps only its own|uninstall and Update planning refusals|queued Update refusals'`:
  14 passed in `801174af-c4f4-48ca-9323-ee92e797c8eb`. This covers delayed plans, one-click Update refusals, two
  independent plans and the existing Host Settings and queued-refusal handoffs.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, changed Markdown dprint checks,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and the isolated test-sleep
  checker passed. The latter inspected 304 delays with no missing rationale. Matching release web builds passed for SSH
  help and the final host-dialog change. Initial compilation attempts caught an import and a target-specific identifier
  dependency; both were corrected before runtime validation.

The update runs covered the working tree based on `1e95a12c` plus the recorded first-PR patch; the SSH-help run covered
`32733be5` plus the second-PR patch, and the host-dialog run covered `45858459` plus the third-PR patch. The first two
PRs reuse their successful checks after later changes because their tested helpers and contracts are unchanged; the
host-dialog selection covers the new isolation interaction. URL-only ledger amendments change no test inputs. The native
`cargo build` from the launcher-retry plan's first outcome, later delivered as `6b831d4f`, is reused: this stack changes
no helm, supervisor or fixture-binary inputs. Main through `3755dab8` added only queue claims and other delivery reports
after base `1e95a12c`; the complete diff was read and introduced no product, specification or instruction interaction
requiring rebase or new runtime checks.

Broader Rust, browser, installer and native desktop runtime suites were skipped because focused browser coverage and the
compile checks cover the changed behavior, with the desktop limitation stated above. CI is manual-only and was not
dispatched.

## The review gate's outcome

Each PR passed one fresh-context gpt-6.1-sol review at high effort, with corrections reconciled by the same reviewer.
The host-dialog design also passed the required fresh-context scope reassessment. Commit and PR wording passed fresh
cold reads; one invalid reader that opened its diff too early was replaced. No findings remain. Documentation passes are
complete, all feedback entries are removed, each ledger entry names its change, bookmark and PR, and all three PRs
remain open drafts.

### Landing

Landed as #2405, #2407 and #2408, merged bottom-up, with the same content the report describes, rebased onto this
round's main (see the end of these notes). It was the seventh of sixteen plans in this landing round, after
launcher-retry-identity, launcher-composer-drafts, terminal-link-and-copy-fixes, transfer-leftover-cleanup,
trash-unknown-totals and reachable-at-small-sizes.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the six plans earlier in this round, and documentation and
plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. Three
touch the same dialogs:

- reachable-at-small-sizes, earlier in this round, lets host setup, removal, uninstall and refusal dialogs scroll as a
  whole in small windows. This plan changes when those dialogs may open, not how they lay out, so the two compose.
- hosts-panel-menu-close closes an open menu whenever the hosts panel changes height. The host dialogs are overlays, so
  holding a host question back until another dialog closes does not resize the panel. A host row does grow when that
  host's result arrives, which would close a menu opened just before; this plan's browser checks open the next menu only
  after the previous host row has finished growing, so they are not affected.
- host-settings-focus, later in this round, also changes the SSH help dialog: it moves focus to Close when SSH help's
  connection commands finish loading while the help text holds focus. This plan changes where focus goes when SSH help
  closes. The two act at different moments, so they compose. The Add host form's setup offer appears inside that form
  rather than as a dialog of its own, so it is not one of the host questions this plan holds back while another dialog
  is open.

The other conflicts were in the review feedback queue's index, where each plan removes its own entries; all removals are
kept.

#### Fixes made during the landing

None.

#### Possible follow-ups

- SPEC_impl.md still describes only the older, narrower rule that host results wait while Host Settings is open. After
  this plan, host setup and uninstall questions and host-action refusals wait while any modal dialog is open, and are
  shown one at a time. Nothing breaks today, but a future change could miss that contract; for example, a modal dialog
  that stays in the page while hidden would hold host results back indefinitely. Possible follow-up: describe the wider
  rule in SPEC_impl.md.

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
