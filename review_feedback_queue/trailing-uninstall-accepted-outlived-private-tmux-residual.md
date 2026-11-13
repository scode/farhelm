# Uninstall can leave an earlier private tmux server running

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Removing a remote host can leave a private tmux server that already outlived an earlier supervisor stop.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g012/c1`.

### g012/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 4 (candidate, open):

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d. TLDR: Removing a remote host can leave a private tmux server that already outlived an earlier supervisor stop. Details: The changed Provisioning section removes the socket-addressed tmux stop; its replacement stops the unit control group and explicitly stops terminal hosts. backend.rs (crates/farhelm-helm/src/provisioning/backend.rs 3254..3352) confirms those are the stop helper's actions and that every terminal-host stop must return 0 before cleanup can complete. service.rs 2826..2842 dispatches the frozen farhelm_program and state_dir from StopSupervisor to that helper. A tmux server already outside the unit group can remain. Suggested bucket: other (residual process cleanup; no established security or lost-work route). Confidence: confirmed contract acceptance and implementation match, source-only; no execution. proposed_drop: SPEC_impl.md, Provisioning, 7860..7863 explicitly states: "Uninstall has no tmux step: a private tmux server that outlived a supervisor stopped before uninstall (a host never updated to a release that ends it at startup, or one whose startup ending failed and whose supervisor was later stopped) keeps running. The maintainer accepted that residual on 2026-11-11 when the step was removed." The f96dd4363d97dc7e07dea5b8947c96be6bf912e8 surrounding contract retains this exact acceptance; the later removal of a redundant Mac no-tmux sentence does not change it. possible_cover: that exact specification acceptance; no other coverage sources searched. Could-be-highest: no established route; this candidate concerns processes surviving an explicitly authorized uninstall, not incorrect termination or lost processes. Caveat: does not excuse unrelated missing terminal-host cleanup, wrong-path cleanup or termination beyond confirmation. Synthesis revision adds implementation and later-contract checks; original consequence, bucket and proposed disposition are unchanged.

Returned proposal (claims-005, event 4): **covered**, suggested bucket **other**.

Both current specifications explicitly accept precisely this surviving server after its supervisor was already stopped. The narrow cleanup consequence is real in the stated topology but accepted; it is not a finding about incorrect termination or missing terminal-host cleanup.

Original stronger consequence: Uninstall may leave an earlier private tmux server running. Both history entries retain this residual-process consequence and disclaim incorrect termination, missing terminal-host cleanup or loss of processes. Surviving processes are not established lost work.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Propose coverage only for the explicitly accepted out-of-control-group residual. Independent confirmation is conservative because the proposed dismissal concerns surviving user processes; it cannot cover termination beyond confirmation or another cleanup route.

Existing parent disposition (event 4): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: covered-same-root-g006/f2. Full005exactremoteuninstallstopped-supervisor/outofunitgroupprivatetmuxsurvivoracceptance inSPEC/impl. Terminalhoststopconfirmationremainschecked, noincorrectterminationorunrelatedcleanupaccepted. Same006f2rootneedsbothprovenance/sourcevariants, preserveuserprocessaccess/resourceconsequence, notautomaticallylostwork. Landedplan corroboratingintentnotfuturefix. Freshchecknarrowpriorstopresidualonly.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g012 record](records/review-through-00de0b6c/groups/g012.json). Record ID: `c1`.

