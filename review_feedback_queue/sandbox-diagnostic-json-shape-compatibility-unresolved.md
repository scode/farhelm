# Sandbox diagnostic json shape compatibility unresolved

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930

## TLDR

Structured sandbox failure descriptions would be rejected instead of displayed. Whether the configured production origin
sends those shapes remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/manager.rs:4068-4094; crates/farhelm-helm/src/tensorlake/api.rs:91-93 at 88bf83e267b5066815448885c61ed4148b851930`.

Source assessment (read with the later corrections below) (review record g044/l16, event 37):

Reviewed commit 88bf83e267b5066815448885c61ed4148b851930, persists through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: Farhelm treats a supported structured sandbox diagnosis as an unreadable
API reply, loses the reported state/reason, and falls back to SSH instead of using the status decision. Exact route:
Option<String> error_details at api.rs:91-93 rejects object/array diagnostics; HttpTensorlake::json returns Decode for
failed whole-response serde decode (http.rs:103-115, get_sandbox:210-216). check_sandbox maps Decode to
SandboxCheck::Unavailable (tensorlake/hosts.rs:95-96), sandbox_gate maps Unavailable to Dial (manager.rs:3334-3365), and
attempt runs that branch (4068-4094). Tip retains fallback and peer diagnosis mapping at manager.rs:4151-4177.
Authoritative accepted-shape contract, dated before Farhelm change: upstream tensorlakeai/tensorlake commit
aaf2f7e586f438d0d8daf92470e0aecc06efe832 (GitHub commits API date 2026-10-28) has
src/tensorlake/sandbox/models.py:738-766, SandboxInfo.error_details: Any | None; immutable URL
https://raw.githubusercontent.com/tensorlakeai/tensorlake/aaf2f7e586f438d0d8daf92470e0aecc06efe832/src/tensorlake/sandbox/models.py
. Same commit docs/sandbox-errors.md explicitly retains older JSON object/array diagnostics and names direct get's
error_details field:
https://raw.githubusercontent.com/tensorlakeai/tensorlake/aaf2f7e586f438d0d8daf92470e0aecc06efe832/docs/sandbox-errors.md
. This demonstrates supported response-shape incompatibility, not merely a theoretical malformed server response.
SPEC_impl.md Tensorlake account:5693-5709 requires lenient decoding and usable diagnostics; Tensorlake hosts:5857-5869
requires state-based dial decisions. Suggested bucket other, confidence confirmed source/primary-contract mismatch;
runtime incidence/duration not measured. Stronger outcomes depend on which live statuses carry structured diagnoses: no
deployed resume_failed-by-name payload was observed and no suspended-host auto-resume, real gateway hang,
security/data/process loss is asserted. Fix diagnostic decoding to preserve valid state/identity despite optional
structured human-readable details; format/bound/sanitize the value at display trust boundary, including l15's separate
key scrub. Only later api.rs patch adds max_pending_secs; http.rs unchanged; no repair through tip. proposed_drop false;
possible_cover unknown, parent handles coverage. New root is optional-field type strictness turning a valid status
answer into Unavailable, distinct from l15's missing key scrub on accepted strings. Source-derived regression may feed a
2xx get reply with valid known status and object/array error_details, assert status remains authoritative and no
fallback dial; no such runtime test was run. All checks were read-only source/documentation retrieval, no
cloud/runtime/service experiment. No open source lead remains.

Later qualification (review record g044/l16, event 44):

Pass-6 synthesis revises confidence of existing l16, not NEW discovery; prior verified label was too broad about
producer equivalence. Reviewed full hash 88bf83e267b5066815448885c61ed4148b851930 through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR remains conditional: structured sandbox diagnosis would cause Farhelm to
discard status and dial via unavailable fallback. The exact local Option<String> -> Decode -> Unavailable -> Dial chain
and upstream SDK accepted object/array diagnosis model at aaf2f7e586f438d0d8daf92470e0aecc06efe832 are confirmed as
recorded. Newly supplied g050:l6@23 establishes that SDK lifecycle defaults to sandbox.tensorlake.ai, whereas Farhelm
explicitly pins api.tensorlake.ai (http.rs:20,48-54 and SPEC_impl Tensorlake account). g050:l4@25/l5@24 likewise
preserve this exact producer distinction and note local comments record live checks 2026-11-03. No supplied or
previously read primary source establishes that the configured api.tensorlake.ai origin returns the newer SDK's
structured error_details, routes to the same producer, or has removed the older schema. Therefore actual Farhelm-origin
response incompatibility/user impact is unverified after investigation; do not retain a verified deployed
contract-mismatch assertion from the SDK alone. Missing premise: authoritative response schema or retained 2xx sandbox
response from Farhelm's pinned origin containing object/array diagnosis. Readable SDK consumer/docs establish
compatibility differences, not the private old-origin producer; a synthetic stub only proves local rejection. No
endpoint experiment permitted/run. Bucket other, confirmed consumer transformation but production premise unverified;
proposed_drop false, possible_cover g050:l4 overlaps broader optional-field whole-response strictness but its
integer-unit normalization is distinct, g050:l5 cursor continuation is distinct, and g050:l6 standalone endpoint failure
was set aside, not used as a new finding. Do not claim endpoint removal, 404/redirect, runtime omission, automatic
resume, gateway hang or security/data/process loss. Fix direction contingent on actual wire contract: preserve
authoritative state when optional human-readable diagnostic shape differs. Existing exact local/upstream references
remain in history; no reason to repeat unchanged unavailable producer investigation. No open exploratory lead introduced
by this confidence correction.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g044/l16`; supporting records
`g034/c14`, `g071/c18`. The retained private log preserves earlier wording and its corrections. This record does not
claim runtime reproduction.
