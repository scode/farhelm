# The Antigravity trust hint has a status role but no live accessibility announcement

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The new Antigravity trust hint emits no live announcement request for screen readers.

## Details

SPEC.md:1737–1745 requires the trust band to announce without taking focus, and :1765–1769 extends the shared hint to
Antigravity. trust_band.rs:378–407 adds an unfocused `Role::Status` wrapper, and :423–436 puts the actual text in a
child label. GPUI creates an AccessKit node with the role (vendor/gpui-pre/src/element.rs:431–450) and its Div metadata
bridge has no live-property write (vendor/gpui-pre/src/elements/div.rs:3625 onward). For the Cargo.lock-pinned AccessKit
consumer 0.38.0, node.rs:889–892 looks for the explicit property, then inherits, and defaults to Off. AccessKit macOS
0.26.3 node.rs:161,273 maps Status to a native group/subrole. Event.rs:236,299 gates the adapter’s announcement path on
a live value other than Off. The pinned AT-SPI 0.19.1 adapter likewise reflects Off as no politeness (node.rs:480–486).
Thus the rendered status role alone does not request a live announcement. Current tests at trust_band_tests.rs:383–432
check the role, text and retained focus and do not check the emitted live semantics. Wire a labeled, nonfocused live
status into the native accessibility path and verify the emitted node properties. Shared band behavior predates this
agent; Antigravity’s addition exposes the same omission in the assigned scope, with no later fix found so far.

Confidence: confirmed by inspection for the emitted metadata and macOS adapter gate. Actual speech/AT behavior was not
exercised.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agUI/l15`.
