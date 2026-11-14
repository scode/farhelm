# Proposed source-based exclusion: examined raw paste-off fixture produces one initial output record

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Proposed source-based exclusion: examined raw paste-off fixture produces one initial output
record

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g114/l2`.

### g114/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 21 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Turn 4 final primary-source revision, retaining the original
missing-gate concern and conditional consequence. TLDR originally suspected: paste=false recorder readiness could return
before caught_up, so its caller would get terminal-restoration refusal instead of the paste-off diagnostic. Repo facts
remain: prompt_recorder waits for a running child, default-off paste mode and the file (core.rs:19867-19889), not
explicit caught_up; prompt_session gates caught_up first (:11999-12008); adoption processes one queued event before
returning (hosts/terminal.rs:1916-1927). A genuine second initial output event could therefore exercise the previously
recorded conditional failure route. Source-only primary checks now refute the proposed producer for the examined fresh
healthy raw fixture. Upstream dash v0.5.12 printf emits characters into its stdout buffer (src/bltin/printf.c:129-174,
bltin/bltin.h:54-64, output.h:102-118). evalbltin flushes after the builtin (eval.c:942-966); output.c:239-254,356-375
writes the accumulated eight-byte format in one full initial request. Tagged sources were read from
https://raw.githubusercontent.com/herbertx/dash/v0.5.12/ and retained in private dash-0.5.12-* snapshots. Present-host
read-only metadata identifies /bin/sh as dash 0.5.12; this does not establish an identical historical distro build.
Linux v6.12 sources were read from https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/ and retained in
linux-6.12-* snapshots. Added final check: tty_io.c:965-1041 uses a chunk at least as large as the eight-byte count and
hands that entire count to the line discipline; it does not subdivide this fresh write. With stty raw -echo,
n_tty.c:2358-2444 takes the no-OPOST branch and hands the full batch to the PTY driver. pty.c:111-119 passes it to flip
buffering; tty_buffer.c:262-291,302-333,552-568 inserts/commits the fresh batch together with sufficient room. Unix98
master starts raw (pty.c:898-904); n_tty.c:1522-1538,1610-1658,1695-1746 copies the small batch and publishes
commit_head after it. The fresh master read has no ring wrap/prior data and returns all eight or none
(n_tty.c:1963-1994; repo host.rs uses a 64KiB read at :503-535). The repo journal then supplies one initial Output
frame; adoption consumes it before returning. If output starts only after adoption, the initially empty terminal is
already caught_up. No additional initial snapshot, resize, gap, exit or input-ack event was established for this
live/no-input/no-resize fixture. Hence one fresh successful raw eight-byte write without additional output does not
supply the hypothesized partial-first-frame/remaining-fence route in this inspected source path. Explicit distinction
from supplied g131/l1: its newline fixture retains OPOST/ONLCR and can deliver text and CRLF separately; this fixture
first turns OPOST off and emits no newline, so that source proof does not apply. Generic permission for short terminal
reads is not used as proof of a split. Primary dash kernel.org and Debian raw web endpoints were inaccessible; the
upstream maintainer's GitHub tagged source was accessible, not outside-repo evidence declared unavailable without
checking. No runtime or reproduction was run. Broader limit: these checks are not a universal atomicity promise for
every shell build, distro patch or historical kernel. A reportable failure elsewhere still needs a concrete supported
producer of multiple initial events/split output under this exact fixture; none was found. That is the remaining premise
for the original general concern, not an open uninvestigated repository-source lead. Immediate-parent helper used tmux
queries, so the host catch-up gate is new to the conversion; later scoped changes through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db do not add an explicit wait. The absence of the explicit wait alone does not
show a broken oracle when the fixture supplies equivalent one-record readiness. Suggested bucket: other for the
preserved test-reliability concern; could-be-highest no. Confidence: confirmed source refutation under fresh successful
dash/Linux-v6.12/no-additional-output premises; broader actual failure unverified, no observed flake. proposed_drop:
source-based exclusion of the only investigated platform route, for independent parent disposition; no parent
drop/lowering accepted, original conditional route and uncertainty preserved. possible_cover: g131/l1 shares the
adoption-readiness class at a different producer, not proof for this fixture. Source refs/earlier full conditional
enqueue sequence remain in this ID's prior events. Contract: test-authoring requires the actual readiness premise and
distinguishing refusal; the examined source supplies that premise without the explicit check. No tests, builds, runtime
probes, process control, live-install action or prohibited coverage search.

Returned proposal (claims-074, event 21): **retain-unverified**, suggested bucket **other**.

The missing explicit gate and conditional two-event failure are supported. The latest withdrawal basis refutes the
examined fresh dash/Linux path, but does not establish a general supported-platform guarantee. Retain the portability
uncertainty rather than turn the narrow refutation into a universal one.

Original stronger consequence: The paste=false test could assert before host catch-up and receive terminal-restoration
refusal instead of the required paste-off diagnostic. No production prompt malfunction, observed flake, security harm or
user-work loss was claimed. Latest events propose excluding only the examined producer route; that proposed withdrawal
is preserved here.

Unresolved premise or limitation: A concrete supported /bin/sh and PTY path that exposes this exact fresh raw eight-byte
fixture as multiple initial stream events. Generic short-read permission is not that evidence. The checked
dash/Linux-v6.12 healthy path does not provide it.

Proposed basis: Keep as investigated uncertainty if recording the broad concern. A parent exclusion can rely on the
examined healthy producer only at that narrow scope, preserving the unsupported-platform caveat. The supplied g131/l1
analogy uses a different newline producer and is not an exact alias established by this batch.

Existing parent disposition (event 21): rejected. Parent read full claims074 and actual complete owner event21. The
owner investigated the exact eight-byte raw no-newline live fixture through pinned dash0.5.12/Linux6.12 write, PTY
buffering/read, journal and adoption paths; one initial Output record supplies equivalent caught-up readiness, or
initial no-output adoption is already caught up. No second snapshot/resize/exit/input-ack event or actual supported
split producer was established. The inspected newline/OPOST analogue does not apply. Reject the failure allegation for
lack of its concrete multiple-initial-event premise; absence of an explicit gate alone does not invalidate equivalent
readiness. This does not claim universal atomicity across every shell/distro/kernel, and does not require proof that all
hypothetical platform failures are impossible. Preserve original conditional route, all investigation, limited platform
facts and what new producer evidence would reopen it. No observed flake or highest consequence. Parent declines focused
agent's proposed portability uncertainty based solely on unshown alternatives.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g114 record](records/review-through-00de0b6c/groups/g114.json). Record ID: `l2`.
