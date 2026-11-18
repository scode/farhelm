# A quick Wayland drop can disappear without a notice

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Releasing a file over a terminal before the file manager has returned its list can do nothing. Farhelm gives no
indication that it missed the drop.

## Details

Bucket: **other**. GUI migration final whole-tree review, R2 Sol Wayland early drop.

Source references: `vendor/gpui-pre-linux/src/linux/wayland/client.rs:474–509, 2948, 3026`.

Drop invalidates the pending URI read before looking for the drag's window. Before that read finishes there is no drag
window to submit a drop to, and the eventual read rejects its old generation. No upload path gets a chance to show an
error. The local file stays intact.

Sol rated this source-confirmed and noncritical. There is no separate verification verdict. FILTER.md's rare,
self-correcting filter is possible cover for a one-off quick drop that works on retry. A consistently slow producer can
miss ordinary drops without a visible failure, so full coverage is unclear. SPEC.md “Attachments” asks for visible
failures.

Hold a pending drop for its matching offer until the list resolves, with the latest coordinates, or report why the drop
failed. Check release-before-read and genuine cancellation separately.
