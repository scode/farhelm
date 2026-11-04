# Opening history can replace a waiting announcement batch

Reviewed commit: 921feddad94cd0161f41b4c0ee0778120813115a

## TLDR

Choosing shown announcement history from an already-open Help menu before the automatic dialog is admitted can replace
this client's pending batch. Those notices are not automatically offered again during that authenticated page lifetime.
Their durable queue rows remain pending; a later load offers another recovery opportunity, not a guaranteed showing.

## Details

Suggested bucket: `other`, for suppressed notice visibility with durable queue state preserved. Confidence: confirmed
source-level overwrite, persistence and admission-bound contracts. `could_be_highest: false`; no destructive consequence
is established. `proposed_drop: null`.

At the reviewed commit, unchanged through `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`,
`crates/farhelm-ui/src/app_bar.rs:735–742` performs the only once-per-load announcement read.
`crates/farhelm-ui/src/announcements.rs:33–41` publishes pending announcements and shown history together. An
already-open Help menu can then expose its history item through `app_bar.rs:899,1031–1052`.

`crates/farhelm-ui/src/modal_isolation.rs:85–117` initially renders a placeholder, installs its mounted handler, spawns
an evaluation, awaits the response and only then renders the child that isolates the rest of the page. Choosing history
before that child renders overwrites `displayed` with history and sets `from_history=true` at `app_bar.rs:1048–1052`,
replacing the client's only pending-batch vector. Closing history takes the displayed vector but skips the shown POST
(`:745–784`). There is no independent preserved pending vector or another automatic read on this mounted page.

`SPEC.md`, “Announcements,” `:3394–3426`, requires offering the pending batch and treats history as a separate read-only
presentation. This replacement loses that page's pending offer even though storage behaves correctly.

The admission interval is not proven brief. Neither this HostModal await nor its pinned Dioxus evaluator imposes a
finite timeout. Cargo.lock pins dioxus-document/web/desktop to `0.7.10` at `:1400–1403,1668–1671,1314–1317`. Matching
dependency references are `dioxus-document/src/eval.rs:20–27`, `dioxus-web/src/document.rs:188–264`,
`dioxus-desktop/src/document.rs:88–110`, and desktop `src/query.rs:43–115,197–209`. Farhelm's
`webview_watchdog.rs:60–67,167–182,203–234` times its own probes and logs; it does not bound this evaluation or force a
reload.

While another modal actually holds admission, Help is inert, so that wait alone is not an actionable history-selection
window. The relevant window is before the announcement child renders and isolates Help, including its evaluation/IPC
boundary. Its actual duration is unmeasured; neither an indefinitely usable Help menu nor a short fixed interval is
asserted.

The queue is not falsely marked shown. `crates/farhelm-helm/src/announcements.rs:119–140` reads pending rows with
`shown_at IS NULL`, and only `:143–168` updates explicitly posted ids. History close sends no POST. Idempotent producer
enqueue (`announcements.rs:83–116`; `release_announcements.rs:28–85`) neither deletes nor marks the omitted rows. A
later authenticated load with a successful GET retrieves still-pending rows unless another client has dismissed them.
Actual presentation also depends on Welcome/feedback completion, modal admission and avoiding the same overwrite again.
Reload is therefore a recovery opportunity, not an unconditional guarantee that notices are shown.

Preserve pending state independently from history, or queue/refuse history opening until the pending batch is handled. A
focused regression can hold modal admission and the queue response, return nonempty history and pending to an open Help
menu, choose history and check that pending is subsequently offered without a reload.

Possible cover: no exact matching acceptance, planned repair, known bug, queue item or ledger decision was found in the
required recording sources at the review tip or recording-time main. The cited related concerns have distinct triggers
or consequences unless explicitly grouped here. No full FILTER.md match was established; its sub-second race filter does
not establish a bounded modal-admission interval.

Grouped findings: `g037-l4` and `g036-l8` identify this same source race. `g036-l5`/`g037-l2` is a distinct stale-answer
delivery concern; its unresolved old-event premise is not needed for this pre-admission overwrite.

Caveats: the compiled release-announcement table initially ships empty, so impact requires pending entries and nonempty
shown history. No runtime reproduction or measured timing is claimed. The dependency references establish the
evaluator's matching-version behavior. No corruption, permanent durable deletion or user-work/security consequence is
established.

Review provenance: automated committed-code trailing review, run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`; `g037/g037-l4`,
latest findings line 12, preserves the dependency investigation and conditional reload recovery; `g036/g036-l8`, latest
line 20, corroborates the route and explicitly adopts those bounds without claiming a measured interval.
