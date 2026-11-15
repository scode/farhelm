# Parity: Templates and quick launch

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Creation (the Templates dialog, quick launch, number keys, launcher sequences and the Cmd+J template list).

| Old spec                                    | Tests | Covers                                                              | Status                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------------- | ----- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/templates.spec.ts`                   | 48    | template bolts use a fixed outline or filled drawing for their mark | partly: the list, summaries, ⚡ and number keys (`templates::dialog_tests`, `templates::shortcuts_tests`, `templates::summary_tests`); the editor, its fields, Save, rename and Duplicate (`templates::editor_tests`, `templates::fields_tests`); leaving, deleting, `tl:` and Save as template are still to come                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `tests/quick-launch.spec.ts`                | 30    | quick launch submits the wrapper prefix saved in its agent template | partly: the ⚡ chord's direct launch through New's prefill, the catalog and the launcher's submit path, unseen, the mark kept to success and cleared by a reveal, a slow one showing the launcher's progress pill until its reply (`templates::dispatch_tests::the_chord_launches_the_marked_template_unseen`, `a_revealed_quick_launch_is_no_longer_unseen`, `actions_tests::the_quick_launch_chord_runs_once_per_press_from_a_terminal`), an incomplete mark opening New with its choices and the needs-more-choices banner, which an edit takes away (`an_incomplete_mark_opens_new_with_its_choices_and_banner`), a model-only template picking its agent type and a model the agent type does not offer refused (`a_model_only_template_picks_its_agent_type_from_the_catalog`, `a_model_the_agent_type_does_not_offer_is_refused`), a refused mark opening New with the refusal (`a_refused_mark_opens_new_unchanged_with_the_refusal`), a draft the launcher refuses shown with its reason, the offline-host case (`a_draft_the_launcher_refuses_is_shown_with_its_reason`), a missing or unusable mark opening Templates (`a_missing_or_unusable_mark_opens_templates_with_its_hint`), a failed template read refused (`unavailable_templates_refuse_a_press_on_the_page`), a press consumed while a refused launch waits behind its notice (`a_press_waits_while_a_refused_launch_waits_behind_its_notice`), presses during a quick launch's preparation waiting (New, a second ⚡ and the sequence prefix) and its result after the user moved on offered by the notice (`presses_during_a_preparation_wait_and_its_result_respects_the_moved_on_rule`), a reset dropping a preparation (`a_reset_drops_a_preparation`), the sequence prefix refused while a launch is pending, as the old app refused it (`the_chord_launches_the_marked_template_unseen`), other dialogs keeping the keyboard (`the_chord_does_nothing_under_another_dialog`), and a sequence's Enter launching or opening New with its banner or refusal (`a_sequences_enter_launches_unseen`, `a_sequence_that_cannot_launch_opens_new`); the sequence's prefix, key capture, cancels and pill (`templates::armed_tests`, `components::tests::sequence_pill::a_click_on_the_sequence_pill_reaches_what_it_covers`). Naming, number keys, the quick switcher's row and a lost reply's retry wait for their pieces |
| `tests/launcher-template-shortcuts.spec.ts` | 8     | back-to-back bolt and number chords stack on New without submitting | partly: back-to-back ⚡ presses applying in order to the open launcher's draft, each its own refusal unit, with Launch left to the user (`templates::dispatch_tests::presses_apply_to_the_open_launcher_in_order`), a press waiting for the launcher's catalog read (`a_press_waits_for_the_open_launchers_catalog`), a Launch press dropping presses still waiting for it (`launch_drops_the_batches_waiting_for_the_catalog`), a missing mark or failed read refused there without opening Templates (`a_missing_mark_or_failed_read_refuses_in_the_launcher`), a shortcut's refusal shown over a press's reason and a later press's reason shown over it (`a_shortcut_refusal_shows_over_a_press_reason`, `a_press_after_a_shortcut_refusal_shows_its_own_reason`), and a sequence in the launcher applying, or refusing a word or an unbound key there (`a_sequence_in_the_launcher_applies_or_refuses_there`, `templates::armed_tests::the_launcher_form_says_apply_and_sits_at_the_bottom`); number keys, Clone's and Replace with's drafts, a sequence begun from a launcher text field, and Tab completion in the launcher's form wait for their pieces                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `tests/template-list-shortcut.spec.ts`      | 4     | Cmd-J exact name launches through the fresh quick-launch path       | not yet ported                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

## Template cases of specs in other areas' files

Specs whose rows sit in another area's parity file but which have template or quick-launch cases this area ports. The
rows stay in those files; the cases and the tests replacing them are listed here.

| Old spec                                    | Its file                 | Template cases                                      | Status                                                                             |
| ------------------------------------------- | ------------------------ | --------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `tests/quick-switcher.spec.ts`              | sidebar and session view | the `tl:` section and the Quick launch row          | not yet ported                                                                     |
| `tests/github-checkouts.spec.ts`            | launcher                 | quick launch into a managed checkout                | not yet ported                                                                     |
| `tests/custom-permissions.spec.ts`          | launcher                 | custom words in templates                           | not yet ported                                                                     |
| `tests/terminal-create-idempotency.spec.ts` | launcher                 | quick-launch double submission                      | not yet ported                                                                     |
| `tests/popover-template-movement.spec.ts`   | foundations              | the key menu closes when its row's template is gone | replaced: `templates::dialog_tests::the_key_menu_closes_when_its_template_is_gone` |
| `tests/tooltip-coverage.spec.ts`            | foundations              | the Templates dialog's hover help                   | not yet ported                                                                     |

## Deviations

Differences from the old app at the reference commit, each with its reason, and each also reported to the migration's
orchestrator.

- **One escalation banner per batch.** When one shortcut press or completed sequence applies several templates, the
  permission-escalation banner is judged once: the draft right after the batch's last agent switch against the draft
  from before the whole batch, and its Undo goes back to before the batch. The old app judged each template of a press
  separately, from the draft before that template, so the two can differ when earlier templates of one press changed the
  agent, the permission or the launch kind before the last switch. Chosen because the press is one action for the user,
  applied all or nothing (the migration's templates goal asked for the banner to be judged once per batch). Tested by
  `templates::apply_tests::escalation_is_judged_once_for_the_batch`,
  `a_permission_chosen_before_the_last_switch_still_raises_the_banner`,
  `a_permission_chosen_after_the_last_switch_raises_no_banner` and
  `a_later_edit_after_the_switch_keeps_the_switchs_banner`.
- **GitHub destinations refused for now.** A template whose destination is a GitHub repository is refused with "its
  destination is a GitHub checkout, which this launcher cannot open yet" until the GPUI launcher holds managed checkouts
  (the launcher area's managed-checkout piece). Tested by
  `templates::apply_tests::a_github_destination_is_refused_until_managed_checkouts_exist`.
- **Shortcut names in one platform's form.** The Templates dialog's help, hint lines and the Templates button's hover
  name the ⚡ chord, number keys and the sequence prefix as this platform spells them (⌘P on a Mac, Ctrl+Shift+P on
  Linux). The old app, a web page that could not tell which form its client handled, printed both ("⌘P / Ctrl+Shift+P").
  The desktop app knows, and SPEC.md's Keyboard shortcuts rule is "Each shortcut shows only the form this client
  handles". Tested by `templates::dialog_tests::the_button_help_names_the_chord_and_the_marked_template`.
- **The dialog's header stays on screen.** The old Templates dialog scrolled as one piece, its header included. The GPUI
  dialog frame keeps the header (title and Close) pinned and scrolls the body, as every other dialog of the app does; at
  a window too short even for the header the whole card scrolls.
- **A long summary is cut without an ellipsis.** A row's summary stays on one line, as the old one did, but one that is
  longer than the row is cut at the row's edge rather than ending in "…": the summary is several text elements (the
  template's values are host text, each drawn on its own), and an ellipsis can only end one. The launcher's footer
  summary has the same cut.
- **The ⚡ and key buttons show a hover fill.** The old buttons showed only an edge under the pointer; the GPUI button
  component draws the ghost tier's hover fill, the nearest look it has.

- **The number-key menu's labels.** The menu's header (the template's name over "Number shortcut") is the shared menu
  header the session and host menus draw, but its key labels are in the interface face, not the old monospace. Disabled
  controls fade to half opacity, the component layer's, not the old 40%.
- **The ⚡ and key buttons while a write is in flight.** The old list dimmed them to 40% while busy; here they keep
  their look and focus and ignore presses (the button component's busy state), since a dimmed, disabled button gives up
  focus, which would fall to the dialog and lose the user's place. Tested by
  `templates::dialog_tests::a_write_in_flight_leaves_focus_and_the_dialog_alone`.
- **A recipe-and-place target's summary.** The old summary named a template's environment target by the environment
  code's label for it; until the launcher's environments work brings that label, the summary names the target's recipe
  ("New dev").
- **One binding hint for a sequence's unbound keys.** When Enter outside the launcher finds both an unbound number key
  and an unbound ⚡ in a sequence, the old app opened Templates with both hint lines; the GPUI dialog opens with one
  hint, for whichever unbound reference came first in the queue. Tested by
  `templates::sequence_tests::key_refusals_choose_a_hint_and_a_launcher_sentence`.
- **Names come from the pushed list.** The editor checks a new or changed name against the templates the helm pushed,
  not a list it read itself, so the old refusal "The template list is still loading" is gone. A name is refused as
  unprovable only while the helm reports the templates unavailable or before its first snapshot, never for an empty
  list, and the refusal now reads "The template list could not be read, so Farhelm cannot check that no other template
  has this name; save again once it is back" in place of the old "could not be loaded ... retry loading it above", since
  there is no Retry to press. Tested by
  `templates::editor_tests::a_new_name_is_refused_while_the_templates_are_unavailable` and
  `save_refusals_come_in_the_old_order_and_send_nothing`.
- **No list reload after a save.** The old dialog reloaded its list after a save, and a reload that raced the save could
  lack the template just saved. The helm pushes the changes a command causes before that command's reply
  (`ui_channel::run` in `crates/farhelm-helm/src/ui_channel.rs` sends a command's changes and then its reply, pinned by
  `ui_channel_tests::the_changes_a_command_causes_arrive_before_its_reply`), so the list already holds the saved
  template when the editor learns the save is done. Tested by
  `templates::editor_tests::save_writes_the_whole_template_once_and_keeps_focus`.
- **While a save is in flight.** The old editor disabled every control while it was busy and made the dialog itself a
  Tab stop. Here the text fields turn read-only, the buttons (Save, the rows, "+ New template", ✕, "+ Add field") turn
  busy, keeping focus and ignoring presses, and the choices ignore presses without changing their look, so focus never
  falls out of the control the user was on.
- **The editor's small controls.** The model suggestions are neutral buttons with the neutral fill where the old ones
  had none, and the add menu's entries have the template rows' padding (10 by 12 pixels) where the old ones had 8.
  "Replace name" has hover help ("Clear the name so a new one can be typed"), which the old button lacked and every
  control here needs.
- **Where focus returns from a Templates opened by a shortcut.** The old dialog returned focus to the marked-menu toggle
  or the Templates button by name, whoever opened it. A Templates the ⚡ or a number key opens with a hint returns focus
  to what held it before it opened (usually the terminal), under SPEC.md's general rule that closing something returns
  focus to where it was before it opened: a shortcut is a key press that moved no focus, and the GPUI dialog has no
  handle on the sidebar's Templates button. Opened from the Templates button, it still returns focus to that button.
  Tested by `templates::dialog_tests::closing_a_hint_opened_dialog_returns_focus_where_it_was` and
  `closing_returns_focus_to_the_templates_button`.
- **Host names in the Host select.** The select's options are the app's own words, so a host's name is shown as its
  escaped presentation (hidden characters as `<U+…>`) but is not direction-isolated from the words around it, as host
  text elsewhere is; the select has no host-text option labels yet.
- **A stored environment placement.** The old Host field offered environments beside hosts. Environment placements come
  with the launcher's environments work, so the Host select offers hosts only; a template that stores a recipe and place
  shows "Choose a host" with the line "This template places sessions in an environment, which this editor cannot show
  yet. It stays as stored unless you choose a host or remove this field.", and the placement is saved unchanged until a
  host is chosen or the field removed.
- **The editor's screenshot.** The `template-editor` golden has no Delete in its footer yet: it comes with its piece.
- **A rename's bindings are read from the pushed preferences.** The old rename read the preferences again after saving
  the new name and said "shortcut bindings could not be read" when that read failed. Here the store is that read, so the
  message is said while the helm reports its preferences unavailable, or before they have arrived. Tested by
  `templates::editor_tests::unreadable_bindings_stop_the_rename_before_any_move`; the moves and their refusals by
  `a_rename_saves_moves_the_mark_and_key_then_deletes_the_old_name`,
  `a_rename_with_nothing_bound_goes_straight_to_the_deletion`,
  `a_refused_mark_move_stops_the_rename_and_keeps_the_old_template` and
  `a_refused_key_move_stops_the_rename_and_keeps_the_old_template`. The old "a shortcut write was interrupted" words are
  kept for a step that cannot start, which here only a retired store refuses (a reset closes the dialog with it), so no
  test reaches them.
- **A refused deletion of a renamed template's old name.** As in the old app, the status line then shows only the helm's
  words, not which step of the rename failed; by then the template is saved under the new name with the mark and key
  moved, and the old one is still there. Tested by
  `templates::editor_tests::a_refused_delete_of_the_old_name_shows_the_helms_words`.

- **The sequence's key rules, as decided for the GPUI port.** The sequence takes keys through the actions module's key
  capture, ported from the old armed listener's classifier (its oracle cases are
  `templates::armed_tests::sequence_keys_are_sorted_as_the_old_classifier_did` and
  `other_keys_sort_as_cancelling_and_passing_on`). Two cases had to be decided again. A keypad digit cancels the
  sequence and goes on to its target, as in the old app, so a word never starts with one by accident; only a key that
  carries its native identity can be told apart, so a keypad digit an input method commits (or the test bridge injects)
  reads as the main row's digit. And a second prefix while armed ends the sequence and is consumed, which is what the
  old app's cancel amounted to (its terminal dropped the chord); passed on, it would reach the prefix's binding and arm
  a new, empty sequence.
- **The session launcher's vertical margin on wide windows.** This area fixed the shared dialog frame's vertical margin
  to the old `max(24px, 5dvh)` (`app.css` at the reference commit, line 128). The old launcher's backdrop alone used a
  flat 24 pixels in windows at least 1000 pixels wide (line 6629, "Leave room for populated launch rows"), so the GPUI
  launcher now keeps 5% of the height there, a little less room than the old one on a tall, wide window. Restoring that
  exception is the launcher area's, which was told when the frame changed.
- **The sequence pill fades in without scaling up.** The old pill faded and scaled up from 96% over 200 ms (off under
  reduced motion). The GPUI pill keeps the fade (the component layer's one-shot entrance, also off under reduced motion)
  but not the scale: GPUI cannot scale a box (SPEC_impl.md, "Animation time").
- **The sequence pill is centered on the main area, not the terminal.** As the old stylesheet did, the pill is centered
  on the main pane (9 pixels lower in the Mac window with a session open). The old rule assumed a 30-pixel details bar
  under the terminal; until the details bar is ported, the pill sits about 15 pixels above the terminal's center. Its
  look was compared with the old `.launch-sequence-*` rules, since there is no reference screenshot of the pill; the
  golden is `sequence-pill`.
- **Cancels the GPUI app sees differently.** A lone Alt no longer cancels a sequence (GPUI reports a modifier alone as a
  modifier change, with no key-down; SPEC.md names only Meta, Control and Shift as not cancelling). A dialog that opens
  while a sequence is armed without a click or key (a result the user started finishing) ends the sequence at the next
  key, which goes to the dialog, rather than when it opens, since nothing tells the sequence a layer opened; the pill
  shows until then. Tested by
  `templates::armed_tests::a_dialog_keeps_the_keyboard_from_the_prefix_and_an_armed_sequence`.
- **Input-method text while a sequence is armed.** Text committed by an input method (a CJK source, a dead key's accent)
  arrives without a key-down, so the capture never sees it and a focused terminal still sends it to its program. The old
  listener cancelled on a composing key. Refusing it falls to the terminal's input handler.
- **The launcher's text fields keep the sequence prefix.** SPEC.md has an unselected launcher field accept the prefix
  and a selection keep Cut. The GPUI launcher's fields keep the chord whatever their selection (GPUI Kit's own Cut on a
  Mac, the text-field row on Linux), so in the launcher a sequence begins only from a control that is not a text field.
  Catching Kit's Cut in the capture phase for the launcher's fields alone needs an additive `TextInput` builder and the
  launcher's fields to use it, more than a few lines, so it is left for its own change.

- **Where New shows a quick launch's banner.** The needs-more-choices, number-key and sequence banners, and a quick
  launch's refusal, are drawn in the launcher's notices region beside its actions, where its refusals go, rather than at
  the top of the body as the old `.partial-template-banner` was (the launcher area asked for that region, so the
  launcher keeps one place for what it has to say). The banner keeps the old look: a warning-colored edge, the raised
  background, the words in bold. Every banner stays until the draft is edited; the number-key banner's six-second expiry
  comes with the number keys. A refusal inside the open launcher, which SPEC.md puts beneath the search, is in the same
  region until the composer search exists. The banner or refusal is drawn first in that region, as its own element,
  above a press's reason, the helm's words or the YOLO question, which it never hides. Tested by
  `templates::dispatch_tests::an_incomplete_mark_opens_new_with_its_choices_and_banner`,
  `a_refused_mark_opens_new_unchanged_with_the_refusal` and `a_shortcut_refusal_shows_over_a_press_reason`.

- **Hidden quick-launch preparation does not block the page.** On the page a quick launch first has the launcher read
  its model catalog out of sight, the one read a press waits for (helm-local, usually a frame or two), and the
  launcher's slot holds it meanwhile, so New, the template shortcuts and the sequence prefix treat the launcher as busy.
  SPEC.md has hidden preparation "block page input without focusing its hidden fields"; pointer input on the page is not
  blocked, and a click or key during the read counts as moving on, so the completion falls back as SPEC.md says (the
  session appears without being opened, or New is offered by a notice). This is the templates goal's default for a
  hidden preparation; blocking the page would need a layer that draws nothing and takes no focus.

## Superseded

- **The phone-width one-pane layout** (below 620 pixels wide: the list or the editor, with a "‹ Templates" back link).
  The desktop window cannot be made narrower than 720 pixels, so the layout has no place; SPEC.md's sentence was
  reworded when the dialog was ported (decided by the migration's orchestrator on 2026-11-14).
- **Retry beside a failed template list.** The old dialog read the list itself and offered Retry when the read failed.
  The GPUI app's templates are a collection the helm pushes and pushes again on every change, so there is nothing for
  the dialog to ask for: it says "Templates unavailable:" with the helm's words and keeps showing the last templates.
  Tested by `templates::dialog_tests::an_unavailable_list_says_so_and_keeps_the_last_templates`.
