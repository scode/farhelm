# Recipe leave save live draft mismatch unresolved

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

Save before leaving a recipe reads the live draft. A supported input sequence saving data different from the visible
draft remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-ui/src/tensorlake/recipe_editor.rs:361-373,685-687 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g039/g039-l4, event 13):

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a; persistence checked at tip
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: Recipe Save-before-leaving reads the current draft, but a supported user
sequence saving data different from the visible draft has not been established. Correction: earlier g039-l4 events
proved a structural conflict with SPEC_impl.md:1035-1055, Answers bind to what was drawn; they did not prove the claimed
stale-visible-state consequence. The confirmed facts remain: unit leave slot at recipe_editor.rs:341; rendered token
check then save(()) at :687; callback draft read/request construction at :361-392; tip equivalents :390, :759, :410-441.
Interface g056:g056-l3, findings.jsonl:11, supplies narrower source-backed qualification. Its application and Dioxus
0.7.10 inspection found only explicit user writers to the draft, current-listener synchronous dispatch, a refreshed
use_callback closure every render and no yield between token validation, draft read and request serialization. Fields
stay editable inside the inline role=status question; editing and allowing a visible rerender before Save saves that
updated visible draft and is not a mismatch. Desktop event-to-DOM timing remains a hypothesis, not a proven supported
input sequence. Missing premise: actual shipped input sequence changes relevant non-visible draft state while the Save
answer still has authority from different displayed data. Readable source dispatch/render boundaries do not establish
native event/DOM/paint scheduling; an instrumented browser/desktop trace of activation DOM, handler/render timing and
submitted body would settle it. No runtime experiment authorized or performed, and no repeat framework investigation
undertaken because evidence is unchanged. Confidence: confirmed live-read implementation fact; consequence unverified
after investigation. Suggested bucket other provisionally; no credential/data/process loss route. Remediation for
literal contract enforcement: pass a payload captured by the answer's render, not necessarily question-opening data
(later visibly edited draft must not be discarded). proposed_drop false. possible_cover: duplicate g056:g056-l3, exact
supplied owner record line 11; recipe-editor group may also cover it. Preserve owner's uncertainty and source scope:
recipe Settings close integration and exact-version framework dispatch, not a demonstrated native scheduling race.
Revision reason: reconcile supplied interface investigation and retract the earlier overstatement of confirmed
user-visible consequence. No new discovery.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g039/g039-l4`; supporting
records `g056/g056-l3`, `g064/f1`, `g056/g056-c4`. The retained private log preserves earlier wording and its
corrections. This record does not claim runtime reproduction.
