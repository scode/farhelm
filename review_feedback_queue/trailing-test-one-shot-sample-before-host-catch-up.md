# Persistence test can skip an uncaught-up host

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Persistence test can skip an uncaught-up host

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g131/l1`.

### g131/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR

The converted test takes one status sample without establishing that its newly adopted terminal is eligible for
sampling. A source-permitted multi-event adoption can leave that pass skipping the fixture and failing its persistence
assertions. Whether this exact short-line fixture produces the required event split is unverified after investigation;
no failure was observed. This concerns test reliability, not user data or a verified product failure.

## Details

Contract: the supplied test checklist requires the fixture premise before the action and the named readiness boundary.
ticker.rs:1331-1338 selects a terminal only when caught_up and owned-live. The changed test starts the host at :5213,
installs a durable row, seeds four recognized Waiting observations at :5227-5240, and runs exactly one sample_pass at
:5251 before requiring Running, the in-memory/durable START key and no pending retry at :5253-5279. Neither spawn_host
(:1858-1891), install_durable_running_entry (:1960-2010), nor the caller waits for caught_up. Database awaits supply no
ordering contract with the terminal runtime.

Useful additional checks and results in this synthesis: (1) hosts/client.rs:789-813 sends only Read and Info in the
adoption exchange; :1103-1142 queues stream records/acknowledgements as HostEvents but handles InfoReply separately.
hosts/terminal.rs:2681-2708 pins the child using Info only. (2) terminal-host/src/journal.rs:139-155 initializes this
no-snapshot journal with no resizes, no gap and no exit, and :344-394 starts a default-position Read directly in its
tail. The fixture sends no input/resize, and sleeps after printf. host.rs:548-568 and :582-623 acknowledge only resolved
input. Therefore an initial InfoReply, Welcome, zero-input acknowledgement, snapshot, resize or gap is not a concrete
extra-event route for this healthy fixture. (3) More than one output event is source-permitted even though the line fits
in one storage block. host.rs:344-369 reads currently available PTY data, writes already-queued frames, then handles
client messages; :407-411 refills after them. :503-537 stops PTY draining on EAGAIN. :628-650 materializes Output frames
immediately and advances the journal cursor. journal.rs:207-229 coalesces new bytes in storage, but :447-469 copies
bytes from the cursor's current offset; later bytes cannot merge into a previously queued frame or rewind that cursor.

Conditional concrete enqueue sequence, rather than a generic scheduling assertion: Read is handled in a loop before the
first Info and refill queues O1 for the bytes then available. A later loop drains additional bytes, writes O1, handles
Info1 (queues Reply1), then refills O2. The client receives Reply1 and O2, sends Info2, and its second Info round trip
can decode both O1 and O2 before returning. hosts/client.rs:1016-1030 decodes buffered frames before checking request
completion, and :1078-1099 decodes every whole frame. Adoption's fence is then at least two (hosts/terminal.rs:1821). It
processes only one queued event at :1916-1926, decrements the fence once (:3343-3344), and spawns the background runtime
(:1928). Its status remains caught_up=false until the remaining fenced event is processed (:4008-4027). If sample
selection precedes that processing, it skips the entry, preserves seeded Waiting and never allocates START. There is no
source-level readiness dependency that prevents this final ordering. An explicit bounded wait for the adopted handle's
caught_up/owned-live state before this one-pass stimulus would address that eligibility contract.

Exact remaining premise and confidence limit: the unmodified platform sh/printf and PTY must expose this particular
short line in available chunks separated across the host's loop/refill boundaries; repository source does not contain
that shell's output implementation or specify atomic visibility of its output on the PTY. A second raw read within the
same drain would coalesce before refill and would not suffice. No claim is made that this split occurred, is common, or
is forced by the source. The queue/control ordering permits it; the current source does not establish or refute its
occurrence for the external fixture. This is unverified after investigation, retained as a finding under the caller's
requested disposition, not a confirmed flake. Evidence that settles the remaining premise could be platform shell/PTY
source guarantees covering this write and read visibility, or existing instrumentation showing the exact
output/refill/fence/caught_up ordering at the sample boundary. A failure trace is not required to identify the missing
readiness contract; none was demanded, generated or observed. No reproduction or runtime experiment was requested or
performed.

Separate original consequence and proposed-drop basis preserved: the initial producer-frame concern was a false pass on
blank startup text instead of literal 'work resumed'. That argument is source-refuted for this test's actual
transition/persistence subject. A blank screen differing from the seeded Claude dialog legitimately yields generic
Working through ticker.rs:586-639 and agent_kind/screen_reader.rs:365-375, :575-592; that still exercises the asserted
transition, allocation and durable write. Parent ticker.rs:7147-7227 already used the same manually seeded dialog,
printf command and assertions, without a frame oracle (parent spawn_pane :2379-2390). Literal producer text is not a
necessary assertion. This source-based proposed drop does not refute the separate new host catch-up eligibility
question. Earlier 'normally first record' wording is superseded; typical behavior and absence of observed failure are
not proof.

Suggested bucket: other (maintainer test reliability if the remaining premise holds). Confidence: unverified after
investigation for the concrete failure; source-confirmed missing explicit readiness dependency and enqueue sequence
conditional on separated output availability. proposed_drop: false for the catch-up issue; true only for the separately
preserved producer-text argument, on the exact source basis above, not specification acceptance. Could-be-highest: no;
no security or user-work-loss route. possible_cover: none; coverage sources not consulted. Caller has accepted no drop.
All conclusions are source-derived, not execution observations.

## Discovery-turn platform check (revision in progress)

Read exact latest g133/l2: its output-free fixtures have no setup events and start caught_up=true; this agrees with the
separate short-line output question here. Primary references checked: Open Group printf/read/write pages returned HTTP
403, including Issue 7 printf; their contents were not relied on. Linux read(2),
https://man7.org/linux/man-pages/man2/read.2.html, RETURN VALUE and ERRORS, expressly permits short terminal reads and
EAGAIN for nonblocking descriptors; it supplies no whole-line read guarantee. Linux v6.12 primary source
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/n_tty.c shows process_output_block stops before
newline with ONLCR (:489-542), do_output_char emits CRLF separately (:393-401), and n_tty_write calls these in sequence
(:2250-2264). pty_write in https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/pty.c:104-111 passes each
driver write into peer flip buffering. Thus even one shell write can have separate driver deliveries, conditional on
OPOST/ONLCR. The kernel buffer documentation https://www.kernel.org/doc/html/latest/driver-api/tty/tty_buffer.html
describes push as queued work; no complete-shell-write visibility guarantee is stated. This strengthens the source basis
without claiming an observed fixture split. Next readily accessible check: default PTY output flags and whether repo
startup preserves them. Earlier investigation/consequences/drop bases remain unchanged; exact runtime interleaving
remains unobserved. Summary now carries the explicit unverified-after-investigation label requested by caller.

## Completed primary-platform check

The Linux default-flag premise is resolved:
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/tty_io.c:119-129 defines tty_std_termios with
OPOST/ONLCR; pty.c:853-864 uses it for Unix98 slaves without clearing those output flags. Reviewed repo
terminal-host/src/spawn.rs:371-400 preserves kernel output flags, changing only erase and IUTF8. The exact command
includes a newline. Therefore the separate text/CRLF driver-delivery path applies on this supported Linux source, even
if printf uses one syscall. tty_buffer.c:512-525 commits and queues each delivery separately; its worker (:428-469) can
consume committed text before the later newline delivery. n_tty_write explicitly is not serialized against reads
(:2211-2212). Together with the recorded host enqueue order, this supplies a concrete source-permitted multi-event
premise, replacing the earlier need to know the shell implementation. No primary contract read supplies
whole-line/whole-write visibility atomicity. What remains unverified is occurrence/interleaving on an actual run, not an
unavailable shell-source precondition; no execution trace is required to assess the missing catch-up dependency. Retain
the caller-requested unverified-after-investigation summary and avoid claiming an observed flake; the missing readiness
contract and conditional Linux path are source-supported. Exact historical kernel build and occurrence have not been
inspected. All original consequences and separate producer-text proposed-drop basis remain preserved. g133/l2 quiet-host
result does not refute this output path.

## Whole-range attribution check

The exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already contains
waiting_to_running_sample_persists_the_authoritative_work_start at ticker.rs:7013-7093, with the same printf string,
manual four-Waiting seed, single sampler call and persistence assertions. Therefore the producer-text-only argument
predates the complete reviewed range, not merely the immediate parent; its actual-subject refutation remains independent
of that attribution. In-range path history and -G search attribute the new host-helper/catch-up boundary to
1ed482dde220452e424973faf38a5e1318986333. The complete diff from that commit through pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db changes ticker comments only and does not add readiness to this test; no later
repair found. These are source checks, not execution. No new finding ID is introduced.

## Confidence reconciliation before discovery pass 2

Confidence is now CONFIRMED, SOURCE-VERIFIED CONDITIONAL FAILURE ROUTE on the checked Linux default-PTY implementation,
with no observed flake. The previous retained unverified-after-investigation label is superseded: carrying it solely
because the caller had requested a label was incorrect; that request applied while a source premise remained unsettled.
No actual source premise remains missing for this conditional route. The primary-platform check resolved the former
external-output premise: preserved OPOST/ONLCR causes the exact newline-bearing command to have separately committed
text and CRLF peer-buffer deliveries even if the shell uses one syscall. The independent worker may publish the text
before the writer continues with CRLF; reads are not serialized against that writer. The already-recorded
host-loop/refill and Read/Info sequence can therefore queue two events for this exact command. Adoption consumes only
one synchronously, and the test has no dependency requiring the independent runtime to consume the remaining fenced
event before its sole sampler selection. Under that source-permitted interleaving the entry is excluded by
caught_up=false, remains the manually seeded Waiting, and the Running/START assertions fail. It does not require data
corruption, shell-specific multi-write behavior, an invented setup record or a trace proving past incidence.

Trigger/limits preserved: this is conditional on split PTY visibility across separate host refill iterations, the
specified Read/Info interleaving, and background consumption occurring after selection. A single processed event or a
runtime that drains before selection does not fail. Occurrence, frequency and the local machine exact kernel build are
unmeasured and are not needed to establish source validity. The cited Linux v6.12 path is a concrete supported-platform
implementation; no macOS-specific failure is claimed. This remains other-bucket test reliability and no
product-data-loss claim. All preceding source checks, consequences, proposed-drop bases and their history remain
retained; producer-text-only false-pass remains separately refuted and out-of-range at the exclusive base. proposed_drop
remains false for the catch-up issue. possible_cover update: supplied neighbour g114/l2 uses the same adoption/catch-up
cause at a distinct paste-off fixture; its eight-byte escape contains no newline, so the text/CRLF platform proof here
does not automatically settle its own output-split premise. No parent drop/lowering accepted and no new finding identity
introduced.

Returned proposal (claims-083, event 12): **retain-verified**, suggested bucket **other**.

The final history establishes a conditional Linux source route: separately visible text and CRLF can become two queued
adoption events; adoption processes only one before returning; the test's sole pass can exclude the still-uncaught-up
host. This is a test false failure, not an observed flake or a product-data-loss claim. The superseded unverified label
must not overwrite the final source-verified disposition.

Original stronger consequence: The initial concern was a false pass on blank startup text instead of literal 'work
resumed'. That argument was later source-refuted for this test's transition/persistence subject: a differing blank
screen can legitimately produce generic Working and still exercise the asserted transition, key allocation and durable
write. It also predates the exclusive base. The intermediate statement that the line was 'normally' one event was an
assumption and was withdrawn. The separate catch-up concern progressed from unverified to a source-verified conditional
Linux route; no runtime failure was observed, no trace was demanded, and no parent drop was accepted.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the final conditional failure route with other priority for maintainer-test reliability. Keep the
separately refuted/out-of-range producer-text concern in the history. The input mentions g114/l2 at a different
paste-off fixture; its newline-free output needs its own event-split proof, so it is not asserted as an exact alias
here. Linux references: https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/n_tty.c,
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/pty.c,
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/tty_buffer.c,
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/tty_io.c.

Existing parent disposition (event 12): verified. Parent explicitly accepted named retain-verified proposal from
claims-083 after inspecting it. The final history establishes a conditional Linux source route: separately visible text
and CRLF can become two queued adoption events; adoption processes only one before returning; the test's sole pass can
exclude the still-uncaught-up host. This is a test false failure, not an observed flake or a product-data-loss claim.
The superseded unverified label must not overwrite the final source-verified disposition. Retain the final conditional
failure route with other priority for maintainer-test reliability. Keep the separately refuted/out-of-range
producer-text concern in the history. The input mentions g114/l2 at a different paste-off fixture; its newline-free
output needs its own event-split proof, so it is not asserted as an exact alias here. Linux references:
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/n_tty.c,
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/pty.c,
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/tty_buffer.c,
https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/tty/tty_io.c. Checked source/results: ["At
1ed482dde220452e424973faf38a5e1318986333, ticker.rs:1858-1891 returns Fresh adoption without waiting for caught_up.
:5213-5251 installs the host and synthetic Waiting evidence, then samples once; :5253-5279 requires Running and a
durable START key. :1331-1338 excludes a host whose caught_up is false.","At that revision, hosts/terminal.rs:1795-1821
counts events decoded during two Info round trips; :1858 initializes caught_up from fence/end; :1916-1928 processes one
queued event and spawns the runtime. :3343-3344 decrements once per event; :4008-4027 requires the whole fence drained
before publishing caught_up.","At that revision, terminal-host/src/host.rs:344-411 reads currently available PTY data,
writes queued frames, handles client messages, then refills. :503-537 stops draining on EAGAIN; :628-650 materializes
Output frames. journal.rs:207-229 coalesces stored bytes but :447-469 copies only bytes currently available at the
cursor, so later bytes cannot merge into an already queued frame. hosts/client.rs:1016-1030 and :1078-1142 decode all
buffered whole frames and queue output separately from Info replies. This permits the recorded O1/Reply1/O2/Reply2
order.","Checked primary Linux v6.12 source online. n_tty.c:420-429 and :539-547 separate ordinary text from newline
conversion; :2387-2402 invokes block output then character output. pty.c:111-118 pushes each write to peer buffering;
:914-917 preserves default output flags. tty_io.c:124-131 defaults OPOST|ONLCR. tty_buffer.c:486-496 reads committed
bytes and :529-535 commits/queues work separately. Repo spawn.rs:371-400 preserves these output flags. The exact
newline-bearing command therefore has a source-permitted split delivery. These are corrected raw-file line coordinates;
the input history's Linux coordinates differ.","The source route is conditional on split visibility across host refill
iterations, the specified Read/Info ordering, and runtime consumption after sample selection. A single event or an
earlier runtime drain does not fail. No incidence, frequency, local kernel version or macOS failure was
measured.","Exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a:ticker.rs:7013-7093 already has the printf command,
four-Waiting seed, single pass and persistence assertions. Thus the separate producer-text-only argument predates the
range. Diff from the reviewed commit to 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db changes ticker comments only and adds
no test readiness dependency."] Current coverage/results: ["Both specifications searched for sampling, capture recovery,
catch-up and work-start persistence; SPEC_impl.md:4164-4272 specifies owned-sample and work-start behavior, but does not
accept one-shot test sampling before eligibility. SPEC.md's cosmetic-status acceptance is not test-reliability
coverage.","TODO.md Planned is empty. BUGS.md and active TRIAGE_OUTCOMES.md have no matching test/eligibility
claim.","plans/queue searched for the actual fixture, persistence, failed capture and catch-up; no unlanded plan states
a repair for this fixture. The tmux removal plans are recorded landed in plans/REPORTS.md:29-32.","INDEX.md and matching
queue text searched. The reconnect-replay fence item has a different production reconnect/resize trigger; successor-exit
and other readiness tests concern different fixtures. No exact existing queue coverage found.","FILTER.md checked. A
test false failure without a person racing a control, upgrade-carried state, diagnostic-only consequence or failed
session operation does not fully match any filter."] Exact cited cover: [] Original stronger consequence and missing
premise remain preserved: "The initial concern was a false pass on blank startup text instead of literal 'work resumed'.
That argument was later source-refuted for this test's transition/persistence subject: a differing blank screen can
legitimately produce generic Working and still exercise the asserted transition, key allocation and durable write. It
also predates the exclusive base. The intermediate statement that the line was 'normally' one event was an assumption
and was withdrawn. The separate catch-up concern progressed from unverified to a source-verified conditional Linux
route; no runtime failure was observed, no trace was demanded, and no parent drop was accepted."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g131 record](records/review-through-00de0b6c/groups/g131.json). Record ID: `l1`.
