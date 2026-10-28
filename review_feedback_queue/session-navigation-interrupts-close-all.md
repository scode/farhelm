# Session navigation interrupts Close all

Reviewed commit: 4a99d51e560d873d8b2a8d25999926b2083507c5

## TLDR

Switching sessions while Close all waits for a response leaves later tunnel listeners open without attempting their
closure or reporting that the batch was interrupted.

## Details

Suggested bucket: **other**. Confidence: **confirmed conditional source mechanism**; native timing and frequency were
not measured. Continued listener exposure is established, but an additional security consequence is not. A concrete
harmful exposure route would warrant reassessing the bucket.

`crates/farhelm-ui/src/tunnels.rs:632–648,791–841` runs the captured listener IDs serially in a component-scoped task.
The Dioxus core 0.7.10 contract at `src/global_context.rs:115–139` cancels such tasks on component unmount. Selection
replaces the current session and unmounts its keyed view (`crates/farhelm-ui/src/lib.rs:1901–1904,2002–2003`). The
tunnel owner is a child of that view (`session_view.rs:74–89,2105–2107`).

The normal row-selection guard checks shared operations and pending row work (`list/view.rs:2987–3005,3417–3418`),
whereas Close all owns only its private `closing` signal. It therefore does not prevent this navigation. Later requests
are never issued; the helm close endpoint closes only the named listener (`crates/farhelm-helm/src/tunnels.rs:527–559`).
Reopening the view lists the remaining listeners but does not resume the batch. No transport failure is needed.

`SPEC.md:2091–2095` and `SPEC_impl.md:615–621` require attempts of the activated snapshot, continuing after failures.
The helper tests at `tunnels.rs:956–1017` do not exercise view teardown. A focused regression could hold the first close
response, switch sessions, release the response and verify every captured ID was attempted. Admitted batch work needs an
owner that outlives selection, including its admission and error reporting; retaining a task alone while it still uses
dropped component signals is insufficient.

Possible cover: the timing filters need a brief, harmless trigger that has not been established. Remaining listeners
persist until another close, revocation, deletion or quit. `removed-host-strands-tunnel-listeners.md` has a different
trigger. The infallible GUI–helm transport decision (`TRIAGE_OUTCOMES.md:18918–18944`) does not cover requests never
issued because their UI task was cancelled. No matching Planned item, unlanded plan, BUGS entry or ledger decision was
found. Introduced here and unchanged through `d64ae3a81547729100529d210d3ea50434ff933d`. Proposed drop: none.

Provenance: trailing-review run `6b8af8f0-1a32-459b-89e9-6cea539af957`, `tunnel-pill/f1`. Source review only.
