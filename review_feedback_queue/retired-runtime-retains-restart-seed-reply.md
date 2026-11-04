# Retired runtime requests can retain replies; history-seed control can strand an accepted Restart

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

An input/paste reserved before retirement can remain retained with no receipt; the same late-send lifetime on the
runtime's control channel can also leave an accepted Restart waiting for a history-seed reply after the runtime has
ended.

## Details

Suggested bucket: **high**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source mechanism, not runtime-observed.

Source locations: bd4d00ab1767cdccbd91c58bdf968928eaeec5a3:crates/farhelm-supervisor/src/hosts/terminal.rs:2099-2102;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:2281-2292,2350-2357,4166-4169.

finish closes the bounded input receiver then calls try_recv only until its first error (assigned terminal.rs:2099-2102;
tip :4166-4169). The sender uses mpsc::Sender::send (assigned :1094-1099; tip :2233-2245), and the pinned dependency is
Tokio 1.53.2 in both revisions (Cargo.lock:6277-6280). Tokio bounded.rs:816-823 reserves a permit before sending;
:1690-1698,1721-1730 allows that permit to send after receiver closure, with chan.rs:196-198,551-556 pushing without
checking receiver lifetime. If the sending thread is descheduled after reserving but before pushing, finish can observe
Empty (chan.rs:440-462) and drop the receiver; receiver Drop drains only already-pushed values (:503-545). The send can
then resume and push the request after that drain. Its fate sender and input bytes remain in the channel because
HostTerminal's Inner still holds input_tx (tip terminal.rs:1910-1924; original :697-710); chan.rs:576-590 drains late
values only when the final channel reference is destroyed. InputRequest has no drop-time answer (tip :1640-1669), and
receipt.fate waits directly on its oneshots (:1563-1585), so this is not merely the conservative Unknown outcome claimed
in my initial unreported-candidate disposition. Shipped prompt delivery waits on the receipt but has an outer timeout
(tip service/core.rs:2306-2345); grid Pump watches receipts without its own deadline but remains separately cancellable
and its output stream observes terminal liveness (service/grid_stream.rs:1207-1281; SPEC_impl.md:2466-2482). Do not
claim a permanently hung UI, unbounded memory growth, security impact, or user-work loss: the established defect is a
stranded receipt and bounded late request retention per ended terminal, plus unnecessary prompt timeout.

finish closes the mpsc receiver and drains only immediately available requests. Pinned Tokio 1.53.2
(Cargo.lock:6277-6280) bounded.rs:438-449 explicitly allows outstanding permits to send after close and requires recv
until None to guarantee a complete drain; :816-823 implements Sender::send with reserve followed by Permit::send.
chan.rs:440-462 permits Empty while a permit remains; :503-534 drains only values already present on receiver drop;
:576-590 drains again on last channel reference destruction. A possible stronger consequence is a late request's oneshot
sender retained in the channel while surviving HostTerminal handles keep mpsc senders alive, leaving receipt.fate
pending.

An input/paste reserved before retirement can remain retained with no receipt; the same late-send lifetime on the
runtime's control channel can also leave an accepted Restart waiting for a history-seed reply after the runtime has
ended. This additionally holds that session's lifecycle claim until supervisor shutdown. Exact control route: Inner
retains the control Sender (terminal.rs:1204-1248); history_seed creates a oneshot, sends Control::Seed on the bounded
channel and directly awaits the answer (:2350-2357). A producer paused after reserving mpsc capacity can push Seed after
Runtime's control_rx destructor drained; the reply sender remains in the channel while the awaiting method borrows the
live HostTerminal. No custom Runtime/InputRequest destructor repairs this (:2816-2827; prior definition checks). Pinned
Tokio 1.53.2 source previously verified establishes the same reserve/late-push/final-owner-drop mechanics for either
bounded channel. A shipped Restart obtains lifecycle at service/core.rs:12691-12695 and keeps it across a direct
restart_seed await :13075-13088. restart_seed tests Gone before asking but does not atomically coordinate that test with
the send: host_restart.rs:683-689. A terminal host can end after that Gone test, its runtime finish/drop can run while
the seed sender is paused after reserving, and the resumed Seed is never answered. The only local SEED_SETTLE timeout
(:669-677) encloses the preceding readiness loop, not the later history_seed await (:688-689). I read
core.rs:12540-13125 around the full intervening control flow; no enclosing timeout wraps this direct seed call. The
actual handler spawns the accepted Restart as a supervisor-owned mutation, with no timeout around restart_session, and
the connection retains only a reply waiter (handlers.rs:2270-2347). A client disconnect therefore does not release this
seed wait or session claim. This is a narrowly established stuck-session-Restart consequence, not a permanent whole-GUI
hang, unbounded memory growth, security or user-work-loss claim. The input/paste consequences and g130/f3 alias remain:
a retained paste owns its budget, a second budget waiter holds Inner, and prompt delivery is outer-timeout bounded
(core.rs:2306-2345), grid input cancellable (grid_stream.rs:1207-1281). Shutdown's analogous control reply is bounded in
the shipped host-ending caller by 15 seconds and falls back to verified termination (host_launch.rs:214,675-712);
checkpoint_now has its own timeout (terminal.rs:2404-2422). Suggested bucket revised to high for the confirmed rare
accepted-Restart/session-lifecycle stall; input-only retained-request consequence remains other.

Possible coverage and caveats: existing f2 filter ambiguity for input timeout alone no longer covers this stuck session
lifecycle; FILTER partial-operation acceptance excludes loss of ordinary Stop/Restart/Delete usability, and
rare-safe-failure acceptance excludes state needing restart/manual repair. Focused required coverage searches for seed
reply/timeout, restart hang and control-channel reply found no exact specification, Planned item, unlanded plan, BUGS,
queue or ledger acceptance. SPEC_impl.md Restarting a host agent and Who owns an accepted action govern history
retention and supervisor ownership, not accepting lost local control replies.

Additional reviewed locations: g130/f3:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/terminal.rs:1544-1559,1642-1660,4166-4169.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g126/f2`, `g130/f3`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
bd4d00ab1767cdccbd91c58bdf968928eaeec5a3; persisted/runtime-control extension inspected at
ca9b0a19856dc136f63ad2d14f1076027edce1eb. Same retired-runtime bounded-mpsc reserved-send root as f2, not a new local
ID.. This was a source-only review; runtime occurrence and frequency were not measured.
