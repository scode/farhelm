# Parity: Templates and quick launch

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Creation (the Templates dialog, quick launch, number keys, launcher sequences and the Cmd+J template list).

| Old spec                                    | Tests | Covers                                                              | Status         |
| ------------------------------------------- | ----- | ------------------------------------------------------------------- | -------------- |
| `tests/templates.spec.ts`                   | 48    | template bolts use a fixed outline or filled drawing for their mark | not yet ported |
| `tests/quick-launch.spec.ts`                | 30    | quick launch submits the wrapper prefix saved in its agent template | not yet ported |
| `tests/launcher-template-shortcuts.spec.ts` | 8     | back-to-back bolt and number chords stack on New without submitting | not yet ported |
| `tests/template-list-shortcut.spec.ts`      | 4     | Cmd-J exact name launches through the fresh quick-launch path       | not yet ported |

## Template cases of specs in other areas' files

Specs whose rows sit in another area's parity file but which have template or quick-launch cases this area ports. The
rows stay in those files; the cases and the tests replacing them are listed here.

| Old spec                                    | Its file                 | Template cases                                      | Status         |
| ------------------------------------------- | ------------------------ | --------------------------------------------------- | -------------- |
| `tests/quick-switcher.spec.ts`              | sidebar and session view | the `tl:` section and the Quick launch row          | not yet ported |
| `tests/github-checkouts.spec.ts`            | launcher                 | quick launch into a managed checkout                | not yet ported |
| `tests/custom-permissions.spec.ts`          | launcher                 | custom words in templates                           | not yet ported |
| `tests/terminal-create-idempotency.spec.ts` | launcher                 | quick-launch double submission                      | not yet ported |
| `tests/popover-template-movement.spec.ts`   | foundations              | the key menu closes when its row's template is gone | not yet ported |
| `tests/tooltip-coverage.spec.ts`            | foundations              | the Templates dialog's hover help                   | not yet ported |

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
