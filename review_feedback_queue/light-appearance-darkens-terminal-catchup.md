# Light appearance darkens catch-up text on a dark terminal

Reviewed commit: afb41f43c8ea35c3a7333e06045ad11cf1702534

## TLDR

Light appearance makes the terminal's catch-up explanation dark even though the terminal stays dark. The explanation can
be difficult to read while an ordinary terminal attachment is catching up.

## Details

Suggested bucket: **other**. Confidence: **confirmed stylesheet and ordinary render route**. Display duration and
practical visual impact were not measured; no session failure or user-work loss is claimed.

`crates/farhelm-ui/assets/app.css:371–377` keeps `--terminal-bg` at `#282c34` but changes `--fg-dim` to `#414b5f`. The
transparent `.terminal-connecting` placeholder uses that foreground (`7092–7104`) over the terminal pane's dark
background (`7513–7520`). `src/session_view.rs:2765–2789,2827–2848` and `assets/terminal.js:2482–2507` establish the
ordinary attachment route. The separate reconnecting card does not provide this placeholder's background.

Still present at review tip `98bb15850f5f5ba1df6ed66873338261c45ff948`: CSS `371,377,7176–7188,7590–7604`. Use a
foreground intended for the permanently dark terminal rather than the surrounding interface palette. Verify the actual
placeholder surface in both appearances.

Possible cover: the Steel design accepts a permanently dark terminal, without accepting unreadable catch-up prose. No
exact Planned, BUGS, queue or ledger coverage was found. The rare-glitch filter does not fully match the ordinary
attachment trigger. Proposed drop: none.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g023/f2`, retained after later
CSS and shared-menu changes were checked.
