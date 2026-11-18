# Enter can reach the terminal after dismissing a sequence or answering a card

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Holding Enter to dismiss an empty quick-launch sequence or answer an approval can submit a line already sitting in the
terminal. A second quick Enter can also reach the terminal after an approval card disappears.

## Details

Bucket: **high**. GUI migration final whole-tree review, R2 Sol held sequence Enter (I) + R3 Opus 2 (L).

Source references: `crates/gui/farhelm-gui/src/templates/armed.rs:337–355, 415`,
`crates/gui/farhelm-gui/src/actions.rs:496–502`, `crates/gui/farhelm-gui/src/layers.rs:1809, 1844`,
`crates/gui/farhelm-gui/src/focus.rs:214–262, 331, 509–518, 769, 1660, 1732–1738`,
`crates/gui/farhelm-gui/src/approvals/card.rs:704` at the reviewed commit (main
`crates/gui/farhelm-gui/src/approvals/card.rs:705`), `crates/gui/farhelm-gui/src/approvals/region.rs:405, 453`,
`crates/gui/farhelm-gui/src/terminal/set.rs:397` at the reviewed commit (main
`crates/gui/farhelm-gui/src/terminal/set.rs:400`), `crates/gui/farhelm-gui/src/terminal/attached.rs:1376–1388, 1926`,
`crates/farhelm-helm/src/approvals.rs:345, 876`, `crates/gui/farhelm-input-mapping/src/keys.rs:116, 259`.

The empty sequence consumes its first Enter and drops its display-only pill without registering a closing press. Focus
remains on the terminal. Auto-repeat is then ordinary terminal input. On an independent approval card, Enter sends the
answer and the card disappears on the helm's update. Focus recovers to the selected terminal (including a shell tab),
but a card is outside the layer-close path that registers repeat suppression. That allows a held repeat and, after focus
recovers, a second tap.

Sol rated the empty-sequence route source-confirmed and critical. Opus verification I confirmed it and rated it
noncritical because it needs holding the key past its repeat delay and submits the user's own line. Opus reported the
card route as likely and generally noncritical, with a stronger possible consequence for an agent permission prompt. Sol
verification L rated it critical and traced an ordinary shell with an unsubmitted command, so no particular agent prompt
is needed. The manifest records the disagreement and the maintainer's decision to queue both together at high.

Extend gesture-owned repeat suppression to the empty-sequence dismissal and to keyboard answers whose controls
disappear. Preserve focus recovery and unrelated input. For a card answer, suppress held repeats and same-key taps
within the existing 500 ms interval. Test the real answer, card removal and terminal delivery. SPEC.md lines 3289–3295
permits terminal focus recovery and some unexpected keys. Its separate repeat fence at lines 3310–3312 names dialogs and
menus. Whether that acceptance reaches this gesture is unclear.
