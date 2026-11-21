# Triage outcomes

## shutdown-wait-test-does-not-witness-waiting.md

- Outcome: `fix code`.
- Assessment: confirmed by code reading on main e9ea6d08 (not run), by a delegated checker whose verdict triage
  reviewed. `the_stop_wait_is_bounded_and_covers_launches_still_starting`
  (crates/farhelm-supervisor/src/service/core.rs:~27112-27166) has a stated purpose in its inline comment: "the wait
  must not end while it is in flight, and must pick it up once it registers". It takes a `starting_launch()` guard,
  spawns a task that sleeps 200ms, writes the mark, calls `track_pending_hold`, then drops the guard. It then only
  asserts that `wait_for_pending_holds` returns true. A broken wait that ignored `starting_launches` would find nothing
  pending and return true at once, so the test would still pass. The function's own doc (core.rs:~7003) promises: "the
  wait does not end while any are in flight".
- Decision: recorded without a per-item question on the maintainer's 2026-10-26 instruction to auto-triage every
  remaining item that is not likely mooted by the planned infallible GUI–helm connection (TODO.md), is clearly a bug
  needing no spec or product judgment, and is straightforward to fix without complexity or scope creep. The user can
  revise it.
- Completion criteria: Make the test fail if the wait returns before the in-flight launch registers. For example, have
  the registration task set a shared flag (e.g. an `Arc<AtomicBool>`) after `track_pending_hold` and before
  `drop(starting)`, and assert the flag is set as soon as `wait_for_pending_holds` returns, before joining the task. Do
  not rely on `JoinHandle::is_finished`, which can race the task's final poll. Update the test's comment to say what
  witnesses the wait. Optional confirmation: make the wait ignore `starting_launches` and check that the test now fails.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over it, block and bring it back. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/stop-wait-starting-launch-witness.md`.

## account-test-does-not-refresh-healthy-orphan-list.md

- Outcome: `fix spec`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The old `OrphanList` went
  with 1eb4e3751 (#3407), and the desktop list in `crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs` goes
  further: by design (lines 41-42, coordinator decision 2026-11-15) a successful Test re-reads nothing, whether the list
  was healthy or failed. The list re-reads only on opening, Refresh, a key Save or Clear in this opening, a Delete, or a
  change in the fleet's Tensorlake hosts (lines 13-18, 152-191). `SPEC_impl.md:6878-6881` still says "A successful Test
  resumes failed discovery with one reload" and that another window's key save needs "Refresh, Test or reopening
  Settings", and the paragraph after it still describes the old `OrphanList`. The mismatch the finding raised is now
  unconditional: Test never refreshes.
- Decision: the maintainer picked "Make the spec match the app" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q3): where the desktop app deliberately does less than the spec promises, the spec is corrected
  to describe the app.
- Completion criteria: Correct SPEC.md and/or SPEC_impl.md so they describe the desktop app's current behavior for "Spec
  says Test refreshes the Tensorlake sandbox list, but the app never does", without adding behavior. No code change.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment). Remove the feedback file and its index entry.
- Execution: `pending`.

## attention-update-accessible-name-omits-target.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): Half of this is fixed. The
  attention list's Update button now carries the host's version and whether the update is required, as its hover help,
  which doubles as the accessible description (`crates/gui/farhelm-gui/src/session_list/attention.rs:885-893`;
  `hosts/update.rs:321-357`; `components/mod.rs:122`). The accessible name is still the bare label "Update"
  (`attention.rs:895-905`), with no host. The Hosts panel's inline Update is named "Update <host>"
  (`hosts/update.rs:371-385`). With several update rows, a screen-reader user tabbing through the buttons hears
  identical "Update" names.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Attention list Update buttons don't
  say which host they update"), with a test where practical. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate: "if any
  of these spiral into something very complicated, the executor should make a near term todo item to follow up on it
  instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the feedback
  file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## block-copy-omits-wide-continuation-column.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): A block (Alt-drag) copy
  takes the same columns on every row through `GridRow::text(from, to)`
  (`crates/gui/farhelm-grid-model/src/selection.rs:325-340`). When `from` falls on a wide character's second cell,
  `text` drops the character and emits no blank for that cell (`crates/gui/farhelm-grid-model/src/lib.rs:117-149`,
  "Starting on its spacer cell omits it"). That row's copied text is then one column shorter on the left than the other
  rows. The module still says the rules derive from xterm (`selection.rs:3-4`), and xterm emits a blank there.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## bold-brightening-ignores-redefined-palette.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The supervisor sends an
  unredefined palette entry as its index, and sends only redefined entries as direct colors
  (`crates/farhelm-supervisor/src/grid_output.rs:266-288`). The desktop renderer then promotes indices 0-7 to 8-15 for
  bold from its own theme (`crates/gui/farhelm-gui/src/theme.rs:432-445`). If a program redefines bright red (9) but not
  red (1), bold red still arrives as `Palette(1)` and is drawn with the theme's 9, not the program's.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## combined-punctuation-word-selection-differs.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): Desktop double-click word
  selection was ported from the old grid client's algorithm (`crates/gui/farhelm-grid-model/src/selection.rs:3-11`,
  113-171). Its separator test compares a single UTF-16 unit, the first unit of each cell (`separator` at 139-143,
  applied through `cell_start` and `cell_end` at 154-158). Punctuation carrying a combining mark therefore still counts
  as a separator by its base character. xterm, which the module says the rules derive from, compares the whole cell
  string.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## container-tab-close-overpromises-process-termination.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop tab Close
  question still says "Closing kills this terminal's shell and every process it started", and the Confirm help says
  "ending its shell and everything it started" (`crates/gui/farhelm-gui/src/terminal/set.rs:160-162, 197-202, 726-734`).
  The protocol's `CloseTab` docs still promise every process it left behind
  (`crates/farhelm-proto/src/lib.rs:3872-3880`). SPEC.md:2209-2214 has exceptions: a `podman exec` without a terminal
  keeps running inside the container, a container whose Podman monitor sits in a separate scope is not stopped, and
  hosts without a usable systemd user manager have narrower cleanup (2192-2204).
- Decision: the maintainer picked "Make the spec match the app" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q3), whose recommendation discards this one: the Close warning errs on the side of caution.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## credential-cancel-erases-delivery-report.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop Credentials
  section keeps the old behavior on purpose. Back and Cancel close the editor with an empty failure list
  (`crates/gui/farhelm-gui/src/settings/credentials/editor.rs:433-445`, `close_editor(..., Vec::new(), ...)`).
  `close_editor` replaces the overview's delivery-failure lines unconditionally (`settings/credentials.rs:362-376`). The
  module docs say so: "until the next editor closes (Back and Cancel clear them, as the old app's did)" (lines 30-32).
- Decision: the maintainer picked "fix code for the ones I name" for the minor cosmetic group (2026-11-21 six-question
  triage page for findings that cited removed UI code, Q6) and did not name this one, so it is discarded.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## desktop-smoke-supervisor-precedes-cleanup-record.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop app still
  spawns its supervisor detached and only then writes `desktop-supervisor.json`
  (`crates/farhelm-desktop/src/lib.rs:1431-1448`; a failed write is only logged). The smoke harness's `stop` finds the
  supervisor only through that record (`scripts/gui-agent.py:2048-2058` `supervisor_to_stop`, used at 2735-2737), so a
  supervisor the app started but never recorded is not stopped.
- Decision: the maintainer picked "Add the first two tests, discard the rest" (2026-11-21 six-question triage page for
  findings that cited removed UI code, Q5).
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## desktop-updater-read-failure-enables-auto-install.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): This is current desktop
  code. `automatic_updates_enabled` returns `true` when the desktop state file cannot be read
  (`crates/farhelm-desktop/src/updater.rs:472-483`), and the updater worker asks it at each install gate
  (`updater.rs:435-438`). If the state file becomes unreadable after a successful startup, a background install can go
  ahead despite the user's saved opt-out.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Automatic app updates can resume if
  the settings file becomes unreadable"), with a test where practical. Size gate: about 100 lines of diff (code, tests
  and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity
  gate: "if any of these spiral into something very complicated, the executor should make a near term todo item to
  follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove
  the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## drag-preview-neighbor-diverges-from-drop-unresolved.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop pin drag
  recomputes the drop target only on pointer moves, edge-scroll ticks and release
  (`crates/gui/farhelm-gui/src/session_list/pin_drag.rs:1170-1177, 1275-1278`, `resample_pin_drag` at 1440). The
  layout-change resample runs only for Shift sweeps, not pin moves (`resample_sweep_after_layout`, 1514-1524). The drop
  line is drawn on the pin it names, so if the list scrolls under a still pointer (a wheel scroll mid-drag), the line
  stays on the old neighbor and the release moves the pin next to a different one. On macOS GPUI repeats the last move
  every 16 ms while the button is held (module docs 53-56), which re-samples, so this route is Linux-only. Not
  reproduced at runtime.
- Decision: the maintainer picked "Acceptable, and say so in the spec" (2026-11-21 six-question triage page for findings
  that cited removed UI code, Q2); the principle ("a control the screen still shows after its target changed may be
  used, as long as the helm refuses what is no longer valid or the result is trivially undone") is added under
  `frozen-sandbox-retains-header-lifecycle-answers.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## environment-launch-opens-earliest-session.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): This is current desktop
  code. When an environment launch's run completes, the launcher reads `GET /api/sessions?host=<new host>` and opens the
  session with the smallest `created_at` (`crates/gui/farhelm-gui/src/launcher/launch.rs:871-899`). It is a timestamp
  guess rather than the exact session the launch created, so two sessions on the new host with tied or out-of-order
  whole-second timestamps could open the wrong one. No shipped sequence that puts a second session there before
  completion has been shown.
