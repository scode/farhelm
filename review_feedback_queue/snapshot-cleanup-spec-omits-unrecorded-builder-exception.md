# Snapshot cleanup spec omits unrecorded builder exception

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

The product says reusing any Farhelm-built snapshot is refused, but an unrecorded build snapshot can be accepted as a
recipe source and later deleted by builder cleanup.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `SPEC.md:327-328 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g039/g039-l2, event 7):

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c. TLDR: A recipe can accept a snapshot made by an interrupted
Farhelm build and later lose its usable source when that build is retried or deleted, although the product specification
says that save is refused. Details: SPEC.md:327-328 promises that a recipe cannot reuse a snapshot Farhelm built for any
recipe. SPEC_impl.md:5798-5802 explicitly excludes snapshots taken before the helm stops but not yet recorded, says the
reuse save succeeds, and says the next builder sweep deletes them. Source confirms the narrower guarantee:
crates/farhelm-helm/src/tensorlake/recipes.rs:215-244 checks only each recipe state's current and retired snapshot IDs;
build.rs:342-350 builds the sweep keep set only from state.snapshot; build.rs:1168-1185 terminates the builder then
deletes all its other snapshots without considering RecipeSource::Snapshot references. A user can list that ID in
Tensorlake and save it as another recipe's source, then retry/delete the interrupted builder recipe. Suggested fix
within this documentation scope: qualify the product guarantee with the existing SPEC_impl exception, or make source
references participate in cleanup protection if the blanket guarantee is intended. Suggested bucket: other, for the
established misleading contract and dangling reusable recipe source. This does not establish loss of a running host or
unique user work: it is a Farhelm package-build snapshot and ordinary recreation remains possible. A route deleting
irreplaceable user changes would require reassessment toward highest; no such route was shown here. Confidence:
confirmed for specification mismatch and source mechanism; no runtime reproduction (source-only charter). Temporal
check: pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b still carries the blanket SPEC.md statement at 425-426 and
the explicit exception at SPEC_impl.md:5967-5970. Direct tip inspection confirms recipes.rs:217-246 still checks state
snapshot IDs only, build.rs:344-350 still retains state.snapshot IDs only, and build.rs:1259-1276 still deletes all
other snapshots attributed to the builder. proposed_drop: false. possible_cover: implementation exception may already be
an accepted limitation; parent must match coverage. Caveat: this finding asks to reconcile the product contract, not
silently override the implementation's explicit accepted limitation. Revision reason: corrected the original
implementation-spec line citation (5798-5802, not 5829-5833), and replaced partial later-diff evidence with direct
inspection of all relevant tip guards.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g039/g039-l2`; supporting
records `g065/g039-l2`, `g068/s3`. The retained private log preserves earlier wording and its corrections. This record
does not claim runtime reproduction.
