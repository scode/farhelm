# Provider error cleaning can miss transformed secret

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

Error cleaning can miss a key after truncation or normalization changes its spelling. An actual provider response
echoing that key remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/http.rs:168-193 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g050/f1, event 13):

Reviewed da24fb6822d289a144e18677cdd248cc35093246, unchanged through 8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR:
specially shaped upstream errors can return most/all of a saved credential after truncation or normalization, but
production disclosure remains unverified after investigation. Existing f1 source checks establish truncation before
exact replacement (http.rs:168-185) and unsafe-character removal after replacement (:187-193), error Display through
mod.rs:301-309. g051:l3 (g051/findings.jsonl:16), supplied by parent, corroborates these exact transformations under the
same reviewed commit. Its ordinary short-key boundary-padding case and raw U+200B-split case duplicate f1; its synthetic
maximum-length-token example adds no established real-token consequence. Preserve its qualifications: a literal JSON
backslash-u escape is not the raw-character producer; actual keys are described locally as well under 100 bytes;
accepted 4096-byte input does not prove usable provider tokens that long. Producer investigation from g051: official
https://docs.tensorlake.ai/platform/authentication documents tl_apiKey_ prefix, bearer placement, project scope and
generic 401 causes, but no required echo/split/padding format or maximum;
https://docs.tensorlake.ai/api-reference/v2/sandboxes/create has no fetched 401 schema establishing it; public
managed-agent README has a fixed AUTH_REQUIRED example, not a universal guarantee; public sandbox/client.py consumes
remote errors, not the auth producer; root openapi.yaml lacks sandboxes. Two attempted source fetches were unavailable,
so their contents are not inferred. These are current documentation observations reported by g051, not independently
fetched historical deployment evidence. Combined with this group's validation/request/fake/tests/doc checks in prior f1
events, no accessible checked producer establishes the trigger. Missing premise remains authoritative producer code or
retained real response with a split/boundary credential that survives display limits; an owned stub verifies only local
transformation. Suggested bucket other for established defensive-contract gap, highest-capable if usable credential
exposure route established. Confidence confirmed transformation, production consequence unverified after investigation.
proposed_drop false. possible_cover exact g051:l3; consolidate with f1, not new discovery. No new investigation is
justified merely to relabel this unchanged missing producer premise. No runtime/network experiments performed. Full
earlier f1 mechanism/contract/checks remain in its history.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g050/f1`; supporting records
`g051/l3`, `g039/g039-l6`, `g073/f8`, `g060/f6`. The retained private log preserves earlier wording and its corrections.
This record does not claim runtime reproduction.
