# Resize benchmark can report zero delivery time after transmitting its output

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The manual resize benchmark can report zero last-frame delivery time after it has transmitted its resize output.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source defect, not a measured flake. A normal benchmark path drains frames inside every resize step
(tests.rs:1610-1612), discards their arrival times, then calls drain_until (:1615-1624), whose arrival accumulator
begins at zero (:1501). If the last step's drain has already consumed all final frames, done is true and no new frame
updates the accumulator (:1503-1510), so last_arrived_ms (:1633) is zero. There is no contract requiring a frame to
remain for that final helper call. Additional sender inspection at tip grid_stream.rs:602-675,680-770 confirms
production cycles run independently, wait for status or relayout settlement, and can finish delivering history as the
earlier drain consumes it; nothing fences final convergence to occur only after entry into drain_until. Keep the most
recent arrival on Link or pass it across drain boundaries.

Source locations: 70cd68a03c101493e0adbf00bb27bb8004adfe41
crates/farhelm-supervisor/src/service/grid_stream/tests.rs:1496-1510,1610-1633.

measure_resize carries and applies queued frames inside each resize iteration (tests.rs:1610-1612), discarding
Link::carry's arrival timestamp. Its final drain_until (1615-1624) initializes arrived to Duration::ZERO (1501) and
returns it if no further frame is available and the viewer has already converged (1509-1510). When the last iteration's
drain consumes the final resize frames, last_arrived_ms (1633) therefore reports zero. Keep the most recent arrival on
Link itself, or carry the previous timestamp into drain_until.

measure_resize carries and applies queued frames inside every resize iteration but discards Link::carry's arrival
timestamp. Its final drain_until initializes arrived to Duration::ZERO and immediately returns that if the viewer is
already converged and its queue is empty. Thus a run whose final frames were consumed by the preceding loop reports
last_arrived_ms=0. Source: assigned patch p0540.diff lines 334-353 and 437-477; exact immutable file line numbers to
follow.

the manual resize benchmark can report zero last-frame delivery time after it has transmitted its resize output.

Possible coverage and caveats: none found. Refreshed coverage searches across SPEC.md, SPEC_impl.md, TODO.md Planned,
BUGS.md, plans/queue, queue INDEX/matching files and TRIAGE_OUTCOMES.md found no exact resize-benchmark timestamp
defect. Matching terminal contract material was read fully during initial pass and tip SPEC_impl.md Grid streams
:2442-2482 reread in synthesis. Shared resize/measurement hits concern GUI geometry and do not match trigger,
consequence or scope. FILTER.md's rare-diagnostic filter remains no established full match: the branch is ordinary
benchmark draining, and no rare-trigger premise is shown.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g178/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41;
persistence verified at ca9b0a19856dc136f63ad2d14f1076027edce1eb by exact source reads and an empty git diff for both
grid_stream.rs and its tests.. This was a source-only review; runtime occurrence and frequency were not measured.
