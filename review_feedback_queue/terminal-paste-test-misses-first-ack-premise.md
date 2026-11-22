# The bracketed-paste loss check does not establish that the runtime processed its first acknowledgement

Reviewed commit: d7603d243f45ac82e90fe66247aad9d155b5e80f

## TLDR

The interrupted paste check can pass without demonstrating that the runtime processed the first acknowledgement.

## Details

**other.** Source inspection.

d7603d243f45ac82e90fe66247aad9d155b5e80f crates/farhelm-supervisor/src/hosts/terminal/tests.rs:2227-2240,2257-2290;
crates/farhelm-supervisor/src/hosts/client.rs:182-185;
crates/farhelm-supervisor/src/hosts/terminal.rs:3185-3195,4088-4107

The improved test distinguishes the acknowledgement from later work for the successful paste, but its loss case can pass
with every piece still outstanding in the runtime. The test documents a bracketed paste with its first piece
acknowledged and its second in flight at loss. It sends the first acknowledgement, reads the second piece, and
disconnects. The window admits all three pieces, and send_pending can put them on the wire before reading any
acknowledgment, so reading piece two is no fence. The asserted Unknown receipt and recovery marker are also the expected
results if the runtime never processed the first acknowledgement, because the reconnect path requires only an opened
paste with some input outstanding. The adjacent multi-paste test explicitly awaits the fully written first paste and
covers a different boundary (:2305-2344). This is a coverage and description gap retained in the child, not evidence of
a shipped input-fate defect or a failing test. Reuse the output-processing fence already added just above if the intent
is to prove partially acknowledged loss, or state the weaker premise. The reviewer suggested other; confidence confirmed
for the test-oracle gap, , The stand-in's send is only a socket write (hosts/terminal/tests.rs:137-169).
HostConnection::next_event is intended to hand out every decoded old-connection event before its error
(hosts/client.rs:815-831), so the finding does not assert that the correct implementation loses a successfully decoded
acknowledgement. It identifies a regression the assertions would not distinguish: skipping application of that
acknowledgement can leave the same observed Unknown and closing marker. No platform-specific delivery race was
established or is needed for this coverage conclusion.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`terminal/terminal-paste-loss-after-ack-premise`.
