# Doc comment on ticker::reduced_motion misstates outline_fade's reduced-motion behavior as showing its final state

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

The doc comment explaining why `ticker::reduced_motion` still exists contradicts the actual behavior of the
permission-escalation outline, which could mislead a future change into removing the reduced-motion check and breaking
the static red outline.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/ticker.rs:366`.

In `crates/gui/farhelm-gui/src/ticker.rs:366-368`, the doc comment added by `6f1ea749a9e455f05302092a092fc3514bf0d227`
says: `the permission control's fading outline, for example, shows its final state at once under reduced motion.` In
fact, `outline_fade`'s final state (progress = 1.0) is completely transparent (`color.opacity(0.)`), which is what
GPUI's `AnimationElement` would draw on its own. The entire reason `components::outline_fade`
(`crates/gui/farhelm-gui/src/components/transition.rs:124-142`) must still read `ticker::reduced_motion(cx)` is that
under reduced motion it holds its initial full-color outline for as long as the banner is shown (`SPEC.md`, "Creation":
"Reduced motion replaces slide and fade with a static outline while the banner is shown"; `SPEC_impl.md`, "Animation
time: the ticker": "while the outline keeps its first, full color for as long as it is drawn"). Verification: compare
`crates/gui/farhelm-gui/src/ticker.rs:366-368` with `crates/gui/farhelm-gui/src/components/transition.rs:124-142` and
`SPEC_impl.md:1367-1370`. Fix: update the doc comment on `ticker::reduced_motion` to say that `outline_fade` holds its
initial full-color border rather than letting GPUI jump to the faded-out final state.

Reviewer metadata (g27_reduced_motion_gpui_and_core/f2): suggested bucket: other (internal doc comment inaccuracy with
no security, data-loss, or immediate user-visible impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g27_reduced_motion_gpui_and_core/f2`.
