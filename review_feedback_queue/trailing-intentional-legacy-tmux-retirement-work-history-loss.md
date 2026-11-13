# Retiring legacy tmux terminals destroys their retained output

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Updating ends old tmux terminals, their processes and retained screen/scrollback. Restart of such a session starts with an empty terminal. The assigned removal does not itself establish loss of a surviving terminal host's history or processes.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g163/c1`, `g164/f1`.

### g163/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

TLDR: Updating ends old tmux terminals, their processes and retained screen/scrollback. Restart of such a session starts with an empty terminal. The assigned removal does not itself establish loss of a surviving terminal host's history or processes.

Details: Original consequence retained: removing capture_pane_history removes tmux-to-host history transfer, while startup retirement destroys the legacy server's retained output and ends old work. Revised attribution: git diff --numstat of service/core/old_terminals.rs between this commit and its parent is empty; the parent already invokes OldTerminals::end_private_server at core.rs:6433–6438, calls tmux.no_server_after_startup, and returns None for legacy row lookup at :6965–6969. Therefore the deletion removes a path already unreachable through normal reload, rather than independently introducing the retirement. Suggested bucket: highest consequence considered, loss of potentially user-owned output/processes; could-be-highest: yes. Confidence: source-confirmed retirement consequence and narrow contract acceptance; no execution. proposed_drop: proposed only, parent has accepted no drop. Exact acceptance at SPEC.md:3006–3010: 'This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program.' SPEC.md:3016 says, 'After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal.' Exact exclusion at :3013: 'A session whose agent or tabs already run in terminal hosts keeps them.' SPEC.md:1938–1952 separately requires retained host scrollback, a blank new screen, rollback of a confirmed-ended failed replacement, and keeping tabs running.

Replacement lifecycle checks/results: core.rs:12381–12483 pins the old child and sweeps AgentOnly; :12486–12493 reads the old host seed before opening a generation. host_restart.rs:418–490 waits for settled final output, asks history_seed(MAX_SNAPSHOT), preserves old size, logs missing history and refuses cancellation during supervisor stop. Its :237–255 passes the blob to the spawned replacement; :294–388 publishes before ending only the captured replacing host. :740–782 confirms failed replacement cleanup and sweeps AgentOnly before definitive rollback. core.rs:12798–12975 skips restoration after publication, restores prior terminal only on definitive failure, leaves ambiguous replacement terminal-less, and retains shared tab cells. host_handover.rs:645–861 filters agent rows by Host kind, adopts by session and generation, and excludes Tab roles from earlier-agent cleanup. :875–980 checks session/agent/generation for earlier-host fallback and refuses an unreadable newer record; :992–1053 supersedes only an earlier Agent host of a confirmed later Host row naming another socket, never a current host or tab. These are source refutations of specific highest routes: confusing surviving tabs with the retired agent, deleting a current host as superseded, choosing older readable history over an unreadable newer record, or rolling back a successfully published replacement to its old terminal.

Startup retirement boundary: old_terminals.rs:290–349 sweeps converted legacy agents with AgentOnly and old tabs by Tab id, never WholeSession. :363–408 excludes recorded host tabs from scope-derived old tabs. The private-server shutdown is pinned to its peer/start identity and socket command line (previously checked :176–279); the retirement allowance does not authorize a wrong-target action. No such route from the assigned removal was established. Source-refuted wrong-target close routes remain separately recorded in c2. Exact accepted host residuals are separate from tmux retirement: SPEC.md:1942–1945 allows terminal-less ambiguous failed replacement and starts empty when old history cannot be copied; SPEC_impl.md:2417–2419 says an ambiguously failed restart can leave the old host/history alive while the next Restart starts empty. These residuals are preserved as caveats, not generalized permission to discard surviving host work. possible_cover: none. Independent checking of all proposed drops remains required; no runtime behavior was observed and no reproduction was written.

Returned proposal (claims-103, event 8): **covered**, suggested bucket **highest**.

Ending designated legacy terminals, their processes, screen and scrollback, followed by empty Restart/Clone/Replace is explicitly accepted. The acceptance excludes surviving terminal hosts and wrong-target termination. The strongest consequence remains highest pending the independent coverage check.