- Decision: the maintainer picked "Acceptable, and say so in the spec" (2026-11-21 six-question triage page for findings
  that cited removed UI code, Q2); the principle ("a control the screen still shows after its target changed may be
  used, as long as the helm refuses what is no longer valid or the result is trivially undone") is added under
  `frozen-sandbox-retains-header-lifecycle-answers.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## exit-notice-hides-oldest-scrollback-row.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop app ported the
  same layout. `crates/gui/farhelm-gui/src/terminal/attached.rs:1814-1827` (`placement`) and `lift` at 1988-2003 lift
  the shown rows by a whole row whenever the program has exited and the fitted pane has less than one spare row.
  `screen` at 2114-2149 draws them at `mt(-lift)` inside the `overflow_hidden` grid area at 2392-2398. None of this
  looks at the scroll position, and the viewport's top is clamped to the oldest retained line
  (`crates/gui/farhelm-grid-model/src/viewport.rs:60-74,103-107,115-121`). Scrolled all the way back after an exit, the
  oldest line (or the "Some earlier output could not be recovered" notice line) is clipped and no earlier scroll
  position can bring it into view.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("After an agent exits, its oldest
  output line can't be scrolled into view"), with a test where practical. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a
  complexity gate: "if any of these spiral into something very complicated, the executor should make a near term todo
  item to follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket),
  remove the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## feedback-discord-link-omits-external-indicator.md

- Outcome: `fix code`.
- Assessment: unresolved by a delegated reader against main 28f177284 (code reading, not run): The desktop feedback
  dialog's link is still labelled just "Chat on Discord" (`crates/gui/farhelm-gui/src/feedback.rs:596-605`). However,
  the rule the finding cited ("A link that opens a page outside the app ends in `↗`") was part of SPEC_impl.md's Dioxus
  GUI section and was deleted by the old app's removal commit `1eb4e3751` (#3407). Current SPEC.md (4777-4778) names
  only the words. The desktop app is mixed: Welcome, Announcements and Checkouts links carry `↗`, while the Discord,
  Tensorlake pricing (`hosts/setup.rs:470-480`) and prompt links do not. Whether `↗` is still a convention to enforce is
  a product decision nothing current settles.
- Decision: the maintainer picked "fix code for the ones I name" for the minor cosmetic group (2026-11-21 six-question
  triage page for findings that cited removed UI code, Q6), naming "the link missing the arrow".
- Completion criteria: The feedback dialog's Chat on Discord link ends in the external-link ↗, matching the desktop
  links that already carry it. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment). Remove the feedback file and its index entry.
- Execution: `pending`.

## frozen-sandbox-retains-header-lifecycle-answers.md

- Outcome: `fix spec`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The header's Stop, Restart
  and Replace buttons are disabled while the environment is frozen
  (`crates/gui/farhelm-gui/src/session_pane/header.rs:1133-1189`), and the frozen check is applied when a question opens
  (`allows_active` at 795-799, `session_list/menu.rs:659-666`). Two paths miss it: - **Open header question.** A
  question that is already open stays open when the sandbox is suspended elsewhere: layers close only when their subject
  is gone (`layers.rs:498-504`). Its Confirm runs `restart::confirm` / `replace::confirm`
  (`session_list/restart.rs:248-250`, `session_list/replace.rs:270-279`) with no new frozen check. -
  **Interrupted-session card.** Its Replace button is disabled only by `menu::refused`
  (`session_pane/card.rs:427,506-523`). `replace_from_card` (about 614-640) checks only that the card still shows, not
  frozen state, so it asks and sends. The card's Restart click does check frozen (597-601), but the button is drawn
  enabled. In both cases the helm refuses because the host is not connected.
- Decision: the maintainer picked "Acceptable, and say so in the spec" for controls that stay answerable after their
  target changed when nothing harmful follows (2026-11-21 six-question triage page for findings that cited removed UI
  code, Q2). This entry carries the spec change for the group of five.
- Completion criteria: SPEC.md's GUI concurrency text (beside the 2026-10-26 convenience-state principle) states the
  principle: a control the screen still shows after its target changed may be used, as long as the helm refuses what is
  no longer valid or the result is trivially undone. No code change. Remove the feedback file and its index entry.
- Execution: `pending`.

## full-host-tooltip-conflicts-with-line-cap.md

- Outcome: `fix spec`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): SPEC.md promises the full
  value on hover in two places: long host names in the expanded rail (2397) and the session title on an approval card's
  From line (4364-4365). In the desktop app both go through ordinary sentence help: the rail avatar via
  `HoverHelp::sentence` at `crates/gui/farhelm-gui/src/session_list/host_rail.rs:696-699`, and the approval title at
  `approvals/card.rs:291-296`. Ordinary help is drawn with `line_clamp(TOOLTIP_MAX_LINES)` at 280 pixels wide
  (`layers/tooltip.rs:1010-1015`), which is the eight-line cut SPEC_impl.md:871-872 specifies. A session title can be up
  to the 64 KiB rename budget and will be cut. A 64-character alias full of escaped formatting characters may also
  exceed eight lines.
- Decision: the maintainer picked "Make the spec match the app" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q3): where the desktop app deliberately does less than the spec promises, the spec is corrected
  to describe the app.
- Completion criteria: Correct SPEC.md and/or SPEC_impl.md so they describe the desktop app's current behavior for
  ""Full name on hover" tooltips are cut at eight lines", without adding behavior. No code change. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment). Remove the
  feedback file and its index entry.
- Execution: `pending`.

## grid-input-omits-x10-only-mouse-reporting.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The emulator still does
  not track X10 mouse mode 9. `crates/farhelm-terminal-emulator/src/modes.rs:12-27,51-54,83-92` has no X10 state and
  reports Off, and `crates/farhelm-supervisor/src/grid_input.rs:52-53` documents that "a program that sets only it gets
  no reports". The finding's comparison point, the raw-stream browser terminal that honoured mode 9, is gone. Every
  terminal is now this kind, so a mode-9-only program gets no clicks anywhere. DECRQM 9 answers "not recognized", so a
  program that asks first can tell.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## grid-shift-enter-loses-independent-altgraph.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop app has the
  same blind spot one layer lower. GPUI's Linux backend builds modifiers from Shift, Alt, Ctrl and Logo only
  (`vendor/gpui-pre-linux/src/linux/platform.rs:1669-1680`), so AltGr (ISO Level 3) never shows up. The input mapper
  documents "no ... AltGraph bit" (`crates/gui/farhelm-input-mapping/src/keys.rs:44`). AltGr+Shift+Enter therefore
  reaches the supervisor as Enter with Shift, which `crates/farhelm-supervisor/src/grid_input.rs:291-312` rewrites to
  Farhelm's newline (ESC CR) on the stated assumption that AltGraph never arrives.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## hovered-link-double-click-selects-one-word.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop selection
  press handler (`crates/gui/farhelm-gui/src/terminal/attached.rs:4001-4017`) uses `view.word_range(cell)` for every
  double click, even though the same handler has already resolved the pressed link (`pressed_link`, around 3960-3964). A
  double click on a multi-word hovered link therefore selects one word, where xterm.js would select the link's whole
  range. SPEC.md does not say which a double click should select.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## initial-oversized-grid-cannot-attach.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The supervisor half the
  finding was narrowed to is unchanged. The helm forwards the client's attach size as is
  (`crates/farhelm-helm/src/terminal.rs:286`), and the host accepts each dimension up to 10000
  (`crates/terminal-host/src/protocol.rs:142-147`, `crates/farhelm-supervisor/src/hosts/terminal.rs:2346-2348`).
  `grid_output::screen` copies the emulator's size into the screen with no 1000 cap
  (`crates/farhelm-supervisor/src/grid_output.rs:329-363`). The grid stream's `hello` (`service/grid_stream.rs:445`)
  then fails to encode and ends the attachment (816-821). The desktop app's fit clamps to 1000
  (`terminal/attached.rs:1846-1864`), so only a non-shipped client can trigger it.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## legacy-quickstart-stops-surviving-supervisor.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): `docs/old_readme.md:35`
  still says the app's embedded helm and managed local supervisor "both stop when the app exits", while lines 60-61 of
  the same file say the supervisor keeps running. The current desktop app detaches the supervisor and leaves it running
  (`crates/farhelm-desktop/src/lib.rs:571,1379-1386`), as SPEC.md:39 and 113 state. The same parked file also still
  describes the removed browser route (59-62).
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Parked quickstart contradicts itself
  on whether the supervisor survives quitting"), with a test where practical. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a
  complexity gate: "if any of these spiral into something very complicated, the executor should make a near term todo
  item to follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket),
  remove the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## light-appearance-darkens-terminal-catchup.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop terminal's
  "Connecting" placeholder (`crates/gui/farhelm-gui/src/terminal/attached.rs:2170-2189`) draws its text in the interface
  palette's `fg_dim`. That is `#414b5f` in the light appearance (`theme.rs:292`). The text sits on the pane's ground
  (2447), which comes from `style::defaults(None)` and is the fixed terminal background `#282c34` in both appearances
  (`theme.rs:379-381`). That pair is roughly 1.6:1 contrast.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes (""Connecting" text in a terminal is
  near-unreadable in light appearance"), with a test where practical. Size gate: about 100 lines of diff (code, tests
  and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity
  gate: "if any of these spiral into something very complicated, the executor should make a near term todo item to
  follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove
  the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## literal-deleted-suffix-breaks-desktop-owner.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): `process_image` in
  `crates/farhelm-supervisor/src/procs.rs:275` still strips a trailing ` (deleted)` on every platform, macOS included.
  An executable actually named that way is recorded under a different path, and `ManagedSupervisor::started_from` /
  `observe` (`crates/farhelm-supervisor/src/managed_supervisor.rs:229-262`) compare paths, so the app treats its own
  supervisor as foreign. The development override (`FARHELM_DESKTOP_FARHELM`,
  `crates/farhelm-desktop/src/bundle.rs:43,253-264`) still accepts arbitrary paths.
- Decision: the maintainer picked "Add the first two tests, discard the rest" (2026-11-21 six-question triage page for
  findings that cited removed UI code, Q5).
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## literal-this-machine-alias-rewritten-in-full-labels.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop app's full
  display name (`crates/gui/farhelm-gui/src/hosts/identity.rs:36-38`) calls `named(&host.name, host.is_local())`, which
  rewrites any local host whose name is exactly "this machine" to "Local (this machine)" (74-80). An alias of "this
  machine" on the local host produces that same name, so the rail tooltip, Hosts rows and launcher show "Local (this
  machine)". The helm does not refuse that alias.
- Decision: the maintainer picked "fix code for the ones I name" for the minor cosmetic group (2026-11-21 six-question
  triage page for findings that cited removed UI code, Q6) and did not name this one, so it is discarded.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## muse-capability-docstring-denies-reporting-and-resume.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The cited docstring
  survives in `crates/farhelm-proto/src/launch.rs:433-435` (on `offers_named_conversation`): "Muse, Cursor and OpenCode
  accept a one-time selector but still have no conversation reporting or stored resume command". A few lines below,
  `captures_named_conversation` (`launch.rs:477-492`) returns true for Muse, and Muse has a supervisor integration
  (`crates/farhelm-supervisor/src/agent_kind/mod.rs`). The comment is wrong about Muse. Nothing user-visible depends on
  it.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Developer note wrongly says Muse has
  no conversation reporting"), with a test where practical. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate: "if any
  of these spiral into something very complicated, the executor should make a near term todo item to follow up on it
  instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the feedback
  file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## older-muse-restart-refusal-blames-command-mode.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The supervisor still
  offers `RestartOffer::NoResumeCommand` for any integrated launch with no resume template, including an older Muse
  agent launch (`crates/farhelm-supervisor/src/agent_kind/mod.rs:1911-1916`). The desktop app's `offer_clause` turns
  that offer unconditionally into "this command launch did not opt into Resume, so it has no resume command and Replace
  starts it over" (`crates/gui/farhelm-gui/src/session_list/restart.rs:115-118`). The Restart hover help and the
  interrupted card (`session_pane/card.rs:293`) both use that clause. The protocol's own explanation says the same
  (`crates/farhelm-proto/src/lib.rs:1440-1444`). The launcher's Restart-with text already tells the two apart
  (`launcher/entry.rs:358-362`).
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Restart explanation calls an older
  Muse agent session a "command launch""), with a test where practical. Size gate: about 100 lines of diff (code, tests
  and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity
  gate: "if any of these spiral into something very complicated, the executor should make a near term todo item to
  follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove
  the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## pointer-capture-drop-crosses-new-modal-unresolved.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop app's pin drag
  follows the pointer through capture-phase window handlers
  (`crates/gui/farhelm-gui/src/session_list/pin_drag.rs:917-990`), which receive the release whatever layer covers the
  list. Its docs say app shortcuts still act mid-drag and name a Rename dialog opened by its shortcut during a drag
  (:21-24,36-38). The drag cancels when its row stops being movable, when the listing goes away, when Hosts takes the
  list's place, or when the window deactivates (:69-73), but not when a dialog opens. `pin_drag_released` (:1261-1307)
  then sends the move from the rows as last drawn, with no check for an open layer: `focus_layer_open` is consulted only
  for Escape (:1011,1027). The finding's open premise about the WebKit native event path has no GPUI counterpart, so the
  route is established in source.
- Decision: the maintainer picked "Acceptable, and say so in the spec" (2026-11-21 six-question triage page for findings
  that cited removed UI code, Q2); the principle ("a control the screen still shows after its target changed may be
  used, as long as the helm refuses what is no longer valid or the result is trivially undone") is added under
  `frozen-sandbox-retains-header-lifecycle-answers.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## program-selected-cursor-color-never-reaches-viewer.md

- Outcome: `fix spec`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The grid protocol still
  has no cursor-color field. `Cursor` carries position, visibility, shape and blink
  (`crates/farhelm-proto/src/grid.rs:1034-1042`), and `TerminalState` carries only default foreground and background
  overrides (:1121-1146). The desktop app paints the cursor in the fixed theme color:
  `TerminalColor::of(TERMINAL.cursor)` at `crates/gui/farhelm-gui/src/terminal.rs:1309`. The emulator still accepts OSC
  12 and answers queries from the stored value, so a program that sets the cursor color sees it reported back but never
  drawn.
- Decision: the maintainer picked "Not required; state it once" for terminal behavior in rare cases (2026-11-21
  six-question triage page for findings that cited removed UI code, Q4). This entry carries the spec change for the
  group of ten.
- Completion criteria: SPEC.md (Terminal experience) states: the terminal targets what common agent CLIs and shells use
  and need not match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not
  supported. No code change. Remove the feedback file and its index entry.
- Execution: `pending`.

## progress-hover-keeps-short-host-name.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop app's host
  progress card titles itself with the host's short name (`crates/gui/farhelm-gui/src/hosts/progress/card.rs:411,442`:
  `identity::short_name`). That card is the hover of the attention list's working rows, and those rows also show the
  short name (`session_list/attention.rs:474,836,1011`). For two hosts whose names share a first label, nothing on the
  attention row or its hover card tells them apart. The Hosts panel's own row uses the full display name
  (`hosts.rs:806`), so the problem is limited to the attention list.
- Decision: the maintainer picked "fix code for the ones I name" for the minor cosmetic group (2026-11-21 six-question
  triage page for findings that cited removed UI code, Q6) and did not name this one, so it is discarded.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## recent-setup-description-omits-wrapper-prefix.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The finding's own appended
  section already names the desktop app, and it still holds. `selection_summary` and `summary_before_permissions`
  (`crates/gui/farhelm-gui/src/launcher/recents.rs:311-340`), which build a recent row's visible text, hover help and
  accessible name, never mention `wrapper_prefix`; the file has no mention of it at all. Meanwhile `SetupKey` compares
  the whole `LaunchSelection`, so different prefixes stay separate recents (:96-123), and Enter restores and launches
  the stored prefix. `launcher/view/recents.rs` doesn't add it either.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Recent setups don't show their
  wrapper prefix"), with a test where practical. Size gate: about 100 lines of diff (code, tests and docs, not counting
  this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate: "if any of these
  spiral into something very complicated, the executor should make a near term todo item to follow up on it instead of
  adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the feedback file, and say
  so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## replace-selection-focus-contract.md

- Outcome: `fix spec`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The specification conflict
  is still in current SPEC.md. "Replace selects its replacement if its source is still selected when it finishes"
  (`SPEC.md:3242-3243`) sits against "Once the user has moved on, the result moves no focus, opens no dialog and
  switches no session" (`SPEC.md:3528`). The desktop app follows the Replace rule: when the user has moved on it still
  selects the replacement, through `selection::open_automatically`, just without focusing its terminal
  (`crates/gui/farhelm-gui/src/session_list/replace.rs:23-27,416-422`). Nothing in the code is shown to be wrong; the
  two spec rules need reconciling.
- Decision: the maintainer picked "Make the spec match the app" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q3): where the desktop app deliberately does less than the spec promises, the spec is corrected
  to describe the app.
- Completion criteria: Correct SPEC.md and/or SPEC_impl.md so they describe the desktop app's current behavior for "Spec
  disagrees on whether a finished Replace may switch sessions after the user typed", without adding behavior. No code
  change. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a
  changelog fragment). Remove the feedback file and its index entry.
- Execution: `pending`.

## request-cards-cover-tunnel-controls.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The open-tunnels list is
  an ordinary popover on rung 2 of the layer ladder, dropping below the pill in the session's top row
  (`crates/gui/farhelm-gui/src/terminal/tunnels.rs:30-36`, `layers.rs:139-145`). The approval region sits on rung 5,
  above every layer that is not raised (`layers.rs:155-156`). It starts 12 px below the terminal area's top, centered,
  up to 660 px wide (`approvals/region.rs:17-21`), and nothing reserves or avoids the tunnel list's area. Only the
  tunnel warning is raised above the cards. The rendered overlap depends on pane width and was not measured.
- Decision: the maintainer picked "fix code for the ones I name" for the minor cosmetic group (2026-11-21 six-question
  triage page for findings that cited removed UI code, Q6) and did not name this one, so it is discarded.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## sandbox-checkout-hides-required-folder-setting.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The helm refuses a
  new-environment managed checkout when no global default checkout folder is set ("set a default checkout folder in
  Settings → Checkouts before launching a repository in a new sandbox",
  `crates/farhelm-helm/src/sessions.rs:1389-1398`). The desktop launcher deliberately skips the checkout-folder check
  for a new environment (`launcher/view/checkout.rs:337-341`), and `environment_refusal` has no such check
  (`launcher/request.rs:732-762`). So the inline setup field never shows, and Launch stays available until the helm
  refuses. The comment at checkout.rs:337-341 cites SPEC's recipe-target "exception", which concerns the preview, not
  the folder setting.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("New sandbox checkout doesn't show the
  missing default checkout folder"), with a test where practical. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate:
  "if any of these spiral into something very complicated, the executor should make a near term todo item to follow up
  on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the
  feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## snapshot-comparison-ignores-link-grouping.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): `link_eq` in
  `crates/farhelm-terminal-emulator/tests/common/compare.rs:189-206` still treats any two generated `<n>_alacritty` ids
  with equal URIs as equal, cell by cell. That includes the direct live-versus-restored comparison `diff(&live, &same)`
  at `tests/common/roundtrip.rs:309-312`, which needs no renaming. A restore that split or merged anonymous links would
  pass. The desktop grid model groups OSC links by id (`crates/gui/farhelm-grid-model/src/links.rs:170-178,265-305`), so
  such a change would be visible to users as different hover and underline extents.
- Decision: the maintainer picked "Add the first two tests, discard the rest" (2026-11-21 six-question triage page for
  findings that cited removed UI code, Q5); this is one of the two tests.
- Completion criteria: Add the missing regression check the Assessment describes ("Terminal snapshot tests can't catch
  changed link grouping"), so the product regression it names would fail a test. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment). Remove the feedback file
  and its index entry.
- Execution: `pending`.

## suspended-sandbox-keeps-row-confirmations-enabled.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): A row's Stop or Replace
  question is opened through `ask_about_row` (`crates/gui/farhelm-gui/src/session_list/menu.rs:555-598`) and closes only
  when its subject (the session) is gone or its anchor row is no longer drawn (`prompt.rs:80-86`). A row that becomes
  frozen (suspended, stopped, deleting) stays drawn. The confirm handlers `stop::confirm`
  (`session_list/stop.rs:224-235`) and `replace::confirm` (`session_list/replace.rs:269-278`) send the frozen request
  without checking `Environment::frozen` or `held`. The module docs say frozen and deleting phases withhold Stop and
  Replace (`session_list/environment.rs:15-29`), and only the menu and header controls enforce that.
- Decision: the maintainer picked "Acceptable, and say so in the spec" (2026-11-21 six-question triage page for findings
  that cited removed UI code, Q2); the principle ("a control the screen still shows after its target changed may be
  used, as long as the helm refuses what is no longer valid or the result is trivially undone") is added under
  `frozen-sandbox-retains-header-lifecycle-answers.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-deleted-raw-pty-cursor-control-sequence-oracle.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): No current test sends DEL,
  an arrow key's escape sequence or Ctrl+C through a real attachment to a raw-mode program and checks the bytes it
  received. Each layer has its own check, but nothing covers the whole path.
  `crates/farhelm-supervisor/src/grid_input/tests.rs` pins the key encoder's bytes.
  `crates/farhelm/tests/terminal_host_runtime.rs:266` sends only `go\r` to the child, and
  `crates/farhelm/tests/terminal_host_client.rs:688` sends only ESC+CR (Shift+Enter). In
  `crates/farhelm/tests/e2e/host_processes.rs:323`, `typed_line` types text and Enter only.
- Decision: the maintainer picked "Add the first two tests, discard the rest" (2026-11-21 six-question triage page for
  findings that cited removed UI code, Q5).
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-legacy-container-delete-existing-object-removal-oracle-lost.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): Legacy removal still
  exists in `crates/farhelm-supervisor/src/container.rs:184-205` (`podman rm -f`, then `container exists` must return
  1), and SPEC_impl.md:8371 still requires it. No test makes the removal itself necessary. In
  `only_confirmed_absence_releases_the_cleanup_fence` (container.rs:276-298) the fake engine exits 125 on `rm` and a
  fixed code for everything else, without recording or changing state.
  `crates/farhelm/tests/e2e/containerized_sessions.rs:260` deletes a session that never had a container. Removing the
  `rm` call would leave every test passing.
- Decision: the maintainer picked "Add the first two tests, discard the rest" (2026-11-21 six-question triage page for
  findings that cited removed UI code, Q5); this is one of the two tests.
- Completion criteria: Add the missing regression check the Assessment describes ("Nothing checks that Delete actually
  removes an old container"), so the product regression it names would fail a test. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment). Remove the feedback file
  and its index entry.
- Execution: `pending`.

## trailing-ordinary-host-remove-bypasses-sealed-attempt-outcome.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): `forget_host_locked`
  (`crates/farhelm-helm/src/hosts.rs:1339-1408`) returns provider and local-cleanup errors without writing the seal
  record. Its comment at :1372-1375 says the caller writes `sealed_outcome`. But `remove_host_for` (:1211-1219, used by
  `DELETE /api/hosts/{id}`) and `remove_host_checked` (:1240-1256, used by agent removal and whole-environment session
  Delete) never do; only `finish_seal_locked` (`sessions/sandbox.rs:309-322`) records outcomes. The desktop app offers
  Delete sandbox / Delete container again on a host whose deletion failed
  (`crates/gui/farhelm-gui/src/hosts/offers.rs:297-300`), and that route takes the unrecorded path.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Deleting a host whose earlier
  deletion failed keeps showing the old failure"), with a test where practical. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a
  complexity gate: "if any of these spiral into something very complicated, the executor should make a near term todo
  item to follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket),
  remove the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-podman-unregistered-explicit-cleanup-lacks-retained-operation-reason.md

- Outcome: `fix spec`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop Containers
  section keeps a leftover Delete's failure only in its own state: `failure` in
  `crates/gui/farhelm-gui/src/settings/containers.rs:127`, set at :310-311 and :328. A reply that arrives after Settings
  closes goes nowhere, and reopening starts with no failure. On the helm side, `DELETE /api/container-orphans`
  (`crates/farhelm-helm/src/environment/podman.rs:949-1010`) returns the error only as a 502 and keeps no failed
  operation. SPEC.md:5201 says this list follows "the sandbox-leftovers pattern", and SPEC.md:553 says that pattern
  keeps unfinished cleanup's error and Retry even after closing and reopening Settings. SPEC.md:5202-5205 separately
  accepts log-only reporting for a Delete that times out.
- Decision: the maintainer picked "Make the spec match the app" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q3): where the desktop app deliberately does less than the spec promises, the spec is corrected
  to describe the app.
