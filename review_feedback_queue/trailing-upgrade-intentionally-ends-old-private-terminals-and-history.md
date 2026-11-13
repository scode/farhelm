# Proposed drop: ending old terminals is explicitly specified

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The upgraded supervisor stops agents and shell tabs still in old tmux terminals and loses their retained screen and scrollback; subsequent Restart starts an empty terminal. This could otherwise be loss of user-owned processes/work.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g008/f1`.

### g008/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

TLDR: The upgraded supervisor stops agents and shell tabs still in old tmux terminals and loses their retained screen and scrollback; subsequent Restart starts an empty terminal. This could otherwise be loss of user-owned processes/work.

Details: Assigned SPEC_impl.md patch lines 101-111 states that old tmux terminals end at startup; lines 429-501 remove the history-preserving tmux-to-host Restart path. The complete SPEC.md Terminal experience section at the same revision explicitly requires this breaking behavior, at lines 3003-3016: "This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program." It continues: "one whose agent had already ended keeps its outcome, but its terminal's last screen, which lived in the server, is gone; old tabs are closed." It also says: "After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal." SPEC_impl.md:2880-2930 describes the replacement cleanup/conversion contract. Suggested bucket: highest if unintended, because the consequence includes terminating user-owned processes and losing retained output; could-be-highest: yes. Confidence: confirmed source/contract acceptance, not runtime-observed. proposed_drop: explicit product specification acceptance quoted above; preserve for independent highest-drop check. possible_cover: none; no coverage sources searched. Caveat: this excludes independent cleanup ownership or implementation failures, which are not excused merely by the intended migration. Revision closes the initial lead by establishing the missing acceptance premise.

Synthesis source check: the immediate parent revision already contains the identical explicit acceptance in SPEC.md:3003-3016. Its core.rs:6434 already calls OldTerminals::end_private_server, and its core.rs:6969 already resolves TerminalKind::Tmux to no terminal. Thus this assigned documentation rewrite catches up to existing behavior and product acceptance; it does not itself introduce the loss-of-process/output transition. Original consequence and potential highest bucket remain recorded, with proposed_drop still requiring the caller independent check. No additional implementation defect is established by this concern.

Returned proposal (claims-003, event 7): **covered**, suggested bucket **highest**.

Terminating old terminals and losing their retained screen/scrollback is explicitly specified product behavior. The reviewed implementation-specification rewrite catches up to behavior introduced earlier within this review range; being pre-existing to that one patch does not itself make the concern out of range.

Original stronger consequence: Upgrade stops user-owned agents and shell tabs, loses old retained screen/scrollback and makes subsequent Restart empty. Preserve highest absent explicit acceptance; no runtime loss was observed. Later documentation attribution does not erase earlier in-range introduction.

Unresolved premise or limitation: None for explicit acceptance of this exact behavior. Acceptance does not cover stopping another owner's terminals, cleanup implementation errors, misleading later confirmations or effects beyond old terminal processes/output.

Proposed basis: Exact product acceptance of terminating old terminals and dropping their history, independently checked before exclusion. Scope excludes implementation ownership failures and later misleading destructive consent.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: covered. Full003in-rangedbfcintentionallyendsoldprivateagents/tabsandlosesserverheldscreen/history;1edSPECrewritesexistingin-rangebehavior. Exactproducttextacceptstermination,historylossandnewemptyRestart/Clone/Replace. Parentproposesexactacceptancedrop, notoutofrangebecausepredatesassignedpatch. Freshcheckhighestwork/outputloss andlimits: nootherowner/cleanupimplementationerror/laterfalseconfirmation accepted, ordinaryupgradealoneenoughso no rarefilterbasis.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g008 record](records/review-through-00de0b6c/groups/g008.json). Record ID: `f1`.

