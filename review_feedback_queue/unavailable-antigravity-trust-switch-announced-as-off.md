# An unavailable Antigravity trust switch is exposed as off without a disabled accessibility state

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Accessibility metadata reports an unread trust setting as off without marking its switch unavailable.

## Details

settings/configured.rs:90–116 makes Reading and Unreadable unavailable and uses false only as the disabled control’s
visual fallback. settings/agent_trust.rs:432–443 passes both values. components/checkbox.rs:275–285 always exports the
fallback as `Toggled::False`. Its disabled path dims the mark, omits focus and click handling and does not export a
disabled flag (:246–252, :308–333). The GPUI Div bridge writes toggled and available actions but no native disabled
metadata (vendor/gpui-pre/src/elements/div.rs:3625–3698). The switch is still an identified native Switch node. A failed
read can show a separate alert (settings/configured.rs:119–135), but that does not convey the switch state, and there is
no alert during loading. Export a disabled state for the unavailable switch and avoid presenting an unknown stored value
as known off in its semantic state.

Confidence: confirmed by inspection for the emitted node and actionable-state mismatch. Exact screen-reader phrasing was
not exercised. The shared Checkbox behavior affects the assigned Agent trust control at the pinned tip.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agUI/l17`.
