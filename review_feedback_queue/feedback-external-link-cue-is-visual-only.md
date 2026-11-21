# The Discord link's new browser-opening cue is visual only

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The feedback dialog’s Discord link omits the browser-opening cue from its accessible description.

## Details

The patch adds the arrow to the painted label and overrides the accessible name with the original text
(crates/gui/farhelm-gui/src/feedback.rs:595–606). The generic link role and name identify Discord, but they do not
distinguish a system-browser handoff from in-app navigation. The hover help says who is available on Discord and omits
the browser behavior. Button supplies the accessible label (components/button.rs:1036–1042) and has no equivalent
external-browser description or destination property. This produces a new disparity in the cue, while the actual opener
and draft preservation remain correct per SPEC.md:4783–4784 at the reviewed commit. Add an accessible description such
as “Opens in your browser” while keeping the concise link name. The source confirms the link metadata gap. Actual speech
output is unverified.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `menu/l10`.
