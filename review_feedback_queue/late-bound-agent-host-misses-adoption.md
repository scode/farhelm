# A late-starting agent can remain unattachable until supervisor restart

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

If an interrupted terminal starts only after its replacement supervisor finishes both startup scans, the agent can keep
running while Farhelm cannot open its terminal. Another supervisor restart is needed to find it.

## Details

Suggested bucket: **high**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
static source route; frequency unmeasured

Source locations:
a4345e86a3f51f4c5abf3b2e97737991df900142:crates/farhelm-supervisor/src/service/core/host_handover.rs:697-705,752-757;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core/host_handover.rs:1120-1185,1327-1337.

The native starter has its full launch spec before the old supervisor dies, then finishes its independent readiness path
after both replacement reloads and reclaim. The missing socket became terminal=None; readoption retries only Host
placeholders, and reclaim skips known-session records. Pinned core.rs:6474–6490,6553–6561,8317–8385;
host_handover.rs:1120–1185,1327–1337; host_reclaim.rs:258–264,636–649,816–826; terminals.rs:908–941. Preserve g151/f1
provenance. SPEC.md:1544–1556 is possible exact-operation crash coverage, but attachment recovery is separately promised
at :2127–2133 and SPEC_impl.md:2044–2069 recognizes the outstanding start. Coverage remains ambiguous. Explicit
no-old-runtime empty Restart history and separately approved unknown-agent Restart consequences do not waive this
attachment failure.

previous f1 event. The tip still scans only the socket directory for agent rows
(ca9b0a19856dc136f63ad2d14f1076027edce1eb:service/core/host_handover.rs:578-665); no agent startup settlement appeared
in the focused call-site search. Newly investigated possible consequence: Restart with no terminal treats status as
unknown and does not require working-agent consent (70cd68a03c101493e0adbf00bb27bb8004adfe41:core.rs:12235-12244). Need
inspect later host Restart settlement to determine whether it can kill the late-started working agent.

a host that starts after both replacement-supervisor reloads leaves its running agent unattachable until another
supervisor restart; reconnect and original-create replay do not adopt it. Details and exact compatible native-start
sequence remain in f1-synthesis-final-event: full launch spec delivered before the old supervisor crashes, surviving
native starter delayed until both reloads and reclaim scan finish, then its independent fork/readiness path succeeds.
Missing socket becomes terminal=None; retries require Host placeholders; reclaim skips known-session agent records. Tip
references: core.rs:6474-6490,6553-6561,8317-8385; host_handover.rs:1120-1185,1327-1337;
host_reclaim.rs:258-264,636-649,816-826; service/terminals.rs:908-941. Suggested bucket high; confidence confirmed
conditional static source behavior, no missing runtime-frequency premise. Semantic duplicate g151/f1, not a new
discovery. possible_cover revised explicitly: tip SPEC.md:1544-1556 accepts an exact-operation crash leaving processes
untracked or subsequently killed unasked. TRIAGE_OUTCOMES.md:7966-7982 applies that acceptance to an invisible
unlabelled tmux tab. This is a serious possible cover for f1, but the spec's :1551-1553 enumerates cleanup, resume and
destination exceptions, not failed attachment recovery; Terminal experience :2127-2133 promises rebuilding and
attachment across supervisor restarts, and SPEC_impl.md:2044-2069 recognizes the known recorded starter that startup
omits settling here. Applicability to this known-row/known-record agent state remains ambiguous rather than decided. The
attachment failure remains retained after independent review. Three adjacent consequences have independently checked
narrow dispositions: (1) unknown-agent Restart stopping undetected working processes is proposed covered by
SPEC.md:1367-1372,1504-1505; core.rs:12999-13072's outstanding-host settle/reap is a Restart action, not startup repair.
(2) g151's no-runtime Restart history consequence is the same f1 interaction, not a new local discovery:
core.rs:13082-13087 selects no seed for terminal=None and host_restart.rs:421-428 supplies an empty seed. Proposed
coverage is narrowly SPEC_impl.md:2148-2149, A session with no old runtime to read restarts with an empty terminal,
within the full Restarting a host agent paragraph :2120-2194 read during this turn; this does not accept missed
attachment. (3) checkout-loss proposal is narrowly source-refuted for agent/post-clone-hook work in the swept private
claim: spare_take.rs:151-182 moves entries into the destination and removes the empty claim before success;
launch.rs:1964-2007 runs the post-clone hook only after take/clone succeeds and :1201-1211,1271 execs the agent only
after preparation; spare_checkouts.rs:541-555,599-619 removes the selected private claim, not already-moved destination
entries. A late take can fail preparation but this route does not establish deleting files the agent already wrote.
Retain that refutation for independent audit; do not generalize it to arbitrary human writes or every checkout-loss
mechanism. Other required coverage sources were searched and produced no exact attachment match; TODO Planned is empty,
relevant Near-term retry work is narrower, surviving-tab attribution queue/ledger work is different, and the
rare-self-correction/crash-status filters do not expressly cover a terminal that cannot be opened without restarting its
supervisor.

Possible coverage and caveats: SPEC_impl.md:2353-2355 describes the immediate missing-socket test but never explicitly
accepts a still-live recorded starter subsequently producing an inaccessible agent; :2044-2069 recognizes surviving
starters, and :2366-2374 explicitly settles the equivalent tab-start state. Treat this as a possible specification
conflict, not settled acceptance. TODO Planned is empty (:23-25); the unavailable-runtime Near-term item (:70-102)
covers existing runtimes/placeholders, not terminal=None. Focused coverage searches across BUGS.md, unlanded plans,
queue INDEX/matching files and TRIAGE_OUTCOMES.md found no exact match. The startup-surviving-tab queue item and ledger
entries (:17948-17963,19577-19586) concern wrong tmux-pane attribution, not late terminal-host startup. FILTER.md
rare-self-correction does not cover manual supervisor restart to recover attachment; the crash-status filter excludes
inability to open the session.

Additional reviewed locations: g151/f1:
a4345e86a3f51f4c5abf3b2e97737991df900142:crates/farhelm-supervisor/src/service/core/host_handover.rs:697-705,752-757;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core/host_handover.rs:1327-1337.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g093/f1`, `g151/f1`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
a4345e86a3f51f4c5abf3b2e97737991df900142; persistent at pinned tip ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a
source-only review; runtime occurrence and frequency were not measured.
