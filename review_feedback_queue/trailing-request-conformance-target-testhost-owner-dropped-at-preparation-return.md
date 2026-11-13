# Keep the target host alive through conformance observation

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Keep the target host alive through conformance observation

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g126/l2`.

### g126/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 9 (finding, open):

# Keep the target host alive through conformance observation

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR

The terminal-control contention test disposes of its target host before sending the control it is supposed to test. A still-running output task does not preserve the healthy target-terminal premise. The result can be a premise failure or an observation of a route whose host has already been torn down.

## Details

Synthesis traced ownership end to end. handlers.rs:10895-10901 binds the TestHost owner in the prepared async future, while only its HostTerminal is inserted into the session entry. adopt_test_host at hosts/terminal/test_host.rs:365-372 returns these as separate values; TestHost exclusively owns the host serve-task JoinHandle, child process, and temporary socket directory (78-108, 434-487). HostTerminal::Inner at hosts/terminal.rs:1232-1280 owns the supervisor client runtime and channels, not a TestHost or its serve-task/child ownership. Cloning the terminal into a session or attachment cannot retain the stand-in host. The preparation future returns only (message, listener) at handlers.rs:10982, so _target_host is dropped before that future completes and before its JoinHandle resolves at 10985. No successful preparation path for these cases carries the owner into the later observation.

TestHost::drop at test_host.rs:104-108 requests task abortion and calls kill_child. kill_child at 126-132 takes the child, calls kill and wait; dropping the remaining TestHost fields also removes its temporary socket directory. Task abort is a cancellation request, not synchronous proof that the serving future has already finished. Kill/wait errors are ignored. These caveats do not restore the fixture's ownership or healthy-live-host guarantee: cleanup has already been initiated, and under the fixture's normal successful child cleanup the target child is reaped before observation begins. This finding is the unconditional premature disposal, not a claim that every run reaches one particular asynchronous terminal state.

The selected controls are real channel-2 operations: request_modes.rs:305-310 declares Detach, Resize, PauseOutput and ResumeOutput as InlineControl, using the chosen context's channel/session. In handlers.rs:3049-3073 they dispatch to set_attachment_paused, handle_detach or handle_resize. Pause/resume mutate the matched attachment watch (connection.rs:1546-1558); Detach takes and joins its forwarder (handlers.rs:1981-2006); Resize resolves its terminal and issues resize if attachment ownership still matches (2019-2084). Thus the patch does not necessarily bypass every relevant handler when the host ends: a surviving attachment can still exercise its lock/map branch. The lost premise is that these actions act on the live target terminal required by the test's own setup comments and live-route check (10893-10894 and 11036-11058). That check tests !forwarder.is_finished(), not host ownership, child liveness or host-task availability. Grid output reacts separately to runtime status (grid_stream.rs:602-668), so a live forwarder can temporarily coexist with prior host disposal. Conversely, if teardown has propagated, the premise can fail. No failing or passing schedule was observed or reproduced. The independent BusyHost::agent_host remains correctly retained and proves only channel-1 typing; it cannot keep channel 2 alive.

Later revisions: all relevant path changes from this commit through pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db were inspected. The handler changes only remove grid/terminal-kind protocol fields; TestHost changes only module wording. At tip, the same owner remains local at handlers.rs:10855, the future returns (message, listener) at 10941, and the output-task check occurs later at 11002-11017. The lifetime defect is still present. Keep the owner in the outer fixture/prepared result through action and cleanup. This record is specific to this conformance fixture, not another fixture's guard or exit oracle.

Suggested bucket: other (maintainer test validity and possible failure). Confidence: confirmed source lifetime defect; runtime schedule outcome unobserved. could-be-highest: no established security or user-owned-work route. proposed_drop: false. possible_cover: none; excluded metadata and queue not read. No tests, runtime probes, or reproductions. Original consequence retained with explicit distinction between disposing of the host and the later asynchronous output-task state.

Returned proposal (claims-080, event 9): **retain-verified**, suggested bucket **other**.

The preparation future disposes of the channel-2 target's host owner before the terminal control and live-route observation. Retaining the client terminal handle or a temporarily live forwarder cannot preserve ownership of the stand-in host.

Original stronger consequence: Earlier wording described shutdown before the action and either a live-premise failure or a dead-terminal cleanup path. Preserve both possibilities, while qualifying that abort requests are asynchronous and kill/wait errors are ignored. Some matched attachment/lock branches can still execute, so disposal does not prove universal handler bypass.

Unresolved premise or limitation: None for premature host-owner disposal. Whether an actual run fails the live-forwarder premise or passes while teardown has not propagated remains schedule-dependent and unobserved. No production defect or user-work loss is established.

Proposed basis: Record a separate other-priority test-lifetime finding. Retain the target TestHost owner through action, observation and cleanup. Do not treat the existing discard of the generic admitted-management gap as acceptance of this different lifetime defect.

Existing parent disposition (event 9): verified. Parent explicitly accepted named retain-verified proposal from claims-080 after inspecting it. The preparation future disposes of the channel-2 target's host owner before the terminal control and live-route observation. Retaining the client terminal handle or a temporarily live forwarder cannot preserve ownership of the stand-in host. Record a separate other-priority test-lifetime finding. Retain the target TestHost owner through action, observation and cleanup. Do not treat the existing discard of the generic admitted-management gap as acceptance of this different lifetime defect. Checked source/results: ["At 1ed482dde220452e424973faf38a5e1318986333, handlers.rs:10893-10902 creates the target terminal and local _target_host for Detach, Resize, PauseOutput and ResumeOutput. handlers.rs:10947-10973 attaches that terminal; 10982 returns only (message, listener). The local host owner is dropped before the preparation JoinHandle completes and before the later observation.","hosts/terminal/test_host.rs:78-108 gives TestHost exclusive ownership of the serve-task handle, child and temporary directory; Drop aborts the task and calls kill_child. Lines 126-132 take the child and call kill/wait, ignoring errors. adopt_test_host at 365-372 returns the client terminal and owner separately. hosts/terminal.rs:1232-1280 retains runtime channels and state, with no TestHost owner.","handlers.rs:11036-11058 checks a retained attachment and !forwarder.is_finished(), rather than host ownership, child liveness or serving-task readiness. grid_stream.rs:602-668 handles terminal/runtime state asynchronously. Task abortion is requested rather than synchronously joined; child kill/wait errors are ignored. These caveats leave exact later task state unobserved but do not preserve the fixture's healthy-live-host premise.","request_modes.rs:305-310 selects real channel-2 inline controls. handlers.rs:1981-2084 still exercises attachment lookup/ownership and detach/resize behavior where the attachment survives. The claim must not say every handler branch is bypassed. The independent channel-1 BusyHost host remains retained and can still witness typing.","The diff through reviewed tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db changes only protocol fields in the relevant handler regions and module wording in TestHost. The premature owner disposal remains. The introducing revision is within the supplied range."] Current coverage/results: ["Current SPEC.md, Waiting between operations on one host, requires responsive terminal controls; current SPEC_impl.md, Every terminal in a host, describes stand-in/unit-test hosts but does not accept premature fixture-owner disposal.","TODO.md's Planned bucket is empty. Targeted searches of plans/queue, BUGS.md, INDEX.md and active TRIAGE_OUTCOMES.md for conformance, target-host drop/ownership, terminal-control tests and live-route premises found no exact cover.","Read the existing conformance-test-never-admits-management-candidates.md and active outcome at TRIAGE_OUTCOMES.md:1351-1361. Their cause is generic management candidates being refused before admitted waits. They do not cover disposal of the host used by inline terminal controls. Shared fixture identity is insufficient for coverage or an exact-root alias.","FILTER.md has no boundary accepting a premature fixture teardown or lost test premise. No archive searched."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Earlier wording described shutdown before the action and either a live-premise failure or a dead-terminal cleanup path. Preserve both possibilities, while qualifying that abort requests are asynchronous and kill/wait errors are ignored. Some matched attachment/lock branches can still execute, so disposal does not prove universal handler bypass."; "None for premature host-owner disposal. Whether an actual run fails the live-forwarder premise or passes while teardown has not propagated remains schedule-dependent and unobserved. No production defect or user-work loss is established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g126 record](records/review-through-00de0b6c/groups/g126.json). Record ID: `l2`.

