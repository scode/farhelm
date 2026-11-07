# Queued recipe edits after save unresolved

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

Form events arriving after Save captures its draft may disappear when the editor closes. A complete native producer
remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-ui/src/tensorlake/recipe_editor.rs:361-412,520-541,608-626,807-822 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g064/f7, event 25):

Reviewed full hash: 68790603037bb51ccd8655e4a948c7ffb747760a; persists at tip
8a8c9579e04299410652f0e4681975a7198dfc7b:410-461,581-687,899-914. New discovery finding resolving lead f7; distinct from
f1 (which concerns reading draft at the Save answer). TLDR hypothesis: an accepted form edit arriving after Save
captures its payload may be displayed while the request is in flight and then disappear when success exits the editor;
no shipped native sequence demonstrating that effect was established by this source-only search. Confirmed structure:
Save checks busy (362), clones draft (365), validates, sets busy (380), and creates a serialized owned request before
spawn_forever (384-396). Success calls on_saved unconditionally for the original request (402-408), and the parent
leaves the editor. The form's individual oninput/onchange/software/tunnel callbacks directly mutate draft without
testing busy; readonly/disabled are only rendered attributes derived from locked=busy (352). Scope-checked all tip draft
writers and guards: initial new/existing setup, explicit user controls, no shared dispatch busy filter. Therefore the
state transition Save(D0), then a still-drawn field/software handler mutating draft to D1 before lock rendering, then
success can discard D1; two Save answers themselves are correctly refused by synchronous busy. Actual event/lifetime
calibration: Dioxus core 0.7.10 runtime.rs:369-379 permits multiple events before render and dispatches only currently
mounted listeners. Desktop callback handling is synchronous and rendering is separately polled, but no source guarantee
or instrumented trace establishes that the required native Save-then-field event pair reaches the still-enabled form
before the busy DOM update. The new ops.rs:314-342 shipped precedent establishes confirmation-answer bursts, not
automatically every input/paint ordering or a field edit accepted while actually readonly. No claim of arbitrary
callbacks surviving a completed disabled render/unmount. Ordinary editing before Save is expected and not this route;
submitting after a completed disabled render has no supported native producer identified. Missing premise: concrete
native sequence with field/software action after request capture yet before lock application, its accepted D1 displayed,
and ensuing success discarding it. A trace of native input/DOM busy state, Rust draft mutations and submitted request
would settle that; source review cannot measure it and runtime experiments are prohibited. Confidence: structure/state
transition confirmed conditionally; user-visible consequence unverified after investigation. Suggested bucket other
provisionally; consequence if demonstrated is only unsaved form text/choices, not saved recipe work or credentials.
proposed_drop:false, retained investigated uncertainty for parent judgment; possible_cover: unknown, no coverage search.
A fix if this race is demonstrated is to ignore form mutations synchronously while busy or retain later dirty state on
completion. No security or stronger loss route asserted.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g064/f7`; supporting records
`g056/g056-c13`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
