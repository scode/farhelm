## What this was about

The Templates list's bolt looked filled whether marked or unmarked, making the quick-launch template hard to identify.
Save also looked available after saving, leaving completion unclear. The maintainer chose an outline unmarked bolt, a
filled amber marked bolt, and Save greyed out whenever the editor has no unsaved changes.

The two draft PRs implement those choices. The bolt keeps its size and position when its shortcut label appears. Save
refuses clean activations, becomes available after an edit, and retains keyboard focus after saving. Both TODO entries
are removed.

## Things you should know

The leave prompt's Save, the existing busy lock, and other uses of the bolt character are unchanged. Normal browser
screenshots were inspected; no video or special capture harness was added.

The source-only sleep annotation checker did not finish: three attempts exceeded its 60-second source-check budget.
These are incomplete checks, not passes. No delay calls or process-environment changes were added. An earlier lint
attempt exited 143 with no diagnostic; the final targeted lint passed.

## Open questions and possible follow-ups

An older template with no stored "switches launcher to" choice is shown as an unsaved draft when selected in Templates:
the editor infers Agent or Command, so Save is available and leaving can ask to save even without a hand edit. The
editor opened automatically after "Save as template" instead starts from a clean draft. These existing entry-route rules
remain; this work does not normalize them. The sleep checker needs a completed run when its source scan can finish
within its budget.

## The PRs

- [#2844: distinguish the quick-launch template bolt](https://github.com/scode/farhelm/pull/2844/changes), head
  `596ec9d4d9efb4b0e96778e270c57720715b5981`.
- [#2845: show when a template needs saving](https://github.com/scode/farhelm/pull/2845/changes), head
  `6e7457f4d7f29a86876642197a0013fae82e24a7`, based on #2844.

## Checks run, reused and skipped

- Passed `cargo build -p farhelm`, both release web builds with
  `dx build --package farhelm-ui --platform web --release`, `cargo clippy -p farhelm-ui --lib -- -D warnings`,
  `cargo fmt --all -- --check`, configured Markdown formatting, and `python3 releasing/check-changelog.py format`.
- Recorder `3a309a01-b066-45ed-8c73-4b29a375883f`: all 17 CSS-token tests passed, including both palettes. Reused for
  the final stack because PR2 changes no stylesheet.
- Recorder `91300ce8-4a56-4df4-beec-eece769568ea`: 8 browser cases passed, four per engine, covering the bolt drawing
  and existing mark/rename/delete behavior. It covers #2844 and remains applicable because PR2 changes neither the bolt
  nor its action.
- Recorder `799110b3-6859-42f4-8d22-d020d89e6e44`: 28 browser cases passed, 14 per engine, covering Save state and
  focus, busy keyboard isolation and scrolling, create/edit/rename/undo, legacy names, unsaved departures, phone
  controls, custom permissions, and number-menu movement. It covers #2845's product behavior before the final
  screenshot-only test refinement.
- Recorder `82fb2c1d-52e5-4271-9923-293b6f5385c6`: the refined Save test passed in both engines, explicitly checking its
  settled neutral fill before the normal screenshot. No product code changed after the 28-case run. All browser runs
  used pinned tmux, three worker slots, and zero retries; no failures or skipped cases occurred. Chromium and WebKit
  screenshots were inspected.
- Incomplete: `python -B scripts/check-test-sleeps.py`, as described above. Skipped the full browser, backend, and
  native-desktop suites because the selected shared-UI checks cover the changed drawing and editor guard. No published
  documentation shots were refreshed.

## Review gate outcome

Each PR passed its required fresh gpt-6.1-sol/high adversarial review. The first review caught vertical movement when
the shortcut label appeared; the icon was anchored and the test now checks its position within the row. The reviewer
verified the correction and the test cleanup fix. PR2 had no findings, and its settled-color test refinement also passed
review. Both commit messages passed independent cold reads. No findings were rejected.
