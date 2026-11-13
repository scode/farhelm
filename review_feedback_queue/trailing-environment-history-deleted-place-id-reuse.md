# Reusable place intent does not freeze a pending action's endpoint authority

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Reusable place intent does not freeze a pending action's endpoint authority

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g050/c2`.

### g050/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 5 (candidate, open):

# Reusable place intent does not freeze a pending action's endpoint authority

Reviewed commit: 52cc5d7250d262f93b730d91a07c240d33c2ea5f; synthesis checked consumers and contracts at 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

## TLDR

Remembering a recipe and container-host choice is separate from binding the machine a user confirms. Missing history targets remain unavailable as specified; a pending approval must still reject a changed machine.

## Details

The original deletion proof is valid but narrow: hosts.id is AUTOINCREMENT at store.rs:2715, preventing a deleted numeric ID from selecting a newly allocated row. SPEC.md at the foundation:4798–4800 explicitly says 'Recent setups retain the recipe, container host when applicable, explicit agent selection and repository-or-home intent after Delete removes the environment. Applying one creates a fresh environment. A missing recipe or container host stays unavailable until the user changes that choice.' This proves neither endpoint stability of an existing row nor safety of a previously drawn approval.

The endpoint contract explicitly keeps reusable identity separate: SPEC_impl.md:6015–6018 says 'Recipe templates, Clone, quick launch and recent setups carry a recipe/place target separately from installed-host identity, never both. Missing recipes or place hosts are inapplicable.' Its history paragraph at 6020–6023 requires captured recipe/place and repository-or-home intent without a disposable-host foreign key. Consumers honor that division: create_form/draft.rs:1978–1985 gates ordinary installed-host history separately, while 2001–2005 restores the saved environment target into a fresh draft; environment_ui.rs:451–467 renders the current place name, and 716–732 uses last_container_host only as a picker default. These checks support storage retaining reusable placement rather than freezing an old installation there. They are not a whole-UI authority audit.

The separate answered-action contract remains: SPEC_impl.md:9090 says 'What a card shows is what an approval does.' At 9097–9098 it says 'A launch is also refused if the target host’s connection changed while the card waited, since the approval was for the machine the card named.' Parent-supplied g020/l1 establishes the violating pending-agent-card route after an operator retarget and the shipped credential-delivery consequence. No new duplicate finding is asserted here. That known highest route is not excused by reusable placement or AUTOINCREMENT. A proper fix binds or revalidates the pending launch's place authority, independently of the history record.

Suggested bucket: other for unavailable retained history; could-be-highest: no from missing IDs alone, yes for the separately established g020/l1 changed-endpoint approval. Confidence: confirmed source/contract distinction; no execution. proposed_drop: only the unavailable retained-history suspicion, on the exact storage/reuse specification above; no drop of pending-action endpoint authority. Parent has accepted no drop. possible_cover: g020/l1 explicitly supplied by parent. No unresolved premise for this storage distinction; remaining confirmation correctness belongs to the owning action interfaces.

Returned proposal (claims-031, event 5): **covered**, suggested bucket **other**.

The original unavailable-retained-place behavior is expressly required. Deleted-ID reuse is independently refuted. Neither statement excuses a pending approval following a changed physical endpoint of an existing row.

Original stronger consequence: Initial concern was unavailable remembered placement, without an established highest route. Later history preserves the supplied g020/l1 highest consequence: recipe credentials and commands may reach a changed endpoint after stale approval. This recommendation does not dismiss that external finding.

Unresolved premise or limitation: No missing premise for unavailable-place retention. Endpoint stability of a surviving row is a separate action-interface obligation, not established by retained numeric identity.

Proposed basis: Apply specification cover only to missing-place retention and source refutation only to deleted-ID reuse. Require a scoped independent check because the history explicitly carries the separate wrong-machine security concern.

Existing parent disposition (event 5): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-missing-place-intent-cover-plus-deleted-ID-alias-refutation-only. Full031 explicitSPECretainmissingrecipe/placeunavailableuntiluserchanges; targetdistinctinstalledidentityandeditabledefaultnotauthority. AUTOINCREMENTrefutesdeletedIDreuse. ExactcovermissingintentandnarrowIDrefutationonly, preserveexternal020l1retargetsurvivingphysicalendpointclaimnotdedup/droppedbythis. Freshgatebothstatedpartsandoriginalsuppliedhighestconsequence; no allapprovalsafetyclaim.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g050 record](records/review-through-00de0b6c/groups/g050.json). Record ID: `c2`.

