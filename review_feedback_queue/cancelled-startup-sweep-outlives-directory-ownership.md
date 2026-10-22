# Blocking cleanup outlives supervisor ownership

Reviewed commit: 2ea2826caa0eec867c4e2bddc01badbac7bbe6c7

## TLDR

Stopping a supervisor can leave old filesystem cleanup running after a successor accepts new launches. A stale startup
sweep can delete a newly claimed checkout; a delayed failed-create unlink can delete a successor retry's launch input.
Both share an ownership-lifetime gap. The conditional source sequences are established; resulting launch failures have
not been reproduced.

## Details

Suggested bucket: **high**, for interruption of a newly accepted preparation. Confidence: **confirmed conditional source
sequence**. Overlap frequency and resulting preparation failures are unmeasured. No loss of existing user-owned work,
security consequence, database corruption or bricked installation is established.

`crates/farhelm-supervisor/src/service/core.rs:7079–7087` starts blocking cache/spare cleanup with a frozen live-claim
sample. The closure captures the state path and sample, without retaining `StateDirOwnership`. A started Tokio
`spawn_blocking` task survives cancellation of its waiter; the locked dependency is Tokio 1.53.2, whose
`src/task/blocking.rs:106–120` documents this behavior.

Normal stop cancels serving and returns after output-client shutdown (`service/core.rs:15898–15915`). Before the ticker
and builder start (`7116–7117`), that startup sweep has no serving task holding another supervisor reference. Ownership
can therefore drop: its lock lasts only for the ownership value, and the process registry holds weak references
(`1189–1204,4497–4501`). Runtime shutdown waiting for the old blocking thread does not keep that separate file lock.

A successor can claim the directory, skip any repository lock the old sweep currently holds (`repo_cache.rs:80–82`),
finish startup and accept a spare take. If the old worker was first cleaning another repository, its later visit can
encounter that new claim. The new generation is absent from its old live-claim sample, so `spare_checkouts.rs:541–554`
can discard it. This needs neither two simultaneous owners of one cache lock nor replacement of a namespace.

### Failed-create unlink after runtime shutdown

The same ownership gap also affects a path-only filesystem worker during failed-create cleanup. At
`7fe7ad3c36414cd31b84fd0970237602e72f3bdc`, `service/launch_artifacts.rs:320–325,525–529` awaits
`tokio::fs::remove_file` before the failed create abandons and settles its Pending row (`service/core.rs:10271–10339`).
Tokio 1.53.2 moves only an owned pathname into blocking work (`src/fs/remove_file.rs:13–16`; `src/fs/mod.rs:312–325`).
Its runtime drops yielded asynchronous tasks on shutdown while already-started blocking work finishes separately
(`src/runtime/runtime.rs:35–44,500–517`). The CLI drops its temporary runtime after supervisor run returns
(`crates/farhelm/src/main.rs:1440,2056–2057`).

If the predecessor's already-started unlink is descheduled before filesystem removal, normal shutdown can release
supervisor ownership while that path-only worker remains. A successor can reload the still-Pending row and retake it in
the same generation. Its own fail-closed clearing removes the old spec before publishing a new one at the same path
(`service/core.rs:9633–9677,10138–10175,14468`). The predecessor unlink can then remove the new spec. The shim either
cannot read it or fails its consume unlink and returns failure before agent execution (`launch.rs:1042–1064`). No
filesystem I/O failure or crash is required. The relevant files remain unchanged at the reviewed tip.

This is additional evidence for the same ownership-lifetime defect, rather than a second queue item: supervisor
authority must outlive already-started filesystem work before any successor can mutate the state. Merely adding a guard
to the startup sweep would leave the runtime unlink example open. No credential disclosure or existing-user-work loss
was established for this example either.

Possible cover: `SPEC.md:1331–1343` accepts exact-point crashes and coincident independent faults. This schedule uses a
normal startup stop and successor launch, with no identified crash or second fault. `TRIAGE_OUTCOMES.md:16045–16071`
addresses different archive-journal damage and explicitly leaves general startup ownership outside that fix.
`SPEC_impl.md:55–56` protects the helm runtime's ownership, not this supervisor closure. No exact Planned, BUGS, queue
or FILTER coverage was found. Proposed drop: none. Retain ownership through the blocking worker's actual completion, or
establish an equally strong boundary before a successor can act.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g047/discovery-f2` and
`g058/f-late-unlink-after-runtime-shutdown`; both source sequences checked by the parent. The latter extends the same
record and is not counted again. No reproduction was performed or is required to retain this conditional finding.
