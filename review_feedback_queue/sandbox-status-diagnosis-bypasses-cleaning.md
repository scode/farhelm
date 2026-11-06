# Sandbox status diagnosis bypasses cleaning

Reviewed commit: 8a7f66ede90651d47d1facc1e877ce81f842af1a

## TLDR

Accepted sandbox failure descriptions bypass normal cleaning and redaction before reaching clients. An actual provider
description containing a credential remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/lifecycle.rs:186-205 at 8a7f66ede90651d47d1facc1e877ce81f842af1a`.

Source assessment (read with the later corrections below) (review record g034/l8, event 33):

Reviewed commit 8a7f66ede90651d47d1facc1e877ce81f842af1a; previous temporal checks establish persistence at pinned
8a8c9579e04299410652f0e4681975a7198dfc7b lifecycle.rs:204-224. Selected-interface supplement, not a new discovery. TLDR:
accepted string failure diagnoses from status JSON bypass normal Tensorlake diagnostic bounds/cleanup and key redaction
before returning to clients. Plain SandboxInfo.error_details (api.rs:91-93) comes from generic success JSON decoding
(http.rs:103-115), then lifecycle wraps it in identity-Display Transport (:186-193) and the REST error chain is returned
verbatim (lib.rs:2607-2629); UI refusal_text only trims (UI api.rs:2604-2621). Normal http::clean_message :181-193
supplies key replacement, unsafe-character stripping, whitespace collapse and 300-character display cap, while success
JSON has only an 8 MiB reply cap. Governing SPEC_impl.md Tensorlake account and TensorlakeError docs :221-224 promise
cleaned/bounded/redacted server diagnostics. Suggested bucket other for the source-confirmed boundary violation,
confidence confirmed propagation/guard omission. Echoed-key consequence remains unverified after investigation; supplied
g044:l15 corroborates the sibling manager/host-reply path but preserves the SAME unavailable real provider
credential-echo premise. Its immutable upstream models/error docs establish accepted diagnostic shapes, not copying the
request Authorization key; no new echo investigation was run. Missing premise remains actual credential-bearing provider
diagnosis producer, settled by server implementation or an authoritative/captured provider example; controlled JSON can
verify only local omission. possible_cover g044:l15 is related successful-status key-scrub root with exact manager
peer_text/host-reply locus, while l8 additionally records lifecycle's bypass of bounds/cleanup in the verbatim refusal
path. Parent may consolidate with both caller scopes retained. g044:l16's strict optional-field shape is DISTINCT:
unsupported object/array rejects before this string propagation route; its schema evidence does not establish secret
exposure. proposed_drop none. No injection execution, observed credential leak, highest consequence, deployed incidence
or runtime/cloud experiment asserted. Fix clean/redact accepted human-readable status diagnosis with the request key
available without altering resource IDs; handle structured decoding separately. Existing original events preserve all
source details and qualifications.

Additional source evidence (g044/l15, event 39):

Supplement to full local l15 history; known duplicate interface synthesis, not NEW discovery. Local reviewed hash
88bf83e267b5066815448885c61ed4148b851930, persistent through pinned tip as recorded. Selected owner g071:l4@17 reviews
ca28b604b876770b1ab757783416f83ad1709313 startup termination formatting at tensorlake_add.rs:580 without small reason
bound/cleaning; its pending_reason:584 does use manager::peer_text, so is bounded/escaped but not key-aware. Selected
owner g034:l8@23 reviews 8a7f66ede90651d47d1facc1e877ce81f842af1a lifecycle error_details:186-205 directly
transported/formatted, lacking small bound/presentation cleaning/key masking, still present at tip:204-224. Both share
local l15's root that successful HTTP status JSON bypasses error_message/clean_message. Preserve each sink and reviewed
source scope, not a new defect merely per endpoint. Crucial distinction: assigned manager resume_failed path DOES call
peer_text (4089-4090), which caps input at 1024 bytes and Debug-escapes controls (725,740-761); do not import the
siblings' raw 8-MiB diagnostic consequence into this bounded manager sink or call it wholly unsanitized. Local gap is no
key-aware masking, not no bound/escaping. Bucket other; confidence confirmed distinct sink contracts/omissions; actual
credential echo remains unverified after investigation for all supplied paths. No upstream real-key producer is
established; synthetic stub would prove bypass only. proposed_drop false; possible_cover g071:l4 and g034:l8 same broad
success-diagnosis cleaning boundary with distinct sinks/qualifications; parent may consolidate retaining those
distinctions. Non-2xx raw-body truncation g050:f1/g051:l3 is a separate mechanism, not relied on here. Existing l16 is
also distinct: accepted structured field type causes whole status decode failure before any sink. No unchanged
unavailable credential/gateway premise reinvestigated; no source lead opened by this intake.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g034/l8`; supporting records
`g044/l15`, `g071/l4`, `g071/c16`, `g071/c19`. The retained private log preserves earlier wording and its corrections.
This record does not claim runtime reproduction.
