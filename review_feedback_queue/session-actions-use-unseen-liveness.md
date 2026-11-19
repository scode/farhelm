# The launcher takes broader consent from unseen updates

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

The Restart with and Replace with launcher can authorize stopping work beyond its displayed warning. The remaining work
is the launcher. The row menu and interrupted card now carry their shown state into the request.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R1 Sol 2 + Opus 1.

Source reference at the reviewed commit: `crates/gui/farhelm-gui/src/launcher/view.rs:1293–1297, 1306–1314`.

Restart with and Replace with compute the warning's guard again at the press. The launcher can then ask a separate YOLO
question. Each affirmative answer needs the guard of the warning the launcher displayed at that answer, including when
the request waits for a host setting or a checkout preview.

The bucket stays highest because the guard can authorize terminating work the launcher did not show. Keep the warning
live and the supervisor's execution check. The accepted split and line budget are in
`plans/queue/drawn-frame-actions.md`, "Decisions".
