# Light theme primary and danger buttons have identical hover and held colors

Reviewed commit: b45cdddfe77e1bc87efeacdc9d25e19092746cb7

## TLDR

In Light appearance, pressing and holding a primary or destructive button produces no visual change from hovering it,
because both states use the exact same color.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/theme.rs:343`.

Commit b45cdddfe77e1bc87efeacdc9d25e19092746cb7 added primary_button_hover and danger_button_hover for resting hover
fills while mapping Button::held_fill (crates/gui/farhelm-gui/src/components/button.rs:850-861, 904-905) to
accent_fill_hover and danger_fill_hover. In DARK (crates/gui/farhelm-gui/src/theme.rs:252-253, 265, 286), hover and held
differ (0x5c72da vs 0x495fc7 for Primary; 0xb44757 vs 0x983644 for Danger). In LIGHT
(crates/gui/farhelm-gui/src/theme.rs:343-344, 356, 377), primary_button_hover and accent_fill_hover are both
rgb_token(0x4059c7), and danger_button_hover and danger_fill_hover are both rgb_token(0x983644). Meanwhile,
the_held_fill_belongs_to_available_buttons_at_rest (crates/gui/farhelm-gui/src/components/tests/button.rs:485-536)
states "The held tint must stay distinct from the hover tint in both palettes", but only asserts
assert_ne!(palette.button_hover, palette.button_held) at line 493 and never asserts assert_ne!(hover, held) inside the
per-tier loop (lines 494-541), so the Light-theme collision went uncaught. Fix: give Light primary/danger hover and held
states distinct colors (keeping WCAG contrast against on_fill, and keeping accent_fill_hover/danger_fill_hover
appropriate for selections/banners or using dedicated held tokens), and assert assert_ne!(hover, held) for every tier in
the_held_fill_belongs_to_available_buttons_at_rest. Verify by adding assert_ne!(hover, held) in the loop and confirming
it fails on current LIGHT values.

Reviewer metadata (g34_button_restyle_core/f2): suggested bucket: other (minor visual feedback gap in Light appearance
plus a gap in the unit test asserting distinct hover/held colors; no security, data-loss, or material functional
degradation).; confidence: confirmed (source analysis of theme.rs:252-253, 265, 286, 343-344, 356, 377;
button.rs:850-861, 904-905; components/tests/button.rs:485-536).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g34_button_restyle_core/f2`.
