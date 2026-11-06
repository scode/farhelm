# Snapshot cleanup races new host registration

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

Snapshot cleanup can remove a generated snapshot before a newly admitted host records its use, causing setup failure and
rebuild cost.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location: `crates/farhelm-helm/src/store.rs:6409 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g047/l1, event 9):

Reviewed hashes: foundation 88bf83e267b5066815448885c61ed4148b851930; production add
ca28b604b876770b1ab757783416f83ad1709313; pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: concurrent recipe
deletion and sandbox creation can discard the recipe's reproducible prepared snapshot and fail new-host setup, even
though recipe deletion ultimately refuses. Confirmed pinned interleaving: recipes.rs:672-675 observes zero host users;
before or during clean_for_delete at 678, registration commits (store.rs:6598-6641, originally ca28 store.rs:6409-6452).
Registration checks destination/alias conflicts, not recipe cleanup admission, and its transaction ends before
returning. Add's existence-only reconciliation at provisioning/tensorlake_add.rs:283-291 succeeds while cleanup still
retains the recipe row. build.rs:1351-1364 holds a registry.deleting reservation, then 1397-1411 retires/deletes the
recipe snapshot with no recheck of host users. At that boundary setup's ensure_snapshot either captures the ready id at
build.rs:661-674 without checking deleting, or sees no snapshot and fails at start_definition's Deleting guard
(590-593). Recipe deletion's final transaction at recipes.rs:440-468 now returns InUse because the host exists, after
the reproducible build state was discarded. Scope of guards rechecked: all add_confirmations references are
initialization plus ordinary/Tensorlake add acquisition (service.rs:173,1054; tensorlake_add.rs:266); recipe deletion
never acquires it. The per-host provisioning lock at tensorlake_add.rs:312 does not bridge to recipe deletion.
Registry.deleting is private to RecipeBuilds and fences starting builds, not host insertion or ready-snapshot reads;
DeleteGuard drops that reservation at build.rs:1450-1457. Both DB transactions are individually serialized but no
transaction spans the initial users read, asynchronous cleanup, registration and final deletion. Router lib.rs:899-904
invokes delete_recipe directly; its own_state_change middleware (middleware.rs:66-74) only calls run_owned, whose
lib.rs:2609-2616 implementation spawns the request with no shared admission lock. Thus no route ownership guard repairs
the race. Source read at ca28 confirms the same add/cleanup ordering; diff through pinned tip leaves build.rs/recipes.rs
unchanged and changes only a plan-step description in tensorlake_add.rs. SPEC.md Topology refuses deletion of a recipe a
host uses; recipes.rs:669-671 explicitly intends nothing to change on that refusal. Fix needs common
cleanup/host-admission serialization or a deletion reservation enforced by registration, rather than another final
existence check. Suggested bucket: other. Demonstrated consequence is limited to reproducible recipe-build state, setup
availability and retry/rebuild cost; no route to lost user sandbox files, running sessions, credentials or other
user-owned work was established. Confidence: confirmed source-level interleaving; no runtime/API experiment per brief.
proposed_drop: false. possible_cover: parent will consult add/build/recipe interface owners; their responses were not
available in this pass and no queue/coverage search was performed. Caveats: deletion completing before insertion is
correctly reconciled by rolling back the never-created host; that orphan hypothesis remains refuted. No substantive
unresolved lead remains. Separate discovery was not performed.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g047/l1`; supporting records
`g065/r3`, `g066/g066-l4`, `g068/s2`, `g047/interface-g066-l4`, `g071/c6`. The retained private log preserves earlier
wording and its corrections. This record does not claim runtime reproduction.
