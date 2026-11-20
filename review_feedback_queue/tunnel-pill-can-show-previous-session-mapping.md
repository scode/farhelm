# Tunnel hover help can show the previous session’s ports

Reviewed commit: 873dd66563593db94464f39257448ff4f9557b10

## TLDR

If Farhelm automatically changes the open session while the pointer rests on a tunnel pill, the pill can show the new
session’s label alongside the previous session’s port mapping in hover help.

## Details

The new tunnel restore path can render a replacement session’s pill immediately from its pending-close snapshot. The
pill’s menu anchor follows the selected session, while the hover owner still uses one ID for its fixed position. A
still-open tooltip can therefore retain its old text even though the displayed pill has changed. The current menu
remains tied to the selected session.

At reviewed tip `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see
`crates/gui/farhelm-gui/src/terminal/tunnels.rs:179–214`, `crates/gui/farhelm-gui/src/terminal/tunnels.rs:927–948`, and
`crates/gui/farhelm-gui/src/layers/tooltip.rs:545–561`.

Suggested bucket **other** for stale port-mapping help. Source-confirmed by trailing reviewer g083/g083-13, with the
older general hover-owner behavior documented in g083/g083-8.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g083/g083-13.
