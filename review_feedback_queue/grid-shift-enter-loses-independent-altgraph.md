# AltGraph+Shift+Enter can be rewritten as Farhelm newline in grid terminals

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

On a keyboard/browser that reports AltGraph independently of Ctrl and Alt, holding AltGraph while pressing Shift+Enter
can insert Farhelm newline ESC CR instead of the ordinary Enter CR the existing terminal sends.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source behavior
with documented Linux Firefox modifier mapping; no runtime/native keyboard reproduction.

Source locations: 7d52f1cda4c301e8811acf23263b6ab66ce1930a:crates/farhelm-ui/assets/grid-input.js:253-271,314-319.

at tip assets/shift-enter-key.js:118-129 explicitly refuses to arm Shift+Enter while getModifierState(AltGraph) is true.
Grid assets/grid-input.js:265-270 recognizes third-level shift only for keyCode 0 or greater than 47, so Enter keyCode
13 is still sent; keyMessage :341-346 and modifiers carry no AltGraph bit. Supervisor grid_input.rs:295-312 assumes
AltGraph does not reach the supervisor and prepends ESC for key Enter with mods SHIFT.

On a keyboard/browser that reports AltGraph independently of Ctrl and Alt, holding AltGraph while pressing Shift+Enter
can insert Farhelm newline ESC CR instead of the ordinary Enter CR the existing terminal sends. Source route at tip:
grid-input.js:93-95 drops AltGraph from mods; :265-270 never treats Enter keyCode 13 as third-level shift; :302-324
therefore sends it when deadKeyPending is false; :341-346 sends only mods. grid-terminal.js:1440-1446 forwards that key
to the supervisor. grid_input.rs:295-312 incorrectly assumes AltGraph never arrives and prefixes ESC when
key=Enter/mods=SHIFT. In contrast shift-enter-key.js:118-129 refuses the AltGraph chord, terminal.js:5580-5594 leaves
the xterm CR unprefixed, and vendored evaluateKeyboardEvent uses CR for Enter without altKey. Static producer: Mozilla
MDN getModifierState documentation (https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/getModifierState,
Modifier keys on Firefox table) documents Linux GTK Level-3/Level-5 Shift as AltGraph independently of physical
Control/Alt. Pending-state caveat resolved: pressing AltGraph immediately before Enter can make both clients swallow the
first sending key; the route instead types an AltGraph character first while keeping AltGraph held, then Shift+Enter.
grid-terminal.js:1416-1418 clears pending on that typed character; no later AltGraph keydown is needed. Thus the held
modifier remains while the dead-key filter is clear.

Possible coverage and caveats: none found after searches of SPEC.md, SPEC_impl.md, TODO Planned (empty), BUGS.md,
unlanded plans/queue, queue INDEX and TRIAGE_OUTCOMES.md. SPEC.md:2094-2101 restricts newline rewriting to the exact
chord with no other modifier; SPEC_impl.md:1706-1736 promises key parity and does not accept this omission.
SPEC_impl.md:1594-1608 describes the old binding that checks AltGraph. plans/queue/typed-quick-launch.md:22-62 preserves
Shift+Enter targeting, not terminal encoding, so it does not cover this cause. No AltGraph ledger or queue match; the
existing near-term composition-preview omission is unrelated to this non-composing key. FILTER rare-self-correcting does
not fully match an actual keyboard chord whose encoding remains wrong each time, rather than an unsupported wire message
or timing fault. Fix direction: retain the AltGraph/exact-chord distinction when building grid key messages and when
deciding the supervisor newline prefix; do not merely drop an Enter that should remain CR. Tests should compare the
Farhelm ShiftEnter augmentation as well as bare xterm _keyDown.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g154/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7d52f1cda4c301e8811acf23263b6ab66ce1930a;
unchanged relevant behavior at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review;
runtime occurrence and frequency were not measured.
