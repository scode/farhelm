# Parity: Other dialogs

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Lifecycle operations, Feedback, Welcome card, Announcements, Partial deletion.

| Old spec                                  | Tests | Covers                                                                                                              | Status         |
| ----------------------------------------- | ----- | ------------------------------------------------------------------------------------------------------------------- | -------------- |
| `tests/trash-dialog.spec.ts`              | 14    | trash list stays keyboard-scrollable while every list action is disabled                                            | not yet ported |
| `tests/feedback.spec.ts`                  | 12    | feedback uses dark fields and pinned chrome at normal and short heights                                             | not yet ported |
| `tests/welcome.spec.ts`                   | 7     | welcome dismissal by ${dismissal} is shared and survives reload                                                     | not yet ported |
| `tests/announcements.spec.ts`             | 10    | announcement dismissal by ${dismissal} survives reload                                                              | not yet ported |
| `tests/quick-switcher.spec.ts`            | 8     | opening a template reports the launcher's catalog read failure                                                      | not yet ported |
| `tests/session-confirmations.spec.ts`     | 4     | restoring confirmations reports save failure and recovers                                                           | not yet ported |
| `tests/delete-precondition.spec.ts`       | 3     | an ${skip ? "opted-out tabs-only" : "unconfirmed"} delete sends the precondition and shows the supervisor's refusal | not yet ported |
| `tests/replace.spec.ts`                   | 18    | plain Replace retries an unknown outcome with the same key (${surface})                                             | not yet ported |
| `tests/restart-with.spec.ts`              | 42    | named restart with requires a target when no conversation was captured                                              | not yet ported |
| `tests/restart-replace-progress.spec.ts`  | 6     | plain Restart shows host and elapsed time until a real refusal arrives                                              | not yet ported |
| `tests/session-action-completion.spec.ts` | 3     | row Replace YOLO arrival respects ${focus} focus                                                                    | not yet ported |
| `tests/session-action-shortcuts.spec.ts`  | 4     | browser Delete chord shares header confirmation and restores terminal focus without forwarding input                | not yet ported |
| `tests/terminal-replay-rename.spec.ts`    | 19    | rename exposes peer title and refusal characters and seeds an escaped draft                                         | not yet ported |
