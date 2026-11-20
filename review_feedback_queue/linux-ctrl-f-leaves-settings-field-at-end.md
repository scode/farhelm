# Ctrl+F can move focus out of a Linux Settings field

Reviewed commit: 2b69c3e6cd4dedeb76658913396c5350461130cb

## TLDR

On Linux, pressing Ctrl+F at the end of a Settings text field can move focus to Search. The next keystroke can go into
the search query instead of the field.

## Details

Settings binds Ctrl+F to Search and its text fields bind the same chord to moving the caret. When a field has one caret
at the end and no selection, the Kit movement action propagates. GPUI dispatch can then deliver the chord to the
enclosing Settings handler. The field uses its normal caret behavior at other positions.

At reviewed tip `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `crates/gui/farhelm-gui/src/actions.rs:409`,
`crates/gui/farhelm-gui/src/components/text_input.rs:535`, `crates/gui/farhelm-gui/src/settings.rs:259` and
`vendor/gpui-pre/src/window.rs:7064`. The locked gpui-base 0.9.1 movement handler propagates the no-op in
`movement.rs:319–328`. `SPEC.md:2398–2402` reserves the chord for Linux fields that use it to move the caret.

Suggested bucket **high** for ordinary editing sending subsequent input to a different field. Source-confirmed by
trailing reviewers g019/l11 and g059/l8. The existing test exercises Ctrl+F away from the end.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g019/l11, g059/l8.
