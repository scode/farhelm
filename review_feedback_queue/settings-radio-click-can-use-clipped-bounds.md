# A clipped Settings radio can receive an accessibility click at the wrong bounds

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The new Settings card geometry changes where an accessibility activation is aimed when a radio row is partly offscreen.
A failure on a supported native adapter remains unverified.

## Details

The card layout gives each option full-width padded bounds
(`crates/gui/farhelm-gui/src/components/radio_group.rs:580–599,640–647`). The shared GPUI action path uses the center of
those bounds (`vendor/gpui-pre/src/element.rs:428–460`, `vendor/gpui-pre/src/window.rs:7574–7594`). The original row and
the new card use different geometry. Static comparison did not establish whether a shipped accessibility adapter
dispatches Click on a partly clipped row before scrolling it into view, or whether the card geometry moves the center
outside its usable hitbox. That ordering is the missing premise for a click to be ignored or delivered incorrectly. The
independent review could not settle it from the pinned adapter or layout source.

Confidence: Unverified after investigation.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `previews/l7`.
