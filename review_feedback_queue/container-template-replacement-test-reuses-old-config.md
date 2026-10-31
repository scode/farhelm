# Container template replacement test reuses the old configuration

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The template regression cannot detect keeping the old container settings because its replacement uses identical values.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

launcher.rs:1375–1395 seeds the same configuration expected after an explicit object edit. Preserving the old object
would satisfy that assertion. The wire roundtrip, null/refusal cases and empty-launcher application tests remain
meaningful, while no companion seeds a different existing configuration for positive apply_template replacement.
Production currently replaces the object correctly at 851–852; SPEC.md:732–734 states that contract. This is an oracle
gap, not a product replacement defect.

Source locations: `crates/farhelm-proto/src/launcher.rs:1375-1395`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery pass 2 final: retained test-evidence gap, reviewed d6645f771bee8ed03a66486ef8755834937d759d; suggested bucket
other; confidence confirmed from fixture/assertion source; proposed_drop=false; possible_cover=none found. TLDR: the
regression test cannot detect a template keeping old container settings instead of replacing them, so this replacement
contract has weaker test evidence than the test name suggests. The starting state already contains config (1375-1379);
both the omitted-edit case and the explicit object-edit case expect that identical config (1381-1384). Therefore the
apply_template result assertion at 1395 would also accept preserving the old object on a positive edit. The wire
roundtrip assertion at 1392 remains meaningful and the null/command refusal checks are useful; this finding does not
call the whole test vacuous. Complementary evidence was inspected: the archive-template test 1409-1422 and helm
agent_launch.rs:400-466 establish setting from an empty launcher, but not replacing a distinct existing object. Browser
templates.spec.ts:1036-1095 proves editing stored template fields, then applying into a fresh New draft; it does not
seed a different existing draft config before application. Draft tests:2982-3039 exercise copy/direct editing, and
3173-3207 exercise Restart refusal, neither positive replacement through apply_template. The neighboring
wrapper-template regression deliberately uses distinct old/new values (launcher.rs:1429-1464), providing the needed
comparison shape. Product code currently performs unconditional replacement at launcher.rs:851-852 and no product defect
is alleged. SPEC.md:732-734 explicitly says present settings replace the complete choice. Coverage searches across
SPEC/SPEC_impl, empty TODO Planned, BUGS, queue index/queue files, TRIAGE_OUTCOMES and plans/queue found no existing
decision or item covering this assertion gap. No test was run or mutated. A meaningful strengthening is to give the
object edit a different image/port/token path from the initial state; no implementation performed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g113/f6`, `a036/g113-f6`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
