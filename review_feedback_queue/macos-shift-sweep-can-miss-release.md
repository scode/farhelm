# A macOS Shift sweep can keep running after release

Reviewed commit: 1e7e2030cf0a3ddf9ae4b60ece65aedbbdfa8f3d

## TLDR

If the Control modifier changes during a Shift sweep on macOS, Farhelm can miss the end of the drag. At the edge of the
sidebar, the view can keep scrolling and change the marked range after you release the mouse.

## Details

GPUI classifies the Control modifier independently on button down and up. If it is held only at release, the release of
the physical primary button can arrive as a secondary release. The sweep uses the primary release to settle the
selection and stop edge scrolling. Keep release handling tied to the button that began the gesture.

At `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `vendor/gpui-pre-macos/src/window.rs:2898–2951,2967–2992`,
`crates/gui/farhelm-gui/src/session_list/pin_drag.rs:960–985,1211–1256`, and `SPEC.md:2273–2278`.

Suggested bucket **other**. Source-confirmed for the macOS modifier case by g063/l8. A separate Wayland concern about
the window missing a release altogether remains unverified after investigation.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g063/l8.
