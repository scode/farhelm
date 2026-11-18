# Pasting copied files into a Wayland terminal can do nothing

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Copying a file in a Wayland file manager and pasting into Farhelm can paste its text representation or do nothing
instead of attaching the file.

## Details

Bucket: **high**. GUI migration final whole-tree review, R2 Sol Wayland file clipboard.

Source references: `vendor/gpui-pre-linux/src/linux/wayland/clipboard.rs:137, 302, 329`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:1631`.

Wayland MIME selection only accepts images and plain text. A file manager offering actual file references as
text/uri-list therefore never produces the clipboard file item Farhelm's file-first classifier expects. A file-only
offer is ignored.

Sol rated this source-confirmed and noncritical. There is no separate verification verdict. SPEC.md “Attachments”
distinguishes file references from text that happens to look like a path. The clipboard best-effort rule near line 3115
covers clipboard failure. A standard offer the backend does not read is a possible interpretation of that rule, but the
spec expressly describes file attachments from the clipboard.

Decode Wayland file-reference offers into the backend's file item before ordinary text or image fallback, without
treating typed paths as file references.
