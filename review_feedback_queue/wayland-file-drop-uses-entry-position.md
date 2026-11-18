# Wayland can upload a file where a drag entered the window

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Dragging across a terminal and releasing on the sidebar can upload the file to the session host if Wayland is still
reading the drag's file list when the pointer moves.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R2 Sol Wayland drop position (F).

Source references: `vendor/gpui-pre-linux/src/linux/wayland/client.rs:474–509, 2930–2995, 3008–3013`,
`vendor/gpui-pre/src/window.rs:6148`, `crates/gui/farhelm-gui/src/terminal/attached.rs:2431`.

Enter starts an asynchronous URI-list read and captures the entry point. Motion before that read finishes returns
without updating the drag position. If the pointer stops outside the terminal before the read completes, Drop carries
the cached entry coordinates. GPUI treats them as the release point and can call the terminal upload handler and later
insert the attachment path.

Sol rated the route confirmed and critical. Opus verification F confirmed it and rated it noncritical given the short
producer-response window and the existing vendored backend behavior. Highest follows from an established route that
sends a local file across the host boundary without a drop there. FILTER.md's sub-second filter excludes trust-boundary
effects.

Keep the latest Motion coordinates during the pending read and use them when completing the offer. Verify enter over
terminal, motion outside before list delivery, then release after delivery with no intervening motion.