Original stronger consequence: Startup ends all work in the designated legacy tmux terminals and destroys their last screen/scrollback; subsequent Restart cannot recover that history. No waiver for host-terminal history or wrong-target kills is inferred.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact product acceptance of legacy-only destruction. Same root as g164/f1; preserve both provenance histories when consolidating.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g163 record](records/review-through-00de0b6c/groups/g163.json). Record ID: `c1`.

### g164/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Restart of an old tmux session opens an empty terminal; its old screen and scrollback are no longer recoverable. Details: assigned patch 00951-004.patch:25–150 deletes normalize_capture, PaneHistory and the history-capture support used by the removed migration path. This is potentially loss of user-owned terminal history (could-be-highest: yes), but SPEC.md at the reviewed revision, lines 3003–3017 explicitly accepts ending old terminals and empty restarts. Exact acceptance: "one whose agent had already ended keeps its outcome, but its terminal's last screen, which lived in the server, is gone; old tabs are closed." Also: "After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal." Suggested bucket: highest if treated as a defect; proposed_drop: explicit product acceptance of old terminal destruction and empty restart, requiring independent acceptance check. Confidence: confirmed source deletion and explicit specification acceptance; startup/caller validation complete for this removal, no runtime observation. possible_cover: none; coverage sources not searched. Caveat: the prior commit introduces the destructive startup transition; this patch removes the retired history migration, so this record concerns removal implications rather than an independent defect in retired code. Checked parent coordinates: normalize_capture begins at 5200 and PaneHistory at 5287. Completed source checks: service/core.rs:6141–6189 calls OldTerminals::end_private_server after opening the store and before reload; :6675–6679 unconditionally treats a stored tmux terminal as absent; :6805–6825 proposes TerminalEnded for active old-kind rows. service/terminals.rs:187–207 gives old terminal absence and restart guidance. No remaining supervisor references to PaneHistory or normalize_capture were found. These checks support that the removed migration path cannot recover a legacy terminal already ended at startup. All evidence is source/contract analysis, not runtime observation. The proposed drop remains subject to the caller independent acceptance check. Coverage-repair clarification: preserve the strongest removal consequence: startup intentionally ends every still-running legacy terminal and the processes in it, as well as losing its last screen and scrollback; restarting cannot restore the old terminal history. Exact additional acceptance at pinned SPEC.md:3006–3008: This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program. This is the preceding startup transition's consequence, not newly introduced killing in the assigned helper deletion. No parent drop has been accepted. possible_cover: parent reports likely overlap with g099/g110/g113/g118; not independently established and not used to dismiss this record. Synthesis: no source premise changes the proposed-drop assessment. Exact strongest acceptance, pinned SPEC.md:3006–3016: "  the user to move them. This release ends them: the first time a host's supervisor of this release starts, it ends that\n  tmux server, and with it everything that still ran in an old terminal, without needing a tmux program. A session whose\n  agent was still launching, running or being stopped in one becomes interrupted, as after a reboot, and gets the notice\n  Status describes; one whose agent had already ended keeps its outcome, but its terminal's last screen, which lived in\n  the server, is gone; old tabs are closed. Programs the old agents and tabs left running are stopped, for an agent by\n  the session's own record (its launch scope and process markers, as a Stop of an agent that already exited finds them)\n  and for a tab wherever its id can be found; what cannot be attributed to an old tab is left for the session's Delete,\n  which stops everything of the session. A session whose agent or tabs already run in terminal hosts keeps them. A\n  failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those\n  sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still\n  running. After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal.". The full consequence remains stopped legacy agent/tab work plus unrecoverable terminal history; it is not lowered for rarity or possible overlap. Independent acceptance/overlap decision remains with the caller. Source-only; no execution evidence.

Returned proposal (claims-103, event 10): **covered**, suggested bucket **highest**.

This is the same deliberate legacy-terminal destruction as g163/c1, including stopped processes and unrecoverable retained output. It is explicitly accepted for legacy terminals; preserve the strongest consequence and both histories in the independent coverage/deduplication check.

Original stronger consequence: Startup stops legacy agents, tabs and their attributed remaining work and loses all retained terminal history; Restart cannot restore that old screen/scrollback. This remains highest if treated as a defect.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification acceptance and same-root consolidation with g163/c1. Parent-reported overlap with other groups is not independently established here and is not a drop basis.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g164 record](records/review-through-00de0b6c/groups/g164.json). Record ID: `f1`.

