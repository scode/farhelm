# Confirmation focus and optional session questions

## What this was about

Opening a confirmation and pressing Enter used to cancel it. The maintainer chose the confirming button for session
delete, replace, restart and tab close, host setup, removal and uninstall, and permanent trash deletion. YOLO and
Restart with keep Cancel. The same plan adds an optional Settings switch so everyday session actions happen on the first
click. Both TODO entries are removed in the second PR.

## Things you should know

**skip session confirmations** starts off. It covers Delete and Replace in a session's menu, Restart, Replace and Delete
at the top of an open session, Replace on the interrupted-session notice, and shell-tab Close. It applies to the helm;
the current window changes immediately and other open windows pick it up when they reload. YOLO, host and trash
questions keep their separate rules. Restart with still opens its dialog, and Replace with still opens the session
launcher.

A skipped question retains the consent of the state shown on screen. An ended agent with open tabs permits closing those
tabs, but still protects an agent that has since restarted. A direct Restart of an idle, waiting or unknown agent does
not authorize stopping it if it starts working before the request is handled. A refusal uses the existing error and
refresh path; it never opens a confirmation by itself.

When a question opens, its enabled confirming button takes focus; if it is disabled, Cancel does. A held or composing
Enter cannot answer the question it just opened. Browser testing found native autofocus unreliable on repeated openings
in both engines, so a small mount-time focus handoff supplements it. Switching a tab-close question from one tab to
another also hands focus to the new answer. Host dialogs retain their quiet focus and Escape behavior.

The new preference upgrades the helm database. As accepted during planning, there is no downgrade path: an older helm
refuses to open that database. No installed app, user database or live host installation was changed during execution.

The docs pages **Stop, restart, and resume**, **Work in a session**, and **Manage hosts** describe the behavior. No
website deployment or screenshot publication was performed.

## Open questions and possible follow-ups

No decision remains for this plan. The documentation's existing screenshots can be refreshed after landing; the capture
flow uses main and cannot show an unlanded switch. Browser WebKit coverage exercises the renderer family, not the native
desktop shell or a Mac release installation.

## The PRs

