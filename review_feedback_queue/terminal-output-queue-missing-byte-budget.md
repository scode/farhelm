# One remote terminal can make the helm retain almost two gibibytes of output before its queue limit trips

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Opening one hostile remote terminal can make the helm retain almost 2 GiB of output before its message-count limit
trips, affecting the process that serves all hosts.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F22 / SEC-TERMINAL-BYTES`, reviewer `security_general`, pass 2. Confidence: **definite**. Review disposition: **would
surface**. Queue priority at recording: **highest**.

Anchor against the reviewed commit: `crates/farhelm-helm/src/client.rs:1995`. Recorded from the completed review without
rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: The
accepted fixed supervisor-metadata retention rule is related but does not settle the almost-2-GiB terminal payload
allowance. The existing hint-flood ledger decision calls for proportionate availability remedies; retain this distinct
byte-budget issue without claiming measured swapping or OOM.

Each terminal attachment has a queue limited to 256 messages, but incoming terminal data can use the protocol's general
8 MiB frame allowance. The helm queues each payload unchanged. Although the queue was sized around the honest
supervisor's 32 KiB output chunks, that chunk limit is not enforced on receipt. A hostile supervisor can fill one queue
with 256 payloads of 8 MiB minus the five-byte frame header, retaining almost 2 GiB before the next message triggers
detachment.

The normal attachment sequence provides a window in which none of those messages can be consumed: the helm registers the
queue before sending the attach request, but exposes its receiving end only after the supervisor replies `Attached`. A
hostile peer can send the large data frames first and then send the successful reply. This is a finite burst, not a
scenario requiring an indefinitely unanswered request or a frozen browser. Merely opening a terminal can therefore
impose gigabytes of memory pressure on the helm, which serves other hosts and runs inside the native desktop process.
Swapping or out-of-memory termination has not been reproduced, but the frame allowance, queue capacity, and absence of a
consumer during this window are established by the source.

Apply a byte budget to queued incoming terminal data, including data received before attachment succeeds, and visibly
detach when that budget would be exceeded. Enforcing a documented maximum terminal-data chunk at receipt is another
option; splitting large frames alone is insufficient unless admission stops at the budget. Add a controlled-peer
regression that sends large frames before `Attached` and verifies refusal or detachment within a small fixed allowance,
without constructing a multi-gigabyte fixture.
