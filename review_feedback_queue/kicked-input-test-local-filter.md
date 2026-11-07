# Kicked-input assertions can omit the attack

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A takeover regression assertion lets the client discard the displaced viewer's attempted keystrokes before the
supervisor sees them. Passing therefore does not establish the supervisor's input ownership enforcement.

## Details

Source coordinates from the original records: `crates/farhelm/tests/e2e/session_lifecycle.rs:824`;
`crates/farhelm-helm/src/client.rs:2311-2337,4171-4175`;
`crates/farhelm/tests/e2e/session_lifecycle.rs:812-829; crates/farhelm/tests/e2e/harness.rs:401-438`. References below
use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The migration changes ghost input to send_keys in the distinct-lease test (823-825), same-connection test (2583), and
cross-connection test (2630-2632). Host streams are grids, so harness.rs:401-438 encodes GridInput::Text, invokes
send_grid_input and silently returns on Err. At pinned tip helm client.rs:2334-2359 first removes the local channel,
then publishes Detached via signal_detached (1684-1691), then starts an independent upstream Detach acknowledgement task
(2579-2585). send_grid_input:4194-4198 locks that same local map and refuses the missing entry before enqueueing any
frame. The distinct-lease test waits for expect_detached at line 812, whose implementation only returns after receiving
TermEvent::Detached (harness.rs:3005-3015), before sending its ghost at 824. This local rejection is deterministic. The
other two tests do not explicitly await detach; no deterministic local-drop claim is made for them. Indeed
handlers.rs:1822-1839 expressly permits a full loser queue's detach notice to arrive after Attached. Their attack can
still be prefiltered under ordinary scheduling and their barrier cannot prove the attack reached the supervisor.

Raw-path verification: handlers.rs:1695-1702 leaves displaced connection-local routes intact. connection.rs:460-507
obtains the old route, locks the active attachment map, and checks both channel number and owning connection before
backend.send_input; a rejected stale frame then removes the route at 565-580. The automatic acknowledgement uses
handlers.rs:3112-3113 to remove the route before handle_detach checks ownership. Thus send_input alone bypasses the
client map check but does not reliably reach the ownership guard: its demux may already have sent the acknowledgement
that retired the route. This refines the earlier suggested fix; a direct send_input substitution is insufficient for
deterministic server-guard coverage.

A raw FrameReader/FrameWriter connection (the pattern already used at session_lifecycle.rs:1245-1255) can handshake and
attach with grid:true on a nonzero channel, be taken over, read Detached without acknowledging it, then write
Frame::data on the old channel with an encoded short GridInput::Text. No intervening detach, stale input, or connection
teardown may remove its route. Use a request/reply barrier on the attack connection, then a distinct legitimate echo on
the winner, or keep both channels on one raw connection and order ghost then winner marker. The message must remain one
frame; a short encoded text message is well below send_input's 32 KiB chunk limit, while FrameWriter avoids that helper
entirely. GridInput::encode/decode validates the text (proto grid.rs:1570-1616); the host backend decodes it
(grid_stream.rs:1071-1095), and text encoding preserves the carriage return (grid_input.rs:158-177). A legitimate
pre-takeover round trip of this same encoding establishes that a missing echo is not merely malformed input.

The shipped server correctly enforces the two-part guard under one lock; this is a test-coverage finding, not a shipped
supervisor bypass. Introducing source connection.rs:444-507 and handlers.rs:1695-1702 match the investigated tip path;
the assigned tests survive unchanged. SPEC.md Terminal experience requires supervisor enforcement independently of a
compliant client. Suggested bucket other; confidence confirmed by source; proposed_drop false; possible_cover none.
Synthesis complete; no runtime mutation experiment was authorized or run.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g044/f2`,
`g041/g041-s3`, `g046/f2`. The retained append-only histories preserve all earlier claims and corrections. Reviewed
source references name their own revisions; the inclusive reviewed tip is the hash above. This review used source
inspection only, with no product runtime experiments or live-install changes.
