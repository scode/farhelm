# Delete can discard an upload's only result while its healthy caller waits forever

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Delete can discard an upload result during temporary connection backpressure, leaving the upload waiting forever even
after the connection resumes normal traffic.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F19 / COR-UPLOAD-REPLY`, reviewer `correctness_systems`, pass 2. Confidence: **definite**. Review disposition: **would
surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/service/uploads.rs:350`. Recorded from the completed
review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

Upload replies use a bounded priority queue. To keep Delete from waiting behind a blocked upload reply, cancellation can
discard a reply already waiting for room; after cancellation, further replies are attempted once and silently discarded
if the queue remains full. Neither path retains the response for later delivery or closes the connection. This includes
required begin/commit results and abort notifications. For example, an upload can publish its file, wait to enqueue its
success response, then lose that response when Delete cancels the transfer. If the helm resumes reading before the
connection's 60-second no-progress limit, the connection remains healthy but the upload's result is gone permanently.

The helm waits directly for the begin reply, and commit waits for its reply or an upload-ended notification. Losing
those messages can leave the HTTP upload pending indefinitely while ordinary status traffic continues. The existing
full-queue cancellation test explicitly verifies that the transfer ends with only the older frame queued; it does not
check what happens to the caller once reading resumes. Separate file-cleanup completion from responsibility for
delivering the required final response. Delete must remain prompt, but a bounded connection-owned path must retain the
result or abort until queued; if it cannot do so safely, close the connection so pending requests fail. Extend the test
to drain old frames and require either a final upload response or explicit connection closure.
