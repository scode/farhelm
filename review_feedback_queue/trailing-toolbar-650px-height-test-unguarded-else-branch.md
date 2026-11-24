# 650px environment toolbar height test passes even if the row never condenses or Stop has a wrong height

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

The layout test meant to guard the condensed environment button's 23-pixel height in a 650-pixel session pane does not
actually assert that the row condensed or that the neighboring action buttons have a valid height, so a regression that
keeps ordinary heights or breaks Stop's height can slip past it.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_pane/header_tests.rs:3046`.

Introduced in e89dea16c5e37ec6f98bd69a30cd0d307134e543. At 1b3fe49af86979aebc2719965682a17f8c6366ac,
`an_environment_sessions_actions_stay_whole_in_a_650_pixel_pane`
(`crates/gui/farhelm-gui/src/session_pane/header_tests.rs:2951-3067`) states in its doc comment (lines 2956): "The
condensed environment button keeps its 23-pixel height, vertically centered with the 21-pixel actions." However, lines
3046-3054 write:

```rust
let expected_height = if stop.size.height == px(21.) {
    px(23.)
} else {
    px(26.)
};
assert_eq!(
    bounds.size.height, expected_height,
    "the environment control keeps its layout's height ({name})"
);
```

Neither `assert_whole_on_one_line` (`header_tests.rs:2189-2241`) nor this test asserts `stop.size.height == px(21.)`
(nor `stop.size.height == px(26.)` in the `else` branch). As a result: (1) if a change stops the 650 px environment
toolbar from condensing at all (leaving Stop and Suspend/Resume at 26 px), the test silently takes the `else` branch and
passes without exercising the 23 px condensed environment button height; and (2) if Stop regresses to any height other
than 21 px (for instance 23 px) while the environment control is 26 px, the `else` branch still passes. No other unit
test asserts the 23 px text-button height of `Condensed::Padding` (`components/tests/button.rs:838-853` only tests
`Condensed::Padding` with a fixed 16 px custom child box at 22 px total height, not text height). How to verify: read
`header_tests.rs:2189-2241,2951-3067` and `components/tests/button.rs:823-857`. Fix: assert
`assert_eq!(stop.size.height, px(21.), ...)` (or if both layouts are ever intended in this loop, match
`(stop.size.height, bounds.size.height)` strictly against `(px(21.), px(23.))` and assert that the condensed case is
witnessed at least once, rather than using an unconstrained `else`).

Reviewer metadata (g35b_button_restyle_toolbar_tests/f2): suggested bucket: other (test-quality weakness in a unit test;
no direct user impact until a future regression occurs; cannot reach highest).; confidence: confirmed; possible cover:
none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g35b_button_restyle_toolbar_tests/f2`.
