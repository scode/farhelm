# Snapshot-owner PID reuse can bypass host-tab exclusion

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Snapshot-owner PID reuse can bypass host-tab exclusion

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g109/c2`.

### g109/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. Original TLDR/strongest concern preserved: old-terminal startup cleanup can unintentionally stop an already-host-backed shell tab and lose its processes or user-owned work. Could-be-highest: yes; suggested bucket: highest for the conditional wrong-target termination route. This is reconciliation of received g097/l4, not a newly discovered independent issue. Actual source checks: old_terminals.rs:457 records pid/ppid/start identities, :464 reads the current environment of each old direct-child pid with no intervening start-identity check, and :473-476 associates the stale subtree identities with that current owner. If a short-lived old pane exits/is reaped and its number is reassigned to a newly started host-tab child, the new session/tab markers can enter PaneOwner::Tab. Ordinary shipped marker premises were checked in the prior c2 revision: core.rs:14690-14697 and host_tabs.rs:629-641 inject the session and tab UUIDs, and launch.rs:1289-1303 removes the agent marker before shell exec. The direct owner list at old_terminals.rs:369-375 bypasses the retained-host-record exclusion at :399-401. The call from assigned core.rs:7338 therefore reaches old_terminals.rs:329-342, which names the misattributed session/tab's scope and exact marker, even though sweep.rs:1595-1646 discards the reused old root as Ok(None); marker/scope discovery continues. Server-process verification at old_terminals.rs:223-277 gates using any subtree and correctly discards AlreadyGone/mismatch/timeout cases, but does not validate each child's owner identity. Confidence: source-confirmed conditional route; no actual PID reuse, termination or work loss observed. Prior ordinary-path refutation is retained and narrowed: accurate attribution plus AgentOnly and retained host records protect normal tabs. It does not refute this separate owner-misattribution path; neither unreadable record contents nor deliberate marker tampering is needed. proposed_drop: only possible bounded-PID-race acceptance, pending independent interpretation; no parent drop accepted. Exact SPEC_impl.md:5431-5435: 'Process-number reuse inside a short window is accepted, for now.' and 'Code that signals or adopts a process by its number may lose the race in which that process exits and the kernel hands the number to an unrelated process within a short, bounded window: between reading a number and checking it, between reaping a child and signalling its group, while a watchdog fires just as its child finishes, or during a polling loop of at most a few seconds.' Exact exclusion at :5438-5439: 'What is not accepted is carrying a bare number across unbounded waits or asynchronous work, or storing it for later.' The accepted-result window here is bounded by the absolute 15-second old-terminal deadline at :204-233, longer than the few-second polling example; child owner attribution itself runs synchronously, whereas the resulting owner is used later after reload work. These boundaries must be judged independently rather than treating rarity as a refutation. possible_cover: g097/l4 is the same snapshot-owner cause; preserve both IDs/provenance for parent deduplication. No prohibited coverage sources, runtime tests or reproductions. All readily checkable source premises examined; close the renewed c2 lead as this finding. No claim that the separate metadata-error caveat or every cleanup path is defect-free.

Returned proposal (claims-071, event 9): **retain-verified**, suggested bucket **highest**.

The conditional wrong-tab termination route is supported. Accurate markers and retained records protect ordinary attribution, but do not protect a tab UUID admitted directly from a recycled pane PID's environment. No execution or work loss was observed.

Original stronger consequence: Old-agent cleanup could kill a host-backed shell tab and lose its processes or user-owned work. The earlier general dismissal was narrowed and the owner-misattribution route renewed; do not reinstate the earlier dismissal silently.

Unresolved premise or limitation: No missing source step for the conditional route. Actual PID reuse in this interval was not observed; the scope of the PID acceptance and upgrade filter needs the parent's independent disposition.

Proposed basis: Keep the conditional highest finding, reconcile the history-supplied alias, and independently check any proposed specification/filter dismissal or reliance on the earlier ordinary-path refutation.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g109 record](records/review-through-00de0b6c/groups/g109.json). Record ID: `c2`.

