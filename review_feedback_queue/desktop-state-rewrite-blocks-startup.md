# The desktop can refuse to open over its state file

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

If its desktop settings file cannot be read, decoded or rewritten, Farhelm can stop at a startup alert while sessions
keep running in the background.

## Details

Bucket: **high**. GUI migration final whole-tree review, R7 Opus 1.

Source references: `crates/farhelm-desktop/src/lib.rs:700–706`, `crates/farhelm-desktop/src/state.rs:89–100, 115–136`,
`crates/farhelm-desktop/src/updater.rs:473–493`.

Startup forces an update_state rewrite before creating a window. It also strips device secrets stored by older app
versions. A malformed field, unreadable state or failed temporary write therefore takes the startup refusal path. This
review found the behavior survived the migration and is recoverable with state repair.

Opus rated it source-confirmed and noncritical. There is no separate verification verdict. SPEC.md “Healthy local
filesystems” explicitly accepts a full disk and local I/O failures preventing progress on the affected host. That covers
the failed-write example. The malformed-decode route remains, including the question of whether an older valid state can
trigger it under “Upgrade compatibility and client scale”.

Give usable state a path to open the window without a mandatory rewrite, or provide a repair path for invalid state.
Preserve the old-secret removal requirement when making the rewrite best-effort, as the updater's read already is.
