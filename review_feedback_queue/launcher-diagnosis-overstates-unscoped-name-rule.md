# Launcher diagnosis overstates unscoped name rule

Reviewed commit: 2878ee2842bd96417ed5e86aeb350c2fae4eb921

## TLDR

The recorded launcher diagnosis incorrectly says only unscoped text can offer a session-name action; explicit name
searches can offer it too.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `TODO.md:436-437 at 2878ee2842bd96417ed5e86aeb350c2fae4eb921`.

Source assessment (read with the later corrections below) (review record g002/g002-f1, event 22):

Reviewed commit: 2878ee2842bd96417ed5e86aeb350c2fae4eb921. TLDR: the recorded launcher diagnosis gives future
investigators an incorrect rule about when the launcher offers a session-name action. It says a name row is only offered
for unscoped text with no other matches, although explicit name: text also offers that action. Exact assigned source:
patch 148-001 lines 19-20, mapping from its +428 hunk to TODO.md:436-437. The complete packet was re-delivered during
discovery to verify that wording. crates/farhelm-ui/src/launch_composer.rs:1082-1091 at the reviewed full hash
explicitly returns ComposerSearchResult::Name(value.to_string()) for SearchScope::Name with a nonempty value.
scoped_query at :897-919 recognizes the name label independently of gh. The unscoped fallback is a separate path at
:957-969, not the only producer. SPEC.md Creation search paragraph (:1490-1497 in this range) recognizes name: among
supported labels, establishing this is a reachable documented action. Fix the diagnosis to say that gh: does not offer a
name row and that the implicit fallback is restricted to unmatched unscoped text; qualify its existing only claim rather
than changing launcher behavior. Bucket other: a narrow diagnostic-record error, with no demonstrated effect on
launching or user work. Confidence confirmed. proposed_drop:false. possible_cover:not searched; parent handles coverage.
Caveats: this does not establish the maintainer's wrong-repository cause, does not contradict the narrower gh: refusal,
and does not reverse g002-l1's naming-focus refutation. No security/data-loss route is claimed. No later-repair
assertion is made beyond the assigned patches; the reviewed historical statement itself is wrong. This is a new local
discovery, distinct from supplied g015:l5 and g090:c5. This event corrects the earlier approximate line range while
preserving its claim and qualifications.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g002/g002-f1`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
