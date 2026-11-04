# Input-window test cannot establish that refused input stays off the wire

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The terminal input-backpressure regression's claim that rejected input never reaches the host is stronger than what it
observes.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static
test-observability gap; no runtime mutation or false pass observed.

Source locations:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/client/tests.rs:505,528-530.

tests.rs:477-491 reads exactly three Input frames; :497 checks only read_frame's lookahead Vec, whose decoding at :44-65
can leave the socket unread after returning a complete frame. A wrongly transmitted rejected Input may therefore remain
in the socket. At :520-522 the test sends the identical chunk again and accepts any Input, so that rejected frame can
satisfy the final read while the retry frame goes unobserved. The counter assertion at :498 does not detect a frame
queued/written despite failed admission without incrementing accounting. This depends only on the extra frame fitting
the socket, rather than a demonstrated runtime schedule; no mutation experiment was permitted. Use a distinct later
control-frame sentinel and read through it to establish the no-send boundary. Current production code checks admission
before queueing (client.rs:831-842; InputWindow::admit :559-571), so no current product defect is claimed.

tests.rs:477-496 reads exactly the first three Input frames; :497 checks only buf.is_empty(), which is read_frame's
lookahead buffer rather than the socket; :521-524 later sends the same chunk again and accepts any Input frame. A fourth
frame left unread in the socket by the refused send can satisfy that final read, while the accepted retry becomes an
unobserved fifth frame. The sent-count assertion at :498 does not distinguish a frame sent without admission/accounting.
Use a later distinct frame/sentinel and read through it, or drain socket bytes at an explicit boundary, to witness no
rejected frame.

The terminal input-backpressure regression's claim that rejected input never reaches the host is stronger than what it
observes. This is a test-coverage defect, not a current shipped input-loss finding. Source basis at tip:
tests.rs:485-500 consumes exactly three Input frames; :505 checks only the helper's lookahead Vec, not all unread socket
bytes. read_frame at :54-68 returns as soon as its Vec contains a complete frame, with no subsequent socket-drain
observation. :506 and :515-523 do establish unchanged accounting; they do not establish absence of an unaccounted frame.
:528-530 retries identical bytes and accepts any Input without a sentinel or a final stream check, so a previously
transmitted but unaccounted rejected frame can satisfy this read instead. The limited confirmed assertion defect is
independent of whether any particular kernel buffers a whole extra MAX_INPUT frame: no kernel-buffer capacity or
measured mutation-survival claim is made. A hypothetical wire regression is the counterexample to the oracle, not a
claimed behavior of the existing client. Current production code is correct at the inspected boundary: client.rs:852-863
calls InputWindow::admit before queue, and :569-581 returns InputWindowFull before updating counters.

Possible coverage and caveats: none found in the required candidate-focused source searches recorded in the earlier
revision. The required spec, Planned TODO, BUGS, queue/matching items, plans and ledger searches did not identify an
explicit acceptance or a same-trigger/consequence/scope fix.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g107/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 1eef3d64e36cd31b56e5b7e815fa3cc183b55637.. This
was a source-only review; runtime occurrence and frequency were not measured.
