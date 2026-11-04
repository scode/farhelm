# The paste-receipt regression checks before acknowledgement processing

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The regression does not prove that a long paste stays unconfirmed until all its pieces are acknowledged. It can miss a
receipt completing after the first piece. No current production receipt defect is established.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source-level
absence of a processing witness

Source locations:
70cd68a03c101493e0adbf00bb27bb8004adfe41:crates/farhelm-supervisor/src/hosts/terminal/tests.rs:2203-2206.

At reviewed commit 70cd68a03c101493e0adbf00bb27bb8004adfe41, hosts/terminal/tests.rs:2203–2205 writes a partial
acknowledgement and polls pending immediately; :2206–2210 then writes the final acknowledgement and accepts Written.
Server::ack:163–169 proves socket delivery, not runtime processing. The default current-thread runtime and Tokio1.53.2
write_all immediate-ready path permit the first acknowledgement to be processed only afterward. A processed
output-record fence like :2286–2292 distinguishes the competing receipt behavior. The test file is unchanged through the
pinned tip. Actual production terminal.rs:1580–1585 waits for every piece; the parked cross-agent prompt design is not a
shipped harm claim. SPEC_impl.md:1901–1972 requires whole-paste delivery; no exact acceptance, planned fix or queue
duplicate was found.

tests.rs:2203 writes the first InputAck through Server::ack (163-169), then 2204-2205 polls receipt.fate once
immediately; the write completing establishes only socket delivery, not that the runtime has handled the
acknowledgement. The final acknowledgement is then sent at 2206 and Written accepted at 2207-2210. A runtime that
incorrectly completes the receipt on its first acknowledgement can pass if it processes the first acknowledgement after
pending_once. Establish a processed acknowledgement fence, e.g. an output record followed by waiting for that record in
the terminal, before asserting pending, as the same patch does at 2286-2292.

The regression can miss a receipt that completes after only the first piece. The separate agent-prompt design is
disabled in this build and is not evidence of a shipped truncated-prompt submission.

Possible coverage and caveats: the completed source and coverage checks found no exact exception, planned fix or queue
duplicate. SPEC_impl.md:1901–1972 requires whole-paste delivery; it does not accept an unfenced regression assertion.
The existing browser settings-write/cleanup records concern different ownership boundaries. Planned is empty, and no
filter matches this test-assurance defect.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g136/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41.. This
was a source-only review; runtime occurrence and frequency were not measured.
