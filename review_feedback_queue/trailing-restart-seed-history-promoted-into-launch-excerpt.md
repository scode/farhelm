# Growing a restarted terminal can put inherited history in its launch-error excerpt

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Growing a restarted terminal can put inherited history in its launch-error excerpt

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g166/c2`.

### g166/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 20 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR
Considered whether deleting the resize, later-respawn and foreign-pane excerpt tests allows a failed launch to display another run's or another session's terminal text. The replacement reads the session entry's own terminal host, and each restart uses a fresh host and blank screen.

## Details
Assigned patch lines 1-106 and 204-303 remove assertions for reused tmux panes, including history invalidation on a queued resize. service/launch_artifacts.rs:59-76 reads entry.terminal directly and quotes only HostEvidence::Owned { dead: true }; it has no tmux target resolution or sibling-window fallback. hosts/terminal.rs:2229-2239 and 2272-2284 read that host handle's own emulator or its published view under its mutex. service/core/host_restart.rs:4-14 and 34-52 describe the one-child host lifecycle and seed only history into a blank screen. SPEC_impl.md, Launch failure classification, states: 'A host runs one child for its whole life, so everything on its screen is this launch's output; the screen is quoted only once the host's exit record has been processed, which follows the child's drained output, and a Restart's new host starts with a blank screen, so no quote reaches back into the previous run.' This refutes the removed reused-pane mechanism; attribution of host adoption itself belongs to the replacement's other review groups.

Suggested bucket: highest if unrelated private terminal text could be disclosed; could-be-highest: yes. Confidence: confirmed source refutation of the removed mechanism, no execution. proposed_drop: source refutation, no reused pane or tmux target lookup in the replacement excerpt path. possible_cover: none. Caveat: this is not a whole replacement-host adoption audit.

## Synthesis revision
Original suspected disclosure of another run or session terminal text remains preserved; no proposed drop is accepted. Checked the actual callers: status.rs:580-658 obtains evidence and launch artifacts from the same entry, id and generation, requires a present dead terminal for inferred wrapper failure, and then appends that entry's excerpt. handlers.rs:996-1043 waits on that entry's host final-state boundary before requesting the wrapper-failure quote. The host-only test a_never_started_host_launch_quotes_the_emulators_screen (core/tests/host_terminals.rs:174-217) asserts the specific complaint in the durable error. The final-state boundary test at :783-821 establishes that the child is already dead, asserts a wait is pending without the exit record, then sends last output and the record and checks completion and the text. hosts/terminal/tests.rs:1662-1716 separately distinguishes Info reporting an exit from the ordered exit record: it requires no early exit notification and then checks the final words. These retained assertions address surviving excerpt completeness and timing; the deleted late-resize test concerned a reused tmux pane and does not exercise a host mechanism. Host restart tests separately exclude the old screen from the new host (see c4 revision). No concrete new excerpt or prompt-identity route was found. This is source/assertion analysis, not executed test evidence; host adoption beyond the checked caller boundary remains other groups' scope.

## Discovery interface input
The supplied neighbour context g100/c1 reports a distinct history-promotion route: growing the restarted terminal can move inherited seed history onto its formerly blank screen before a shell-startup failure is quoted. This is not a new local discovery and needs source validation before retaining or revising c2. The original suspicion of another run's or another session's text remains preserved, including its original potential-highest disclosure consequence; processed exit ordering and one-child lifetime alone do not settle inherited-history provenance. The prior blanket refutation is provisional pending that check. Missing premise: verify the actual grow-lines behavior and publication/resize/quote path at the pinned source. No execution or reproduction.

## Interface synthesis resolution
Known neighbour g100/c1 is source-supported and supersedes this record's blanket previous-run refutation. Checked vendor/alacritty_terminal/src/grid/resize.rs:25-28,46-71: a height increase pulls up to the added height from history into the visible area. snapshot/v1/mod.rs:242-268 seeds old primary history above a blank screen, but does not mark that history as immune to later promotion. host_restart.rs:294-305,363-388 publishes the new host after readiness/confirmation, without waiting for shell startup to consume its launch spec. The already-read excerpt callers and screen-row traversal can therefore quote promoted old history if shell startup subsequently fails. Exit-record ordering proves completeness, not cell provenance. This is a known cross-group interaction, not new discovery.

Established consequence: a misleading same-session durable diagnostic containing previous-run text (suggested bucket other); no cross-session disclosure or user-work-loss route is established by the resize interaction. Original possible-highest unrelated-session disclosure wording and source refutation remain preserved above for independent judgment, and have not been silently lowered or accepted as dropped. Confidence: confirmed conditional source route, not executed behavior. proposed_drop: false for the previous-run attribution defect. The earlier source-based blanket refutation is withdrawn only to that extent. possible_cover: g100/c1 and its parent-supplied g013/f2 provenance describe this same route; the parent should consolidate them. The surviving tests checked in turn 2 establish ordered final output and blank initial seed, not resize-stable provenance. Neighbour whole-range attribution says the runtime route already exists at the exclusive base and the later contract/caveat rewrite is in-range; independent source provenance check remains with the group owning that changed helper/spec, so this ID does not claim this assigned test deletion introduced the runtime bug. No prohibited coverage sources or runtime reproductions.

Additional direct caller checks in this turn: handlers.rs:2060-2083 resolves the viewer terminal and sends an owned resize; hosts/terminal.rs:4058-4070 applies it to the emulator. host_launch.rs:508-518 passes the seed as initial_snapshot and :639-645 returns while tracking the pending launch spec. These close the readily checkable publication/resize premises of the received known history-promotion route. No new independent cause was discovered.

Returned proposal (claims-104, event 20): **duplicate**, suggested bucket **other**.

Its latest established conditional mechanism is identical to g165/c3: inherited same-session seed history enters the screen on growth and is quoted after startup failure. Consolidation must preserve its original potential-highest cross-session disclosure concern and withdrawn blanket refutation; independent check is required for that lowering.

Original stronger consequence: Initial suspicion included disclosure of another run's or another session's private terminal text, potentially highest. Later records withdraw the blanket previous-run refutation but establish only misleading same-session durable diagnostics. This consequence lowering remains unapproved pending independent drop check.

Unresolved premise or limitation: Unrelated-session private text requires another source premise not established by this seed route. Parent must also resolve the related in-range documentation scope; the runtime and blanket host rationale predate the exclusive base. Supplied references to g013/f2 and g100/c1 were not independently read, as they are outside this input batch.

Proposed basis: Exact same-root alias to g165/c3 for consolidation, not silent deletion. Independent judgment must cover the original cross-session-security refutation/lowering and preserve unresolved range/filter scope.

Existing parent disposition (event 20): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g166 record](records/review-through-00de0b6c/groups/g166.json). Record ID: `c2`.

