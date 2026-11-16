# Parity: Foundations (M1)

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Keyboard focus, Things changing underneath, Losing the connection to the helm, One GUI at a time; and
SPEC_impl.md, GUI architecture.

The popover-movement specs asserted that a menu closes when its anchor moves. SPEC.md now says an open menu moves with
its session, so those rows are superseded. One closing rule still stands: changes in the Hosts attention list retire
session menus anchored below it (SPEC.md, "Session list"); that case belongs to the attention-list row of
`tests/host-updates.spec.ts` in [Sidebar and session view](sidebar-and-session-view.md).

The screenshot batch (`docs/gpui/screenshot-batch.md`) captures the first slice (session list, the open session's
terminal, row menu, Delete question, rename dialog, empty fleet) beside the foundation goldens; of its reference scenes,
the Delete question and the rename dialog are ported, and the rest wait for the parts of their pictures that no area has
built yet. `scripts/gui-e2e-smoke.py` drives the same slice end to end through the bridge.

The old app's selects had no spec of their own; the shared select (`crates/gui/farhelm-gui/src/components/select.rs`,
tested in `components/tests/select.rs` and pinned by the `select` and `select-open` goldens) replaces the native
`<select>` with two recorded deviations. The native pop-up was the platform's own menu, which on macOS opened over the
control with the chosen option on top of it; the GPUI select draws its list as an anchored menu of the layer manager,
below the control (above it when there is no room below). And the native menu was at least as wide as its control and
grew to its longest option, where the GPUI list is exactly the control's width and cuts a longer option with an ellipsis
(the migration's orchestrator chose "exactly" for the launcher's floating lists on 2026-11-14). Smaller differences that
follow from drawing a menu: the arrow keys wrap from the last option to the first, as in every menu here; a placeholder
("Choose a recipe") shows in the closed control only, not also as a disabled first entry of the list; and typing a
letter does not jump to an option.

### `tests/buttons.spec.ts`

Tests: 5. Covers: unavailable filled actions stay quiet and compact questions keep answer order.

partly: screenshot goldens `buttons--{light,dark}`

### `tests/chrome.spec.ts`

Tests: 3. Covers: the three-level surface ladder renders distinct, correctly grouped levels.

replaced

- screenshot goldens `session-list`, `session-details-bar` and `session-row-menu` (the three-level surface ladder: the
  sidebar and the details bar on the raised level over the ground, and the floating menu)
- selected session identity stays legible while Rename disables its open button: nothing to port, since the GPUI app
  never disables the row under Rename; the layer manager blocks the page behind the dialog instead
  (crates/gui/farhelm-gui/src/layers_tests.rs `the_page_behind_a_dialog_takes_no_clicks_and_empty_space_keeps_focus`),
  so the row draws unchanged
