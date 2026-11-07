# Short helm tag collision destroys other builder

Reviewed commit: da24fb6822d289a144e18677cdd248cc35093246

## TLDR

Two helms sharing a Tensorlake project can collide in their short ownership tag; a build can then terminate the other's
builder and delete its snapshots.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Source location: `crates/farhelm-helm/src/tensorlake/config.rs:79-104 at da24fb6822d289a144e18677cdd248cc35093246`.

Source assessment (read with the later corrections below) (review record g050/f2, event 14):

Reviewed da24fb6822d289a144e18677cdd248cc35093246 for short tag, destructive build route introduced
744ea4122d033ef616454fe592ecb2173593736c, still present at 8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: a
shared-project helm with a colliding four-character tag and matching recipe name can destroy another helm's
builder/snapshots. Prior synthesis establishes generated 128-bit local namespaces (store.rs:378-382) map to 36^4 tags
(config.rs:92-104), local recipe names can match, builder_name(:110-113) binds only tag/name, start_definition's guards
are local, cleaning unconditionally adopts a name hit(:1088-1095), and sweep_builder(:1259-1276) terminates it and
deletes snapshots absent from this helm's local keep-set(:344-350). Provider wire/HTTP code carries no full registry
ownership metadata/guard. Parent-supplied g068:s1 (g068/findings.jsonl:15) and g070:s1 (g070/findings.jsonl:17)
independently corroborate exactly this route at tip; recording found.id is attribution bookkeeping, not proof of
ownership. Their narrower local-row, pending-create or sender/cancellation refutations do not refute cross-helm
ownership. Confidence remains confirmed source conditional on tag collision; suggested bucket highest for another helm's
process/snapshot loss; proposed_drop false. possible_cover exact g068:s1 and the g050-f2 portion of g070:s1,
duplicates/corroboration only, no new discovery. Parent-supplied g071:l5 (g071/findings.jsonl:13), reviewed
ca28b604b876770b1ab757783416f83ad1709313 and unrepaired at tip, confirms the same account/tag premise but concerns HOST
names with an additional four-hex suffix and first-use provisioning adoption/configuration. Preserve g071:l5 separately:
existing endpoint credentials can be replaced before copied-state refusal; its later Delete is explicitly warned and
bound, so do not extend it into a stale-name/unwarned-deletion claim. Do not merge that different generated-host route
into deterministic builder f2. Original account-only commit itself remains non-destructive; later build caller creates
f2. No new code experiment or inventory reading; supplied interface text reconciled against existing pinned-source
route. Full mechanism/contract/reproduction remains in earlier f2 history.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g050/f2`; supporting records
`g032/g050:f2`, `g043/g050:f2`, `g065/g050-f2`, `g068/s1`, `g039/g039-i3`, `g069/g069-s2`. The retained private log
preserves earlier wording and its corrections. This record does not claim runtime reproduction.
