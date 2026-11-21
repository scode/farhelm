# The ntfy test no longer exercises the footer Save lock while Send test is pending

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The changed Notifications test can pass if Save wrongly becomes available on an edited form while Send test is pending.

## Details

The parent of 5c68c1c4386801657504080c65bb3b5eb13c1119, crates/gui/farhelm-gui/src/settings/tests.rs:3788–3799,
activates the visible Save while the test is pending and requires no additional command. The tip replaces those Save
presses with `!drawn(FOOTER_SAVE)` at :3915–3924. That form is already clean, as the prior save was acknowledged at
:3905–3913, so it has no Save regardless of Test's state. The dirty case at :3931–3945 answers the test immediately and
never examines the footer until after that answer. The edited implementation can distinguish those states:
settings/editing.rs:300–306 omits a clean form and :324–336 marks a dirty form's footer Save busy using `holds_save`.
Settings/ntfy.rs:464–467 reports the pending test. The companion in-flight save test (settings/tests.rs:3645–3666)
checks a different busy condition. Restore the witness on the dirty Send test branch by verifying the shown Save is busy
and issues no PUT before the test reply, then verifying an explicit Save is available for the same draft once the reply
arrives.

Confidence: confirmed by static inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footerTests/w2`.
