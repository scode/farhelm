# session-toolbar golden scene height (104 px) overflows and clips on macOS

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

The screenshot golden for the session toolbar is sized only for Linux's 30-pixel top row, so capturing it on macOS clips
the top and bottom of the taller 48-pixel title-band rows without failing its readiness check.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/goldens/mod.rs:795`.

Introduced in 04dff94006d5625581f5e47d63f5c5fff557a936. At 1b3fe49af86979aebc2719965682a17f8c6366ac,
`crates/gui/farhelm-gui/src/goldens/mod.rs:793-798` registers `session-toolbar` with fixed window size `(1428., 104.)`.
`SceneView::render` (`crates/gui/farhelm-gui/src/goldens/mod.rs:183-191`) wraps the scene in a centered flex container
with `p(px(12.))` (24 px vertical padding), and `session_toolbar`
(`crates/gui/farhelm-gui/src/goldens/session_pane.rs:78-96`) stacks two `SessionPane::golden_header` rows with
`gap(px(12.))`. In `crates/gui/farhelm-gui/src/session_pane/header.rs:561-565`, each header row uses
`min_h(title_band::HEIGHT)` when `title_band::ON` is true (`cfg!(target_os = "macos")`,
`crates/gui/farhelm-gui/src/desktop/title_band.rs:46-50`), where `title_band::HEIGHT` is `px(48.)` instead of Linux's
`ROW_MIN_HEIGHT` (`30.`). On Linux the content plus padding is `30 + 12 + 30 + 24 = 96 px <= 104 px`. On macOS the two
rows plus gap alone are `48 + 12 + 48 = 108 px > 104 px` (and `132 px` with `SceneView`'s 24 px padding), so centering
in a 104 px window clips 2 px off the top of the ordinary row and 2 px off the bottom of the condensed row with zero
outer margin. Because the 26 px / 21 px buttons remain vertically centered inside each 48 px row,
`session_toolbar_unready` (`crates/gui/farhelm-gui/src/goldens/session_pane.rs:101-152`) only checks button bounds and
does not catch the clipped rows. How to verify: inspect the heights in `goldens/mod.rs:188,795`,
`goldens/session_pane.rs:82`, `session_pane/header.rs:561-565`, and `desktop/title_band.rs:46-50`, or run
`scripts/check-gui-goldens.py --scene session-toolbar` on macOS. Fix: derive the `session-toolbar` scene height from
`title_band::ON` (e.g., `if crate::desktop::title_band::ON { 140. } else { 104. }`, or check `session-header` row bounds
in `session_toolbar_unready`).

Reviewer metadata (g35b_button_restyle_toolbar_tests/f1): suggested bucket: other (maintainer golden-test tooling only;
macOS goldens are supported by `scripts/check-gui-goldens.py:318` and `goldens/desktop.rs:73-98`, though
`tests/goldens.txt` currently tracks only Linux captures; cannot reach highest).; confidence: confirmed; possible cover:
none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g35b_button_restyle_toolbar_tests/f1`.
