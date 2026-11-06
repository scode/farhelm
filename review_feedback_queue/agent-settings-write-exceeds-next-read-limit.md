# Agent settings write exceeds next read limit

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

Saving agent settings can produce a file larger than Farhelm's own next-read limit, making later settings operations
refuse it.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/provisioning/agent_endpoints.rs:32,43-69 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g048/e1, event 41):

Reviewed commit ca28b604b876770b1ab757783416f83ad1709313. NEW discovery pass 2 finding. TLDR: Configuring a Tensorlake
host can enlarge a valid Claude settings file beyond Farhelm's own 1 MiB input limit. The first setup succeeds, but
every later setup with that endpoint fails on the file Farhelm itself wrote, until the user shrinks or manually
reformats it. Suggested bucket other: correctness/recovery inconvenience for large settings, without established work
loss, disclosure or automatic destruction. Confidence confirmed by source and exact locked serializer semantics; no
runtime test. proposed_drop false; possible_cover unknown; parent handles coverage.

Input/output mismatch: agent_endpoints::claude accepts a JSON object with object env, adds the two endpoint keys and
serializes the WHOLE document with serde_json::to_vec_pretty (:11-32). configure_agents at tensorlake_add.rs:619-636
reads previous bytes and installs the returned bytes without a maximum-output-size check. read_agent_file at
tensorlake_backend.rs:118-122 accepts at most 1024*1024 bytes on every read. Any accepted compact file sufficiently near
that limit can be enlarged past it even by endpoint-key addition; pretty-printing substantially expands compact
structured files as well. The next ordinary rerun again reaches configure_agents and fails before it can merge, at
read_small_file; execute_tensorlake_add at tensorlake_add.rs:350-374 records a failed setup and tells the user to run
setup again, which cannot resolve this same file-size failure. No concurrent writer or exotic parser input is needed.

Exact source-derived witness (no code executed): a minified JSON document containing only env with 64 string variables
V00 through V63, no endpoint keys. Each entry contributes 8 syntax/key bytes (quoted three-character name, colon, quoted
value), 63 commas plus the 10-byte object envelope total 585 fixed bytes. Set 63 values to 16,374 ASCII a characters and
the final value to 16,429 characters: total values 1,047,991 and total document length exactly 1,048,576, accepted by
the inclusive <= limit. All values are ordinary JSON strings and env is an object; no single environment variable
approaches Linux's individual 128 KiB entry limit. Adding the absent endpoint keys increases the JSON size beyond the
bound even before indentation. The same failure can arise from less artificial compact permission-heavy settings because
this implementation reformats all nesting rather than just appending fields. This witness establishes Farhelm's
accepted-input/output round trip; no claim of common real-world file frequency.

Exact dependency: pinned Cargo.lock:5595-5598 resolves serde_json 1.0.151, checksum
c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14. Its exact cached src/ser.rs:2229-2235 implements
to_vec_pretty without a size limit; PrettyFormatter::new at :1951-1955 uses two-space indentation, begin_array_value
:2000-2005 adds newline/indent, and begin_object_value :2052-2056 adds a space after the colon. Thus no serializer cap
or compact fallback prevents the oversized output. This is exact locked source, not a newer dependency substitute.

Contract: SPEC.md Topology:286-295 supports applying endpoints to reused images and rerunning failed setup while
preserving unrelated settings. SPEC_impl.md Adding Tensorlake hosts:5777-5783 explicitly gives agent-settings reads the
1 MiB bound and says Add, including rerun, applies endpoints. That read limit itself is accepted; the defect is creating
an output that violates the next read's precondition, not demanding unbounded reads. The source can either refuse
oversized merged output before replacing the old file with a concrete remediation, or preserve/choose compact encoding
within a consistent round-trip bound. Consider maintaining headroom or a documented output/input policy for endpoint
additions. Focused unit checks can use near-limit supported JSON and assert emitted output remains within reader
capacity or is refused before install.

Temporal qualification: git log ca28b604b876770b1ab757783416f83ad1709313..8a8c9579e04299410652f0e4681975a7198dfc7b for
agent_endpoints.rs, tensorlake_backend.rs and tensorlake_add.rs lists only c340166d754d3edb3de7fd538c6408948d3536c6,
whose previously inspected change is tunnel-confirmation wording. Serializer/read/merge route is unrepaired through
pinned tip. Caveats: large-file edge case; no token leak or persisted-edit loss established. Existing Codex/Claude
authentication findings, concurrent-editor overwrite and wrong-sandbox adoption have different mechanisms and do not
cover this size mismatch. No runtime, build, installation or endpoint experiment.

Same root confirmed for Codex (e3 merged here, not a second novel finding): agent_endpoints.rs:43-69 inserts a provider
and model_provider, then returns doc.to_string().into_bytes() without an output bound. A valid comment-only TOML
document of exactly 1,048,576 bytes (for example repeated short comment lines) passes the shared inclusive 1 MiB read
cap. Exact locked toml_edit 0.25.17+spec-1.1.0 parser/document.rs:113-125 captures comments/newlines as document
trailing content, take_trailing:465-469 preserves that span, document.rs:93-99 carries it into DocumentMut, and
encode.rs:178-201 emits inserted tables followed by the original trailing content. None of the added assignments removes
that trailing field. Therefore provider insertion increases the serialized size above the next read limit while
preserving all comments, even without a large environment or pretty-print expansion. This is one inconsistent read/write
bound affecting both supported agent formats, not a new endpoint counted as a distinct root. The read/install/rerun
failure route and other-bucket consequence are as above. The exact cached dependency source was available; no parser was
executed. Fix direction must cover both encoders, with refusal before replacement or a consistent serialized-size
policy. Compression/compact JSON alone does not fix the Codex comment-preserving case. Unverified premises: none for the
source-derived round-trip failure; runtime frequency remains unmeasured.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g048/e1`; supporting records
`g048/e3`, `g053/i11`, `g071/c17`. The retained private log preserves earlier wording and its corrections. This record
does not claim runtime reproduction.
