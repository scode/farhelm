# Whole-environment bulk Delete intentionally destroys unmarked peers

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: a whole-environment bulk Delete removes unmarked and unlisted sessions, processes and files. This
destruction remains in the proposed-drop register; no independent approval to drop or lower it has been assumed.
Original concern preserved: deduplicating marked peers, including after answer-time shrink removes the displayed
representative, can destroy sessions not individually shown as targets. Source reconciliation: lifecycle.rs:263-286
deduplicates only sandbox && whole by the same captured host; :302-315 gives the dialog an explicit whole-environment
title and answer; :339-343 renders ProviderUi.whole_delete_warning. view.rs:4559-4581 sends the captured
EnvironmentDelete.scope, and api/sandbox_sessions.rs:188-197 maps Session and Whole to distinct opt-in query parameters.
Its :214-246 never changes the scope or automatically retries Session as Whole. Therefore a remaining marked peer is
only a routing representative for the same fully warned resource, not authority to destroy a different resource. Drawn
warning checked directly at environment_ui.rs:164-172: Tensorlake text says 'this deletes the whole sandbox: every
session, running process, file, checkout, unpushed change and snapshot in it, including sessions Farhelm does not list
because they were started while it was not connected. This cannot be undone.' Podman names the whole container and the
same categories except snapshots. Both offer resuming/updating first as the way to delete only the session. Exact
proposed acceptance remains SPEC.md Sandbox sessions: 'Marked sessions of a sandbox the helm could not ask when t

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g036/c1`.

### g036/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa. TLDR: a whole-environment bulk Delete removes unmarked and
unlisted sessions, processes and files. This destruction remains in the proposed-drop register; no independent approval
to drop or lower it has been assumed. Original concern preserved: deduplicating marked peers, including after
answer-time shrink removes the displayed representative, can destroy sessions not individually shown as targets. Source
reconciliation: lifecycle.rs:263-286 deduplicates only sandbox && whole by the same captured host; :302-315 gives the
dialog an explicit whole-environment title and answer; :339-343 renders ProviderUi.whole_delete_warning.
view.rs:4559-4581 sends the captured EnvironmentDelete.scope, and api/sandbox_sessions.rs:188-197 maps Session and Whole
to distinct opt-in query parameters. Its :214-246 never changes the scope or automatically retries Session as Whole.
Therefore a remaining marked peer is only a routing representative for the same fully warned resource, not authority to
destroy a different resource.

Drawn warning checked directly at environment_ui.rs:164-172: Tensorlake text says 'this deletes the whole sandbox: every
session, running process, file, checkout, unpushed change and snapshot in it, including sessions Farhelm does not list
because they were started while it was not connected. This cannot be undone.' Podman names the whole container and the
same categories except snapshots. Both offer resuming/updating first as the way to delete only the session. Exact
proposed acceptance remains SPEC.md Sandbox sessions: 'Marked sessions of a sandbox the helm could not ask when the
modal opened get the whole-sandbox Delete, once per sandbox, and the modal says which rows are which.' The preceding
paragraph warns 'every session, process, file, checkout, unpushed change and snapshot in the sandbox will be removed,
including sessions Farhelm does not list because they were started while it was not connected'. This basis covers only
destruction WITHIN that quoted whole-resource consent. It accepts neither a Session-to-Whole escalation nor destruction
on an uncaptured host; the checked UI/API route does neither.

Suggested bucket: highest if outside consent, unchanged; could-be-highest: yes because processes and files are
destroyed. Confidence: confirmed source behavior and exact specification acceptance, not executed. proposed_drop:
explicit specification acceptance quoted above, still requiring the caller's independent check; not dropped by this
synthesis. possible_cover: SPEC.md Sandbox sessions only; no excluded coverage sources searched. Caveats: Session-scoped
reachable-environment Deletes retain one request per marked session and keep an Unasked failure on its row in Bulk; no
broader loss is accepted. No runtime tests or reproductions run.

Returned proposal (claims-021, event 9): **covered**, suggested bucket **highest**.

The destruction of unmarked/unlisted peers is established, but exactly matches the warned whole-resource consent the
specifications accept. Keep the proposed drop in the independent register; no drop is authorized by this consultation.

Original stronger consequence: Bulk whole-environment Delete destroys unmarked/unlisted sessions, processes, files and
unpushed work, even if shrink-only admission changes the representative. Highest if outside consent; that original
consequence is preserved.

Unresolved premise or limitation: Independent fresh-context agreement is still required before dropping this
highest-capable claim. Coverage applies only within captured whole-resource consent, not to Session-to-Whole escalation
or a different host.

Proposed basis: Exact explicit specification acceptance plus drawn warning and scope-preservation source checks. Do not
use this proposal as an independent drop verdict.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-warned-whole-resource-unmarked-peer-destruction. Full021
andparentactualSPEC1485-1555: bulkwhole-resourcescopecapturedhost/dedup
andwarningexplicitlyalllisted/unlistedprocessesfilescheckoutsunpushedwork, narrowerresume/updateremedy;
APIseparateoptins/noBulkUnaskedescalation. SPECexplicitpermitswholeforofflineatmodalopeningonceperhost; no complete
membershiprequired. Proposedexactpriorwarnedscopeacceptanceonly, notSession→Wholeorhostretarget. Original
unmarked/unlistedworklossremains highestoutsideconsent; freshgatecheckwordingandcapturedscope,
distinctbulkeligiblepeerrefusalnotcover.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g036 record](records/review-through-00de0b6c/groups/g036.json). Record ID: `c1`.
