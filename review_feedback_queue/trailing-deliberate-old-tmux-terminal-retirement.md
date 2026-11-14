# Old terminal contents and running jobs are deliberately retired

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Old terminal contents and running jobs are deliberately retired

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g053/f1`.

### g053/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: f560058da19e02716e6f943883a9508b447aff4e; preceding status-documentation change
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. Original TLDR and consequence preserved: updating ends remaining old
terminals, their live agents and tabs, and their retained screens; the protocol no longer offers the old byte stream or
terminal-kind reporting. Suggested bucket: highest/loss of processes and terminal contents. Source: f560 lib.rs removes
StreamKind, TerminalKind, SessionInfo terminal-kind fields, TabInfo::kind, Attach::grid and Attached::stream. At dbfc,
service/core.rs:6434 calls OldTerminals::end_private_server; core/old_terminals.rs:176-279 checks the socket peer and
invokes terminate_process_within, removing server files on success; core.rs:7098-7115 converts
launching/running/stop-requested tmux rows with no sentinel through Transition::TerminalEnded. dbfc lib.rs:863-875
documents this classification. Contract: f560 SPEC.md:3003-3017, Terminal experience, and dbfc SPEC_impl.md:2954-3003,
Terminal substrate. Confidence: confirmed source and contract analysis, not observed in execution. proposed_drop:
explicit specification acceptance, requiring independent check; no drop accepted. Exact acceptance text from f560
SPEC.md: ‘This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server,
and with it everything that still ran in an old terminal, without needing a tmux program.’ Also: ‘one whose agent had
already ended keeps its outcome, but its terminal's last screen, which lived in the server, is gone; old tabs are
closed.’ The original caveat remains: ‘A failure to end the server or to stop what ran there is logged and never stops
the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then
start a second agent beside one still running.’ possible_cover: this exact specification acceptance only.
Parent-supplied related claims: g004/f1 concerns generic installer survival assurance during this destructive migration;
g033/r1 concerns failure-hidden duplicate agents. Neither is treated here as coverage or a duplicate of deliberate
retirement. Could-be-highest: yes, established deliberate process and terminal-content loss; rarity does not lower it.
Synthesis revision: preserved the process/content-loss consequence and its highest bucket while separating the three
interfaces. The helm-supervisor protocol-77 bump governs removed raw-stream negotiation; accepted old-page reload
governs page-helm rendering; neither relaxes terminal-host compatibility. Source comparison shows no changes to
crates/terminal-host/src or supervisor hosts/client.rs across the assigned interval, and old server retirement targets
tmux rows rather than justifying termination of existing terminal hosts. Caveat: this is the removal's consequence and
replacement-contract review, not an independent full review of startup cleanup. The failure-hidden duplicate-agent
branch stays in the quoted historical acceptance and belongs to g033/r1 for its separate defect analysis, not a new
duplicate claim here. Excluded coverage sources were not searched.

Returned proposal (claims-033, event 16): **covered**, suggested bucket **highest**.

Ending old processes, tabs and retained screens is an established deliberate consequence, explicitly accepted by both
specifications. Keep highest for the work/process/content consequence until an independent drop check agrees to this
narrow acceptance. This is distinct from installer survival wording and hidden failure/duplicate-agent claims.

Original stronger consequence: Updating ends live old agents and tabs and destroys their retained terminal screens.
Original highest is preserved across all wording revisions; the failure branch can leave an old agent running while
Restart starts another.

Unresolved premise or limitation: None for intended retirement/process/screen loss. This is not a complete independent
review of cleanup implementation, installer assurances (caller-supplied g004/f1), or the failure-hidden duplicate-agent
issue (g033/r1).

Proposed basis: Drop only the deliberate-retirement consequence as exact specification acceptance after independent
agreement. Do not alias it to g004/f1 or g033/r1, and do not excuse incompatible terminal-host wire changes with page
reload or protocol 77.

Existing parent disposition (event 16): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-deliberate-old-terminal-process-screen-retirement. Full033 and
parent-read actual current old-terminal passages explicitly require startup termination of old agents/tabs and loss of
retained screens, while separately accepting logged failure/second-agent residual. Established work/process/content loss
remains highest for gate, not denied or filtered as ordinary upgrade. Proposed exact intentional retirement acceptance
only;004f1 installer survival assurance and033r1 hidden cleanup failure are distinct. Protocol77/page reload is not
permission to break the permanent terminal-host protocol, whose relevant source did not change.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g053 record](records/review-through-00de0b6c/groups/g053.json). Record ID: `f1`.
