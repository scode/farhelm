# A displaced attachment's input pump can submit replacement typing

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

After another attachment takes control, the old input pump can still enqueue an Enter. It can submit text the
replacement attachment is typing into a shell. The source supports the conditional ordering; no harmful command was
constructed.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; occurrence unmeasured

Source locations: crates/farhelm-supervisor/src/service/handlers.rs:1778-1782;
service/grid_stream.rs:1175-1181,1230-1259.

Preserve g035/g176 and other alias provenance. Output teardown is joined, but input cancellation is signalled without a
join; connection epoch alone supplies no attachment-generation fence. Ordinary shipped Enter supplies the submission
premise. Whole-paste collection and sanitization do not settle stale whole-request authority.

handle_attach holds the attachment lock and joins each old OUTPUT task, then merely drops its backend at
handlers.rs:1778-1782, constructs the replacement input pump at :1974, installs it at :2002-2015 and accepts replacement
input. HostAttach::drop at grid_stream.rs:1175-1181 only sends oneshot cancellation; its pump JoinHandle (:998-1002) is
dropped without joining or aborting it. Pump::run's biased selects (:1207-1234) check cancellation only before polling
self.handle(input), not atomically with that future's send. A pump already polled on another worker may have checked
cancellation Pending before takeover, then pause before queue_input's .input.send. The replacement can complete, hand
over its own input first, and the old in-progress poll can then enqueue the displaced input. queue_input at
hosts/terminal.rs:2162-2180 creates a request then sends it with no ownership/cancel check; at tip these lines are
:2227-2245. The shipped supervisor builds Runtime::new at core.rs:17992-18000 in the reviewed commit (:18622-18630 at
tip); Cargo.toml enables rt-multi-thread. Shipped browser keydown handling sends an ordinary Enter as a key message
(grid-terminal.js:1434-1441, grid-input.js:132-135,335-340), and the supervisor encodes it as CR
(grid_input.rs:382-389), so this does not need a fabricated protocol producer. For example, the replacement's
shell-command prefix can be submitted by a displaced Enter before the replacement finishes the intended argument. The
outer attachment lock fences newly received old frames, but cannot fence the old independent pump. Fix direction:
explicitly cancel and join the old input pump before completing takeover/detach and publishing the replacement;
cancellation is already designed to interrupt its wait on the program.

an old view's pending Enter can land after replacement typing and submit a shell command before the replacement user
chooses to submit it.

Possible coverage and caveats: none. Direct discovery-turn confirmation: terminal-host/src/host.rs:570-623 writes queued
bytes in arrival order to the PTY, with no attachment ownership revalidation. SPEC.md:1572-1576 exposes additional
terminals as plain shells; :3930-3948 explicitly preserves takeover/displaced-client rules despite best-effort multi-GUI
consistency. Prior source proof remains: output-only join/backend drop in handlers.rs:1778-1782; cancel-only input Drop
at grid_stream.rs:1175-1181; concurrent-poll gap at :1230-1259; unfenced runtime sender at hosts/terminal.rs:2227-2245
and no takeover generation in InputSequence :1503-1518; ordinary shipped Enter encodes CR (grid_input.rs:382-389). Exact
Tokio 1.53.2 FIFO proof and replacement-publication ordering are in f2-synthesis.json. A constructed destructive command
is not a missing Farhelm source step for unintended shell submission; prior high recommendation was an overly demanding
severity premise and is superseded. No payload or destructive example constructed. This event confirms the route
personally against source rather than relying solely on sibling assessment. This is a revision of existing f2 and a
global duplicate of g176/f1,g035/f2, not a new discovered defect or investigated uncertainty. All historical
source-refutation and proposed-exclusion events remain for independent audit.

Additional reviewed locations: g035/f2:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/handlers.rs:1778-1782,1974-2019;
service/grid_stream.rs:1175-1181,1230-1259; g130/f2:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/terminal.rs:1503-1518,2227-2245; g176/f1:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/grid_stream.rs:1175-1181,1230-1259;
service/handlers.rs:1778-1782; g177/f1:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/grid_stream.rs:1175-1181,1230-1259;
service/handlers.rs:1778-1782; g178/f2: ca9b0a19856dc136f63ad2d14f1076027edce1eb
crates/farhelm-supervisor/src/service/grid_stream.rs:1175-1181,1230-1259; service/handlers.rs:1778-1782.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g048/f2`, `g035/f2`,
`g130/f2`, `g176/f1`, `g177/f1`, `g178/f2`. Original append-only events, revisions, withdrawals and independent
judgments are retained in that run's catalogs. Locations refer to the named reviewed revisions; the original revision
context is ca9b0a19856dc136f63ad2d14f1076027edce1eb; introduced by assigned commit
70cd68a03c101493e0adbf00bb27bb8004adfe41.. This was a source-only review; runtime occurrence and frequency were not
measured.
