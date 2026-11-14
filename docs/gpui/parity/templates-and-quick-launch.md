# Parity: Templates and quick launch

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Creation (the Templates dialog, quick launch, number keys, launcher sequences and the Cmd+J template list).

| Old spec                                    | Tests | Covers                                                              | Status                                                                                                                                                                                                             |
| ------------------------------------------- | ----- | ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `tests/templates.spec.ts`                   | 48    | template bolts use a fixed outline or filled drawing for their mark | partly: the list, summaries, ⚡ and number keys (`templates::dialog_tests`, `templates::shortcuts_tests`, `templates::summary_tests`); the editor, leaving, deleting, `tl:` and Save as template are still to come |
| `tests/quick-launch.spec.ts`                | 30    | quick launch submits the wrapper prefix saved in its agent template | not yet ported                                                                                                                                                                                                     |
| `tests/launcher-template-shortcuts.spec.ts` | 8     | back-to-back bolt and number chords stack on New without submitting | not yet ported                                                                                                                                                                                                     |
| `tests/template-list-shortcut.spec.ts`      | 4     | Cmd-J exact name launches through the fresh quick-launch path       | not yet ported                                                                                                                                                                                                     |

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

- **The number-key menu's own header and labels.** The menu draws its header (the template's name, cut short when long,
  over "Number shortcut") itself rather than through the shared menu header, which does not exist yet, and its key
  labels are in the interface face, not the old monospace. Disabled controls fade to half opacity, the component
  layer's, not the old 40%.
- **The ⚡ and key buttons while a write is in flight.** The old list dimmed them to 40% while busy; here they keep
  their look and focus and ignore presses (the button component's busy state), since a dimmed, disabled button gives up
  focus, which would fall to the dialog and lose the user's place. Tested by
  `templates::dialog_tests::a_write_in_flight_leaves_focus_and_the_dialog_alone`.
- **A recipe-and-place target's summary.** The old summary named a template's environment target by the environment
  code's label for it; until the launcher's environments work brings that label, the summary names the target's recipe
  ("New dev").

## Superseded

- **The phone-width one-pane layout** (below 620 pixels wide: the list or the editor, with a "‹ Templates" back link).
  The desktop window cannot be made narrower than 720 pixels, so the layout has no place; SPEC.md's sentence was
  reworded when the dialog was ported (decided by the migration's orchestrator on 2026-11-14).
- **Retry beside a failed template list.** The old dialog read the list itself and offered Retry when the read failed.
  The GPUI app's templates are a collection the helm pushes and pushes again on every change, so there is nothing for
  the dialog to ask for: it says "Templates unavailable:" with the helm's words and keeps showing the last templates.
  Tested by `templates::dialog_tests::an_unavailable_list_says_so_and_keeps_the_last_templates`.
