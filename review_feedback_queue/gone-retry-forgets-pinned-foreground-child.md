# Gone retry forgets pinned foreground child

Reviewed commit: c8dbdc4fbaf25c3acf1a713552738d8763d6a22a

## TLDR

Recovery can discard a previously identified foreground child after its terminal host disappears. A complete production
sequence leaving that program running after removal remains unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: high.

Source location:
`crates/farhelm-supervisor/src/hosts/terminal.rs:1988-1989 and service/core/host_handover.rs:1690 at c8dbdc4fbaf25c3acf1a713552738d8763d6a22a`.

Source assessment (read with the later corrections below) (review record g018/l3, event 13):

Reviewed commit: c8dbdc4fbaf25c3acf1a713552738d8763d6a22a. TLDR: after a terminal recovery attempt concludes the
terminal host is gone, Farhelm may forget a foreground program that survived the terminal's hangup, leaving it running
after the session or tab is removed. Confidence: likely source concern, unverified after investigation; the identity
loss is confirmed but a production single-failure transition and platform survival behavior have not been dynamically
demonstrated. Mechanism: this patch carries child_pid/start through unavailable_again (terminal.rs:1998-2008,2027-2030),
but gone(socket, adopted) still supplies previous=None (1988-1989). Agent retry installs terminal=None for
RowHost::Absent (host_handover.rs:1690). Tab retry constructs HostTerminal::gone with only the host process
(host_tabs.rs:1177-1179); its after_retry preserves tag metadata, not a child's identity on a Gone handle (236-251).
Delete can root a previously pinned child only while that identity remains in a host status (terminal.rs:747-777;
session_hosts.rs:512-523; teardown.rs:390-400). The gone record itself cannot restore a child root. Concrete readable
checks: client.rs:1266-1271 distinguishes missing/refused socket from timeouts, 1302-1309 can retain unavailable
timeout/closed evidence, and 1220-1238 rechecks only the host process, not its child; a host failure can therefore be
reported unavailable before a later check confirms absence, but readable code does not prove whether a real one-fault
sequence here leaves an unmarked child alive. Direct runtime Gone retains its old status fields
(terminal.rs:3916-3926,4013-4027), which refutes identity loss on that agent path, but does not refute the retry path.
For tabs, even direct Gone is mapped to no reap root (core.rs:16345-16354), and automatic close eventually forgets the
tab (host_tabs.rs:1035-1047,1067-1076); this corroborates the cleanup omission, although it predates this patch and is
not a separately assigned finding. Contract: terminal.rs:747-762 explicitly supports rooting a pinned child that ignored
hangup and outlived a killed host. terminal-host SPEC 'Ending' says host end closes the pty and sends hangup, not
guaranteed process-tree termination. SPEC.md 'Lifecycle operations' accepts unidentifiable detached survivors without a
manager and two independent rare faults, but neither explicitly accepts forgetting an already pinned live child across
outcomes of one recovery episode. The earlier candidate incorrectly treated an unestablished premise as refutation.
Suggested bucket: high for incomplete cleanup; no established user-owned-work loss route, so not highest. proposed_drop:
false. possible_cover: not searched; this is an incomplete edge of the assigned foreground-root fix, not a claim that
the patch introduced the underlying Gone behavior. Missing premise and useful verification: in an isolated
source-derived fixture, retain a readable pinned child while reconnect ends unavailable, make the next scan absent, then
exercise Delete for both agent/tab; separately verify a real ordinary foreground program can survive host hangup without
a usable marker/scope on a supported platform. No runtime/process experiments are authorized in this review, and source
alone cannot settle OS signal survival or that concrete production sequence. A fix would preserve/revalidate child
identity for Gone retry outcomes too, and ensure no tab-close path discards a still-pinned child merely because the host
is gone. No later repair appears in these files through pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g018/l3`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
