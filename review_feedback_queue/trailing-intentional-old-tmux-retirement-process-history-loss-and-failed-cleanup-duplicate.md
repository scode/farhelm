# Retiring old terminals ends their programs and loses their screens and history

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Retiring old terminals ends their programs and loses their screens and history

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g161/c1`, `g162/f1`.

### g161/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR, original wording preserved: Old terminal screens and programs no longer survive the upgrade, and a failure to end their server can allow a later Restart to start another agent. Original consequence includes terminal history loss and removal of live-pane/no-pid and unusable-server guards that formerly prevented unsafe restart assumptions. Suggested bucket: highest; could-be-highest yes, these programs, screens and history are user-owned work. Confidence: confirmed source and contract analysis, not execution. proposed_drop: explicit specification acceptance only, pending caller independent check; no drop accepted. Exact acceptance, SPEC.md:3006–3010 (line breaks normalized): "This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program. A session whose agent was still launching, running or being stopped in one becomes interrupted, as after a reboot, and gets the notice Status describes; one whose agent had already ended keeps its outcome, but its terminal's last screen, which lived in the server, is gone; old tabs are closed." Exact acceptance, SPEC.md:3013–3016: "A session whose agent or tabs already run in terminal hosts keeps them. A failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still running. After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal." Source: service/core.rs:6112–6117 opens the database before process actions, 6142–6151 ends the old server only with state-directory ownership before reload, 6675–6715 never looks up an old-kind pane address while separately preserving host evidence. The retired driver/parser/PaneState symbols have no surviving supervisor callers. Synthesis additionally traced restart through core.rs:12490–12493 (no seed without a runtime) and 13000–13028 (every relaunch uses a host), host_restart.rs:216–257 (blank default or supplied seed into host spawn), 294–388 (generation-fenced confirmation/publication and old-host retirement), and retained tab-host lookup/close and process identity paths recorded under c5. This establishes the old-terminal replacement contract without treating it as blanket permission for other losses. The code's old-kind row remains terminal-less and restart creates a new host; the known failed-retirement residual remains the explicit accepted possibility of a duplicate. Caveat: the acceptance does not settle misleading update-survival guidance, missing warning, wrong-target termination, or later unconfirmed Delete. No concrete distinct route for those arises from the assigned deletion; c4 and c5 preserve the specific investigated excerpt and wrong-target/cleanup consequences and their source refutations. possible_cover: neighbouring groups report the same deliberate retirement and duplicate residual, per caller; no queue/coverage metadata read. This is retained provenance, not new discovery. Synthesis turn 2 complete for this ID. No runtime reproduction.

Returned proposal (claims-102, event 10): **covered**, suggested bucket **highest**.

The original loss and duplicate-agent residual are real source consequences, explicitly accepted by both specifications. The deletion removes a driver already disabled by earlier in-range startup retirement.

Original stronger consequence: Upgrade ends old agents/tabs and their running user work, loses screens/history, and failed retirement can permit another agent beside a survivor. Highest remains appropriate to that original consequence; converting candidate to finding did not accept a drop.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification coverage only. It does not cover misleading survival guidance, missing warning, wrong-target termination, guarded Delete after failed retirement, private-spare cleanup, or host-terminal loss.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g161 record](records/review-through-00de0b6c/groups/g161.json). Record ID: `c1`.

### g162/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 5 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Upgrading ends old tmux-backed agents and tabs, potentially destroying user-owned work in their processes, and discards their terminal history. If cleanup fails, Restart can start a second agent beside an old one still running. Suggested bucket: highest; could-be-highest: yes (process/work loss), unchanged. Confidence: confirmed by source and explicit specification; no behavior observed in execution. Synthesis revision: startup ownership is a retained claim, not a probe. core.rs:6084–6117 takes StateDirOwnership before opening the store; 6147–6151 ends the old server only with that claim; 6172 passes ownership to reload; 6234 stores the same ownership in Supervisor. core.rs:7045–7047 gates the sweep on may_write before committing the conversion, and 7080–7090 gates artifact/notice work likewise. serve at 7916–7928 refuses without begin_serving on the retained claim; its second reload at 7986–8001 runs only after this gate and uses OldTerminals::none, not another server kill. Parent core.rs:6425–6438 already ended the server then called tmux.no_server_after_startup regardless of cleanup outcome. Thus deleting adoption and its unusable-server refusal does not introduce this accepted retirement policy. proposed_drop remains proposed only: SPEC.md, Terminal experience, says exactly (line wraps normalized): "This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program." It also says: "A failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still running." SPEC_impl.md:2880–2934, Terminal substrate: private tmux server, accepts this same bounded retirement/failure policy. Exact acceptance of the consequences is the proposed-drop basis; this does not claim processes or history are preserved. possible_cover: parent identifies duplicates g099/g110/g113/g118/g164, not independently read or dispositioned here. Caveats: no parent drop is accepted; retain this highest finding for independent checking. Acceptance does not settle g128's distinct question about later guarded Delete after a budget-surviving old agent, nor g111's distinct private-spare sweep question. Neither residual question is refuted or proposed for dropping by this record. No tests or reproductions performed. No new defect in the assigned deletion established.

Returned proposal (claims-102, event 5): **covered**, suggested bucket **highest**.

Exact same retirement and failed-cleanup duplicate consequence as g161/c1. Source establishes it, while the specifications explicitly accept it. It predates this driver deletion but begins within the full review range at dbfc6975.

Original stronger consequence: Upgrade ends old agents and tabs, potentially destroys their running user work, discards terminal history, and failed cleanup can leave a second agent beside a survivor. Highest unchanged; no accepted drop.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification acceptance, with duplicate reconciliation against g161/c1. Excludes later guarded Delete, private-spare sweep, wrong-target loss and host-terminal loss.

Existing parent disposition (event 5): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g162 record](records/review-through-00de0b6c/groups/g162.json). Record ID: `f1`.

