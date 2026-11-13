# Power loss can lose a binding after external creation under accepted WAL durability

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Power failure or OS crash can lose a recent registration or pending-create record while its container survives.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g064/c3`.

### g064/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed foundation commit: 04ffdbb35773cef36e73eb798c005ddbeb9a9de6; integrated commit: 56659839785691f76da3af5d8f51b511cff4ba8b; checked through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: Power failure or OS crash can lose a recent registration or pending-create record while its container survives. Details: register_container at store/podman.rs:373-394 and set_podman_prepared_create at :268-278 use ordinary connection calls. The latter atomically records observed login, selected installation facts and pending-send marker. environment/podman.rs:1103-1125 awaits admission before external run and :1160-1165 then saves canonical ID. provisioning/tensorlake_add.rs:391-398 registers before provider work, :452 holds the host provisioning lock, and :521-555 prepares before create. Db::call in crates/farhelm-supervisor/src/db.rs:230-246 returns work from a connection-locked blocking task without strengthening synchronous mode; configure_durability :104-120 selects NORMAL in WAL mode. Db::call_full_sync_if :249 onward is the separate trust-revocation barrier and is not used here. Thus process-crash preservation and pre-send ordering hold, but a power failure can erase their commit tail after external creation. destroy at environment/podman.rs:1298-1301 treats a surviving row with neither ID nor marker as never-created instead of claiming a same-name resource, and :1338-1380 discovers unregistered owned resources by helm label at reachable places. No actual destruction of user work established. Endpoint diffs leave storage and creation ordering unchanged. Suggested bucket: other for established lost registration/cleanup visibility. Confidence: confirmed by source/contracts; no execution. proposed_drop: specification acceptance, for independent parent check. Exact SPEC_impl.md, Helm internals, :5670-5672: 'every ordinary read-write connection selects `synchronous=NORMAL` only after reading an actual WAL mode and otherwise retains FULL. This accepts losing the last commits on an OS crash or power loss without weakening database consistency; process crashes preserve commits.' The following FULL exceptions cover acknowledged trust revocations, not resource registration. possible_cover: g065/g068 own integrated provider/store changes; parent should pass this durability invariant and proposed drop to them. Could-be-highest: preserve original potential consequence of losing track of user work if an actual security or loss-of-work route is established; none is established here. The initial missing-caller rejection is superseded. This record is a finding because specification acceptance is the proposed-drop basis; no drop has been accepted.

Returned proposal (claims-041, event 10): **covered**, suggested bucket **other**.

The integrated route can lose recent ordinary registration/create commits after an OS crash or power loss while the external resource survives. This is the ordinary WAL/NORMAL durability limitation the implementation specification explicitly accepts. The initial no-caller rejection is withdrawn; coverage, rather than source refutation, is the proposed basis.

Original stronger consequence: Losing track of still-running user work could potentially cause security harm or actual loss of work. The history preserves this possibility but establishes neither harm route.

Unresolved premise or limitation: An actual security or destruction-of-user-work route beyond lost registration/cleanup visibility is unestablished. This is not a claim that external resources cannot outlive their rows.

Proposed basis: Exact ordinary-store durability acceptance, limited to OS crash/power failure tail loss, not process crashes, missing pre-send admission, or arbitrary ownership loss. Parent must independently approve coverage and the absence of an established highest consequence.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: ordinary-WAL-NORMAL-power-loss-policy-cover-pending. Full041 real integrated registration/create commits can be lost after OS crash/power failure while external resource survives; initial no-caller rejection withdrawn. Ordinary Db::call uses WAL/NORMAL; pre-send awaited admission/order stays correct, FULL trust-change path separate. Exact global implementation durability rule explicitly accepts ordinary tail loss without database inconsistency, no separate Podman barrier shown. Tensorlake provider-specific orphan paragraphs are corroboration only, not exact Podman permission. Preserve original possible security/work consequence and resource outliving row; gate precise OS/power scope, not process crashes or missing admission. No actual harm route beyond lost ownership visibility established.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g064 record](records/review-through-00de0b6c/groups/g064.json). Record ID: `c3`.

