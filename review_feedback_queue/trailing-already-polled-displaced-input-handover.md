# Keep unchanged already-polled handover separate from newly delayed natural-detach cancellation

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Keep unchanged already-polled handover separate from newly delayed natural-detach cancellation

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g119/c4`.

### g119/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333; exclusive review base: 6e423e30068b62c6cfa276c6d720ee35bb70387a. TLDR and original strongest consequence preserved: queued old-viewer input may enter behind replacement input and change a terminal command, potentially executing destructive input. Suggested bucket: high for an established mixed-input consequence; could-be-highest: true if the mixed command executes a destructive action. Confidence for the original already-polled concern: likely with the precise premise that the old pump is inside a ready send poll when cancellation is signalled and can enqueue after replacement input; no execution or destructive command observed. Original investigation still applies: HostAttach::drop at grid_stream.rs:1169-1175 only signals cancellation; Pump::run:1201-1241 prioritizes it at poll boundaries, not during a poll. InputSequence::send at hosts/terminal.rs:1527-1535 and queue_input:2320-2339 use mpsc admission and a connection epoch, with no attachment ownership fence. The initial broad c4 safety refutation remains retracted. Original proposed_drop basis is preserved as a pending scope proposal only: the HostAttach/Pump mechanism is unchanged from the exclusive base to the reviewed commit's parent (empty diff), and the reviewed diff changes no pump cancellation or input admission code. The removed output reapers never joined the input pump. Parent has accepted no drop or lowering. possible_cover: none searched; caller-supplied g124/l1 is a related, distinct finding rather than coverage of this original concern. Required cross-group synthesis before discovery: g124/l1 establishes a different source lifetime gap, not the original already-polled race. Checked reviewed connection.rs:1510-1517: the natural-detach task moves old.forwarder only, releases attachments at 1515, notifies at 1516, and only then drops the remaining old.input at the block end. The immediate parent connection.rs:2172-2189 explicitly binds and drops backend at 2187 before releasing attachments at 2188. Exclusive-base connection.rs still has this explicit drop at 2187. Therefore an old pump not polled yet can begin before it has received any cancellation, after replacement admission; that particular regression is in-range and is not dismissed by c4's original pre-range comparison. The pinned ActiveAttach replacement at terminals.rs:458 supplies the ordinary HostAttach field whose drop controls that cancellation. Contract remains SPEC_impl.md, Terminal host, Grid streams: an attachment ending drops input not handed to its runtime, while already handed input precedes replacement input. Restoring drop-before-unlock addresses the new delay, but does not certify against the unchanged mid-poll concern. Caller-supplied trigger evidence retained with priority caveat: g124/l1 traces an accepted authenticated API size of 1001 columns to a live host runtime and grid output encoding failure, then natural detach; the bundled GUI itself clamps size to 1000. This reviewer independently checked the source lifetime ordering above, not every link of that size-producer route. Preserve g124/l1's potential highest command-execution consequence and its explicit accepted-API versus bundled-GUI producer distinction for the parent's priority judgment; neither becomes a source refutation here. No new ID or independent discovery is claimed for this already-known interface finding. The remaining uncertainty for the original c4 concern is the actual production interleaving and harmful mixed-command route, not an uninvestigated missing source fence. Source-only analysis; no tests or reproductions.

Returned proposal (claims-076, event 7): **out-of-range**, suggested bucket **high**.

The original already-running-send concern predates the exclusive review base; the removed barriers did not join input. The broad initial safety refutation remains withdrawn. The distinct new natural-detach delayed-cancellation regression is in range and is not covered by this scope result or the already-polled filter.

Original stronger consequence: Old queued input could follow replacement input, alter a terminal command and potentially execute destructive input. Initial blanket cancellation-based refutation was retracted. Later scope proposal concerns only the unchanged already-polled race, not the in-range natural-detach lifetime regression.

Unresolved premise or limitation: No production interleaving or destructive command was observed. High is the conditional mixed-input consequence; moving to highest needs the harmful command route established. Parent must retain the original possible-highest wording and independently judge any scope/filter drop or lowering. g124/l1's accepted API size 1001 route was supplied, not fully rechecked; bundled GUI caps 1000.

Proposed basis: Original mechanism and both implicated files are unchanged before the exclusive base; exact already-polled FILTER boundary is also present. Independent drop check must limit agreement to that mechanism and preserve the separate natural-detach finding.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g119 record](records/review-through-00de0b6c/groups/g119.json). Record ID: `c4`.

