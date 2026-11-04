# Steady-block cursor contract conflicts with a deliberate documented departure

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Grid terminals deliberately blink a block cursor even when the program requests it steady, while the wire contract says
blinking is the default unless the program says otherwise.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed behavior,
confirmed explicit intent and confirmed differing contract text; which source should change is a maintainer
interpretation/decision, not a missing runtime premise.

Source locations: crates/farhelm-supervisor/src/grid_output.rs:42-48,341-355; crates/farhelm-proto/src/grid.rs:161-165
at 5c39b2cd67629ecf7c80b0bc9a053144d1aac011.

vendor/vte/src/ansi.rs:1755-1779 handles DECSCUSR parameter 2 as Some(CursorStyle { shape: Block, blinking: false });
:857-869 derives exactly that value as CursorStyle::default(). vendor/alacritty_terminal/src/term/mod.rs:2432-2438
stores the explicit style, but :1012-1023 returns only the resolved value. grid_output.rs:341-355 compares that resolved
value to CursorStyle::default() and sets blink true, conflating an explicit steady block with the absence of a
program-selected style. The assigned contract (5c39b2cd67629ecf7c80b0bc9a053144d1aac011 grid.rs module docs, screen
frame cursor paragraph) calls only an unstyled default cursor blinking.

Grid terminals deliberately blink a block cursor even when the program requests it steady, while the wire contract says
blinking is the default unless the program says otherwise. The intended exception needs to be reconciled with that
contract, not treated as an accidental unconsidered branch. Details/source basis: reviewed-commit and tip
grid_output.rs:42-48 explicitly call the steady-block blink an accepted difference and attribute it to the
emulator/default distinction and the snapshot's same blind spot. The actual equality fallback at :341-355 implements
that intent. The source-confirmed route from the earlier f1 remains: DECSCUSR 2 becomes Block/blinking=false, equal to
CursorStyle::default(), the resolved getter loses unset-versus-explicit distinction, and the focused renderer animates
blink=true. The assigned introducing contract at 5c39b2cd67629ecf7c80b0bc9a053144d1aac011 grid.rs:161-165 (tip :165-169)
defines a blinking default as one no program styled, and says the program may say otherwise. Tip SPEC.md:2080-2086
promises terminal fidelity. SPEC_impl.md:1706-1736 describes the grid client and named omissions without this exception;
:2517-2547 accepts specified query/timing differences and describes emulator parity, not this viewer cursor exception;
:3051-3057 delegates wire contract and bounds to farhelm-proto, not to grid_output's accepted-departure paragraph.

Possible coverage and caveats: grid_output.rs:42-48 is exact matching deliberate acceptance text and must be preserved
for triage; code module docs alone are not a named coverage source in review_feedback_queue/.agents/recording.md. The
broad 'known differences pinned on both sides' wording in SPEC_impl.md:2546-2547 and its source references could be
intended to incorporate this exception, but do not explicitly match its viewer-blink trigger/consequence/scope. No
matching SPEC.md/SPEC_impl.md exception, Planned item, unlanded plan, BUGS root, queue item or ledger decision was found
by focused cursor/steady/blink/DECSCUSR searches. Earlier xterm-only ledger exclusions at TRIAGE_OUTCOMES.md:11157-11167
and :11316-11326 cover different internal bundle defects.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g042/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41;
behavior and intentional-departure text persist at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only
review; runtime occurrence and frequency were not measured.
