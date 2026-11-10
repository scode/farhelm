# Empty deferred history escapes row accounting

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Empty deferred-history messages escape the row cap and an accumulated backlog would bypass yielding. The structural gap
is confirmed; material backlog or resource exhaustion remains unverified after investigation, and ordinary senders were
not found. A newly traced consequence can make a displayed scrollback URL open a different, undisclosed screen URL. That
source-confirmed action mismatch requires a protocol-valid malicious or broken peer and deferred backlog.

## Details

Current suggested bucket: **highest**, for the paint/action mismatch below. The original availability investigation and
its uncertainty remain intact; the new route does not depend on proving denial of service or resolving its browser
acceptance question.

### Empty-entry trimming can redirect a displayed URL

Additional reviewed commit: befe5c72f7af72bb7e2fae339c0f4ca52ff816da, checked through
0bc984b71384e7b754b55d007b53ebbeb9a076ca. Confidence: **verified source mechanism and action route**; no browser
reproduction or payload experiment ran.

At `crates/farhelm-ui/assets/grid-model.js:931-944`, trimming encounters an empty queued entry while excess rows remain,
advances that entry's offset and subtracts the excess from `queuedRows` without trimming the actual rows. Drain then
uses the shifted empty entry to establish `historyFirst` (`:974`), skips its body and resets `historyNext` (`:999`).
Later real rows retain that shifted first coordinate. The resulting history end (`:1017-1019`) overlaps the screen's
absolute coordinates (`:1011-1014`) at ordinary safe counters. This is separate from the extreme-numbering finding.

The painter reads history by stored index (`grid-terminal.js:914-940`), while pointer mapping assigns its shifted
absolute number (`:555-558,1101-1119`). The text/link source resolves overlapping coordinates screen-first
(`grid-buffer.js:382-420,585-595`). Hover identity and release checks compare that same screen source
(`grid-terminal.js:1305-1337,1708-1715`), so they do not detect that another history row was painted under the pointer.
The provider retains the screen row's plain URL (`terminal.js:1311-1324,1360-1367`), and the ordinary non-loopback URL
disclosure waiver (`:1247-1263`) lets a primary click open it (`terminal-links.js:79-84,163-168`). No final comparison
with the painted row closes this gap. This establishes an undisclosed action-target change, not credential theft or code
execution.

Empty history is protocol-valid (`crates/farhelm-proto/src/grid.rs:224-234`). Deferred receives and sliced drains
(`grid-terminal.js:1929-1940,1995-2015`) supply the backlog boundary. Production `grid_output.rs:904-931` skips empty
histories, and both sender paths use that helper (`service/grid_stream.rs:593-599,771-776`), so no ordinary-output
producer is alleged. The trigger is a malicious or broken protocol-valid peer. SPEC.md's plain-URL rule relies on what
is shown being what opens; hostile-host availability acceptance does not accept this action mismatch.

Handle zero-count queue entries separately before assigning trim offsets, and preserve exact unconsumed-row accounting
and aligned history coordinates. Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical
`g035/g035-l06`; interface groups synthesized the route without identifying another root cause. Existing queue coverage
is extended because the accounting cause is shared. No fix or triage decision was executed.

### Availability acceptance and browser scope

Additional reviewed commit: c5ae8548439cc5bfa33c11ef7ce8da6129d8072a. The acceptance referenced below is SPEC.md,
"Remote input, session defaults, and availability", 4045–4099, especially 4085–4092: denial of service of the helm or
desktop app by a malicious or arbitrarily broken host is accepted outright, even when Farhelm could avoid it. Existing
mitigations remain. The rule does not accept corruption of the helm's state or effects on other hosts' security or data.
SPEC_impl.md, "Supervisor metadata retention and nonresponse", 76–90 points to that broader acceptance rather than
narrowing it to selectively unanswered requests.

That text expressly names the helm and desktop app. Whether it also settles this browser-only availability consequence
remains ambiguous. The separate bundled-terminal-library exception does not by itself settle a defect in Farhelm's own
grid queue. This addition supplies the missing antecedent for the coverage paragraph below; it does not decide that
coverage question, change the bucket, or establish material exhaustion. Provenance: review run
`ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g015/g015-l1`.

### Original mechanism and investigation

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
