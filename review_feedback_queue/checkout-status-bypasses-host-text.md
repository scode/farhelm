# Checkout status can bypass host-text rendering

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

A launcher status line can show repository-search errors without Farhelm’s usual treatment of invisible or right-to-left
characters.

## Details

Bucket: **other**. GUI migration final whole-tree review, R5 Opus DONE (checkout status).

Source reference: `crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1079–1095`.

The launcher draws the repository suggestion's error through `HostText.raw()`. The reviewer saw only helm-generated
words on this route. No misleading host-controlled error has been established.

Opus listed this as a minor observation and supplied no individual confidence or verification verdict. The rendering
path is confirmed by source. SPEC_impl.md's host-text presentation rules govern how to draw this value.

Render the repository error through the existing host-text element. Check that the status and its accessible text still
convey the full error.
