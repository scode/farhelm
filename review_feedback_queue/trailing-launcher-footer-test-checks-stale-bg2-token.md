# Launcher footer test checks stale bg_2 token instead of button_rest for neutral fill

Reviewed commit: 82eca873667133b1ff0e81efb6bcc00b2c3ef3bc

## TLDR

The test that guards against the session launcher's Reset button taking a neutral button fill no longer catches that
regression, because it still checks for the pre-restyle background color rather than the current neutral button fill.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view_tests.rs:536`.

Commit b45cdddfe77e1bc87efeacdc9d25e19092746cb7 changed Tier::Neutral's resting fill in
crates/gui/farhelm-gui/src/components/button.rs:810 from palette.bg_2 to palette.button_rest (translucent
rgba_token(0xffffff11) in DARK and rgba_token(0x1b21300f) in LIGHT). Commit 82eca873667133b1ff0e81efb6bcc00b2c3ef3bc
updated the_footer_keeps_launch_primary_and_reset_quiet in crates/gui/farhelm-gui/src/launcher/view_tests.rs:491-526 for
the restyle's removal of button outlines, but left lines 536-543 asserting that Reset has no quad with
quad.background == Background::from(theme::color(palette.bg_2)) under the message "{appearance:?}: Reset does not take
the neutral fill". Because Tier::Neutral no longer paints palette.bg_2, switching Reset to Tier::Neutral (or any filled
button tier) would still pass this assertion. Fix: assert that Reset paints no resting fill quad at all (or specifically
does not paint palette.button_rest / has no visible background alpha at its bounds), matching Tier::Ghost's contract.
Verify by temporarily switching launcher-reset to Tier::Neutral and confirming the test fails.

Reviewer metadata (g34_button_restyle_core/f1): suggested bucket: other (test-only regression gap; no security,
data-loss, or direct runtime UX consequence).; confidence: confirmed (source analysis of button.rs:810,
theme.rs:243-244/331-332, and launcher/view_tests.rs:536-543).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g34_button_restyle_core/f1`.
