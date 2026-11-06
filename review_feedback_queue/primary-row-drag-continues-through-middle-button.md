# Primary row drag continues through middle button

Reviewed commit: 720e18bf8e78339e59bb876af75a621bfd1a69e9

## TLDR

Releasing the primary button during a middle-button chord leaves a row drag active; releasing the other button can then
commit the move.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-ui/assets/pin-order.js:137-148,152-169 at 720e18bf8e78339e59bb876af75a621bfd1a69e9`.

Source assessment (read with the later corrections below) (review record g085/l3, event 31):

Reviewed commit: 720e18bf8e78339e59bb876af75a621bfd1a69e9. TLDR: If another mouse button is held during a pin drag,
releasing the button that grabbed the pin does not end the drag. Moving with only the other button held can change
placement, and releasing that other button can commit the move. Source mechanism: down starts only for button 0 and
records only pointerId/coordinates (137-148). move samples every event for that id without checking event.buttons
(163-164); up accepts every pointerup for the id without checking the released button (152-158). Pointer Events ends a
mouse pointer on release of its last held button, so primary-release during a chord arrives as pointermove. Farhelm
consequently keeps paint/capture alive after primary release and drops on the final unrelated release. Reproduction
sequence: primary-down on an eligible grip; move to arm; press/hold middle while keeping primary down; release primary;
move with only middle held; release middle over a non-noop neighbor. The primary release should finish/cancel the
primary-button gesture rather than leave it armed for another button. This concerns pointer completion, not l1 geometry
refresh or g086:f1 marked selection. Governing contract: SPEC.md Session list at the reviewed hash:1917-1920 describes a
grip drag ending in release or cancellation; the implementation explicitly restricts the initiating press to button 0.
Official event source checks establish the nontrivial chord premise: https://www.w3.org/TR/pointerevents/ section
3.1.1.1 explains chord transitions use button/buttons rather than overlapping down/up pairs. Immutable official WebKit
source
https://raw.githubusercontent.com/WebKit/WebKit/f2f0dead988af103f8106b25078617041fa67a03/Source/WebCore/page/PointerCaptureController.cpp:555-570
converts both an additional mousedown and a mouseup with buttons remaining into pointermove; 617-620 leaves pressed
state intact on move; 656-663 only releases capture after pointerup. Actual producer is an ordinary multi-button mouse,
not invented DOM events. The middle-button example avoids the right-button contextmenu timing ambiguity confirmed by the
spec's contextmenu notes. Checked primary compatibility/default handling too: PointerCaptureController.cpp:670-673
stores preventDefault from the initial down; Source/WebCore/dom/Element.cpp:497-546 suppresses compatibility mouse
handling after still dispatching the pointer; EventHandler.cpp:2143-2168 skips default mouse-press handling when
dispatch is swallowed. Thus the inspected classic WebKit path does not require a native context menu/autoscroll to
interrupt the middle chord before the erroneous continued gesture. Tests inspected: pin-order.test.js creates
button/pointerId fields but no buttons state, and covers foreign pointers/initial secondary press, not same-pointer
chorded transitions. Suggested fix: track the initiating button's buttons mask; end or cancel when that bit disappears
on pointermove and refuse unrelated-button completion. Add a targeted chord regression exercising the actual event
sequence. Suggested bucket other: rare but established recoverable pin-order/gesture correctness, no
data/process/security loss. Confidence confirmed for the source/event-sequence defect, no runtime reproduction claimed;
platform-specific handling outside the inspected path may cancel earlier, and no claim is made that every
engine/platform exhibits the full sequence. proposed_drop false; possible_cover unknown, excluded coverage sources not
searched. Temporal check: pin-order.js has an identical blob through 8a8c9579e04299410652f0e4681975a7198dfc7b, so no
later primary-button guard was added. This is a novel local verified-source discovery finding.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g085/l3`; supporting records
`g083/c11`. The retained private log preserves earlier wording and its corrections. This record does not claim runtime
reproduction.
