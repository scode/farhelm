# Tunnel control exit can open new ssh connection

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930

## TLDR

Closing an old tunnel can start a new SSH connection if the control handshake fails after connecting. The deployed
ordering and resulting delay remain unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location: `crates/farhelm-helm/src/ssh.rs:415-423 at 88bf83e267b5066815448885c61ed4148b851930`.

Source assessment (read with the later corrections below) (review record g046/f6, event 16):

Reviewed commit: 88bf83e267b5066815448885c61ed4148b851930. TLDR: When an old SSH connection disappears during sandbox
cleanup, the attempt to close it can unexpectedly try to log in again, adding connection work and potentially delaying
suspension/removal cleanup until its timeout. The new ssh_control_exit_args builds the ordinary auto-sharing prefix
(ssh.rs:326-331) and inserts -O exit at 415-423. This is used during suspended-host handling (manager.rs:3759-3774)
through transport.rs:251-271, which gives each command five seconds. A missing/refused Unix socket is safely fatal in
the inspected OpenSSH implementation, as f5 records; this finding is specifically about a socket that connects and then
loses its mux hello. Supported upstream source OpenSSH V_9_6_P1 mux.c:1501-1554 reports failed hello I/O, and 2190-2193
returns -1 without special treatment for TERMINATE. ssh.c:1518-1523 then falls through to ssh_connect at 1551-1555, so
-O exit is not intrinsically local-only on that failure path. A normal shutdown producer is source-supported:
channels.c:2320-2364 accepts a mux-control connection and invokes its callback; mux.c:1089-1104 queues its hello rather
than synchronously writing it. channels.c:2292-2310 writes output only when that channel is write-ready.
clientloop.c:1500-1508 processes channel events before remote network input; a concurrent remote connection loss at
741-760 sets quit_pending, and shutdown at 1537-1549 frees the channels before another local flush. Thus an old master
disappearing during the new stop client's handshake can produce the hello failure without a malicious socket or invalid
destination. Source URLs: https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/mux.c ;
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/ssh.c ;
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/channels.c ;
https://raw.githubusercontent.com/openssh/openssh-portable/V_9_6_P1/clientloop.c . SPEC_impl.md Tensorlake hosts
describes closing the shared masters as best effort after withholding a suspended host's dial; it does not describe
cleanup initiating fresh authentication. Suggested fix direction: make the stop path fail closed on failed mux
handshakes, preserving the exact socket identity while preventing a fresh network connection; an isolated fake-master
fixture can verify that failed hello never invokes the network/proxy path when runtime work is authorized. Bucket other:
redundant connection work and bounded cleanup delay; no demonstrated security, data loss, automatic sandbox resume, or
lasting new master. Confidence: concrete upstream fallback and normal shutdown mechanism confirmed statically for a
supported OpenSSH 9.6p1 example (above the project's stated 7.6 floor), but runtime effect is unverified after
investigation. Checks/results: inspected exact control argv, bounded production caller, mux-client fallback, queued
master hello, and master network-shutdown path. Missing premise: an observed deployed-client interleaving and resulting
cleanup delay; this source-only review cannot establish frequency, elapsed duration, or which client version a user
runs. A deterministic local mux-hello-close fixture plus SSH/proxy invocation observation would settle it; no
runtime/process/network experiment was run. No live gateway behavior is needed to establish the fresh connection
attempt. Caveat: missing/refused socket is refuted separately in f5; successful fresh ControlPersist setup may be
followed by its foreground child's own termination request (ssh.c:1710-1741), so do not infer indefinite persistence.
g052:l1 is a known cancelled-builder helper retention issue; it is not this control-command fallback, and no new report
is made about its existing mechanism. Temporal: no subsequent change to ssh_control_exit_args through pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b; the later ssh.rs patch only negotiates the supervisor idle option.
proposed_drop: false. possible_cover: unknown; parent handles queue/plan/filter checks. This is new discovery f6, not a
rebranding of f1 or either supplied interface record.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g046/f6`; supporting records
`g043/g046:f6`, `g044/l10`, `g049/iface_g046_f6`. The retained private log preserves earlier wording and its
corrections. This record does not claim runtime reproduction.
