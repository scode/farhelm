# Parity: Desktop integration and Linux packaging

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Desktop window chrome, Installation and updates, Desktop Quit, Status.

| Old spec                           | Tests | Covers                                                                   | Status         |
| ---------------------------------- | ----- | ------------------------------------------------------------------------ | -------------- |
| `tests/window-chrome.spec.ts`      | 6     | the macOS inset belongs to the merged top row and leaves details compact | not yet ported |
| `tests/update-readout-css.spec.ts` | 3     | version hint brightens and turns on hover and keyboard focus             | not yet ported |
| `tests/notifications.spec.ts`      | 12    | session notifications                                                    | not yet ported |
