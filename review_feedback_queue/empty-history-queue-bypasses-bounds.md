# Empty deferred history escapes row accounting

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Empty deferred-history messages escape the row cap and an accumulated backlog would bypass yielding. The structural gap
is confirmed; material backlog or resource exhaustion remains unverified after investigation, and ordinary senders were
not found.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed structural
accounting gap; material availability consequence unverified after investigation

Source locations: crates/farhelm-ui/assets/grid-model.js:577-594,615-641,657-689 at
ca9b0a19856dc136f63ad2d14f1076027edce1eb.

applyHistory retains every empty rows array and decoded links table, increments queuedRows by zero, and trimQueue
preserves empty appends even when trimming. drainHistory checks shouldYield only inside its per-row loop; empty entries
therefore do not yield and repeated Array.shift operations amplify a backlog. The shipped renderer defers history, and
receive skips synchronous draining whenever a backlog already exists (tip grid-terminal.js:1892-1910); scheduled drains
are animation frames or message-channel tasks (1792-1855).

A hostile stream peer can retain empty pending-history entries outside the row cap, and processing an accumulated
empty-entry backlog ignores the configured yield callback. Source basis at T:
crates/farhelm-ui/assets/grid-model.js:577-594 appends entries even when their row count is zero; 615-641 counts rows
only and preserves empty entries; 657-689 checks shouldYield only after applying a row.
crates/farhelm-ui/assets/grid-terminal.js:1792-1859 schedules history slices and 1886-1912 leaves arrivals behind a
backlog for a later slice. The queue-cap and yield bypass are confirmed from source; actual exhaustion or a measured
duration of GUI unresponsiveness is not claimed. Earlier material-exhaustion uncertainty remains an impact limitation,
not a missing source premise for the cap defect. Ordinary-producer checks: all production GridOutput::History
construction found by search is in crates/farhelm-supervisor/src/grid_output.rs:914-941; Pack::takes at 738-747 always
accepts the first row, so packing cannot emit an empty intermediate message for a large first row. Empty input emits no
message. Catch-up and live callers both use this packer, service/grid_stream.rs:590-599,769-775. No honest empty-history
producer was found.

Possible coverage and caveats: the above hostile-host availability acceptance, with this browser-scope caveat.
TODO.md:215-221 is bounded honest-history representation assessment in Near term, not Planned and not coverage for this
distinct input route. Initial required coverage searches found no other matching accepted rule, plan, known bug, queue
item or ledger decision.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g157/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f;
confirmed persistent at ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This was a source-only review; runtime occurrence
and frequency were not measured.
