# Sound preview creates dead areas in the toggle row

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The Play overlay leaves parts of its sound switch row unclickable.

## Details

crates/gui/farhelm-gui/src/settings/choices.rs:658–696 draws a title and description and overlays a 24 px Play button.
The absolute wrapper spans the full row height and blocks mouse input independently of the button.
crates/gui/farhelm-gui/src/settings/card.rs:103–127 gives the title and description 18 px and 17 px line heights plus a
gap, so the wrapper extends beyond the centered button. The GPUI contract in vendor/gpui-pre/src/elements/div.rs:778–784
and vendor/gpui-pre/src/window.rs:870–889 hides a hitbox behind that wrapper. The switch's full-row handler is in
crates/gui/farhelm-gui/src/components/checkbox.rs:291–301,330–333. The wrapper creates a blocking hitbox even without
listeners (vendor/gpui-pre/src/elements/div.rs:2471–2479,2494–2497), and hit testing stops ordinary hover at the first
such box while retaining the scroll path (vendor/gpui-pre/src/window.rs:1201–1222). The source test at
crates/gui/farhelm-gui/src/settings/tests.rs:1111–1133 clicks Play itself and checks the emitted sound and unchanged
preference. It does not exercise the dead strip. Give only the button-sized element the blocking hitbox. Introduced in
7271212a1bcdfe35ffc4df6193412e1debcbcfc5. The later list-card change a518858b709e264d369c901fe89bb3186c056b6d adds a
shared agent mark and configurable radio card columns but leaves the preview's overlay and Checkbox intact at the pinned
tip.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `cards/l1`.
