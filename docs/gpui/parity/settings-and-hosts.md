# Parity: Settings and hosts

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Topology, Supported host setup, Local authority and trust between hosts.

| Old spec                             | Tests | Covers                                                                           | Status         |
| ------------------------------------ | ----- | -------------------------------------------------------------------------------- | -------------- |
| `tests/settings.spec.ts`             | 27    | settings focus stays on its painted fill and descriptions stay with their labels | partly (below) |
| `tests/credentials-settings.spec.ts` | 3     | credentials keep fixed names, replace write-only values and discard login drafts | not yet ported |
| `tests/credential-delivery.spec.ts`  | 2     | saved credentials report delivery failures without retaining the secret draft    | not yet ported |
| `tests/ntfy-settings.spec.ts`        | 3     | ntfy settings explicitly save, refuse, preserve and clear a token                | not yet ported |
| `tests/tensorlake-settings.spec.ts`  | 6     | Tensorlake settings save a write-only key, test the saved key and clear it       | not yet ported |
| `tests/tensorlake-recipes.spec.ts`   | 10    | Settings keeps recipe answers sticky and isolated to Tensorlake in ${palette}    | not yet ported |
| `tests/tensorlake-hosts.spec.ts`     | 8     | Add host pins inline recipe answers and focuses Name in ${palette}               | not yet ported |
| `tests/tensorlake-lifecycle.spec.ts` | 21    | wake-to-create leaves ${elsewhere} alone (${mode})                               | not yet ported |
| `tests/tensorlake-tunnels.spec.ts`   | 2     | tunnel failures make a connected host amber and show each mapping                | not yet ported |
| `tests/hosts-panel.spec.ts`          | 2     | Hosts swaps ordinary content and preserves filters, sort, selection and details  | not yet ported |
| `tests/host-picker.spec.ts`          | 6     | host picker supports mouse, navigation, typeahead, cancellation, and focus       | not yet ported |
| `tests/host-updates.spec.ts`         | 14    | attention groups problems, work and updates and opens the exact host             | not yet ported |
| `tests/host-checkout-folder.spec.ts` | 1     | host checkout folder saves, inherits, clears and reaches launch preview          | not yet ported |
| `tests/provisioning.spec.ts`         | 73    | rail ${action} modal stays visible over Sessions and returns to its avatar       | not yet ported |
| `tests/ssh-help.spec.ts`             | 4     | ssh help displays and copies both registered-host commands                       | not yet ported |
| `tests/workspace-trust.spec.ts`      | 3     | managed-checkout trust switches persist independently of Claude's legacy route   | not yet ported |
| `tests/appearance.spec.ts`           | 4     | ${initial.name} before application mount                                         | not yet ported |

## `tests/settings.spec.ts`, partly

The dialog's shell and its Sessions and Hosts sections are ported; the cases about the other sections (Appearance,
Terminal, Checkouts, Agent trust, Notifications, and Recipes, Tensorlake, Containers, Credentials and App) wait for
them.

NOTE: The app does not open Settings yet. Its entry point is `settings::open`, and the gear that calls it is drawn by
the sidebar on the host rail's foot, which is not drawn yet; until it is, only the tests and the screenshot goldens open
the dialog.

Replacing tests, in `crates/gui/farhelm-gui/src/settings/tests.rs` unless named otherwise:

- Opening and focus ("settings keeps both close controls pinned around its grouped scrolling body", the opening half of
  "settings navigation keeps mounted panes and follows the narrow list path"):
  `the_gear_opens_settings_at_sessions_with_focus_on_its_first_switch`,
  `escape_and_both_close_controls_return_focus_to_the_gear`,
  `the_frame_keeps_one_size_across_sections_capped_by_the_window`.
- Navigation ("settings navigation keeps mounted panes ...", its wide half):
  `the_dialog_lists_only_the_sections_this_build_has`, `switching_sections_keeps_a_pending_save_and_a_failure_line`. The
  section buttons are `Button`s in the navigation tier, whose activation and pressed state `Button`'s own tests cover.
