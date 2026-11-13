# List-limit test checks an undrained reply cache

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: List-limit test checks an undrained reply cache

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g126/l1`.

### g126/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 8 (finding, open):

# List-limit test checks an undrained reply cache

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR

The test for the session-list concurrency limit cannot detect a reply arriving before a list slot becomes available. It checks a cached reply list without reading the connection after sending the request, then frees the slots before receiving the answer.

## Details

Synthesis confirmed the exact observation sequence. BusyHost::initialize at handlers.rs:10488-10551 reads the handshake, Attached and initial ReplayComplete before request 3 exists; it then saturates management slots. The list-limit test at 11287-11317 holds all list permits, sends request 3, waits for type_and_see_received, and examines host.replies. Between that send and the absence assertion there is no read_one, reply, concurrent fixture reader, or other operation that can populate the cache. send at 10595-10600 writes only; type_and_see_received at 10669-10695 writes input and directly awaits TestHost's received-byte watch. Only read_one at 10606-10641 adds to replies, and reply at 10646-10661 invokes that reader. The next call is reply(3) at 11311, after drop(held) at 11310.

The server-side connection reader processing the marker is not a client-output observation barrier. connection.rs:455-581 dispatches the marker through the attached terminal's input. Its writer is a separate task at 921-997 and may leave the list reply queued or already written into the duplex stream without the fixture reading it. Thus the absence assertion examines an unchanged cache even when request 3 has already answered. In the previous revision, handlers.rs:11196-11213 read client frames until the terminal echo arrived, collecting control frames encountered before that marker. That was not proof that every concurrently spawned list task had settled, but it did observe replies ahead of the marker in the output stream; this patch removes even that negative oracle.

This is specifically a test-evidence regression: handle_list_sessions at 585-615 still acquires list_admission inside its spawned task and releases it before sending the reply. No current semaphore-bypass runtime defect is alleged. SPEC.md:4707-4736 requires terminal and list progress while management is busy, and this test's explicit contract at handlers.rs:11275-11285 additionally says the list waits for its own limit. The test still detects awaiting the list permit inline on the connection reader, since that blocks the input marker. It loses detection of an early reply or skipped list semaphore. A fix needs an explicit connection-output observation before releasing permits, or a direct deterministic admission/worker-boundary oracle; the typing watch alone is insufficient.

Later-code check: diff from the reviewed commit through pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db leaves this helper and assertion unchanged; later handler edits remove grid and terminal-kind fields only.

Suggested bucket: other (test validity only). Confidence: confirmed by source and caller/data-flow analysis. could-be-highest: no established security or user-owned-work route. proposed_drop: false. possible_cover: none; excluded metadata and queue were not read. No reproduction, test, or execution observation. Original consequence is retained and narrowed to the lost negative reply oracle; the former echo was never a universal scheduling barrier.

Returned proposal (claims-080, event 8): **retain-verified**, suggested bucket **other**.

The listing-limit test checks a reply cache that cannot change between sending request 3 and asserting its absence. The source establishes lost test sensitivity to premature replies, without establishing a production admission defect.

Original stronger consequence: The initial lead suspected queued management requests could appear pending while typing continued. Investigation narrowed the established consequence to the listing-limit test missing premature completion or a skipped list semaphore. It still detects an inline list-permit wait blocking the reader. The former echo observation was never a universal scheduling barrier.

Unresolved premise or limitation: None for the negative-oracle defect. A concrete runtime semaphore-bypass failure or observed mutation-test pass was neither shown nor reproduced, and is not needed for this test-only claim.

Proposed basis: Record a new other-priority test-validity finding. Keep the distinction between source-established oracle weakness and unobserved product failure; do not merge it into the discarded generic admitted-management conformance finding.

Existing parent disposition (event 8): verified. Parent explicitly accepted named retain-verified proposal from claims-080 after inspecting it. The listing-limit test checks a reply cache that cannot change between sending request 3 and asserting its absence. The source establishes lost test sensitivity to premature replies, without establishing a production admission defect. Record a new other-priority test-validity finding. Keep the distinction between source-established oracle weakness and unobserved product failure; do not merge it into the discarded generic admitted-management conformance finding. Checked source/results: ["At 1ed482dde220452e424973faf38a5e1318986333, crates/farhelm-supervisor/src/service/handlers.rs:10488-10551 initializes the fixture and reads initial attachment replies before request 3 exists. send at 10595-10600 writes only. read_one at 10606-10641 is the only fixture operation that appends control replies; reply at 10646-10661 invokes it.","At that revision, handlers.rs:10669-10695 writes a channel-1 text frame and awaits TestHost::received_contains, without reading client output. handlers.rs:11287-11317 sends request 3, awaits that input marker, checks host.replies, releases held list permits, and only then reads the correlated listing reply. An already queued or transmitted early reply is invisible to the negative assertion.","connection.rs:455-581 routes input through the attached input side; the separate writer task starts at 921. Host input reception supplies no client-output drain. handlers.rs:585-615 still acquires list_admission inside the spawned listing task and drops it before sending; no current production semaphore bypass is alleged.","The parent revision's handlers.rs:11196-11213 reads client frames until the echo, preserving replies before that marker. That is a weaker ordered observation, not a guarantee that every concurrently spawned task settled. The diff from the reviewed revision to 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db leaves the input helper and this assertion unchanged apart from unrelated protocol-field removal. The introducing revision occurs within the named review range."] Current coverage/results: ["Current SPEC.md, Waiting between operations on one host: requires terminal/list progress while management waits; it does not accept an unobserved reply assertion. Current SPEC_impl.md terminal switch points and whole-list wire contract provide no acceptance of this test gap.","TODO.md:25-27 has an empty Planned bucket. Targeted searches of plans/queue, BUGS.md, INDEX.md and active TRIAGE_OUTCOMES.md for listing permits/limits, reply draining/cache, BusyHost, input markers and conformance found no exact trigger/consequence/scope coverage.","Read review_feedback_queue/conformance-test-never-admits-management-candidates.md in full and TRIAGE_OUTCOMES.md:1351-1361. They concern management representatives being busy-refused before admitted waits, not a dedicated listing test inspecting an unread cache. Its discard decision does not settle this different root cause.","FILTER.md read in full: none covers a persistent regression-test observation gap. No archive searched."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The initial lead suspected queued management requests could appear pending while typing continued. Investigation narrowed the established consequence to the listing-limit test missing premature completion or a skipped list semaphore. It still detects an inline list-permit wait blocking the reader. The former echo observation was never a universal scheduling barrier."; "None for the negative-oracle defect. A concrete runtime semaphore-bypass failure or observed mutation-test pass was neither shown nor reproduced, and is not needed for this test-only claim.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g126 record](records/review-through-00de0b6c/groups/g126.json). Record ID: `l1`.

