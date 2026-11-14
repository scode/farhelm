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
