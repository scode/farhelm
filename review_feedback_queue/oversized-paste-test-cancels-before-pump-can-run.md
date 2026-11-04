# Oversized-paste regression cancels its pump before any delivery opportunity

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The test meant to prove that an oversized paste writes nothing can pass without ever allowing its input pump to attempt
delivery of earlier paste parts.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static fixture
scheduling and masked competing mechanism; no current production paste defect, measured false pass or runtime flake
claimed.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb
crates/farhelm-supervisor/src/service/grid_stream/tests.rs:1250-1261.

the test creates HostAttach, repeatedly calls synchronous send_input without any await/yield until a part is refused
(tests.rs:1250-1258), then drops it before its first await (:1259-1260). The default farhelm_testtrace macro uses
CurrentThread (crates/farhelm-testtrace-macros/src/lib.rs:247-259), while HostAttach::new spawns its pump
(service/grid_stream.rs:1028-1042) rather than running it inline. Drop signals cancellation (:1175-1181); Pump::run's
biased select polls cancellation before its queue (:1215-1234). Thus the old pump never gets a scheduling opportunity
before cancellation; even if earlier non-last paste parts were incorrectly queued for delivery, teardown would discard
them and the final after-only assertion could still pass.

the test meant to prove that an oversized paste writes nothing can pass without ever allowing its input pump to attempt
delivery of earlier paste parts. This leaves the overflow regression unable to distinguish proper whole-paste buffering
from teardown discarding prematurely queued parts. Source: tests.rs:1250-1258 creates HostAttach and calls synchronous
send_input repeatedly until refusal, without an await/yield; :1259 drops it before the first await at :1260.
HostAttach::new spawns its pump (service/grid_stream.rs:1028-1042), send_input/collect_paste are synchronous
(:1070-1165), and Drop makes cancellation ready (:1175-1181). The pump's biased select checks cancellation before queued
input (:1215-1234). The attribute defaults to CurrentThread: farhelm-testtrace-macros/src/lib.rs:28-38,247-259;
farhelm-testtrace/src/lib.rs:1126-1142 builds new_current_thread for that configuration. Pinned Tokio 1.53.2
src/task/spawn.rs:23-25 guarantees spawn does not synchronously poll its future. Therefore the pump has never run when
cancellation is sent; wrongly queued non-last parts can be discarded by cancellation before delivery, satisfying the
after-only assertion. Existing neighboring test tests.rs:1113-1146 does give a small unfinished paste a processing
opportunity via DA1, so it covers basic non-last-part buffering; it does not establish the large/overflow boundary this
test claims. The same immediate-drop structure in :1056-1063 is context only, not an expansion of the assigned patch.
Fix direction: add a witnessed pump processing/barrier opportunity while the large paste is still incomplete, before
overflow/drop, then assert no paste reached the host; use a deterministic seam/oracle rather than a sleep.

Possible coverage and caveats: none found after exact paste/backlog/oversize/overflow/pump/test-opportunity searches
across SPEC.md, SPEC_impl.md, TODO.md Planned, BUGS.md, unlanded plans/queue, queue INDEX/matching files and
TRIAGE_OUTCOMES.md. SPEC_impl.md full Grid streams bullet :2442-2482 specifies buffering/drop at overflow and does not
accept missing regression evidence. Planned remains empty at TODO.md:23-24. Nearby basic buffering regression is partial
independent coverage, not an exact overflow/drop oracle. FILTER.md rare diagnostic/status/person-race filters do not
match this deterministic test-scheduling weakness.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g178/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb. Test
introduced at 70cd68a03c101493e0adbf00bb27bb8004adfe41 and unchanged through tip.. This was a source-only review;
runtime occurrence and frequency were not measured.
