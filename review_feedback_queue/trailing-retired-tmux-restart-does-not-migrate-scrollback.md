# Retirement proposal retained; resized-seed excerpt counterexample verified

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Retirement proposal retained; resized-seed excerpt counterexample verified

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g184/f1`.

### g184/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Original wording and consequence retained: removing the tmux history-seed API could leave a restarted older session without its prior scrollback if a supported restart path still promises or consumes it. Known retirement consequence remains abandonment of old tmux scrollback, potentially highest if unaccepted. Exact proposed acceptance, SPEC.md:1945-1947: When the terminal no longer exists (the process that kept it ended, after a reboot, or an old terminal this release ended, see [Terminal experience](#terminal-experience)), Restart starts with an empty one. SPEC.md:3016: After an update to this release, Restart, Clone and Replace start such a session in a new, empty terminal. proposed_drop: this exact acceptance applies to retirement only, still pending independent checking. possible_cover: pinned specification for retirement; caller-supplied g013/f2 owns the distinct resized-seed launch-excerpt finding. Could-be-highest: yes for the original retirement consequence; the diagnostic-attribution counterexample has no established security or loss-of-work effect. Confidence: source/contract analysis only. Synthesis correction verified at this same revision: emulator.rs:224-226 delegates resize to Term; vendor/alacritty_terminal/src/term/mod.rs:756-758 resizes both grids; grid/resize.rs:25-28 and46-71 promotes up to the added height from primary history into visible rows. hosts/terminal.rs:2341-2353 queues valid sizes without checking launch-spec consumption,3872-3886 applies the journal resize and4063-4070 calls Emulator::resize. handlers.rs:1906-1912 resizes on attach and2058-2084 accepts owned-viewer resizes. host_launch.rs:579-645 waits for host start/adoption only, then tracks the pending spec; host_restart.rs:294-388 publishes this host without awaiting login-shell spec consumption. status.rs:625-653 invokes append_launch_excerpt for an ended unconsumed-spec launch; launch_artifacts.rs:59-77 reads that visible screen and96-102 selects its last five nonblank rows, including any promoted prior-run history that remains. Therefore my prior blanket assertion that seeded prior scrollback is not the launch excerpt is withdrawn: only the initial seed is blank. A larger viewer before a quiet login-shell failure can yield the misleading old-output excerpt g013/f2 reports. This corroborates a supplied known interface finding, not a new discovery or additional local bug ID. It does not invalidate the separate host-seed retention, frozen-format or surviving-test evidence. The retirement acceptance does not excuse diagnostic attribution. Original source-refuted claim and its correction remain in the append-only history. No readily checkable premise remains open for this interface correction. No drop accepted; no runtime test or reproduction performed.

Returned proposal (claims-115, event 12): **covered**, suggested bucket **highest**.

The old-tmux history retirement is real and explicitly accepted. The host-to-host history-retention extension is source-refuted. This proposed coverage applies only to retirement; it does not dismiss the distinct resized-seed excerpt defect supplied as g013/f2. Preserve the line-11 withdrawal and line-12 correction of the earlier blanket excerpt refutation.

Original stronger consequence: Removing the tmux history-seed API could leave a restarted older session without prior scrollback if a supported path still promised or consumed it; this could lose user-owned terminal history. The broader suspicion that retained host seeds were removed was later refuted. Earlier synthesis also claimed seeded history could never reach a failed-launch excerpt; that blanket assertion was withdrawn and the resize counterexample verified.

Unresolved premise or limitation: None for the retirement consequence or the surviving host-seed chain. Actual occurrence and frequency of the resized-seed login-shell-failure excerpt were not measured; source establishes the conditional route. g013/f2's own record and disposition were not supplied as this batch's input.

Proposed basis: Exact specification coverage for old-tmux history retirement only, plus source refutation of removal of the retained-host seed path. Keep the original potential user-work consequence in the independent drop register. Do not drop or lower g013/f2 through this disposition.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g184 record](records/review-through-00de0b6c/groups/g184.json). Record ID: `f1`.

