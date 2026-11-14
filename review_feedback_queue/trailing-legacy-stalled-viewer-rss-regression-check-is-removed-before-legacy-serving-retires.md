# Legacy stalled-viewer RSS regression check is removed before legacy serving retires

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: While this intermediate revision still serves old terminals, it removes the test that measures
whether their private tmux server and supervisor keep bounded resident memory during sustained output to a paused
viewer. This is lost regression-test protection, not a demonstrated memory leak in Farhelm.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g154/l4`.

### g154/l4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: While this intermediate revision still serves old
terminals, it removes the test that measures whether their private tmux server and supervisor keep bounded resident
memory during sustained output to a paused viewer. This is lost regression-test protection, not a demonstrated memory
leak in Farhelm. Details: the assigned deletion removes memory_stays_flat_while_a_viewer_is_stalled (parent lines
918–1074; assigned patch 00800-000.patch:930–1086). It warms retained history, pauses the real attachment, drains
residual output, starts the out-of-band-progress flood, waits beyond tmux's five-second pause allowance, then reads
seven resident-memory samples from process-identity-bound handles. It requires producer progress during the measurement
(:1016–1025), less than 8 MiB peak growth separately for tmux (:1034–1039) and supervisor (:1043–1048), resumed
delivery, and a test-controlled producer stop (:1050–1073). These distinguish sustained memory accumulation from correct
lossless delivery after a stall; a growing buffer can preserve delivery/order and eventual detach while violating the
removed memory bound. The same-revision SPEC_impl.md:2294–2296 explicitly retains adoption/serving of earlier
supervisors' tmux servers, and :2954–2963 describes old-terminal support throughout this transition. Its backpressure
explanation :1766–1768 states exactly: 'tmux reads ahead into history and pauses the client's stream. The agent
free-runs into scrollback (tmux server RSS stays flat), the bytes queued for the stalled client age past `pause-after`,
and tmux then cuts that client's stream'. Actual retained checks inspected: tmux/stream.rs:2460–2471 still asserts that
the generated attach command sets pause-after, so do not claim that merely dropping that flag would go wholly
undetected. That syntactic oracle does not sample either process under sustained pressure. Searches of pinned
crates/e2e/.config for the removed test, ProcessRss, RSS_SAMPLE, smaps_rollup and resident/RSS growth found no remaining
terminal-backpressure memory test. The retained files.rs:1542–1608 RSS test measures streaming a 64 MiB file upload
through StagedStream, with no terminal producer or viewer. e2e/grid-client/BENCHMARK.md:3–6 explicitly says its opt-in
tool measures a mounted browser grid, has no performance thresholds, does not launch Farhelm and does not measure
helm/SSH/emulator; benchmark.mts:403–417 returns browser content-process metrics. It cannot replace either removed
server-side assertion. Retained grid pause/history/stall assertions establish behavior, not resident-memory plateau.
Suggested bucket: other (maintainer regression protection). Confidence: confirmed by source, scoped retained-oracle
search and actual assertion comparison; no execution. proposed_drop: false for the intermediate historical test gap.
Could-be-highest: no established production security, user-work-loss or catastrophic tooling route; do not infer an
actual memory leak from absent measurement. possible_cover: no matching RSS-oracle finding in the fully read
parent-supplied interface records; c3 owns different mode-restoration and shared lifecycle oracles. Material caveats:
the old measurement belongs to tmux plus the in-process supervisor, not a terminal host, so its removal establishes no
independent ongoing host-RSS defect. Correct fixed queues or static flags do not erase the lost metric, and no claim is
made that every conceivable buffering regression passes every other test. Later source retirement is distinct:
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 ends the old server at startup and resolves Tmux rows as absent;
1ed482dde220452e424973faf38a5e1318986333 removes its driver. Thus this specific old-path subject is retired at tip, not
an instruction to restore a tmux memory fixture now. At the reviewed intermediate revision, keep a measurement against
adopted earlier-release terminal state until that contract retires, rather than relying on the removed create-tmux
switch. Prior lead's legitimate non-transferability distinction remains preserved; it did not refute historical old-path
coverage. No runtime tests, reproductions or live-install actions.

Returned proposal (claims-098, event 15): **retain-verified**, suggested bucket **other**.

The removed test measured bounded tmux-server and supervisor resident growth under a live paused-viewer flood. No
remaining same-revision test with that stimulus and those metrics was found. Behavioral queue/history assertions and
browser-only benchmarks cannot replace those measurements. This proves lost historical protection, not a leak.

Original stronger consequence: The lead asked whether the old path lost its memory-growth oracle despite the legitimate
distinction between tmux and terminal-host RSS. The final finding preserves that distinction, the still-present
pause-after assertion and the later retirement; it never establishes a product leak or catastrophic tooling harm.

Unresolved premise or limitation: None for removal of the historical metric. Any actual production memory exhaustion or
loss of work needs separate evidence. No claim that every conceivable leak escapes every remaining test, or that tmux
RSS certifies terminal-host RSS, is made.

Proposed basis: Retain the historical stalled-viewer RSS test gap, other priority, separately from mode/lifecycle gaps.
Do not request revival of an obsolete tmux fixture at current tip.

Existing parent disposition (event 15): rejected. Full098: deleted647 sustained gated-flood paused-viewer test measures
separate tmux/supervisor RSS growth over seven points after warming, with producer progress and controlledstop.
Behavioralqueue/history checks, uploadmemory andbrowser-onlybenchmark do not replace this exact live legacy metric.
Historical protection lost while supported; dbfc/1ed retire/removesubject bytip, no currenttmuxRSS fixture resurrection
required. Preserve pause-after unitassertion and distinct hostRSSquestions; no actual leak/exhaustion/workloss
established.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g154 record](records/review-through-00de0b6c/groups/g154.json). Record ID: `l4`.
