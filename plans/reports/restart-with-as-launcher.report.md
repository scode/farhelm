## What this was about

Restart with had its own editor, so improvements to the session launcher needed a separate implementation there. It now
opens the same launcher used by New, Clone and Replace with, while keeping the existing session and its restart
contract. Agent settings and the conversation remain editable; command launches retain command, resume-command and YOLO
editing. Launch kind, agent type, host, folder, session name and first message stay fixed.

## Things you should know

Restart with gains launcher search, template shortcuts, catalog retry and Save as template. For agent launches, Reset
choices restores the stored settings and clears the conversation selector. The held folder gets the launcher's advisory
check, without consuming or changing remembered folder defaults. Errors use the shared alert with the lead “Restart
error.” If a reply is lost or unreadable, that message cannot establish whether the session restarted; the refreshed
session supplies the authoritative state.

Enter in the model field now chooses the model; a separate Restart action applies it. This follows the shared launcher's
keyboard policy. Initial focus remains Cancel. A refused restart keeps the editor and its changes. After a submitted
restart returns, the session is refreshed and its terminal reattached, including after a received refusal and after a
transport or reply-reading error leaves the outcome unknown. Plain Restart and plain Replace retain their behavior.

A template that sets a fixed field is refused atomically, even when it repeats the current value. Templates saved from a
launcher or the current Templates editor include the launch kind, so they cannot be applied here as saved. Kindless
partial templates can apply; the existing agent template command can author partial agent settings without an agent or
command choice. Command and resume partials follow the same field rule. The direct recent-setups band is hidden because
it sets fixed fields; recent search results explain the refusal instead.

All 32 original Restart browser cases were retained and ported. Fifteen cases add templates, held-field refusal,
shortcuts, raw command bytes, folder-memory independence, catalog retry and consent changes. The three existing Restart
layout cases remain. No original case was dropped in favor of generic coverage.

## Open questions and possible follow-ups

The Templates editor cannot author launch-setting templates that leave launch kind unset. Supporting those partial
definitions would make Restart templates easier to create; this delivery keeps the agreed fixed-field rule.

## The PRs

