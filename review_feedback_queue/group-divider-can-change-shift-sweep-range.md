# Sweeping across a group divider can change which sessions are marked

Reviewed commit: 1e7e2030cf0a3ddf9ae4b60ece65aedbbdfa8f3d

## TLDR

Shift-dragging across a group divider can move the end of the marked range, even though the divider is spacing between
session rows.

## Details

The sidebar renders the preceding session and its divider in one virtual list item. The sweep uses the full measured
item as a session hit target, including the divider. The specification defines the sweep in terms of session rows. A hit
on the divider can therefore select the adjacent session without touching its row.

At reviewed tip `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `crates/gui/farhelm-gui/src/session_list.rs:1148`,
`crates/gui/farhelm-gui/src/session_list/pin_drag.rs:1505`, and `vendor/gpui-pre/src/elements/list.rs:711`. Keep the
divider in list measurement and exclude its bounds from sweep hit testing.

Suggested bucket **other** for an incorrect marking preview. Source-confirmed by trailing reviewer g025/g025-19.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g025/g025-19, g023/l12, g063/f4.
