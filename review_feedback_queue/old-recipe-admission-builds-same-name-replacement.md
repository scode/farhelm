# Old recipe admission builds same name replacement

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

A previously admitted build can act on a later recipe recreated under the same name instead of the definition that was
originally admitted.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/tensorlake/build.rs:554-627 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g065/r5, event 20):

Reviewed full hash: 744ea4122d033ef616454fe592ecb2173593736c, still present at pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: after deleting a recipe and creating a different one with the same name,
a previously submitted build can still charge for the old recipe's packages/size and attach that snapshot to the
replacement. Source-confirmed interleaving: start_or_join reads/copies the old spec at original build.rs:554-562, awaits
API key, SSH key and namespace at 563-575 before claiming registry.active at 577-626. While that request is suspended
and no active entry exists, DELETE's cleanup can complete and delete the recipe row, dropping registry.deleting; POST
can create a fresh row with the now-free name (assigned recipes.rs:323-356). Old request resumes, sees neither deleting
nor active, and launches its copied old spec. The run's initial state write at original build.rs:649-665 requires only
that some row with that name now exists; update_tensorlake_recipe_state uses WHERE name alone. It cannot distinguish
replacement from the original. Finishing at original build.rs:802-818 only tests whether the current source is Packages,
then publishes old BuiltSnapshot into the replacement. Tip equivalents directly checked: start_definition:566-639,
run:738-755, snapshot construction/publication:883-908. A replacement package recipe with different packages or disk
therefore receives an unrequested old-definition build and snapshot (stale-packages warning/disk check may later expose
mismatch but do not prevent the billed work or attachment). Distinct from r4: a still-absent row safely aborts via
Ok(None); the defect requires successful same-name recreation. Distinct from known g070-l3: that issue concerns
read/join/start versus completion of another build; this crosses completed recipe deletion/recreation and adopts a
different row lifetime. Distinct from r3/g047-l1: no concurrent host registration is necessary. Suggested fix: reserve
build admission against deletion before copying the row, or carry and validate a non-reused recipe incarnation through
task admission/state writes; dropping the temporary deleting guard must not let an old admission operate on a
replacement row. Merely re-reading the latest source without identity does not cancel old accepted work against a
replacement recipe. Governing SPEC_impl Tensorlake recipes/snapshot builds describes unique create versus update,
separate user/build state writers and deletion cancelling the recipe's build; no explicit acceptance of pending old
builds migrating to recreated rows. Suggested bucket other: incorrect prepared software/resource billing and
rebuild/setup inconvenience; no established credential exposure, unique data loss or running-host damage. Confidence
confirmed by source-level interleaving; source-only charter forbids dynamic reproduction. proposed_drop false.
possible_cover unknown; parent handles coverage. Novel ID r5, verified source mechanism; no unavailable external premise
needed to establish wrong row/spec attachment. No claim about frequency or vendor-specific error behavior.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g065/r5`; supporting records
`g068/s6`, `g071/c14`. The retained private log preserves earlier wording and its corrections. This record does not
claim runtime reproduction.
