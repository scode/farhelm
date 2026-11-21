# Escape or Enter can record upgrade highlights before the dialog reaches the screen

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Escape or Enter can mark upgrade highlights seen before they appear on screen.

## Details

SPEC.md:837–845 defines the release record and what Escape does to the dialog, and SPEC.md:5500–5505 requires delivered
actions to use the frame the user was shown. An eligible Show response pushes the layer and focuses its start
immediately (crates/gui/farhelm-gui/src/whats_new.rs:73–81,125–164 and crates/gui/farhelm-gui/src/layers.rs:1244–1295).
GPUI can dispatch the next key using a dirty frame it just drew without presenting it
(vendor/gpui-pre/src/window.rs:6503–6509, compared with the platform draw in present at lines 3887–3902). The root
Escape binding calls dismiss_top (crates/gui/farhelm-gui/src/actions.rs:407–414 and
crates/gui/farhelm-gui/src/focus.rs:801–805). dismiss_top selects the live stack's top layer and invokes its hook with
no presentation check (crates/gui/farhelm-gui/src/layers.rs:1654–1660,1680–1710). The key interceptor advances the input
mark before dispatch, but that mark was checked only when the layer opened (actions.rs:515–529 and layers.rs:1244–1248).
The highlights hook takes the stored version and immediately submits the preference write
(crates/gui/farhelm-gui/src/whats_new.rs:87–105,133–143). A fresh Escape dispatched between the asynchronous open and
presentation can therefore count unseen notes as seen. A fresh Enter or Space can do the same through the immediately
focused Got it button (crates/gui/farhelm-gui/src/whats_new.rs:129,151,349–351). The enabled button installs its click
handler for those keys (crates/gui/farhelm-gui/src/components/button.rs:1034–1045,1349–1352 and
crates/gui/farhelm-gui/src/components/mod.rs:425–440), with no presentation guard. The release write goes through the
same dismissal hook. The later lookup treats the stored version as current
(crates/farhelm-desktop/src/whats_new.rs:63–73). This uses no competing prompt and is separate from l1. The hook and
Escape path remain at the pinned range end. Gate the recording dismissal on first presentation of this layer, using the
existing one-shot presentation signal in crates/gui/farhelm-gui/src/prompt.rs:1149–1157. A dismissal refused before
presentation should leave the record alone. The existing confirmation fixture illustrates the distinction between the
drawn and shown frame (crates/gui/farhelm-gui/src/prompt_tests.rs:3183–3215).

Confidence: confirmed by source inspection, with the key arriving after the layer is installed and before its first
presentation.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `upgradeUI/l10`.
