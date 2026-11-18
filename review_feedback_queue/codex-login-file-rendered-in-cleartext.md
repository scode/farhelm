# Choosing a Codex login file shows its tokens on screen

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

The Codex credential editor shows the entire selected login file, including live access and refresh tokens. Someone
looking at the screen or a recording can see them.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R6 Opus 2.

Source references: `crates/gui/farhelm-gui/src/settings/credentials/editor.rs:129–136, 376–381`.

The editor puts the text of a chosen auth.json into a growing, unmasked textarea. It can be selected and copied. Unlike
pasting visible text, choosing a file can expose a secret the user has never opened on screen.

Opus rated this source-confirmed and noncritical because the older browser app also used a plain textarea. There is no
separate verification verdict. Highest reflects the established on-screen credential disclosure on the file-selection
path. SPEC.md lines 2333–2342 calls the field write-only and permits pasted or selected JSON. It explicitly protects
previously saved values. It does not say choosing a file should reveal the new value.

Keep the staged secret hidden. For a chosen file show its name and validation state, and keep replacement or clear
available without displaying or copying its contents.