- crates/gui/farhelm-gui/src/components/tests/list_row.rs
  `keyboard_focus_rings_a_row_in_the_accent_and_the_selected_row_in_on_fill_and_a_click_does_not` (keyboard focus uses
  accent on ordinary controls and inverse on selection; a mouse click does not: here the ordinary control is an
  unselected session row, which rings in the accent, the selected row rings in the on-fill ink, and a click shows no
  ring under either click rule; a button's accent ring is pinned by the `focus-ring` golden)

### `tests/interface-fonts.spec.ts`

Tests: 3. Covers: interface and technical values load their own bundled font roles.

not yet ported

### `tests/modal-layout.spec.ts`

Tests: 6. Covers: overflowing ${family} keeps vertical margins at width ${width}.

partly: crates/gui/farhelm-gui/src/components/tests/dialog.rs `a_long_dialog_body_scrolls_and_the_answers_stay_visible`,
`a_frame_too_tall_for_the_window_scrolls_as_a_whole`

### `tests/modal-trap.spec.ts`

Tests: 4. Covers: the ${mode} switcher wraps search and close, excluding result options.

partly: crates/gui/farhelm-gui/src/focus_tests.rs `tab_stays_inside_the_topmost_dialog`,
crates/gui/farhelm-gui/src/layers_tests.rs `tab_stays_inside_an_open_menu`

### `tests/modal-focus.spec.ts`

Tests: 1. Covers: rename keeps focus in its field when the selected tab goes away.

partly: crates/gui/farhelm-gui/src/focus_tests.rs
`focus_goes_home_when_nothing_holds_it_and_never_takes_it_from_a_control` (home never takes focus from a control); the
case itself needs the rename dialog and the terminal

### `tests/keyboard-scroll.spec.ts`

Tests: 13. Covers: help menu keeps keyboard focus through a late terminal reveal.

not yet ported

### `tests/tooltip.spec.ts`

Tests: 18. Covers: a session's title and open area share its summary card.

partly: crates/gui/farhelm-gui/src/components/tests/hover_help.rs tooltip tests; the session-row cases are in the
sidebar's parity file

### `tests/tooltip-coverage.spec.ts`

Tests: 3. Covers: template launcher rows and search carry hover help.

replaced: component constructors require hover help (a missing one does not compile)

### `tests/popover-movement.spec.ts`

Tests: 8. Covers: Sort dismisses an open ${kind} before Compact moves its anchor.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/popover-header-movement.spec.ts`

Tests: 5. Covers: a completed Delete notice moves the anchor and closes an open ${kind}.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `an_open_row_menu_stays_open_and_follows_its_button_when_the_band_appears` (the band's appearance moves an open row
  menu with its button and leaves it open, with focus on its command)
- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `a_delete_whose_checkout_was_not_archived_shows_the_helms_words_in_the_band` (the band a completed Delete's notice
  inserts)
- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `a_later_notice_replaces_the_band_and_one_with_nothing_to_report_keeps_it` (a Delete with no notice keeps the band)
- crates/gui/farhelm-gui/src/session_list/delete_notice_tests.rs
  `dismiss_takes_the_band_away_and_focus_goes_home_to_the_open_sessions_terminal` (Dismiss takes the band away; with no
  menu open)

### `tests/popover-host-movement.spec.ts`

Tests: 1. Covers: a concealed same-height host reorder leaves the session ${kind} open.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/popover-expanded-host-movement.spec.ts`

Tests: 6. Covers: expanded progress growth moves and closes the lower host menu.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/popover-identity-movement.spec.ts`

Tests: 8. Covers: pin wrapping above closes the lower actions menu.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/popover-template-movement.spec.ts`

Tests: 1. Covers: rename ${moves ? "closes a displaced" : "keeps an unchanged"} template number popup.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/popover-width-movement.spec.ts`

Tests: 6. Covers: the actual activity clock moves a 9m bell at 10m and closes it.

superseded: SPEC.md "Things changing underneath" (an open menu moves with its session); port the remaining cases (a menu
whose session or anchor is gone closes)

### `tests/shell-scroll.spec.ts`

Tests: 1. Covers: a wheel over the header or the sidebar edge never scrolls the document.

not yet ported

### `tests/feed.spec.ts`

Tests: 11. Covers: the invalidation feed.

superseded by the UI channel (SPEC_impl "The UI channel")

- crates/gui/farhelm-gui/src/main_view_tests.rs `an_idle_window_sends_the_helm_nothing` (a healthy feed performs no
  periodic reads on the list, with a session view open, and with a stale session view open: one test, in which an idle
  window with no session open, with a session open, and with a session on an unreachable host open sends the helm
  nothing for ten minutes)

### `tests/readers.spec.ts`

Tests: 2. Covers: read ordering and recovery.

superseded by the UI channel's ordered pushes; port remaining acceptance cases

### `tests/listing-mutation-fence.spec.ts`

Tests: 1. Covers: a pre-${operation} listing cannot close the newly selected session.

superseded by serial admission (helm ui_channel tests); verify the case

### `tests/m6-5-debts.spec.ts`

Tests: 4. Covers: the M6.5 test debts.

superseded by the UI channel; verify each case

### `tests/auth.spec.ts`

Tests: 6. Covers: an unauthenticated browser gets an in-page token prompt.

dropped: browser sign-in removed (#3401)

### `tests/scratch.spec.ts`

Tests: 1. Covers: scratch dirs land in the stack state dir and keep their prefix.

test infrastructure: a helper of the old suite; no port

### `tests/real-agent.spec.ts`

Tests: 8. Covers: the real-agent helper's pacing and marker-waiting work against the fake agent.

test infrastructure: a helper of the old suite; scripts/gui-agent.py seeds fake agents

### `tests/f20_visual_capture.spec.ts`

Tests: 1. Covers: F20 visual capture matrix.

replaced by scripts/gui-screenshot-batch.py

### `docs-shots/add-a-remote-host.spec.ts`

Tests: 4. Covers: add button.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/first-session.spec.ts`

Tests: 3. Covers: this machine.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/launch-templates.spec.ts`

Tests: 6. Covers: templates button.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/manage-hosts.spec.ts`

Tests: 5. Covers: host menu.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/release-notes.spec.ts`

Tests: 70. Covers: v0.38.0 container launch choice.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/session-list.spec.ts`

Tests: 6. Covers: a row.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/start-a-session.spec.ts`

Tests: 12. Covers: new button.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/stop-restart-resume.spec.ts`

Tests: 2. Covers: header actions.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/visual-review.spec.ts`

Tests: 2. Covers: ${colorScheme} ${width === "main-650" ? "window-650" : width}.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `docs-shots/work-in-a-session.spec.ts`

Tests: 3. Covers: header.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `readme-hero/capture.spec.ts`

Tests: 1. Covers: capture the README hero.

docs media: rebuilt on the GPUI tooling at cutover (M4)

### `readme-video/capture.spec.ts`

Tests: 1. Covers: record the README demo video.

docs media: rebuilt on the GPUI tooling at cutover (M4)
