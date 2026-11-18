# A new Enter after a quick-launch sequence can be mistaken for the original press

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

After using Enter to start a quick-launch sequence, you can press Enter again in the still-open terminal while the
launch is pending. The launch can then open its own session as if you hadn’t used the terminal in the meantime.

## Details

Bucket: **high**. GUI migration final whole-tree review, R2 Opus 4.

Source references: `crates/gui/farhelm-gui/src/actions.rs:496–502`,
`crates/gui/farhelm-gui/src/templates/armed.rs:337–339, 353–360, 400–420`,
`crates/gui/farhelm-gui/src/focus.rs:331–362`, `crates/gui/farhelm-gui/src/templates/dispatch.rs:11–38`.

The quick-launch pill is display-only. Its Enter disarms the capture and dispatches the sequence, leaving terminal focus
in place until the launch result. The interceptor records key presses before capture. The next fresh Enter on the same
focus handle compares equal to last_key, so note_press leaves the moved-on count unchanged even when the key reaches the
program. That equality check has no time bound. SPEC.md lines 3268–3275 excludes repeats of the same submit gesture from
moving on. A later press delivered to the terminal is a different action.

The review manifest attributes this to Opus as R2-4, but the relayed Opus file has no R2-4 section. This route is from
the manifest and a source trace. The original confidence, suggested bucket, and any independent verdict are unavailable.
Source confidence is likely, pending an end-to-end check that the terminal stays focused through the pending quick
launch. This is separate from repeats leaking into the terminal after dismissing a sequence.

Count a fresh press delivered to the terminal as moving on from the sequence's submit, using gesture ownership or a
focus epoch. Preserve suppression for a true submit repeat. Test a delayed launch with a released and pressed Enter in
the original terminal before the answer.
