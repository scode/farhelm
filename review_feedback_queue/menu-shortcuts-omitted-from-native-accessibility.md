# Native menu commands omit shortcut metadata

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The new menu shortcut labels do not reach the native accessibility shortcut fields.

## Details

MenuItem draws the chip as a separate text child and gives the command a plain accessible name, passing the chord only
to aria_keyshortcuts (crates/gui/farhelm-gui/src/components/menu_item.rs:295–301, 324–326, 385–390). GPUI copies that
property to AccessKit's keyboard_shortcut (vendor/gpui-pre/src/elements/div.rs:3625–3636). The pinned AccessKit macOS
adapter (0.26.3, src/node.rs) exposes NSAccessibilityMenuItemRole, but has no reader of keyboard_shortcut or native menu
accelerator selector. The pinned Linux adapter (accesskit_atspi_common 0.19.1, src/node.rs:1027) emits an empty AT-SPI
action key_binding. These versions are pinned by Cargo.lock and remain in use at
470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e. Wire the native shortcut fields or add an explicit hint to command text
surfaced by the adapters. Reduced consequence: the missing command metadata is confirmed, while individual assistive
tools may present descendant text separately. This source-only pass does not claim the chip can never be read or predict
spoken output. The initial glyph-encoding suspicion is superseded because neither pinned adapter reads the property.

Confidence: confirmed by inspection of both native metadata paths.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `menu/l1`.
