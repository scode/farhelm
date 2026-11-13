# pending-cleanup integration oracles retire before tmux runtime support

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: this intermediate change removes deterministic integration checks for still-supported old terminals. The confirmed impact is reduced regression protection, not an observed failure in Farhelm.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g155/c5`.

### g155/c5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: this intermediate change removes deterministic integration checks for still-supported old terminals. The confirmed impact is reduced regression protection, not an observed failure in Farhelm. Details: the assigned patch deletes close_notifies_and_removes_a_tab_while_output_cleanup_is_pending (parent terminal_tabs.rs:1044) and restart_restores_and_notifies_while_output_cleanup_is_pending (parent :1761). Both configured forwarder_cleanup_gate to hold cleanup-result publication. At the reviewed revision, a pinned search across crates/farhelm/tests and crates/farhelm-supervisor/src finds that hook only in its definition (service/core.rs:900) and production use (service/connection.rs:1575-1590), with no remaining test configuring it. The Close request still reaches detach_closed_tab after its window is gone (service/core.rs:15124-15128); that helper notifies TabClosed before returning pending cleanup (:15158-15198). Restart still removes the entry (:13312), converts detach_for_restart failure into a definitive relaunch failure (:13321-13342), then restores its prior outcome and republishes the entry (:13430-13456, :13544-13552). The helper itself sends the failed-restart detach notice before returning the pending-cleanup error (:13885-13906). The deleted tests were request-level oracles for these combined outcomes, beyond proving the lower-level barrier data structure. Surviving output-reap tests at service/terminals.rs:3207-3335 manually publish watch-channel results; the sink lease test at :4452-4509 constructs a synthetic handle and leases. Those retain useful unit coverage but do not invoke these real Close/Restart branches with forwarder cleanup publication held. Retained host Stop/consent/dead-open/restart tests use the other substrate. SPEC_impl.md:2370-2411 still supports mixed tmux/host sessions at this intermediate revision. Later dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 ends old terminals before reload and calls no_server_after_startup (core.rs:6426-6438), changing the product contract; 1ed482dde220452e424973faf38a5e1318986333 removes the tmux driver. Inspection through tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db found the old driver paths remain retired; the only later changes to these assigned test paths are 1ed482d and comment cleanup in 08399d092d20815ced6619c26df2391944ae2b4a. Thus this is an intermediate test-suite gap, not a current-tip missing oracle for an active tmux runtime. Keep the boundary tests through runtime retirement, or carry equivalent request-level tests while that runtime still ships. Suggested bucket: other (maintainer test coverage only). Confidence: confirmed by pinned source and targeted hook/caller searches, no runtime execution or reproduction. proposed_drop: false. possible_cover: caller reports a related intermediate migration-suite gap from g152; exact overlap is not established and no coverage sources were searched. Could-be-highest: no established product security/loss-of-work route. The original concern about lost lifecycle regression protection is preserved; no claim is made that every removed old-terminal contract lacks coverage, or that existing behavior is broken. Retained host/unit coverage refutes only complete loss. No parent drop has been accepted. Readily checkable caller and retirement premises are resolved; this ID is no longer an unresolved lead.

Returned proposal (claims-099, event 13): **retain-verified**, suggested bucket **other**.

The two deterministic request-level cleanup oracles disappear while old terminals are still served. Retain the intermediate test-suite gap; retirement later in this range limits its duration but does not refute it. No product failure is established. g156/l1 concerns different removed recovery scenarios, not these Close/Restart assertions.

Original stronger consequence: Initial concern included loss of all tab cleanup, consent, dead-open and restart-survival protection. Retained host tests and synthetic barrier/registry tests refute that breadth. The first proposed drop based on retained coverage/no runtime defect was superseded; the specific old-terminal request oracles remain missing at the intermediate revision.

Unresolved premise or limitation: None for the bounded missing-oracle conclusion. A shipped cleanup/consent failure would require separate evidence and is not inferred.

Proposed basis: Record test-only intermediate regression-protection loss, preserving later retirement and surviving lower-layer/host coverage. No product-loss consequence was established or lowered.

Existing parent disposition (event 13): rejected. Full099:647 deletes deterministic forwarder_cleanup_gate Close/Restart request tests, includingTabClosed/listabsence andfailedRestart/exited-row restoration while oldservice supported. Synthetic result/barrier tests do not cover same request-publication boundary; no productbug needed to establish missing oracle. dbfc endsdriver andtiphasnoneofgate/sink/reapmachinery; preserve trueintermediategap but no obsoletefixturefixcurrentqueue. Distinct156real recoveryoracles andhostgeneric testsremainuseful. No security/workloss route.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g155 record](records/review-through-00de0b6c/groups/g155.json). Record ID: `c5`.

