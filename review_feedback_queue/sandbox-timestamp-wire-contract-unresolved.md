# Sandbox timestamp wire contract unresolved

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

Sandbox timestamp decoding accepts only a narrow numeric form and unit. Compatibility with the configured production
origin remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/api.rs:81-83,207-208 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g050/l4, event 25):

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246; timestamp fields unchanged through pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: if sandbox/snapshot timestamps use the alternative formats accepted by
Tensorlake's SDK, Farhelm either rejects the entire reply (ISO/numeric strings or fractional epochs) or shows a wrong
creation time (integer seconds/microseconds treated as milliseconds). Source mechanism: SandboxInfo.created_at(:81-83)
and SnapshotInfo.created_at(:207-208) are Option<i64> with no custom decoding; HttpTensorlake::json(:103-115) maps
failed optional-field serde conversion to whole-response Decode. At tip orphans.rs:124/165 forwards sandbox created_at
unchanged and UI tensorlake/orphans.rs:147 divides it by 1000 before utc_minute, assuming milliseconds. For integer Unix
seconds near 1.7e9 this displays a date near January 1970; fractional or string forms fail before display. Primary
compatibility evidence predates Farhelm:
https://raw.githubusercontent.com/tensorlakeai/tensorlake/aaf2f7e586f438d0d8daf92470e0aecc06efe832/src/tensorlake/sandbox/models.py
:55-82 parses numeric/string seconds, milliseconds and microseconds by magnitude and leaves ISO strings to datetime
validation; SandboxInfo.created_at(:768) and SnapshotInfo.created_at(:946) use it. The same immutable revision's
tests/sandbox/test_timestamps.py:24-56 explicitly tests these accepted forms; :81-135 round-trips
SandboxInfo/SnapshotInfo JSON, explaining that ISO support also serves SDK serialization. Rust
cloud-sdk/src/sandboxes/models.rs:783 and :936 keeps created_at as Option<serde_json::Value>, and Python
client.py:1034-1059,:1711-1732 decodes REST-derived Rust JSON into these models. A guessed
tests/unit/sandbox/test_models.py retrieval returned 404; actual tests/sandbox/test_timestamps.py was found via
immutable GitHub tree and read. What this does NOT establish: the tests are consumer/round-trip fixtures, not captured
raw vendor replies; Python's ISO serialization is not proof the HTTP server emits ISO. SDK lib.rs:103-163,:449-458 also
selects sandbox.tensorlake.ai while Farhelm pins api.tensorlake.ai, so producer equivalence is unproven. Farhelm
comments record milliseconds and live type checks; no accessible producer implementation/retained response establishes
alternative forms/units at the configured origin. Actual runtime failure/wrong display remains unverified after
investigation. Missing premise: authoritative raw response schema or retained sandbox/snapshot reply from
api.tensorlake.ai showing seconds, microseconds, fractional or ISO/numeric-string created_at. An owned-stub check can
test Farhelm's conversion only, not deployed producer. Readable public sources above settle SDK acceptance but cannot
settle private origin behavior; no runtime, network endpoint, cloud or credential experiment authorized/performed.
Suggested bucket other, confidence confirmed consumer difference, unverified production premise. proposed_drop false;
possible_cover g044:l16 may cover the whole-response optional-field strictness aspect; parent should consolidate that
portion if its root definition is broad. The separate timestamp-unit normalization assumption (valid integer decodes but
wrong creation date) is absent from g044:l16's structured-diagnosis finding. Do not count a new manager fallback
endpoint as novelty. No established security/data/process-loss consequence. Contract SPEC_impl.md Tensorlake account
promises lenient optional reply decoding; established product creation-time display should represent the actual time.
Suggested fix if broader wire contract is confirmed: normalize timestamps to milliseconds at boundary while preserving
state on irrelevant optional display metadata; never infer units from SDK output alone. No open lead remains under l4.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g050/l4`; supporting records
`g071/c21`, `g032/g050:l4`, `g065/g050-l4`, `g074/f13`. The retained private log preserves earlier wording and its
corrections. This record does not claim runtime reproduction.
