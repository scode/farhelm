# Preserve socket-probe uncertainty before destructive admission

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A temporary resource/connection failure at startup can make a live old session appear ended; its normal guarded Delete
can then stop that live work without authorizing it. This is a source-established conditional failure, not a measured
incident.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/tmux.rs:3075-3082,3146-3163`;
`crates/farhelm-supervisor/src/service/core.rs:6566; crates/farhelm-supervisor/src/tmux.rs:3146-3163`;
`crates/farhelm-supervisor/src/tmux.rs:3075-3097`; `crates/farhelm-supervisor/src/tmux.rs:3075-3097,3146-3163`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **highest**. Source-confirmed conditional behavior does not imply
an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the
finding.

The exact remaining unmeasured fact is incidence/timing: no retained production observation shows a resource/backlog
failure at this startup probe followed by successful construction and a user's Delete. That is not a missing step in
shipped control flow. The reviewed probe explicitly maps any error to false and then returns Ok with Absent; pinned
Tokio 1.53.2 stream.rs:81-127 and mio 1.2.4 uds/stream.rs:8-20 propagate local socket() failures as well as connect
failures. Mio sys/unix/net.rs:15-37 uses nonblocking socket(). The installed socket(2) source manual at
/usr/share/man/man2/socket.2.gz:414-423 documents EMFILE, ENFILE, ENOBUFS/ENOMEM, with resource release permitting a
later attempt; these conditions do not imply that an existing tmux server or its programs died. The installed connect(2)
manual:141-166 explicitly distinguishes nonblocking Unix EAGAIN from EINPROGRESS, while mio ignores only the latter. No
unsupported permission/socket edit is needed to establish a trigger.

Construction can continue after pressure exists only at that probe: the SQLite store and host identity have already been
opened/read before tmux (core.rs:6511-6543,6566). Resource pressure ending before later allocations is recovery from
that one temporary condition, not a second independent rare fault. A temporary system-wide ENFILE/ENOMEM allocation
refusal is sufficient without assuming per-process descriptors happen to free themselves. Finite backlog is a
corroborating source mechanism rather than an asserted Farhelm load incident: tmux 3.7c server.c uses listen(...,128)
and suspends accept for a second on ENFILE/EMFILE (official source cited in prior f1); SPEC_impl.md Terminal host
Connecting documents EAGAIN on Linux and refused connections on macOS for live full Unix listeners. We have not measured
workload size, frequency, backlog occupancy or which errno occurs naturally on a particular installation.
Resource-allocation failure alone suffices, so no backlog workload claim is required for the finding or bucket.

The destructive consequence is fully source/GUI routed in the earlier f1 record and parent-checked guard/handler/sweep
order: false Absent -> empty reload -> Exited/terminal-less row -> sidebar NothingAlive Delete -> guard accepts empty
evidence -> recorded/session scope sweep stops a real old Linux agent. The independent marker/scope sweeps do not refute
this consequence; their successful kill is precisely the violation of NothingAlive. No claim that Restart necessarily
creates duplicate agents is retained; its survivor sweep prevents that simplistic inference. SPEC.md:1672-1677 requires
the guarded action to refuse without stopping the live agent.

Duration correction is retained: an old-terminal creation attempt can move Absent to Running, including a keyed retry
using its durable tmux kind despite the current host default (core.rs:11078-11094; presence.rs:208-234). Ordinary
host-only operations and the resource condition clearing do not repair it; do not say supervisor restart is the only
possible cache-changing event, or that a retry necessarily rediscovers all old-tab session names.

Suggested bucket: highest, unchanged from my prior finding, rather than g024's original high-only position: a
platform-defined supported failure condition has explicit propagation to an established shipped consent-violating kill.
Rarity and absence of a measured occurrence do not lower that consequence. Confidence: confirmed source-level
conditional route; production occurrence and frequency unmeasured. proposed_drop: false. possible_cover:
g024/l2,g032/l2,g033/l2, all duplicates for commissioner consolidation, not specification acceptance. f4 remains
separate because it starts correctly Unusable and needs an early admission fix even when this probe succeeds. No
unresolved source premise remains for this conditional finding; runtime fault injection would validate the prediction
but was prohibited and not performed.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g031/f1`, `g024/l2`,
`g032/l2`, `g033/l2`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source
references name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection
only, with no product runtime experiments or live-install changes.
