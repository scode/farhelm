## What this was about

A suspended sandbox or stopped container could wake after **Start YOLO session anyway** and ask the same question a
second time. New, Clone, Replace with and Restart with now continue after wake and any checkout preparation using the
one-time answer for the same verified installation. If the installation changes or the prepared checkout path differs,
review the target and press Launch, Replace or Restart again. Farhelm asks for approval if the new attempt requires it.

Selecting Grok or Antigravity can offer to set up its Restart hook. **Set up and launch** enables that agent's hook on
all hosts. Previously, a remembered folder could arrive during the save and become the launch destination even though
the question had never shown it. The save now continues the launch only when the question showed a launchable setup and
the same destination and source warning still apply. If the folder was still loading when you answered, the setting
saves and you can press Launch once the folder is visible. An unchanged, ready-to-wake environment can still wake and
continue. Replace with and Restart with also require the same source session and stop or replacement warning.

A saved Custom choice could look deleted while its registration list was loading or a read had failed. The Agent and
Command tabs keep the saved ID visible and explain the current read. A retained registration from the last successful
read can still start a session. If there is no entry, Launch waits. Once a successful list confirms deletion, choose a
different registration on Agent or choose Custom on Command to keep running the saved command.

Recent setups with identical summaries can carry different Custom approval flags. Keyboard users could reach Enter
without seeing which arguments the selected setup would use. Both layouts now show the complete saved arguments below
the results, with the compact wrapper summary in the row.

## Things you should know

A saved resume command can open Restart with while the registration is unread. The form explains the wait and lets you
enter a required conversation ID. Restart becomes available when a usable registration supports resumption and the
normal restart checks pass. Confirmed deletion or loss of resume support rules out that restart.

The maintainer chose the pinned, non-focusing detail for Recent. A bounded text area keeps long flag lists from taking
the results' space. Page Up and Page Down reveal the final arguments before Enter while keeping the command field
focused and the same setup highlighted.

## Open questions and possible follow-ups

None.

## PRs

- [#4999 Keep one-time YOLO approval through host wake](https://github.com/scode/farhelm/pull/4999/changes)
- [#5003 Tie hook setup to the destination shown](https://github.com/scode/farhelm/pull/5003/changes)
- [#5029 Distinguish missing Custom registration states](https://github.com/scode/farhelm/pull/5029/changes)
- [#5037 Show saved Custom approval flags in keyboard recents](https://github.com/scode/farhelm/pull/5037/changes)

## Checks run, reused and skipped

- Recorded `cargo nextest run --locked -p farhelm-gui --lib -E <selection>` with four slots and no retries. The launch,
  YOLO, wake, checkout, sandbox, Replace, Restart and environment launch modules passed 186 tests for the consent layer
  (`94b3ccf4-a9ee-4038-aa65-038cae45b302` on `bb22690a`). Hook setup cases and the folder-default, wake, Replace and
  Restart modules passed 81 (`34892528-cbff-4a08-b9fb-9c6c1b2739c0` on `3b38fb86`). Later rebases preserved their
  dispatch, source and destination checks. YOLO, wake, folder defaults, search and hook setup passed 91 on the rebased
  command surface (`5cd2166f-c491-48f2-a3e1-03d3c56fc9f6` on `fb81f695`).
- Recent, search and folder-list checks passed all 94 cases on `e78c87d9` (recorder
  `ba9f2e27-a2e6-4e94-9d5d-d875208cd7e7`). The final run also verifies the strengthened scroll reset assertion.
- After integrating main's typed Restart hook wording, 64 selected GUI tests passed on `5bd7d1dd` (recorder
  `b74077a2-173d-4bda-8d91-50285a9926c5`). This covers Restart with, both missing-registration tabs, the two hook
  history cases, both Recent paging and selection paths, and existing dialog scrolling. The following changes listed the
  new scenes in the screenshot batch index.
- Reviewed and published 16 new Linux references in light and dark across both registration tabs and both Recent
  layouts. `python3 scripts/check-gui-goldens.py --update --jobs 4 -j 4` rendered 572 images and matched all existing
  references on `3af8fe37` (`c3abcee1-10e8-4d5f-8b7d-14c51d298588`). The same runner in compare mode matched 24
  published images on `77a1ecc6` (`73d028c3-60eb-487f-91e8-a103af7f05d6`), selecting registration, Recent flags, Custom
  restart, YOLO and hook setup scenes.
- `python3 -B scripts/test-gui-screenshot-batch.py` passed all 47 cases at the registration layer
  (`0fc7e8c9-59b2-4f63-b72d-88b369e0a4b8`) and the final tip (`6bbbc555-8b4c-4b4b-aeef-054393612737`). The earlier
  failure identified scene names missing from the index after publication. The first registration render stalled when a
  headless press repeatedly preceded presentation. The fixture now presents the frame and presses once. The passing
  capture runs use a deadline.
- Rust formatting, Markdown and scene-index formatting, test-delay lint, changelog format and whitespace checks passed.
  `cargo clippy --locked -p farhelm-gui --lib --tests -- -D warnings` passed on `77a1ecc6`.
- The native macOS and full workspace suites were skipped. The Linux GUI tests, the fake helm's request and refusal
  witnesses, and both rendered themes cover this launcher change.

## Review gate

Each PR received an independent GPT-6.2 Sol high source review. Rechecks covered consent across wake and checkout
preparation, the destination and source checks for hook setup, retained registration wording and tests, and reading long
Recent flags without changing focus. A scope review kept keyboard paging local to the launcher by reusing the existing
scroll behavior. The final Restart with rebase was also rechecked.
