# Unreadable sandbox ssh key reports ready

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

A damaged or unreadable saved SSH key pair can still be presented as ready, postponing the failure until later sandbox
operations.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Recording judgment: A contributor proposed the rare-diagnostic filter. The match is unclear: Ready remains wrong until
damaged permissions/state are repaired, rather than self-correcting on an ordinary refresh. Retain the unreadable-key
contract mismatch with its external-damage-only trigger.

Source location: `crates/farhelm-helm/src/tensorlake/ssh_key.rs:101-134 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g039/g039-l5, event 21):

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246. TLDR: Settings can claim Farhelm's SSH key is ready when an
existing private file has become unreadable, instead of reporting the promised local-key problem. Details: assigned
SPEC_impl.md:5711-5717 requires ready/not-created/unreadable status and reporting a key directory without a readable
pair. ssh_key.rs:116-121 checks private-file metadata/is_file only, then reads the public half at :122-129;
mod.rs:128-134 maps Ok(Some(key)) directly to Ready. A private ordinary file without read permission for the helm
account passes metadata while denying an actual read. ensure():79-86 accepts the same existing pair; later build
admission at 744ea4122d033ef616454fe592ecb2173593736c:build.rs:630-645 uses it too. Direct tip inspection at
8a8c9579e04299410652f0e4681975a7198dfc7b confirms the unchanged gate at ssh_key.rs:120-153 and Ready mapping at
mod.rs:215-225. Confidence confirmed for this conditional damaged-state diagnostic behavior, not a normal creation
failure. Explicit trigger qualification: no ordinary shipped path creating this permission mismatch was established;
generate creates a private temporary directory, runs ssh-keygen, sets private mode 0600 and renames the pair. The
affected input requires an external file/permission change or restored damaged state while directory/public key remain
accessible. No deployed occurrence, exact OpenSSH message or runtime test is claimed. Suggested fix is a readability
check preserving the existing key and never returning/logging secret contents. Suggested bucket other; checking metadata
does not replace/delete/expose the key, so no highest claim. Interface g051:l2@7, g051/findings.jsonl:7, independently
confirms this mechanism and proposes dropping it as a low-value externally damaged-state diagnostic gap. Preserve that
owner's narrow source scope (ssh_key.rs:116-128 and generation/status callers at the introducing commit, later diff
checked through tip), exact impact/producer qualification, and proposed-drop basis. It is not spec-accepted behavior:
the explicit unreadable-pair contract says otherwise. Local proposed_drop remains false pending parent value/coverage
disposition, since an error-state contract can be incorrect without a normal internal producer. possible_cover: exact
duplicate g051:l2@7. No unchanged-source reinvestigation performed. Revision reason: synthesized supplied duplicate and
made the external damaged-state-only trigger explicit; no new discovery.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g039/g039-l5`; supporting
records `g060/f9`, `g051/l2`. The retained private log preserves earlier wording and its corrections. This record does
not claim runtime reproduction.
