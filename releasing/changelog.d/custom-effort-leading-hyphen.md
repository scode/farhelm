---
kind: fixed
---

`farhelm agent create`, `farhelm spawn` and the template commands now refuse `--model` given without a value, instead of
taking the next option as the value: `--model --yolo` used to start a custom agent with `--yolo` as its model. A model
that starts with a hyphen now goes after `=`, as in `--model=-fast`.

This replaces the entry for an unreleased change that let `--effort` take a separate value starting with a hyphen
(`--effort -fast`), which made `--effort --yolo` start a custom agent with `--yolo` as its effort. That change shipped
only in the 0.46.0 release candidates and is reverted here, so `--effort` behaves as in the last stable release.
`--model` already took a separate hyphen-leading value in the last stable release, which is the bug this entry fixes.
