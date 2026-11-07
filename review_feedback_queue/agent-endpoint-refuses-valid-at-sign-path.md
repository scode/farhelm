# Agent endpoint refuses valid at sign path

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a

## TLDR

A valid at-sign in an agent endpoint's URL path is rejected as though it supplied credentials.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-proto/src/tensorlake/recipes.rs:408-409 at 68790603037bb51ccd8655e4a948c7ffb747760a`.

Source assessment (read with the later corrections below) (review record g063/d1, event 16):

Reviewed commit: 68790603037bb51ccd8655e4a948c7ffb747760a. TLDR: A user cannot save an otherwise valid agent endpoint
whose path contains an at-sign, such as https://api.example.com/@team/v1; Farhelm rejects it as if it contained
credentials. Mechanism: check_base_url at 408 tests the entire post-scheme rest for '@' before separating authority from
path at 411. An at-sign in the path is therefore rejected even though the authority is only api.example.com and there
are no credentials, query or fragment. This differs from known c2's DNS-host validation failure and has a separate
guard/fix. SPEC_impl.md, Tensorlake recipes (5739-5745 in this revision), permits http(s) base URLs without
credentials/query/fragment; helper contract at 383-386 explicitly permits an optional path. There is no stated at-sign
exclusion for paths. Primary syntax: RFC3986 section 3.2 defines userinfo only within authority; section 3.3 explicitly
includes '@' in pchar for path segments. Source: https://www.rfc-editor.org/rfc/rfc3986.html#section-3.3 and
#section-3.2. StoredRecipe::saved at crates/farhelm-helm/src/tensorlake/recipes.rs:95-104 calls this shared check before
either create or update stores anything, so the refusal affects both the UI's shared preflight and protected API writes.
Suggested bucket: other (valid-input correctness/convenience); no stronger security, credential or work-loss consequence
claimed. Confidence: confirmed from the unconditional check and primary URI syntax; no runtime request or live endpoint
is needed to establish the save refusal. Suggested fix: reject userinfo in the authority component, preserving legal
path characters; retain query/fragment and backslash restrictions, and add focused valid-path and authority-userinfo
cases. Temporal status: introduced in this assigned patch at 68790603037bb51ccd8655e4a948c7ffb747760a; the
original-to-pinned-tip diff reread this turn adds build read types but leaves check_base_url unchanged through
8a8c9579e04299410652f0e4681975a7198dfc7b. proposed_drop: false. possible_cover: unknown; no forbidden coverage searches
performed. Novel verified discovery finding for discovery pass 1. Lead d1 is resolved by this finding; no dynamic
premise remains for the bounded refusal claim.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g063/d1`; supporting records
`g065/g063-d1`, `g064/f13`. The retained private log preserves earlier wording and its corrections. This record does not
claim runtime reproduction.
