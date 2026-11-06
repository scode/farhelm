# Packages build result retired under changed image source

Reviewed commit: ca28b604b876770b1ab757783416f83ad1709313

## TLDR

A build that captured a Packages source can have its result retired after the recipe's current source changes to Image.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/tensorlake/build.rs:572,896-924,702-717 at ca28b604b876770b1ab757783416f83ad1709313`.

Source assessment (read with the later corrections below) (review record g070/d1, event 27):

Reviewed full hash ca28b604b876770b1ab757783416f83ad1709313; persists at pinned
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR: after a host confirmation is prepared from a package recipe, switching
that recipe to reuse an image in another window can make confirmation run and bill an entire snapshot build, discard it,
and then fail setup. Concrete supported sequence: start with Packages and no snapshot; probe Add host, which stores
recipe in TensorlakeAddPlan (provisioning/tensorlake_add.rs:201-205,231-249); update the saved recipe to Image via
recipes.rs:367-413 (allowed, no host-use/build fence; retires built state), then confirm the retained plan.
start_tensorlake_add checks row existence only (:280-281 at ca28), not source equality. tensorlake_steps dispatches by
frozen plan.recipe.source and passes the frozen definition into ensure_snapshot (:387-404); start_definition selects
frozen at build.rs:572. It runs fixed base/package/agent steps and takes a snapshot. Completion consults live stored
spec (:897-904), sees Image, puts the new snapshot into retired_snapshots without setting state.snapshot, then
sweeps/deletes it (:913-924). ensure_snapshot's post-build state read requires snapshot (:702-717), so it fails with
'the snapshot build ended without a snapshot; run setup again'. This sequence needs only a successful provider build,
not an uncertain provider status/error premise or timing during completion. Host does not reach create_host_sandbox.
SPEC_impl.md Adding Tensorlake hosts:5737-5753 freezes recipe definition and says a build the add starts uses the
confirmed definition, while Tensorlake snapshot builds:5854-5855 intentionally retires output if current recipe stopped
being managed. The individual retirement rule is accepted; the broken handoff of that result to an already-confirmed
caller is the new integration defect. Surface this contract tension rather than silently abandoning either rule. It does
not reopen l2's accepted active-build join or c1's sender lifetime; it differs from l3's check/admission race because no
concurrent rebuild or state/registry interleaving is required. Suggested bucket other (paid work/delay followed by
failed setup; no unique user data loss). Confidence verified source route; no runtime test. proposed_drop false.
possible_cover unknown; no queue/plan/filter search. Fix direction: hand the caller a build-result lease independent of
current recipe source and defer retirement until consumed, or refuse/replan before billed build if the confirmed plan
cannot be honored. History check for build.rs, build_tests.rs, recipes.rs and tensorlake_add.rs from ca28 to pinned tip
finds only c340166d754d3edb3de7fd538c6408948d3536c6's already-read tunnel confirmation wording change in the caller; no
repair. New verified discovery ID d1, no open lead remains.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g070/d1`; supporting records
`g068/s7`, `g065/g070-d1`, `g071/c13`. The retained private log preserves earlier wording and its corrections. This
record does not claim runtime reproduction.
