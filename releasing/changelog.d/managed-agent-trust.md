---
kind: breaking
---

Workspace trust for Codex, Muse and Pi now comes from separate, off-by-default Settings switches for managed checkouts.
The launcher's trust choice and the CLI's `--trust` option are removed, and old template trust values are ignored.

Codex and Muse can show a hint beside their trust question that opens the corresponding setting. Pi's switch approves
project settings and extensions; it has no trust-question hint. Plain Restart and Replace keep the saved launch's trust.
