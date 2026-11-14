# Retired helpers refuse both old CLI entry and old queued isolation intent

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: old container helper invocations are intentionally unavailable; synthesis also checked that a
queued old launch cannot become an ordinary outside-host command after removal of its container implementation. Existing
accepted access consequence remains high/material UX. Literal SPEC.md:4819–4820 contract: 'Existing sessions support
Delete only. Opening terminals or tabs, exec, Stop, Restart, Clone, Replace and Replace with are refused.' Literal
cleanup contract: 'Delete retains the legacy container-removal path and confirms absence.' The CLI test distinguishes
removal by InvalidSubcommand and UnknownArgument. More importantly, launch.rs:1171–1175 checks spec.container.is_some()
immediately after decode and before checkout preparation or exec, writes RETIRED_MESSAGE to the launch-failure status,
then returns. Exact source rationale: 'Deserializing an old spec must not erase its isolation intent and turn the same
argv into an ordinary launch on the outside host.' Ordinary tab-shell callers emit only directory and shell arguments
(launch.rs:760–780). Cleanup continues to derive ContainerRuntime from the stored launch (service/teardown.rs:533–550);
container.rs:107–166 retains the permanent creation-lock inode, waits for already-running old helpers, and revokes the
creation ticket before removal; its removal confirms absence via container exists status 1 (lines 168–194). This checks
the current-binary entry/queued-spec boundary and the retained cleanup handoff, not unrelated retired implementation
defects. Could-be-highest: yes for the source-refuted new concern of executing a pr

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g090/c4`.

### g090/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (candidate, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc. TLDR: old container helper invocations are intentionally
unavailable; synthesis also checked that a queued old launch cannot become an ordinary outside-host command after
removal of its container implementation. Existing accepted access consequence remains high/material UX. Literal
SPEC.md:4819–4820 contract: 'Existing sessions support Delete only. Opening terminals or tabs, exec, Stop, Restart,
Clone, Replace and Replace with are refused.' Literal cleanup contract: 'Delete retains the legacy container-removal
path and confirms absence.' The CLI test distinguishes removal by InvalidSubcommand and UnknownArgument. More
importantly, launch.rs:1171–1175 checks spec.container.is_some() immediately after decode and before checkout
preparation or exec, writes RETIRED_MESSAGE to the launch-failure status, then returns. Exact source rationale:
'Deserializing an old spec must not erase its isolation intent and turn the same argv into an ordinary launch on the
outside host.' Ordinary tab-shell callers emit only directory and shell arguments (launch.rs:760–780). Cleanup continues
to derive ContainerRuntime from the stored launch (service/teardown.rs:533–550); container.rs:107–166 retains the
permanent creation-lock inode, waits for already-running old helpers, and revokes the creation ticket before removal;
its removal confirms absence via container exists status 1 (lines 168–194). This checks the current-binary
entry/queued-spec boundary and the retained cleanup handoff, not unrelated retired implementation defects.
Could-be-highest: yes for the source-refuted new concern of executing a previously container-bound command on the
outside host; the current binary's explicit guard prevents that route. Suggested bucket for that hypothetical:
highest/security or loss of work. The actual specified helper unavailability stays high/material UX. Confidence:
confirmed source refutation for host-execution concern, specification acceptance for access removal. proposed_drop:
outside-host execution is refuted by the explicit pre-exec guard; helper unavailability is expressly required by the
literal retirement contract. possible_cover: none. No parent drop accepted; retained for independent check. Source-only;
no runtime observations.

Returned proposal (claims-059, event 10): **covered**, suggested bucket **high**.

Refusal of retired container entry points is explicitly specified. The separate stronger concern that queued old
isolation intent becomes an outside-host launch is source-refuted by the pre-exec guard.

Original stronger consequence: Line 5 considered accepted loss of retired access (high UX). Line 10 additionally
considers queued container-bound commands executing on the outside host (highest/security or work loss), refuted by the
launch guard. Both bases need preservation.

Unresolved premise or limitation: Outside-host execution would require bypassing the explicit old-container launch
refusal. No such route is established by the removed CLI dispatch.

Proposed basis: Exact specification acceptance for retired access; explicit pre-exec refusal for the stronger
isolation-loss concern.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-retired-CLI-access-cover-plus-source-refuted-queued-isolation-fallback. Full059
explicit retired entry-point refusal is product contract, with Delete locking/ticket revocation/absence-confirming
legacy teardown preserved. Stronger queued old-container command executing outside host separately blocked before
argv/checkout by decoded container guard. Cover access retirement and refute pre-exec bypass only, not all container
lifetimes; keep original high UX and later highest security/work theories as separate bases for independent gate, no
rarity filter.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g090 record](records/review-through-00de0b6c/groups/g090.json). Record ID: `c4`.
