# Upgrade highlights can take focus after a startup sign-in prompt has closed

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Upgrade highlights can take focus after a startup SSH question has already opened and closed.

## Details

SPEC.md:837–842 requires the answer to leave focus and the release record alone if another prompt takes focus before the
highlights arrive. The lookup captures the original input mark, and the Show arm checks only whether a focus layer is
open at completion (crates/gui/farhelm-gui/src/whats_new.rs:32–48,73–81). A startup SSH question is eligible to open
with its own startup mark, receives focus through the popover (crates/gui/farhelm-gui/src/sign_in.rs:240–281,373–385 and
layers.rs:1269–1294), and closes programmatically when its question disappears (sign_in.rs:154–160,172–184). For a
security-key presence notice, the helm can drop the question when the helper finishes after the touch
(crates/farhelm-helm/src/ssh_sign_in.rs:426–446 and SPEC.md:5687–5697). Entering a layer does not advance the user-input
mark. That mark tracks keypresses and clicks (focus.rs:271–295,306–338,1624–1628). If the startup notice opens and
closes within an in-flight highlights read, the response sees no layer and an unchanged mark, so it opens the dialog.
The existing late-answer fixture keeps the intervening prompt open when the answer arrives (whats_new/tests.rs:164–213).
Capture a per-window focus-layer generation with the lookup and retire a Show answer when it changed. Add a fixture that
closes an intervening prompt without Farhelm input before releasing the answer. The same result guard remains at the end
of the reviewed range.

Confidence: confirmed by inspection, with the stated timing order.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `upgradeUI/l1`, `shots/l9`, `upgradeBackend/l5`,
`goldenRender/l3`, `upgradeSettings/l2`.
