# Reconnect replay can keep an obsolete restoring fence

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

After a resize write interrupts replay and reconnects, Farhelm can keep refusing prompts as still restoring even after
the terminal has caught up. More output or replacement of that runtime can clear it.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
static mechanism; frequency unmeasured

Source locations: bd4d00ab1767cdccbd91c58bdf968928eaeec5a3:crates/farhelm-supervisor/src/hosts/terminal.rs:1858-1945;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:3876-3981.

The complete ordinary attach-resize producer and event-repacking schedule are recorded in original g126/f3. Preserve its
actual event-count versus byte-position proof, client queue byte bound, earlier write-error path, prompt-refusal
consumer and narrow status/filter ambiguity. This is a running runtime with a stale readiness fence, separate from
explicitly accepted unavailable runtimes.

adoption stores the number of stream events already queued on the original connection (C terminal.rs:904; T :1805);
on_event decrements it per event (T :3146), and publish requires fence == 0 as well as the processed Position reaching
adopted_end (:3802-3814). Reconnect/resume starts a newly framed Read from the processed Position without
rebasing/resetting that old event count (C :1858-1945; T :3876-3981). A write can fail before the old queued events are
drained: turn sends input and resizes ahead of next_event (T :2876-2897,2917-2940; C :1417-1444). A shipped attachment's
initial/later resize calls this path without a caught_up gate (T service/terminals.rs:260-280; handler call sites
:1847,2164). No external takeover is required for the conditional failure route: client's write drive also reads output
and can return EventQueueFull before its ready write wins the select (T client.rs:995-1004,1016-1075), leaving the
original queued events unprocessed. A recovered stream need not recreate the original number of Output frames. The real
host appends successive pty reads to one growing block (T terminal-host/src/journal.rs:204-227) and emits only the bytes
currently available from that block per cursor step (:408-468); Host::step/refill can send multiple partial-block
records while a block is growing (host.rs:338-411,625-652). On a later Read those same retained bytes are emitted from
the now-fuller block in fewer records. Thus an old adoption fence containing several partial-block events can exceed all
the records the new Read has left to deliver, even once its byte/resize Position has reached the adoption endpoint. The
runtime then waits with caught_up false; the remaining count is not a byte position or host promise about future
records. Current consumers: T service/ticker.rs:1639-1645 excludes this terminal from sampling, and
service/core.rs:12519-12533 refuses prompts as still being restored.

A connection failure while the supervisor is initially restoring a terminal can leave it marked as still restoring after
the new connection has supplied all its output. Agent status sampling then stays suppressed and prompt delivery refuses
until enough further stream records arrive or the runtime is replaced.

Possible coverage and caveats: unchanged status-subset/rare-healing filter ambiguity; full prompt-refusal effect is not
explicitly accepted by the searched specification/planned/queue/ledger sections. No security, user-work loss, unbounded
resource or permanent UI-hang claim.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g126/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is bd4d00ab1767cdccbd91c58bdf968928eaeec5a3;
persistence and shipped producer at ca9b0a19856dc136f63ad2d14f1076027edce1eb. Clarification under original f3, not a new
root cause: the exact shipped write producer is the accepted viewer attach's terminal.resize(sup, cols, rows) in
service/handlers.rs:1843-1849, plus subsequent owned Resize frames at :2136-2165. Terminal::resize maps Host to
HostTerminal::resize without a caught_up gate (service/terminals.rs:260-280). The runtime accepts the size through
Wake::Size and sends it ahead of consuming the next queued host event (terminal.rs:2884-2897,2917-2940).
Source-permitted schedule: adoption's two Info round trips have queued multiple Output events produced while a journal
block was growing; adoption applies its first event and starts the runtime, retaining the number of remaining events as
fence. A viewer's resize wins a turn before those old events are all processed. conn.resize's flush drives reads as well
as writes; repeated ready reads can append further output events and end the connection with EventQueueFull before the
pending write wins. See client.rs:875-884,995-1004,1016-1075. The exact limit is client.rs:186-200, EVENT_QUEUE_LIMIT =
2*MAX_FRAME + 2*OUTGOING_HIGH_WATER with per-event overhead, not an assumed small event cap; reaching it here is
conditional scheduling/large-output behavior, not a measured common occurrence. Reconnect retains fence and resumes Read
at the processed Position. With the burst now stopped, journal.rs:204-227,408-468 emits the remainder of the now-grown
blocks in fewer Output records than the old partial-block frames counted by fence. Any resumed resize/exit records count
too, but an old fence with more remaining partial events than all those new records still remains positive after actual
byte/resize catch-up. next_event's normal drain-before-error behavior (client.rs:817-829) does not fix an error returned
earlier by the resize request. TLDR/consequence remains repeated prompt refusal and suppressed status sampling after
actual replay catch-up (service/core.rs:12519-12533; ticker.rs:1639-1645), until more events clear the count or the
runtime is replaced.. This was a source-only review; runtime occurrence and frequency were not measured.
