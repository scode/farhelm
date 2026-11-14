# Recipe list clones the complete status matrix for every row

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: as the shared recipe catalog grows, rendering it repeatedly copies every recipe's build
status into every row, even though each row displays only its own statuses. This adds quadratic allocation/copying work
to opening and refreshing Settings.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g060/l3`.

### g060/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 20 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. Discovery pass 2. TLDR: as the shared recipe catalog grows,
rendering it repeatedly copies every recipe's build status into every row, even though each row displays only its own
statuses. This adds quadratic allocation/copying work to opening and refreshing Settings. Details: recipes.rs:478-485
iterates N recipes and evaluates place_status.read().as_ref().cloned() for each RecipePlaces component. The resource
value is Option<Result<Vec<RecipePlaceView>,String>>, and RecipePlaceView derives ordinary Clone over owned
recipe/reason/step Strings (farhelm-proto/src/environment.rs:118-130). This is a deep Vec/String clone per row, not an
Arc/Rc view. RecipePlaces takes that owned full Vec at containers_settings.rs:192-207 and only afterward filters all
entries by row.recipe and container_host at 216-218; entries belonging to all other recipes were already copied. The
endpoint at helm hosts.rs:4347-4368 returns one global vector containing each recipe's recorded/supported places. With N
recipes each supporting P available places, the per-row clone evaluates N times on an N*P-element matrix, yielding
O(N^2*P) copied status entries plus per-child filtering when those children render. This cost occurs before any
framework child-prop equality optimization could avoid child rendering. Bounds checked: tensorlake/build.rs:235-264
selects every stored recipe without LIMIT; recipe create at tensorlake/recipes.rs:251-280 admits valid unique names
without a catalog-count cap. The 64-KiB request-body cap is per recipe, not a list-size limit. API fetch_recipes at ui
api/tensorlake_recipes.rs:47-57 returns all entries; there is no UI paging before the loop. RecipeList also subscribes
to ACTIVITY_NOW at recipes.rs:364; activity.rs:75,99-110 republishes it every 30 seconds, so this allocation path can
repeat in an idle open panel, in addition to ordinary read completions and feed-triggered renders. Useful checks
establish this complexity without a benchmark; no particular freeze threshold, measured latency, OOM or crash is
claimed. Contract/context: SPEC.md:441-479 requires one shared catalog with per-place build status, and
SPEC_impl.md:6513-6516 specifies synchronous shared-state projection; neither requires each row to own every other
recipe's status. Suggested fix: index statuses once by recipe and pass only the corresponding row's statuses, or share
one immutable response without per-row deep cloning; retain the independent per-place request/retry semantics. Suggested
bucket: other for an established avoidable resource-growth/performance defect, with material user-visible severity
unmeasured and no parent priority accepted. Could-be-highest: no established security or user-work-loss route.
Confidence: confirmed source-level allocation/complexity route, not runtime-observed performance. proposed_drop: false.
possible_cover: none in the current supplied neighbour projection; no prohibited coverage sources searched. Caveats:
small catalogs may have negligible practical cost; P counts actual returned places, and no cost is asserted for an
absent/unloaded status response. This does not alias l1 failed-status recovery, l2 Add host compatibility, or g061/l5
successful-status invalidation. Every readily checkable cardinality/type/caller/bounds premise is settled; no open lead
remains for this ID.

Returned proposal (claims-038, event 20): **retain-verified**, suggested bucket **other**.

Each recipe row deep-clones the all-recipes/all-places response before its child filters to that recipe. For N recipes
with P returned places each, this copies O(N^2*P) status entries per parent render. Source establishes avoidable growth,
not a measured freeze, OOM or crash.

Original stronger consequence: Initial lead suspected large-catalog responsiveness and memory overhead. Latest finding
establishes quadratic allocation/copying and child filtering, with no measured latency, particular freeze threshold,
OOM, crash, security or user-work-loss route. Failed-status recovery and successful-status invalidation remain distinct
roots.

Unresolved premise or limitation: No missing source premise for deep copies and asymptotic growth. Material latency,
realistic catalog scale, freeze threshold, memory exhaustion, crash and user-work loss remain unmeasured/unestablished.
Small catalogs may have negligible practical cost; P means actual returned places.

Proposed basis: Retain source-verified avoidable resource growth as other; stronger user-visible severity requires
evidence beyond complexity alone.

Existing parent disposition (event 20): verified. Parent explicitly accepted named retain-verified proposal from
claims-038 after inspecting it. Each recipe row deep-clones the all-recipes/all-places response before its child filters
to that recipe. For N recipes with P returned places each, this copies O(N^2*P) status entries per parent render. Source
establishes avoidable growth, not a measured freeze, OOM or crash. Retain source-verified avoidable resource growth as
other; stronger user-visible severity requires evidence beyond complexity alone. Checked source/results:
["350a4b33b6d4476f4a78e43cea124bc2b828f462:crates/farhelm-ui/src/tensorlake/recipes.rs:478-485 evaluates
place_status.read().as_ref().cloned() for each recipe row. containers_settings.rs:192-218 accepts owned
Option<Result<Vec<RecipePlaceView>,String>> then filters by recipe/container place.","Same
revision:crates/farhelm-proto/src/environment.rs:118-130 derives ordinary Clone for RecipePlaceView containing owned
recipe/reason/step strings. This is Vec/String copying, not Arc/Rc sharing, performed before child equality can avoid
rendering.","Same revision:crates/farhelm-helm/src/hosts.rs:4347-4371 builds a single vector over all recipes and
recorded/supported places. tensorlake/build.rs:235-264 selects all rows without LIMIT; tensorlake/recipes.rs:251-285
imposes validity/uniqueness/credential checks, not catalog count. UI api/tensorlake_recipes.rs:47-57 returns all entries
and the row loop has no pagination. Per-request recipe body size is not a catalog-count cap.","Same
revision:recipes.rs:364 reads ACTIVITY_NOW; activity.rs:75,86-109 republishes every 30 seconds. Therefore the allocation
path can repeat in an idle mounted panel as well as after read/feed-driven renders. No cost is asserted before a status
response is loaded.","git diff to 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db is empty for recipes.rs and
containers_settings.rs. No benchmark or runtime reproduction performed."] Current coverage/results: ["Current SPEC.md
shared recipe catalog and SPEC_impl.md:6010-6057 require per-place status and synchronous readiness projection, not
per-row ownership of every recipe's response. No acceptance of the avoidable cloning found.","TODO Planned empty.
plans/queue recipe/catalog/status/clone/matrix/performance searches found the existing Podman catalog requirement, not
an unlanded indexing/sharing/paging fix.","BUGS.md and full INDEX.md checked. Targeted recipe/status/matrix/quadratic
searches in queue sources found no exact root. Existing typed-template, popover and terminal-row repeated-work entries
concern different data and consumers; they are not aliases.","Active TRIAGE_OUTCOMES.md
recipe/catalog/matrix/clone/performance searches found no exact decision. FILTER.md has no full match for ordinary
catalog growth and repeated allocation. Archive not read."] Exact cited cover: [] Original stronger consequence and
missing premise remain preserved: "Initial lead suspected large-catalog responsiveness and memory overhead. Latest
finding establishes quadratic allocation/copying and child filtering, with no measured latency, particular freeze
threshold, OOM, crash, security or user-work-loss route. Failed-status recovery and successful-status invalidation
remain distinct roots."; "No missing source premise for deep copies and asymptotic growth. Material latency, realistic
catalog scale, freeze threshold, memory exhaustion, crash and user-work loss remain unmeasured/unestablished. Small
catalogs may have negligible practical cost; P means actual returned places.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g060 record](records/review-through-00de0b6c/groups/g060.json). Record ID: `l3`.
