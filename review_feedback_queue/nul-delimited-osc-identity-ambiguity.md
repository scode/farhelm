# NUL-delimited hyperlink identity has unresolved warning consequences

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

Current source status at c5ae8548439cc5bfa33c11ef7ce8da6129d8072a: 8b4f1e16 replaces the ambiguous delimiter key with a
JSON-encoded identity pair at both affected consumers. The historical consequence was never established; the original
diagnosis below is retained for explicit queue cleanup rather than silently removed in this review.

## TLDR

Different hyperlink identities can collide in the warning comparison's string key. Remote grid messages can supply those
strings, but no route from the collision to a wrong opening or required-disclosure bypass was established after
investigation.

## Details

### Later repair

At the additional reviewed tip, `crates/farhelm-ui/assets/grid-buffer.js:339–342` encodes `[link.id, link.uri]` with
`JSON.stringify`. The dependency deduplication path at 467–474 uses the same helper, and the adapter tag map at 642–659
uses that pair identity too. Distinct string pairs no longer alias through embedded NUL delimiters. This is a
**source-confirmed repair of the recorded mechanism**, not proof of every warning or activation invariant.

The queue file still described only the old ambiguity after that repair. This status addition preserves both the
original investigated uncertainty and the later source evidence; it makes no new exploit claim or triage decision.
Provenance: review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g018/g018-c06`.

### Historical investigation

The missing premise is a consequential route from the ambiguous comparison/dependency identity to a required disclosure
or activation check being bypassed. Actual checks covered key creation, strict grouping, warning comparison, pending
dependencies, retained hover, and URI/path-based opening. The URI remains independently retained; the inspected file
path and stat checks do not use the ambiguous tag to choose a different file. Ordinary OSC NUL omission does not dispose
of hostile grid input, but delimiter ambiguity alone does not establish a harmful opening.

Repeating these source reads cannot establish the missing consequence. A concrete supported admitted-input and guard
sequence demonstrating such a consequence, or source proving the relevant guard cannot depend on the ambiguous identity,
would settle it. No payload construction or runtime/browser experiment was performed. Keep the established key ambiguity
and its stronger investigated uncertainty distinct.

Suggested bucket: **other**. Confidence: **unverified after investigation**. unverified after investigation

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-buffer.js:569-576.

Ordinary OSC parsing ignores NUL, but hostile supervisor input is in scope and grid-model accepts it; do not use the
ordinary parser as a global exclusion. Actual URI remains separately retained for disclosure and activation, and
rowLinks compares fields separately. a097/f1 supersedes its initial AGREE with AMBIGUOUS. Preserve exact checks, missing
consequential premise, why further source repetition cannot settle it and needed evidence.

grid-buffer.js:569-576 uses concatenated id/NUL/uri as the tag map key; :401-404 uses the same key to deduplicate answer
dependencies. grid-model.js:199-207 permits NUL-containing strings within byte bounds. Actual supported terminal output
cannot inject NUL into OSC parameters because vendor/vte/src/lib.rs:447-450 ignores it. Correction/clarification to
initial exclusion rationale: grid-model.js:13-26 treats the remote supervisor as hostile, so arbitrary link-table
strings are within that trust model; that host's ability to control text and URI is not itself a security exemption.
Nevertheless the adapter tag is not opening authority: terminal.js:1268-1294 retains found.uri independently, reads the
tag only via linkHoverText, discloses that same found.uri, and activates it through the existing scheme/visibility
checks. rowLinks:286-325 compares id and URI separately and constructs each real hover range without the delimiter key.
Investigation result: collision potential is established algebraically; a security-relevant weakening of the warning
outside the explicitly accepted joined/split/decoy-text limitations is not established.

Different hyperlink identities could receive one hover-comparison tag when their NUL-delimited string keys collide. No
additional wrong-target opening or security consequence was established.

Possible coverage and caveats: tip SPEC.md:2324-2327 and SPEC_impl.md:1690-1704 explicitly limit split/decoy
comparisons, but do not expressly cover delimiter collisions. Searches in all required coverage sources found no exact
acceptance/Planned/BUGS/queue/plan/ledger disposition.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g171/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f;
persists unchanged at tip ca9b0a19856dc136f63ad2d14f1076027edce1eb. Candidate. This was a source-only review; runtime
occurrence and frequency were not measured.