- Saves and refusals ("settings reports and reverts a refused ${choice.field} save" for automatic updates, frame colors
  and naming; "quick-launch naming defaults on and its settings switch persists independently"; "host browser
  permissions persist independently through the settings switches"; "automatic host updates default off and persist ..."
  for the default and the write): `each_switch_saves_its_own_preference_field_under_the_preferences_key`,
  `a_refused_save_restores_the_stored_value_and_shows_the_error_beside_the_switch`,
  `name_sessions_on_quick_launch_reads_as_on_when_the_helm_stored_no_choice`,
  `a_save_in_flight_goes_on_after_the_dialog_closes`.
- Following the helm (the live half of "... persist across clients", now that changes arrive live, and the host
  switches' help explaining the current behavior): `an_open_dialog_follows_a_change_made_elsewhere_with_its_help`.
- The look: the goldens `settings-sessions` and `settings-hosts` (`crates/gui/farhelm-gui/src/goldens.rs`).

Not ported here, and why: the persistence across a second client and reload belongs to the helm's preference route and
the UI channel's pushes, which their own tests cover; "settings gap keeps focus ..." is the layer manager's empty-space
rule (`keeps_focus_on_empty_click` is on for Settings), with no Settings-specific test yet; the text selection half of
it does not apply, since GPUI labels are not selectable; "settings reverses a permanent removal answer without
reloading" needs the host removal question, which comes with the port of Remove host; the narrow list path is not built
(Deviations).

## Deviations

- **No one-pane layout at 620 pixels and below.** The old dialog showed either the section list or the section below
  that width, with "‹ Settings" to go back. The window's minimum width (720 pixels) is above it, so the layout is
  unreachable and is not built.
- **A section switch leaves focus where it was.** The old app moved focus to the shown section's heading, so a keyboard
  user went on from the top of the section. The focus coordinator moves focus to a control only while no layer is open,
  and the heading is not a control, so focus stays on the section button (Linux), or where it was (macOS, where the
  click does not focus the button; a control in the section that was hidden hands focus to the closest one left).
- **A click while another preference save is in flight does nothing.** The old app queued every switch's newest click
  per field and sent it after the earlier one ("the latest click wins"). Here the operation store admits one request on
  the preference row at a time, so the second click is refused; a second click on the same switch leaves it showing the
  value in flight, and one on another switch leaves that switch as it was; the user clicks again after the reply, which
  the embedded helm sends within milliseconds. Accepted by the migration's orchestrator on 2026-11-14, since preference
  saves are helm-local and take milliseconds; revisit only if a user meets it.
- **Every switch reports its refused save.** The old app showed no failure for the two host switches, nor for turning
  Skip session confirmations on, which SPEC.md's best-effort list in "Errors and diagnostics" kept silent because the
  old choice had already taken effect in an optimistic copy. With no optimistic copy a failed save never took effect, so
  the migration's orchestrator decided on 2026-11-14 that these report it too, with new failure lines worded like the
  old ones, and SPEC.md says so in the same place.
- **Turning Skip session confirmations off restores confirmations only once the helm has saved it.** The old app
  restored them in the window at once, whatever the helm answered (`session-confirmations.spec.ts`, "restoring
  confirmations reports save failure and recovers"). Here every session confirmation reads the helm's stored choice, so
  a refused save leaves them skipped, the switch back on and its failure line shown. The maintainer chose this on
  2026-11-14 (follow the helm), and SPEC.md's "Errors and diagnostics" says so.
- **The section list does not stay put while a long section scrolls** (the old `position: sticky`). No section built so
  far scrolls at the minimum window size.
- **The header is about 10 pixels taller** than the old one: the shared dialog frame lays out its title and summary on
  GPUI's default line height, which is taller than the browser's `normal`; Rename's header has the same height.
