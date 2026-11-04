# Idle readoption task remains allocated after its in-process supervisor is dropped

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Dropping an idle in-process supervisor without running its orderly-stop method leaves its background retry task and
notification state allocated until the caller's Tokio runtime ends.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
static source behavior; practical memory impact unmeasured and not needed for the narrow task-lifetime claim.

Source locations:
a4345e86a3f51f4c5abf3b2e97737991df900142:crates/farhelm-supervisor/src/service/core/host_handover.rs:857-879;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core/host_handover.rs:1280-1303.

A host_handover.rs:857-879 spawns the task without retaining a JoinHandle/abort guard. Once readopt_pass returns None,
it waits solely on wake.notified(), and only afterward attempts weak.upgrade(). The task itself retains an Arc to the
Notify, so dropping Supervisor does not close or notify that wait. Source search of A core.rs found no Drop for
Supervisor and only creation/serve notification sites for readoption_wake; T source search likewise found constructor,
serve and orderly-stop notifications, without a drop-time wake or cancellation. The T code directly checked at
:1280-1303 still waits in the same order. The constructor starts it for every owning supervisor (T core.rs:6553-6561).
An orderly stop does wake it (A host_handover.rs:206-208), and the production run owns and drops its entire Tokio
runtime (A core.rs:17989-17998), so those paths do reclaim it. The exposed in-process construction/drop path remains
under focused caller investigation; the narrow suspended-task lifetime is established even if practical impact is small.

Dropping an idle in-process supervisor without running its orderly-stop method leaves its background retry task and
notification state allocated until the caller's Tokio runtime ends. Repeated construction/drop in a longer-lived
embedding runtime retains one such suspended task per supervisor. No end-user outage or material memory exhaustion is
claimed. Details/source basis: A host_handover.rs:857-879 spawns readoption without retaining an abort owner. Once
readopt_pass returns None (:939), the next loop waits solely on wake.notified() (:870) before it checks weak.upgrade()
(:872-874). The task itself holds an Arc to the Notify (:859), so dropping Supervisor neither closes that wait nor
causes a wake. A whole-module Drop/readoption_wake search and the T service-wide counterpart found no Supervisor Drop or
drop-time cancellation. T host_handover.rs:1280-1303 retains that exact ordering and :1319-1343 still returns no due
work when the map has no placeholders. T core.rs:6553-6561 starts the task for each owning constructor. This lifecycle
is exposed rather than purely hypothetical private access: A core.rs:5970-6006 explicitly provides public constructors
for tests and embedders on the caller's existing runtime. A quiet supervisor with no placeholder establishes next=None
without any host failure. Normal production shutdown is narrower and safe from indefinite retention: A
host_handover.rs:206-208 sets stopping and wakes readoption; A core.rs:18071-18093 runs it during orderly shutdown, and
:17989-17998 drops the process-owned runtime on run exit. Thus report the defect only for in-process drop without
orderly stop, not as a production process leak, live host leak, file-lock leak, polling loop or CPU spin. A host
handle/runtime is a different ownership path: hosts/terminal.rs:1119-1134,1644-1660 drops channel senders with its last
handle, and :2499-2520 stops on channel closure; it does not refute this Notify-only task.

Possible coverage and caveats: targeted coverage searches included SPEC.md, SPEC_impl.md, empty TODO Planned (:23-25),
unlanded plans/queue, BUGS.md, queue INDEX and TRIAGE_OUTCOMES.md. No matching trigger/consequence/scope found. T
SPEC_impl.md:2380-2392 fully describes retry and orderly-stop behavior but does not accept retaining an idle task after
its supervisor is dropped. TODO.md:70-102's Near-term mid-life retry work concerns which host terminals to retry, not
the background task's drop ownership. No FILTER.md filter matches ordinary in-process drop retaining a task; no highest
consequence is asserted.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g151/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is a4345e86a3f51f4c5abf3b2e97737991df900142 (A);
persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This was a source-only review; runtime occurrence and
frequency were not measured.
