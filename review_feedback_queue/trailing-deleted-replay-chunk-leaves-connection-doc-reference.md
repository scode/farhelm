# Replay removal leaves a dangling writer-queue documentation link

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: maintainers reading the connection's queue-memory contract are sent to a constant this patch deletes, while the explanation still describes byte-stream chunking rather than the remaining grid messages. This is internal documentation drift, not a demonstrated runtime resource regression.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g124/l2`.

### g124/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: maintainers reading the connection's queue-memory contract are sent to a constant this patch deletes, while the explanation still describes byte-stream chunking rather than the remaining grid messages. This is internal documentation drift, not a demonstrated runtime resource regression. Details: the assigned patch deletes the sole REPLAY_CHUNK definition at former connection.rs:40 but leaves the intra-doc link [`REPLAY_CHUNK`] at new :44 and the plain-name drain-throughput explanation at :867. Pinned git grep over connection.rs and its service/terminal scopes finds no definition/import to resolve the link, only those remaining prose references. The surrounding :43-46 asserts every terminal frame is 32KiB-chunked and worst-case data backlog is 2MiB; GridStream::send (grid_stream.rs:818-825) actually encodes a whole grid message then queues one Frame::data. grid_output.rs:67-78 declares 32KiB a packing target and expressly sends a larger row alone; the grid format hard cap is 1MiB, so the 64-frame queue's paper data bound is 64MiB, not the inherited claimed 2MiB. Attribution caveat: the false universal byte-size wording is already at exclusive range base 6e423e30068b62c6cfa276c6d720ee35bb70387a:connection.rs:53-59; do not claim this removal newly introduced that numeric claim. The newly established defect here is deletion of its linked constant without updating the surviving source contract. The actual grid framing was already present; no memory growth or encoder change is attributed to this patch. Later status: pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:connection.rs:41-60 correctly describes the 32KiB target/1MiB hard cap and removes the link. Scoped -G history names f560058da19e02716e6f943883a9508b447aff4e as the only subsequent matching change, and the complete f560 connection.rs patch confirms it replaces the queue paragraph and the obsolete drain_writer constant reference with grid-message wording. The original charter includes intermediate changes even when corrected later. Suggested fix at reviewed revision: describe grid message packing versus hard cap and remove/replace obsolete constant references; the endpoint already does this for the queue paragraph. Suggested bucket: other (developer-documentation navigation and resource-bound explanation). Confidence: source-confirmed unresolved reference and stale explanation at the reviewed revision, not a rendered rustdoc warning or build failure; no documentation build run. Could-be-highest: no established security or loss-of-work route. proposed_drop: false; later repair is provenance, not assumed parent exclusion. possible_cover: none in supplied neighbour records; prohibited coverage sources not searched. No tests, builds, reproductions or live-install actions. This revision closes l2's alias/name-resolution check; exact later-repair attribution is now confirmed and no source premise remains open.

Returned proposal (claims-079, event 15): **needs-parent**, suggested bucket **other**.

The historical documentation defect is verified, but it is repaired later within the reviewed range. Preserve it as an intermediate defect and an explicit already-repaired disposition; no current fix remains to queue. Parent owns how this completed historical correction is represented.

Original stronger consequence: The lead suggested misleading memory-bound reasoning; the finding narrows attribution to deletion of the linked constant. The inherited numeric claim predates the range, and the later in-range repair is explicitly preserved.

Unresolved premise or limitation: None for the historical dangling-reference defect. No rendered rustdoc warning, build failure or runtime memory regression is claimed or observed.

Proposed basis: Verified at 1ed482d, repaired by f560058d within the range. Do not falsely label it pre-range, presently unfixed, or a runtime resource regression.

Existing parent disposition (event 15): rejected. Parent read full claims079.1ed482 deletes linked REPLAY_CHUNK leaving connection43-46/867 dangling; false universal2MiB bound itself already existed. f560058 repairs references and explains32KiBtarget/1MiBcap/64MiBpaperbound, retained at00de. Preserve real intermediate dangling-reference defect and repair without residual item; no observed rustdoc/build/memory failure or stronger consequence.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g124 record](records/review-through-00de0b6c/groups/g124.json). Record ID: `l2`.

