# Old tmux terminals lose access and work under the explicit migration policy

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: an update removes access to old terminals and their retained screens, and ends the programs still running there.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g118/f1`, `g159/f1`, `g160/c1`.

### g118/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 6 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: an update removes access to old terminals and their retained screens, and ends the programs still running there. Details: the assigned patch deletes the tmux terminal variant, discovery and resolution, including the former resolve_terminal_inner tmux branch (parent terminals.rs:973–1004); the replacement at terminals.rs:242–247 resolves host tabs only and agent_terminal:263–274 serves host handles only. Startup enacts that policy: service/core.rs:6142–6151 calls OldTerminals::end_private_server before reload when holding the state-directory claim. core/old_terminals.rs:176–231 connects to the old socket, identifies the same-user peer and invokes verified termination; :247–251 removes server files after confirmed termination. Suggested bucket: highest if treated as a defect, because processes and retained terminal work are lost; could-be-highest: yes. Confidence: confirmed source and pinned specification. proposed_drop: explicit acceptance in SPEC.md, Terminal experience, lines 3006–3016: "This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program." The same paragraph says "one whose agent had already ended keeps its outcome, but its terminal's last screen, which lived in the server, is gone; old tabs are closed." SPEC_impl.md, Terminal substrate: private tmux server, lines 2882–2888 also explicitly says the old terminals are ended and the driver is gone. possible_cover: those exact pinned specification sections; parent reports likely duplicate retirement/history-loss records in g099/g110/g113/g053, which this reviewer has not read or independently matched. This remains a proposed drop only: no parent drop has been accepted and no event is withdrawn or lowered. Coverage-repair turn redelivered and re-read all 1,146 mandatory patch lines as direct stdout; source bytes and hash are unchanged, and the conclusion is unchanged. Caveat: this record covers the authorized migration loss itself, not unintended destruction of host terminals or unsafe cleanup; other groups own the complete startup-cleanup implementation review. No runtime execution or observed behavior. Parent must independently check this highest-impact proposed drop.

Returned proposal (claims-076, event 6): **covered**, suggested bucket **highest**.

The source does enact the loss of old terminals, their screens and running programs. The current product and implementation specifications explicitly accept exactly that migration. This does not cover unintended host-terminal destruction or unsafe startup cleanup.

Original stronger consequence: Update ends programs still running in old terminals and discards old terminal screens/tabs: loss of processes and retained terminal work, highest if treated as a defect. No withdrawal or lowering has occurred.

Unresolved premise or limitation: None for intentional migration behavior and exact specification cover. Safety of every startup-cleanup operation is not part of this acceptance.

Proposed basis: Explicit specification acceptance of this exact old-terminal migration; independent highest-impact coverage check must agree before dropping.

Existing parent disposition (event 6): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g118 record](records/review-through-00de0b6c/groups/g118.json). Record ID: `f1`.

### g159/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 6 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. TLDR: Updating ends old tmux terminals, their running agents and tabs, and their retained screen/scrollback. If retirement fails, the session is still treated as ended and Restart can start a second agent beside one still running. Suggested bucket: highest (established process/work loss); could-be-highest: yes. Confidence: confirmed by source and the explicit product contract; not observed in execution. proposed_drop: the specification explicitly requires retirement and explicitly accepts the duplicate-agent fallback. This remains a proposal only: no parent drop has been accepted. Exact text, SPEC.md:3006-3007, Terminal experience: "This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program." SPEC.md:3008-3010 says: "one whose agent had already ended keeps its outcome, but its terminal's last screen, which lived in the server, is gone; old tabs are closed." Exact text, SPEC.md:3013-3015: "A failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still running." SPEC_impl.md:2965-3003 specifies this startup lifecycle and failure residual. Source: service/core.rs:6426-6438 ends the private server before reload when state ownership is held, then unconditionally calls no_server_after_startup; the assigned tmux.rs addition sets presence absent; tmux/presence.rs:192-197 stores Absent and publishes it. The earlier lost-work, terminal-history-loss and duplicate-Restart consequences are all preserved, not lowered because they are intentional. possible_cover: parent reports overlap with g162/g164/g099; no queue coverage sources were searched. Material caveats: this group establishes the removal/interface consequence and the exact specification acceptance, not independent correctness of the replacement cleanup's peer identification, process attribution or termination. All assigned source packets have now been redelivered through the corrected evidence manifest; their bytes and hashes are unchanged.

Returned proposal (claims-101, event 6): **covered**, suggested bucket **highest**.

The specified upgrade deliberately ends old agents and tabs, discards their retained terminal contents, and accepts a second agent after failed retirement. These are established work-loss consequences, not consequences lowered because they are intentional. Current and reviewed specifications explicitly accept the exact trigger, consequence and old-terminal scope. This proposal requires the separate independent drop check.

Original stronger consequence: Updating ends running old-terminal agents and tabs and loses retained screens/scrollback. Failed retirement can leave an old agent running while Restart starts another. All remain highest consequences for this proposed coverage decision.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact explicit specification acceptance; consolidate g160/c1 with this record without erasing either record's consequences or history.

Existing parent disposition (event 6): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g159 record](records/review-through-00de0b6c/groups/g159.json). Record ID: `f1`.

### g160/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Original TLDR retained: Users upgrading with old tmux-backed sessions lose those running terminals and their retained screens; restarting them starts an empty terminal. Could-be-highest: yes, ending user-owned processes and losing retained output would ordinarily qualify. Suggested bucket: highest if contrary to the upgrade contract. Confidence: confirmed source and specification analysis; no runtime behavior observed. Proposed_drop: exact specification acceptance, additionally retirement already exists in this commit's parent. The drop is proposed only and has not been independently accepted. SPEC.md:3004-3016 explicitly says: 'This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program.' It also says: 'After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal.' The known residual retains its provenance rather than being counted as new discovery: the same paragraph says, 'A failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still running.' SPEC_impl.md:2882-2889 states the supervisor runs no tmux and its driver is gone. The assigned patch removes driver exports and the HISTORY_LIMIT/restart-seed invariant from tmux.rs. Parent source already calls OldTerminals::end_private_server at core.rs:6434 and resolves TerminalKind::Tmux as None at 6969; ending old terminals predates this deletion. Reviewed source preserves startup ordering at core.rs:6142-6184 and distinct host lookup at 6675-6715; old_terminals.rs:300-342 sweeps AgentOnly or individual Tab targets, excluding new host tab records at 399-402. Synthesis traced the retained host-only lifecycle: host_restart.rs:216-257 supplies an old-host history seed or an empty default to the new host; 294-388 confirms/publishes the replacement before ending the held old host; restart_seed:418-490 uses the held host runtime, with explicit bounded/gone-history behavior. The assigned source supplied no distinct route to missing update warnings or misleading update-survival guidance, so retirement acceptance was not used to settle such a separate defect. Wrong-target fallback and later unconfirmed Delete were instead checked against source and retained test assertions under l1 and c2. Possible_cover: none. Caveats: the original loss-of-work consequence and accepted duplicate-agent residual remain for independent checking. No accidental destruction of already-host-backed sessions was established. This is source-only analysis of deletion interfaces, not an exhaustive independent review of all replacement code.

Returned proposal (claims-101, event 14): **duplicate**, suggested bucket **highest**.

This is the same intentional retirement, retained-screen loss, empty restart and accepted duplicate-agent fallback as g159/f1. Consolidate without losing its explicit caveat that no accidental destruction of already-host-backed terminals was established. The deletion commit's parent already retired old terminals, but retirement is still inside the full review range; do not mark it out-of-range on immediate-parent attribution alone.

Original stronger consequence: Ending old-terminal agents and tabs loses running work and retained screens; Restart starts empty. Failed cleanup can leave an old agent running beside a restarted one. The highest consequence is preserved despite explicit acceptance.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact same-root alias of g159/f1, with explicit specification acceptance. Separate independent check required before either record is dropped.

Existing parent disposition (event 14): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g160 record](records/review-through-00de0b6c/groups/g160.json). Record ID: `c1`.

