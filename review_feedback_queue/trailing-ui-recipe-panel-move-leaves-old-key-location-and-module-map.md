# Moved Recipes panel retains guidance for the old Tensorlake layout

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: a recipe whose Tensorlake snapshot needs an API key tells the user to save a key above
the recipe, but the shared Recipes section contains no key form. Moving the list out of Tensorlake made that setup
guidance inaccurate. Source: assigned tensorlake_settings.rs patch removes RecipeList from TensorlakeSettings at :381
and adds RecipesSettings at :472-482 containing only RecipeList with AccountState.api_key_set. settings.rs:629-633
mounts it in the separate Recipes section; :639-647 mounts TensorlakeSettings in its separately hidden Tensorlake
section. tensorlake/recipes.rs:509-520 disables snapshot Build when !api_key_set and displays exactly "Save an API key
above to build snapshots." Its actual content :463-552 is the recipe catalog/New recipe controls, not a key form.
TensorlakeKey remains at tensorlake_settings.rs:327 and Credentials has the other authorized form. Thus the text
describes the former parent layout. Attribution across the range: the old instruction was correct when the recipe list
sat below Tensorlake's API-key form; the assigned relocation creates this mismatch. The same responsibility move leaves
an independent source-documentation mismatch in the assigned file's module docs at tensorlake_settings.rs:23-25: "The
recipe list sits between the ssh key and the switch. It is its own component (`crate::tensorlake::recipes::RecipeList`)
with its own reads and saves; this section only places it." The new source correctly places it elsewhere, which
establishes rather than refutes that stale documentation. Preserve that future-reader consequence alongside the user

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g077/f8`.

### g077/f8

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 19 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. Discovery pass 2. TLDR: a recipe whose Tensorlake snapshot
needs an API key tells the user to save a key above the recipe, but the shared Recipes section contains no key form.
Moving the list out of Tensorlake made that setup guidance inaccurate. Source: assigned tensorlake_settings.rs patch
removes RecipeList from TensorlakeSettings at :381 and adds RecipesSettings at :472-482 containing only RecipeList with
AccountState.api_key_set. settings.rs:629-633 mounts it in the separate Recipes section; :639-647 mounts
TensorlakeSettings in its separately hidden Tensorlake section. tensorlake/recipes.rs:509-520 disables snapshot Build
when !api_key_set and displays exactly "Save an API key above to build snapshots." Its actual content :463-552 is the
recipe catalog/New recipe controls, not a key form. TensorlakeKey remains at tensorlake_settings.rs:327 and Credentials
has the other authorized form. Thus the text describes the former parent layout. Attribution across the range: the old
instruction was correct when the recipe list sat below Tensorlake's API-key form; the assigned relocation creates this
mismatch. The same responsibility move leaves an independent source-documentation mismatch in the assigned file's module
docs at tensorlake_settings.rs:23-25: "The recipe list sits between the ssh key and the switch. It is its own component
(`crate::tensorlake::recipes::RecipeList`) with its own reads and saves; this section only places it." The new source
correctly places it elsewhere, which establishes rather than refutes that stale documentation. Preserve that
future-reader consequence alongside the user-facing wrong-location instruction; no backend failure is inferred from the
comment. Contract: SPEC_impl.md Disposable environments, Shared recipes and prepared artifacts, assigns Settings →
Recipes the catalog/editor and Settings → Tensorlake the account/key controls; SPEC.md Settings likewise places the key
in Tensorlake and Credentials. Suggested bucket: other, inaccurate setup guidance and stale module responsibility
documentation. Could-be-highest:no established security, billing-authorization or user-work-loss route. Confidence:
confirmed source/layout analysis, not runtime observation. proposed_drop:false. possible_cover:none in supplied
neighbour projection; distinct from g060/l1 status retry, l2 Add-host compatibility and l3 matrix copying. Persistence
check: diff from this commit to pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db is empty for tensorlake_settings.rs
and tensorlake/recipes.rs; settings.rs only changes the unrelated extension-warning test. Both stale statements persist.
Fix direction: point prerequisite help to Settings → Tensorlake or Credentials and update the module's responsibility
map; users can recover by choosing the existing key section. No permanent block, failed save or production lifecycle
defect is claimed. No runtime tests, reproductions or live-install actions.

Returned proposal (claims-050, event 19): **retain-verified**, suggested bucket **other**.

Moving Recipes into its own Settings section leaves both user guidance and module documentation describing its former
position below the Tensorlake API-key form. The source-documentation defect is in scope alongside the visible setup
guidance.

Original stronger consequence: Wrong setup location guidance; the final history additionally preserves the stale module
responsibility documentation. Users can recover by choosing the existing key section. No billing, security or
user-work-loss consequence was alleged.

Unresolved premise or limitation: No runtime observation; source establishes the two incompatible layouts. No permanent
build block or failed save is established.

Proposed basis: Retain both stale statements from the same responsibility move, preserving the user-facing and
future-reader consequences.

Existing parent disposition (event 19): verified. Parent explicitly accepted named retain-verified proposal from
claims-050 after inspecting it. Moving Recipes into its own Settings section leaves both user guidance and module
documentation describing its former position below the Tensorlake API-key form. The source-documentation defect is in
scope alongside the visible setup guidance. Retain both stale statements from the same responsibility move, preserving
the user-facing and future-reader consequences. Checked source/results:
["350a4b33b6d4476f4a78e43cea124bc2b828f462:crates/farhelm-ui/src/tensorlake_settings.rs:320 keeps TensorlakeKey in
TensorlakeSettings; :472-479 mounts only RecipeList in RecipesSettings. settings.rs:629-647 separates the hidden Recipes
and Tensorlake panels.","Same revision:tensorlake/recipes.rs:465-552 contains the catalog and recipe controls, with no
key form; :512-519 disables Build without a key and tells the user to Save an API key above to build snapshots.","Same
revision:tensorlake_settings.rs:23-25 still says the recipe list sits between the ssh key and the switch and this
section places it. This is stale responsibility documentation, not evidence of a backend failure.","Diff to
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db is empty for tensorlake_settings.rs and tensorlake/recipes.rs. The relocation
creates the mismatch in range; the pre-relocation layout made the old guidance accurate. No runtime observation is
claimed."] Current coverage/results: ["Current SPEC.md Settings places recipes in a separate Recipes section and keys in
Tensorlake/Credentials; SPEC_impl.md:6052-6055 assigns those responsibilities explicitly. Neither accepts the obsolete
location instruction or module map.","TODO.md Planned empty. Searched unlanded plans/queue, BUGS.md, INDEX.md and active
TRIAGE_OUTCOMES.md for Recipes panel/key-above/layout guidance. No exact coverage. tensorlake-key-order is about SSH key
offering, and the responsible podman-environments relocation plan is already landed.","FILTER.md rare-diagnostic rule
does not match ordinary missing-key setup, and does not clearly cover the independently stale source responsibility
map."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Wrong setup location
guidance; the final history additionally preserves the stale module responsibility documentation. Users can recover by
choosing the existing key section. No billing, security or user-work-loss consequence was alleged."; "No runtime
observation; source establishes the two incompatible layouts. No permanent build block or failed save is established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g077 record](records/review-through-00de0b6c/groups/g077.json). Record ID: `f8`.
