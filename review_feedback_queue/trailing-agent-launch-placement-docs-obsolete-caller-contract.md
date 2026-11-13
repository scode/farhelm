# Placement documentation misstates ordinary-create inputs and links a missing helper

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Newly added internal documentation gives maintainers the wrong rule for how ordinary agent creates resolve placement templates, and its links cannot lead them to the actual resolver.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g018/f2`, `g052/l8`.

### g018/f2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435. TLDR: Newly added internal documentation gives maintainers the wrong rule for how ordinary agent creates resolve placement templates, and its links cannot lead them to the actual resolver. Details: the assigned 00653-000.patch, confirmed against git show of this commit, adds Placement's docs at agent_launch.rs 158–164 and resolve_with's doc at 196. At 162–164 it states, 'Ordinary creates and spawns pass no recipes or places, so a template carrying a recipe target is refused as unavailable there; only the environment-launch path ([`resolve_environment`]) passes them.' Ordinary creates actually load known recipes and Podman places and call resolve_placed at agent_requests.rs 2447–2475, allowing an earlier valid recipe placement to be overridden by a later ordinary-host placement. The statement hides a cross-caller contract that must survive refactoring, not just an obsolete label. agent_launch.rs 176–180 and the test at 478–536 document and check that stacking rule correctly. resolve_environment is also an intra-doc link at line 196, but a pinned grep over the entire helm crate finds that name only in these two comments; resolve_placed at 186 is the actual helper and the module's imports do not introduce resolve_environment. Correct the caller description to distinguish spawn's restricted resolver inputs from ordinary-create resolution and point both links at resolve_placed. Rustdoc effect bounded by source: both links lack a target and cannot provide the intended navigation when these private docs are rendered. agent_launch is a private module (lib.rs 112) and Placement is pub(crate), so this is not established as broken public API documentation. Searches of the pinned helm crate, Cargo.toml, .cargo, workflow and scripts find no broken_intra_doc_links override, RUSTDOCFLAGS, cargo doc invocation or document-private-items configuration; no repository-enforced rustdoc build failure is established. No documentation build was run, so no emitted diagnostic or fatal build is claimed. Suggested bucket: other. Confidence: confirmed source documentation defect. proposed_drop: false. The earlier candidate proposed dropping it for low-impact/private documentation; that rationale did not refute the actual defect and is superseded, preserved in the append-only history. Original consequence preserved: executable behavior is correct; established harm is misleading maintenance documentation and unresolved internal links, not a shipped runtime failure. possible_cover: none searched; parent owns coverage/filter decisions. Could-be-highest: no; no established security or loss-of-work route. Source-only. Attribution and readily checkable source premises are settled; no open lead remains. Citation correction in this revision: the private module declaration is lib.rs:112, confirmed with numbered pinned source; the previous 117 reference was a line-number error.

Returned proposal (claims-009, event 7): **retain-verified**, suggested bucket **other**.

New source documentation says ordinary creates have no recipe/place catalogs, but their caller explicitly supplies both so an earlier recipe placement can be overridden by an installed host. Both links name a nonexistent helper. This is an in-scope source-comment defect even though executable behavior is correct.

Original stronger consequence: The first candidate proposed dismissal because this was low-impact private documentation with correct runtime behavior. Subsequent history explicitly withdrew that dismissal and retained misleading maintenance documentation and unresolved internal links. No runtime failure, public-doc failure, emitted rustdoc warning or enforced build failure was established.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the source documentation defect at other priority. Correct behavior elsewhere does not refute the false caller contract. Preserve the bounded rustdoc consequence and superseded low-impact dismissal.

Existing parent disposition (event 7): verified. Parent explicitly accepted named retain-verified proposal from claims-009 after inspecting it. New source documentation says ordinary creates have no recipe/place catalogs, but their caller explicitly supplies both so an earlier recipe placement can be overridden by an installed host. Both links name a nonexistent helper. This is an in-scope source-comment defect even though executable behavior is correct. Retain the source documentation defect at other priority. Correct behavior elsewhere does not refute the false caller contract. Preserve the bounded rustdoc consequence and superseded low-impact dismissal. Checked source/results: ["190d844e58c96cf197aa838d0b0eea8062f48435 adds crates/farhelm-helm/src/agent_launch.rs:158-164 and 196. Lines 162-164 say ordinary creates and spawns pass no recipes or places and only resolve_environment passes them; line 196 names that helper again.","Same revision, agent_requests.rs:2447-2475 loads recipes and places for ordinary creates and passes them to resolve_placed. agent_launch.rs:176-186 describes the actual valid stacking contract.","Pinned git grep over crates/farhelm-helm finds resolve_environment only in those two comments. The actual helper is resolve_placed at line 186; no imported target supplies the missing name.","Same revision, lib.rs:112 declares agent_launch privately, and Placement is pub(crate). No public-API documentation failure is established.","Pinned search of the helm crate, Cargo.toml, .cargo, .github and scripts found no broken_intra_doc_links override, RUSTDOCFLAGS, cargo doc invocation or document-private-items setting. No rustdoc build was run; no emitted diagnostic or fatal build is claimed.","No agent_launch.rs change occurs from 190d844e through the reviewed endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db."] Current coverage/results: ["Both specifications searched for agent placement/template resolution. SPEC.md's ordered-template and explicit-flag rules and SPEC_impl.md's Agent requests and environments describe the real contract; neither accepts false caller documentation or missing links.","TODO.md Planned is empty. BUGS.md's sole SSH-helper hang does not cover this documentation defect.","plans/queue searched for agent_launch, resolve_environment/resolve_placed and placement/template documentation. plans/queue/INDEX.md lists no unlanded plans; retained environment-agent-operations.md does not promise to correct these comments.","Queue index and queue files searched for exact resolver names and agent/recipe placement wording; active TRIAGE_OUTCOMES.md searched for the same claim. No exact cover or same-root item found. The existing container-template replacement oracle finding is separate and adds no coverage. Archive not read.","FILTER.md has no general low-impact/private-comment filter. Its rare-diagnostic filter does not fit an always-present false maintenance contract."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The first candidate proposed dismissal because this was low-impact private documentation with correct runtime behavior. Subsequent history explicitly withdrew that dismissal and retained misleading maintenance documentation and unresolved internal links. No runtime failure, public-doc failure, emitted rustdoc warning or enforced build failure was established."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g018 record](records/review-through-00de0b6c/groups/g018.json). Record ID: `f2`.

### g052/l8

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 17 (finding, open):

# Shared template-context docs still exclude all agent recipe catalogs

Reviewed behavior commit: 190d844e58c96cf197aa838d0b0eea8062f48435

## TLDR

The shared template API tells maintainers that agent callers supply no recipes, although the new agent-launch implementation must supply them, even for an ordinary create whose last template selects an existing host. Following the documented rule would make a valid template stack fail. This is inaccurate internal caller-contract documentation; current execution supplies the catalogs correctly.

## Details

At crates/farhelm-proto/src/launcher.rs:600-602 the known_recipes field says 'Saved recipe names the caller can launch. Agent callers supply no recipes; GUI callers use their current catalog rather than guessing.' Agent calls now reach exactly this shared API with recipes. agent_launch.rs:233-238 constructs TemplateContext from placement.known_recipes/known_places. agent_requests.rs:2447-2475 explicitly loads the saved recipe catalog and place IDs for ordinary non-spawn creates, then passes them through resolve_placed. Its comment explains the non-obvious reason: a recipe template may appear before a later installed-host template, so the earlier recipe must still be applicable even though the final create uses an ordinary host. Empty recipes would cause the shared fold's known_recipes check to refuse that valid earlier template. The explicit environment-create path also needs this catalog. Spawn retains the restricted empty-catalog path through agent_launch::resolve at 145-151. The field contract should describe caller-supplied admissible recipes and distinguish restricted spawn from fleet agent creation; it must not classify all agent callers together.

Contract: SPEC_impl.md Agent requests and environments at 6677-6682 permits plain creates whose named template targets a recipe to use the environment path; the same section and shared-template contract preserve ordinary template application semantics. The new resolve_placed contract at agent_launch.rs:176-180 explicitly requires knowledge of both installs and recipes because later placement replaces earlier placement. Correct production behavior does not refute the independently false shared-API explanation.

Full-range attribution: the exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already has this sentence at launcher.rs:588-589, while agent resolution still hardcodes empty recipe/place slices. The complete range history through 190d844e finds only that behavior commit changing those resolver inputs; its parent-to-child patch changes TemplateContext construction from known_recipes:&[] to placement.known_recipes and adds resolve_placed. Thus the new behavior makes the old statement false inside the assigned range. Pinned endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the same sentence at launcher.rs:600-601.

Suggested bucket: other, internal API documentation. Confidence: confirmed pinned-source, actual caller construction, introducing behavior patch and endpoint comparison; no runtime evidence. Could-be-highest: no established security or user-work-loss route. proposed_drop: false. possible_cover: supplied g018/f2 reports the same incorrect agent-catalog distinction in helm Placement documentation plus a separate unresolved helper link. This is an additional assigned proto-field manifestation requiring its own wording change, not a claimed new independent runtime/root-cause issue; parent owns consolidation. Caveats: do not remove the legitimate empty-catalog restriction from spawn, and do not imply recipe catalogs alone grant environment-launch approval. All readily checkable premises resolved; l8 is closed as this documentation finding. No test, build or reproduction.

Returned proposal (claims-033, event 17): **retain-verified**, suggested bucket **other**.

Shared template-context documentation incorrectly excludes all agent recipe catalogs. Ordinary fleet creates now pass those catalogs so earlier recipe templates remain applicable even when later placement selects an installed host. Spawn retains its separate restricted path.

Original stronger consequence: Following the false empty-catalog rule would refuse an otherwise valid template stack. Current execution supplies the catalogs correctly; no runtime failure or security/work-loss claim is made.

Unresolved premise or limitation: None for this field's documentation defect. Caller-supplied g018/f2 concerns the same stale agent-catalog distinction in helm Placement documentation plus a separate unresolved helper link; parent must inspect that named record before deciding consolidation.

Proposed basis: Retain the proto-field manifestation, preferably consolidated with g018/f2's same caller-rule defect if its record agrees. It adds a distinct source location requiring wording repair, not a new runtime defect.

Existing parent disposition (event 17): duplicate. Parent read complete claims-033 g052/l8 and claims-009 g018/f2. Both falsely document every agent caller as lacking recipe/place catalogs after190d844e ordinary non-spawn creates begin supplying them through resolve_placed. The proto TemplateContext field comment at launcher.rs:600-602 is an additional manifestation of the same changed caller invariant already retained with the Placement/resolve comments and renamed helper links in g018/f2. Preserve the added source location and its base-true/new-consumer-false attribution; fixing only the helm wording would leave this field misleading. Runtime ordered-template behavior is correct, spawn remains restricted, and no current refusal/security/work-loss route is alleged. No exact current coverage or filter accepts the false invariant; this combines the documentary defect without dropping its separate manifestations.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g052 record](records/review-through-00de0b6c/groups/g052.json). Record ID: `l8`.

