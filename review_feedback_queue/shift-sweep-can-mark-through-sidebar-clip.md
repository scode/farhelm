# A Shift sweep can mark sessions through the clipped edge of the sidebar

Reviewed commit: 1e7e2030cf0a3ddf9ae4b60ece65aedbbdfa8f3d

## TLDR

In a narrow macOS window with the sidebar partly scrolled out of view, a Shift sweep can keep changing the marked
sessions when the pointer is beside the window.

## Details

The main shell scrolls horizontally and clips its children. The sweep hit test uses the sidebar list's layout bounds,
including the portion clipped by the shell. macOS can report held movement outside the visible window, and the release
uses the same layout bounds for its final mark. The usable sweep target needs to stay within the visible horizontal
bounds. The documented vertical extension remains useful above and below the list.

At `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `crates/gui/farhelm-gui/src/main_view.rs:199–230`,
`crates/gui/farhelm-gui/src/session_list/pin_drag.rs:1275–1293,1553–1572`, and `SPEC.md:2273–2278,2509–2516`.

Suggested bucket **other**. Source-confirmed from g063/l33. The selection remains visible and reversible.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g063/l33.