- Completion criteria: Correct SPEC.md and/or SPEC_impl.md so they describe the desktop app's current behavior for "A
  failed container-leftover Delete's reason disappears when Settings closes", without adding behavior. No code change.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment). Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-recipe-place-status-error-discards-success-and-has-no-retry.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop app ports this
  behaviour on purpose. `crates/gui/farhelm-gui/src/settings/recipes/places.rs:17-21` says an unreadable places
  collection "drops every line for its reason … and it offers no Retry loading of its own", and `place_lines` (:175-179)
  draws only the error, with no Rebuild controls. The helm re-reads pushed collections only on a feed bump or a command
  (`crates/farhelm-proto/src/ui_channel.rs:290-296`; `crates/farhelm-helm/src/ui_channel_tests.rs:481`), so a failed
  read stays failed while nothing else changes.
- Decision: the maintainer picked "fix code for the ones I name" for the minor cosmetic group (2026-11-21 six-question
  triage page for findings that cited removed UI code, Q6) and did not name this one, so it is discarded.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-tensorlake-add-host-inline-container-recipe-compatibility.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop Add host
  Tensorlake form lists every recipe in the catalog as an ordinary enabled option, with no provider compatibility check
  (`crates/gui/farhelm-gui/src/hosts/add/tensorlake.rs:479-545`). Continue needs only a name and a recipe (`:456-458`).
  The embedded recipe editor offers the "Container image" base ("Runs on Podman hosts",
  `settings/recipes/editor.rs:94-115`), and a finished Save selects the new recipe (`tensorlake.rs:24-28, 312`). The
  helm then refuses the plan with "this recipe's base cannot run on Tensorlake"
  (`crates/farhelm-helm/src/provisioning/tensorlake_add.rs:804`), which contradicts SPEC.md:474 ("Unsupported recipes
  remain visible but disabled with a reason").
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Add host offers container-only
  recipes for a Tensorlake sandbox"), with a test where practical. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate:
  "if any of these spiral into something very complicated, the executor should make a near term todo item to follow up
  on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the
  feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## trailing-ui-host-lifecycle-provider-and-start-wording.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): Most of this is fixed. The
  row's phase words say "Starting…" when a stopped container is starting (`hosts/identity/phase.rs:63`), "asked …" uses
  the provider's words (`phase.rs:91-107`, `environment_ui.rs:176-181`), and the Hosts Suspend tooltips are
  provider-aware (`hosts/lifecycle.rs:198,206`). One part remains: the detail line `host_evidence` (`phase.rs:406-433`)
  ignores the `starting` flag and says "Asked the container host to resume at …" during a container start (the helm sets
  `starting: verb == "start"` at `crates/farhelm-helm/src/environment/podman.rs:706`). That line shows in the Hosts
  row's details and on the session's unavailable card (`session_pane/unavailable.rs:263`).
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("A starting container's detail line
  says it is resuming"), with a test where practical. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate: "if any of
  these spiral into something very complicated, the executor should make a near term todo item to follow up on it
  instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the feedback
  file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: `pending`.

## triple-click-truncates-long-logical-line.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop grid model's
  triple-click `line_range` still caps the wrapped-line walk at 1000 rows in each direction:
  `self.wrapped_span(position.line, 1000, usize::MAX)` at `crates/gui/farhelm-grid-model/src/selection.rs:173-178`. Its
  doc calls this "the pinned client's 1000-row search cap", but the finding showed xterm.js has no such cap. A logical
  line longer than 1000 wrapped rows on either side of the click is selected and copied only partly.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## typed-name-matching-repeats-quadratic-prefix-work.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop port keeps the
  same shape. `matching_names` in `crates/gui/farhelm-gui/src/templates/sequence.rs:501-524` checks
  `tiers[tier].contains(&name)` (a linear `Vec` scan) for every matching name, which is quadratic in the number of
  matches before the display cap. Tab, Enter and the indicator use it through `match_word` (`:530`). As in the finding,
  no user-visible slowness was measured.
- Decision: the maintainer picked "Add the first two tests, discard the rest" (2026-11-21 six-question triage page for
  findings that cited removed UI code, Q5).
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.

## wide-wrap-inference-drops-real-boundary-space.md

- Outcome: `discard`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop grid model
  ported the same guess. `pads_wide_wrap` (`crates/gui/farhelm-grid-model/src/selection.rs:357-371`) treats a wrapped
  row with no run in its last column, followed by a wide character, as emulator padding. Copy (`selection.rs:280, 344`)
  and printed-link text windows (`selection.rs:241-280`) then drop that column. The supervisor's row encoder still
  strips default-style spaces, including ones the program printed
  (`crates/farhelm-supervisor/src/grid_output.rs:210-233`). So a real space at the right edge before a wide character
  looks the same as padding, and the doc's claim that it tells a printed space apart is not true for default-style
  spaces.
- Decision: the maintainer picked "Not required; state it once" (2026-11-21 six-question triage page for findings that
  cited removed UI code, Q4); the principle ("the terminal targets what common agent CLIs and shells use and need not
  match xterm in rare modes, chords and wide-character edge cases; a program-set cursor color is not supported") is
  added under `program-selected-cursor-color-never-reaches-viewer.md`, and this finding is discarded under it.
- Completion criteria: No code or spec change. Remove the feedback file and its index entry.
- Execution: `pending`.
