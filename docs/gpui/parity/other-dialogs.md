# Parity: Other dialogs

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Feedback, Welcome card, Announcements, Partial deletion.

| Old spec                           | Tests | Covers                                                                       | Status         |
| ---------------------------------- | ----- | ---------------------------------------------------------------------------- | -------------- |
| `tests/trash-dialog.spec.ts`       | 14    | trash list stays keyboard-scrollable while every list action is disabled     | not yet ported |
| `tests/feedback.spec.ts`           | 12    | feedback uses dark fields and pinned chrome at normal and short heights      | not yet ported |
| `tests/welcome.spec.ts`            | 7     | welcome dismissal by ${dismissal} is shared and survives reload              | not yet ported |
| `tests/announcements.spec.ts`      | 10    | announcement dismissal by ${dismissal} survives reload                       | not yet ported |
| `tests/keyboard-shortcuts.spec.ts` | 5     | the ${mac ? "Mac" : "non-Mac"} overview chord owns terminal input and labels | not yet ported |
| `tests/sounds.spec.ts`             | 6     | device sounds                                                                | not yet ported |
