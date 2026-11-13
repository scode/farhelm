# Old-terminal large-message acceptance loses its regression check

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: old terminals still supported at this intermediate revision lose the browser check that a paste sent as one message over 1 MiB does not disconnect them and still permits later input. This is historical test-only coverage loss, not an alleged production input failure.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g175/c3`.

### g175/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: old terminals still supported at this intermediate revision lose the browser check that a paste sent as one message over 1 MiB does not disconnect them and still permits later input. This is historical test-only coverage loss, not an alleged production input failure. Details: deleted parent terminal-flood.spec.ts:1149-1193 proves receiver liveness with an echo before sending 1 MiB+1 raw bytes as one WebSocket message, requires the socket to drain while open, waits for a post-paste echo behind that ordered input, and checks that the attachment is still the same open socket. The recorded revision still serves adopted old terminals (SPEC_impl.md:2294-2311,2956-2978), and helm terminal.rs:539-554 expressly retains an 8 MiB message/frame cap because old xterm sends whole pastes as one message; :889-891 dispatches byte streams through the old chunked send_input path. A concrete competing regression is lowering TERM_WS_MAX_MESSAGE to 1 MiB: this deleted test fails because the 1 MiB+1 message closes the socket, but the retained grid paste remains below its 64 KiB-per-message bound and does not distinguish that cap regression. Retained checks actually inspected: terminal.rs:1623-1652 tests rejection at TERM_WS_MAX_MESSAGE+1, using the current constant, so it still passes if the constant is wrongly reduced; :2238-2356 accepts one grid message of exactly 64 KiB and :2372 onward checks oversized-grid refusal. terminal.rs:1271-1355's 8 MiB input is sent directly through SupervisorClient, bypassing WebSocket admission; it checks bounded detach, not socket acceptance or post-send receiver liveness. Searches across retained e2e/tests, grid-client, Rust end-to-end and helm tests for large binary messages, megabyte/paste bounds and raw frame sends found no retained positive old-WebSocket acceptance oracle. The grid digest test at terminal-flood.spec.ts:101-153 remains meaningful for grid byte integrity, but is not evidence for acceptance of an old raw message over 1 MiB. Suggested bucket: other, regression testing only. Confidence: confirmed source/contract analysis, no execution or reproduction. proposed_drop: false; earlier grid-contract/production-correctness proposed-drop rationale is withdrawn and preserved in original c3 history. possible_cover: none known; no prohibited coverage sources searched. Could-be-highest: no current security/loss-of-work route alleged from test deletion alone. Original consequence preserved: loss of oversized old-message acceptance, same-socket and follow-up input regression coverage. Later disposition is separate: old terminals retire at dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 and the byte renderer/protocol path is removed at f560058da19e02716e6f943883a9508b447aff4e, making this specific acceptance oracle obsolete then. Historical fix direction: preserve a byte-stream positive socket-bound check through an adopted-old fixture or scripted byte-stream peer until the consumer is removed, rather than restore the retired path at tip.

Returned proposal (claims-109, event 11): **retain-verified**, suggested bucket **other**.

The removed case positively required one raw message above 1 MiB to reach a live old-terminal receiver without replacing its socket. The surviving grid paste deliberately splits input, and the cap-relative rejection test does not pin that positive threshold.

Original stronger consequence: Lines 3/7 initially proposed dropping because grid paste integrity and grid oversize refusal remained; line 9 reopens that proposal and line 11 withdraws it. Preserve the original old single-message acceptance/same-socket/follow-up-input coverage loss. Retained grid digest test remains useful and does not certify OS clipboard interception or exhaustive paste behavior. Unrelated prompt/termination/Delete contracts were not inferred covered.

Unresolved premise or limitation: No actual oversized-input disconnect or production loss is shown. The source-supported cap-reduction example demonstrates distinct test sensitivity only. The old consumer is absent at tip.

Proposed basis: Retain the historical positive-admission test gap with other priority. It is distinct from g175/c2's output/flow/remount gap; neither is a shipped user-work-loss allegation.

Existing parent disposition (event 11): rejected. Full109:647 removes positiveone1MiB+1rawmessage acceptance whileliveoldreceiver/socketcontinuity/pre-andpostecho established. Gridchunkeddigest test andcap-relative8MiB+1negative test cannot detect loweringoldpositivethreshold; sourceconfirmsdistinctintermediateoracleloss. dbfc/f560endoldconsumerbytip, noreviveobsoletewholemessagefixture. Preserve originalgrid-substitute dismissalwithdrawal, usefulgridtest, no actualdisconnect/pasteloss/workloss.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g175 record](records/review-through-00de0b6c/groups/g175.json). Record ID: `c3`.