- [#2682](https://github.com/scode/farhelm/pull/2682/changes): represent Create and Replace explicitly in the launcher;
  behavior unchanged.
- [#2683](https://github.com/scode/farhelm/pull/2683/changes): shared Restart mode, removal of the separate editor,
  coverage, specifications and TODO removal.

## Checks run, reused and skipped

Runtime commands below ran through `scripts/record-test-run.py`. Rust selections used four nextest slots and zero
retries. Browser commands ran from `e2e`, on Chromium and WebKit with three workers and zero retries. All of those
selections used the repository's pinned tmux.

The representation-only first PR passed 116 selected launcher/list Rust tests in recorded run
`3d77adb2-3756-4f5c-9be2-a263442a8b2f`, plus web Clippy. That run covered the source used for `274e90b2`; subsequent
changes before its push were comments and removal of a redundant assertion. Browser coverage was deferred to the feature
that changes behavior.

```sh
cargo nextest run -p farhelm-ui --lib -E 'test(list::create_form::) | test(list::view::)'
```

Feature runs recorded working trees above `274e90b2`. `cargo nextest run -p farhelm-ui --lib` with list, session-view,
API and shared-launch-control filters passed 336 cases in `f470b944-2589-46b8-b926-b4abd673f6ce`. After corrections, the
launcher-draft and modal selection passed 100 cases in `34754b65-929c-4ed8-b633-a0428e4404ba`, including explicit
template defaults and refused Restart initialization. Selected-out cases were filter skips, and no selected test
reported a runtime substrate skip. Later changes only boxed the exact opening snapshot without changing its comparison,
corrected host guidance and updated documentation. Final native
`cargo clippy -p farhelm-ui --all-targets -- -D warnings` and
`cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings` both passed; the final web
bundle was rebuilt with pinned Dioxus 0.7.10.

```sh
# f470b944: initial feature source
cargo nextest run -p farhelm-ui --lib -E 'test(list::) | test(session_view::) | test(api::tests::) | test(launch_composer::) | test(launch_controls::)'
# 34754b65: corrected draft/modal source
cargo nextest run -p farhelm-ui --lib -E 'test(list::create_form::) | test(modal_dialog::)'
```

The first Restart/modal browser run, `95a22c78-df89-44f4-a7c7-bce647e8248c`, executed 100 cases: 92 passed and eight
failed, the same four cases in each engine. It exposed a direct-recent path that bypassed held-field refusal; the other
three failures were test assumptions about focus, shell-word quoting and scoped model search. The failure evidence was
retained. Focused run `fa93581a-cf53-4c63-b9be-4877548ba668` then passed all ten cases across Chromium and WebKit,
covering those four corrections and both held-request focus/error checks. The remaining successful Restart/modal
coverage was reused: the later storage and wording changes preserve those paths. Desktop-width and narrow-width layout
screenshots were also inspected.

```sh
# 95a22c78: 92 passed, 8 failed; retained, not a final-source pass
npx playwright test tests/restart-with.spec.ts tests/modal-layout.spec.ts -g 'restart'
# fa93581a: the four corrected cases and the pending-request focus/error case, both engines
npx playwright test tests/restart-with.spec.ts -g 'first (effort|model) edit|allowed partial agent template|held composer actions|matching recent'
```

The reused 92 successful cases and the initial 336 Rust cases came from the working tree above `274e90b2` with
fingerprint `d1e8a4d59defbe652c1ec853a9f5f7eddbabe377f58165b81cd616bc497efe64`. The corrected ten browser cases and 100
Rust cases came from that same parent with fingerprint
`27f0a562795e19751c85b41755d3e2cf0130ab55bb31840e25c5a387c108d341`. The complete Restart with/modal selection was not
rerun after corrections: its successful cases are reused where the intervening corrections leave their asserted behavior
intact. The final-source 318-case run below covers original launcher and plain lifecycle behavior, rather than standing
in for that Restart with selection.

Final-source browser run `5794833d-e4cb-434b-bbd9-6de08d0d93f3` passed all 318 cases across Chromium and WebKit:
launcher Enter/tabs, New/Clone/Replace, quick launch, template shortcuts, YOLO, header and plain Restart. Focused
template application/save run `3a703c66-faa4-43e1-aaa3-9632efd9b27a` passed all 12 cases across both engines. Both runs
used three workers, zero retries and pinned tmux; neither had skipped, flaky, interrupted or unstarted cases. Their
recorded working tree above `274e90b2` is the source committed as `273168e4` in #2683. Both draft PRs remain open.

```sh
# 5794833d: original launcher/plain lifecycle regressions
npx playwright test tests/launcher-enter.spec.ts tests/launcher-tabs.spec.ts tests/clone.spec.ts tests/replace.spec.ts tests/quick-launch.spec.ts tests/yolo-guard.spec.ts tests/launcher-template-shortcuts.spec.ts tests/terminal-restart.spec.ts tests/header.spec.ts
# 3a703c66: template application and saving
npx playwright test tests/templates.spec.ts -g 'tl: applies|a template fills|Replace with refuses|save as template captures|launcher template save refuses'
```

Unchanged backend restart coverage was reused from `6470f047-38f8-49e5-888c-ac0990e20e3c`:
`cargo nextest run -p farhelm --test e2e -E 'test(restart_with_resume::)'` passed 18 behavioral cases plus one fixture
setup. These ran at `274e90b2` with only pending documentation edits, against the unchanged backend/protocol on pinned
tmux; the frontend request-body tests and browser assertions cover the integration's wire choices. UI asset
JavaScript/style contracts were reused from `b25e74f6-06af-4362-8bf7-791653ddf78a`, 333 passed: the later diff leaves
their JavaScript behavior and stylesheet inputs unchanged.

Rust formatting, changed Markdown/TypeScript formatting, changelog-fragment format and the instruction-link check
passed. The latter retains one pre-existing triage citation warning. The isolated sleep checker inspected 315 delays
with zero missing rationales. The full workspace battery, unrelated browser specs, installer/provisioning tests and
desktop-shell smoke were skipped because the changed contracts are covered by the selected UI/backend/browser checks; no
installer, host setup or desktop-shell behavior changed. CI was not dispatched.

## Review gate outcome

One fresh Opus reviewer at high effort reviewed each PR. Findings were corrected in the same review session; the
feature's final verification had no findings. The principal corrections removed a false error alert during a pending
restart, fixed restart hover text, closed the direct-recent bypass, prevented a failed Restart initialization from
opening New, and preserved raw host text for confirmation. Generic launcher wording was retained where it also describes
relaunch; create-specific race text and held-host guidance were corrected.

A fresh scope reassessment at the executing model and effort found no unnecessary complexity after the corrective
rounds. The exact source snapshot, editable baseline and local terminal hooks preserve separate existing contracts; they
do not introduce another form or confirmation framework. The original three-part outline was reduced to a mode
extraction and one coherent feature PR so no intermediate PR retained an unreachable old editor.