- [#1990: Enter confirms the selected questions](https://github.com/scode/farhelm/pull/1990/changes), first draft, base
  main.
- [#1993: optional session confirmations](https://github.com/scode/farhelm/pull/1993/changes), second draft, based on
  the focus PR.

## Checks run, reused and skipped

Every runtime run below used `python3 scripts/record-test-run.py` with development selections, zero retries and four
nextest slots or one browser worker. Browser child commands were
`npx playwright test <selected specs> --grep <selected cases>` from `e2e`, on Chromium and WebKit. The focus selections
were header/tab/trash/launcher/Restart with confirmations, host removal/uninstall, interrupted Replace/working Restart,
and tab-close retargeting. The setting selections were the seven-action walk, stale-delete refusals, Settings layout,
and YOLO/host/trash/Restart with exclusions. Narrow retries selected only the failing walks and changed exclusion
fixtures; the final retry selected only the complete action walk in `session-confirmations.spec.ts`.

Focus checks:

- Recorded browser runs `8faa6965-3c3a-4426-90b5-00f45ef0fa0c` (14 passed), `d83c7c5f-e499-4ff0-8c3b-bf939d925dbd` (6),
  `0f0833cf-1e47-4877-95d6-96a8cea8aaac` (4), and `1cf26f8a-3d6a-4dd6-b04d-dbe965c0a3e0` (2), on Chromium and WebKit,
  cover prompt kinds, Enter, repeated openings, host quiet focus and tab retargeting. No runtime skips; the host cases
  had their required fleet substrate.
- `cargo nextest run -p farhelm-ui -E 'test(list::row::) | test(session_view::) | test(hosts::settings_dialog::) | test(provisioning::)'`,
  run `432bd6a2-b9dd-46d2-8479-928e753b4544`: 56 passed; 404 filtered out.
- Failed browser run `1f703a6f-119d-4b0e-a0eb-db9536bc3b48` retains 14 passes and 4 repeated-opening focus failures. The
  scripted handoff corrected the product defect; subsequent runs are separate evidence.

Setting and combined-stack checks:

- Recorded `cargo nextest run -p farhelm-helm -p farhelm-ui` with the session-confirmation, skipped-delete and
  delete-confirmation filters, `c787e654-d75f-462e-b820-2bc10d4b61dd`: 5 passed; 1,531 filtered out.
- Recorded `cargo nextest run -p farhelm-helm -p farhelm-ui`, four slots and zero retries,
  `e41dde57-0030-484b-bd73-bd5a8c55c775`: 1,534 passed, 2 failed, no skips. Both failures were one historical database
  fixture retaining the new column while claiming an older schema. The fixture was corrected, and exactly those two
  tests passed with `cargo nextest run -p farhelm-helm -E <the two checkout_config schema-26 tests>` in
  `d50c3264-dd80-4a87-9ac8-1b5191f62f16`. The 1,534 unaffected results are reused; this is not a claim that the failed
  package command passed.
- `cargo nextest run -p farhelm-ui -E 'test(direct_restart_consent) | test(restart_asks_only) | test(restart_prompt)'`,
  selecting the direct Restart consent matrix and the existing working-only question test,
  `ec4e60fa-a192-489e-9c99-a13681a6d03b`: 2 passed; 462 filtered out. Existing open-prompt consent coverage from the
  package run remains applicable.
- Browser `2333acd4-9836-4bd6-a9eb-2e790e595d50`: 18 passed, 2 failed. The successful cases cover default and skipped
  stale-delete refusal, Settings layout, host setup/remove/uninstall and YOLO exclusions. The new action walk failed on
  route-handler cleanup ownership. Retry `d4d6c1aa-e921-4b40-80a1-4f084860bb75` passed both permanent-trash cases but
  failed both walks on the tab reply's wrapper shape. Both fixture defects were fixed; final
  `3ac4f522-3bc9-4dfa-9706-90088b4c0918` passed all 6 selected cases on both engines, reaching all seven direct actions,
  preference persistence/reversal, and corrected trash/Restart with cleanup. No browser runtime skips. A subsequent
  review tightened the tab-prompt absence assertion to check the real prompt while the DELETE is held;
  `8b6015b5-622e-47fc-8f26-fc05cd7e8679` passed both complete action walks with that stronger assertion.
- Build-only failures `c3011ddd-783f-4e19-b592-2bf77d6648cc` and `3d774cce-6c86-4aea-92d2-f976ffbc3f88` retain missing
  preference fields in existing test fixtures. Those were corrected before the 5-test pass; neither failure executed
  runtime tests or counts as a pass.
- `cargo check -p farhelm-ui`, `cargo build -p farhelm -p farhelm-teststate`, and
  `dx build --package farhelm-ui --platform web --release` passed. The unchanged browser fixture binary built for the
  focus unit was reused. `cargo clippy --all-targets -- -D warnings` passed;
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed again after the narrow Restart consent correction.
  `cargo fmt --all -- --check`, `git diff --check`, `dprint check` on changed Markdown,
  `python3 releasing/check-changelog.py format` and `python -B scripts/check-test-sleeps.py` with its isolated pinned
  interpreter passed (298 delays, zero missing reasons). `bun run build` in the website passed after the documentation
  changes.

Evidence is reused across the careful rebases because the incoming desktop updater/readout changes and later plans-only
queue transitions do not change confirmation handlers, preference schema, focus selectors or their specifications. The
full incoming diffs were read. The consent correction was validated separately; only unchanged package coverage was
reused. No full workspace, full browser battery, native desktop, installer or release gate was run: the affected-package
and selected browser checks cover the identified implementation risks, and no change affected those other systems.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reviewed each unit, with follow-ups on its own
thread and the full test-authoring contract. Focus's four findings were corrected and its final review had no actionable
findings. The setting's initial review found a direct Restart consent regression, the wrong tab reply shape in the
action walk, and shared-preference leakage in two exclusion tests. A later review corrected a vacuous tab-prompt absence
assertion. All were accepted and corrected; the final integrated source review had no actionable findings.

A fresh scope reassessment found no unnecessary focus machinery; existing focus mechanisms plus the small handoff and
tab identity key were retained. A second scope reassessment found no unnecessary product complexity in the setting,
guarded action paths or narrow browser fixtures. Independent cold readers accepted the commit/PR wording. Source
reviewers ran no runtime checks; the recorded execution evidence above was collected by the executor. No review swarm,
code delegation, ready transition or merge was performed.

### Landing

Landed alone in its round as #1990 then #1993. After the stack was written, main gained the guarded permission mode
(#1992). That change reworked the YOLO confirmation, made a plain Replace of an older session ask the YOLO question on
hosts that ask before YOLO launches, and changed much of SPEC.md. The landing rebased the stack onto it. The one textual
conflict was in the Restart with browser test: the stack's cleanup rework collided with #1992's new "was agent's own
setting" label. It was resolved by keeping both.

#### Release-relevant

The helm's database moves from schema 47 to 48 (the new preference). As with every schema bump, a helm that has run this
build cannot be downgraded in place. The helm–supervisor protocol version is unchanged.

#### Interaction with the guarded permission mode

An independent fresh-context review traced every action the new switch covers: the row menu's Delete and Replace, the
session header's Restart, Replace and Delete, the interrupted-session notice's Replace, and shell-tab Close. With **skip
session confirmations** on, none of them can skip a YOLO question, including the one #1992 added. A skipped Replace
sends the same request as before, without the YOLO override, and the helm still refuses it as a YOLO launch. The YOLO
question then opens with Cancel focused. The two changes' preference and spec edits compose.

#### Fixes made while landing

- That property had no test. The one existing test that turns the switch on drives the session launcher, which the
  switch never covered, so it would pass either way. A new browser test now covers it ("a skipped header replace still
  asks the YOLO question"). With confirmations skipped and a YOLO session on a host that asks first, one Replace click
  must:
  - send a single Replace without the YOLO override;
  - be refused as a YOLO launch;
  - open the YOLO question with Cancel focused;
  - after Cancel, have sent nothing more and leave only the source session.

  It passed on both engines.
- The landing's first browser run had 26 failures out of 378, a cascade from one test. The plan's own walk through the
  skipped actions hit a background "Route is already handled" race in its status-rewriting fixture. That failed the test
  without running its `finally`, which left **skip session confirmations** on for the shared test helm. Every later test
  that expected a confirmation then failed. The fixes:
  - The fixture now tolerates that race, matching the pattern and reasoning terminal-reconnect's build stamp already
    uses.
  - Both spec files that turn the switch on reset it after every test.
- The walk also stopped a session it had just created and required "exited". On WebKit the stop once landed before the
  launch was confirmed and settled as "error". Either state is an ended agent, which is all the test needs, so it now
  accepts both and waits as long as the other stop-then-wait tests do.
- One SPEC.md sentence said a refused Restart's "next attempt asks"; it now adds "unless session confirmations are
  skipped", matching the paragraph that introduces the switch.

#### Review notes left as they are

- Two tests (in the header and terminal-restart specs) now carry two doc comments each, one left dangling. A comment in
  the list's shared code and one in the list view break mid-sentence. One provisioning test resets the switch to null
  instead of restoring its previous value, which is harmless.
- The open session view now redraws on every preference write, including the selection memory saved on each session
  switch. It does no visible harm.
- The host-update offer's Cancel now stays enabled while busy, matching the setup offer. That is a small change beyond
  focus that the report does not mention.
- Unconfirmed: after a skipped Replace from the row menu meets the YOLO question, the row menu may stay open behind it.
  That would be cosmetic only.

#### Checks

- Run now, on the rebased stack:
  - `cargo fmt --all -- --check`, `dprint check`, the changelog format lint and the sleep checker (298 delays, none
    missing a reason);
  - `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
    `cargo check -p farhelm-ui --features desktop`;
  - the helm, UI and protocol packages in full through the recorder (pinned tmux, four slots, no retries): 1685 passed,
    none skipped, run `40d6437d-f5ac-4f0e-9982-5a5c210a2800`;
  - the asset JavaScript harness (241 passed) and the website build;
  - browser specs on Chromium and WebKit (session confirmations, YOLO guard, guarded permissions, Restart with, Replace,
    header, terminal tabs, terminal restart, delete precondition, settings, trash dialog, provisioning): 352 passed and
    26 failed, run `e16fa1fe-56ea-4b59-acc5-aec79aa07262`, kept. These were the cascade described above, which started
    at 07:12 UTC. The new YOLO test passed there on both engines.
- Run after the fixes:
  - every spec file that failed there, on both engines: 141 passed and 1 failed, run
    `2c3b172b-e22a-4ca1-bcde-ef19bbd6d8e3`, kept. The failure was the WebKit stop-state premise described above;
  - after that fix, the session-confirmations spec three times per engine: 6 passed, run
    `6bf0052c-51c0-4637-bf20-5b953ae99ac3`.
- Reused: the executor's focus and setting evidence. The landing changes are test fixtures, one new test and one spec
  sentence.
- Not run: the full browser battery, desktop runtime, installer and release gates. The confirmation and permission
  surfaces this stack and #1992 share are covered by the selections above.
