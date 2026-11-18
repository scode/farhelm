# Composed text can reach the terminal during quick launch

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Some input-method text typed for a quick-launch sequence goes to the underlying terminal while the sequence stays armed.

## Details

Bucket: **other**. GUI migration final whole-tree review, R2 Sol composition bypasses capture.

Source references: `crates/gui/farhelm-gui/src/actions.rs:496–502`,
`crates/gui/farhelm-gui/src/terminal/attached.rs:4099, 4186`, `vendor/gpui-pre-linux/src/linux/wayland/client.rs:2349`.

Quick-launch capture sees key-down only. A Wayland multi-character input-method commit can arrive as InsertText, bypass
the capture and flow through the terminal input handler to the attached program. Dead-key and composition commits can
use the same input-handler boundary.

Sol rated this source-confirmed and noncritical. There is no separate verification verdict. SPEC_impl.md “Actions and
keys” explains the capture cannot see such commits and says the text receiver can consult the armed slot. Its
quick-launch section explicitly records the gap as “not refused yet”. Neither passage states that leaking the text is
accepted product behavior.

Have the terminal receiver consult the armed capture before forwarding committed text. Verify a platform-style commit
with the pill active and with ordinary terminal input afterwards.
