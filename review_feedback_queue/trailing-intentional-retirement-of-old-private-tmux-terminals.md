# old tmux terminal retirement loses retained history and ends programs, explicitly accepted

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Upgrading a supervisor with sessions still in old tmux terminals ends their programs and discards their retained terminal history; a later Restart starts with an empty terminal.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g099/f1`.

### g099/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

TLDR: Upgrading a supervisor with sessions still in old tmux terminals ends their programs and discards their retained terminal history; a later Restart starts with an empty terminal.

Details: The assigned patch removes tmux_restart_seed, Replacing::TmuxWindow and the tmux seed's CancelOnDrop. old_terminals.rs:159-168,176-233 ends the old private server. host_restart.rs:199-219 handles a held host runtime or none and defaults to no seed. core.rs:12486-12493 reads the held host's seed before claiming the new generation, and 13000-13028 forwards that runtime and seed.

Literal specification acceptance: SPEC.md, Terminal experience, 3006-3010: "This release ends them: the first time a host's supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal, without needing a tmux program." At 3013-3016: "A failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still running. After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal." SPEC.md, Lifecycle operations, 1945-1947 also accepts an empty Restart after an old terminal this release ended; SPEC_impl.md:2880-2932 describes retirement and its failure residuals.

Synthesis source checks: Removing the tmux-specific CancelOnDrop does not remove the retained host seed's ownership/cancellation mechanism. hosts/terminal.rs:2443-2450 sends the seed request to the existing runtime; 3155-3161 handles it there. seed_now at 3733-3766 owns the blocking encode, uses the runtime's cancel token, and brings the emulator home even if the request's answer receiver disappears. host_restart.rs:418-490 waits for settled output, requests MAX_SNAPSHOT, and refuses a missing seed during supervisor stopping. host_restart.rs:294-387 publishes the new host before ending the old one; core.rs:12805-12807 bypasses restoration after publication. Definitive restoration at 12933-12948 retains the previous host under publishable; ambiguous restoration names no terminal and aborts the previous runtime. These checks establish preservation of these specific host-seed/caller contracts, not blanket correctness of every failure path; l1 separately records an unchanged unwind-policy concern.

Suggested bucket: highest, unchanged (programs and retained history can be lost). Could-be-highest: yes. Confidence: confirmed by source/contract analysis; no execution. proposed_drop: literal specification acceptance of deliberate retirement and empty Restart only; independent drop check still required and no parent drop accepted. possible_cover: parent reports likely duplication with g053/f1, not independently read/matched here.

Material caveats: This proposal does not settle misleading survival guidance (distinct g004/g071 concerns identified by parent), hidden failure warnings (g033), or independent startup cleanup bugs. Those require their own trigger/consequence analysis. The retained host seed was checked in source; nothing was reproduced or observed at runtime.

Returned proposal (claims-065, event 10): **covered**, suggested bucket **highest**.

Termination of old tmux programs and loss of their retained terminal contents are expressly accepted. This proposal is limited to deliberate retirement and empty relaunches.

Original stronger consequence: Upgrading with programs in old tmux terminals ends those programs and loses their retained output; Restart is empty. Keep highest until the independent acceptance check agrees. This does not cover misleading survival guidance, hidden failure warnings, or independent startup cleanup defects.

Unresolved premise or limitation: Independent drop agreement remains required. Any exact alias to g053/f1 remains for the parent to match.

Proposed basis: Propose covered only for the exact deliberate retirement/history-loss consequence accepted in SPEC.md Terminal experience and SPEC_impl.md Ending the old private tmux server.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-old-terminal-retirement-and-empty-relaunch. Full flagged065 read. Exact deliberate old-server program and screen loss, subsequent empty relaunch and nonfatal failed retirement are expressly accepted; preserve highest pending gate. This does not cover misleading survival guidance, wrong victim selection or independent host-history seed failures. Reported053f1 alias still needs the actual join.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g099 record](records/review-through-00de0b6c/groups/g099.json). Record ID: `f1`.

