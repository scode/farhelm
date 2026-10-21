# A delayed refusal can reopen a cancelled Replace question

Reviewed commit: 273445e48a0efc18cd145213ff5a0d9b67e2028e

## TLDR

With a header Replace YOLO question open, trying Replace in the sidebar and cancelling the header question while that
request waits can produce another actionable question after cancellation. Another affirmative click is still required.

## Details

`crates/farhelm-ui/src/session_view.rs:2755–2758` makes the header question busy only while its own `replacing()` flag
is set. That flag is false after its first refusal finishes, and a sidebar request does not set it. Header Cancel
therefore remains enabled while the duplicate sidebar request waits. Cancellation releases the source's shared question
reservation and may retire its key (`2805–2817`). The delayed sidebar refusal can then acquire the empty reservation and
reopen a question (`crates/farhelm-ui/src/list/view.rs:2318–2328,2450–2456`) without checking cancellation freshness or
whether its source/key still owns the intent.

This is the header-first direction only. A sidebar question observes the shared page lock (`view.rs:4303–4304`) and
disables Cancel while busy (`yolo_confirm.rs:287–292`), blocking the reverse ordinary-click sequence. The production
Replace path can wait for a live supervisor read before refusing (`crates/farhelm-helm/src/sessions.rs:3547–3560`), so
the first sequence's human-action window need not be subsecond.

Fence stale refusals with cancellation freshness or coalesce an existing same-source question before duplicate dispatch.
A regression can hold the sidebar refusal, cancel the header question, release the refusal, and require that no question
reappears. The added fixture releases both refusals before cancellation and therefore misses this boundary.

Suggested bucket: **other**, for cancellation/confirmation state. Confidence: **likely**; the static state sequence and
busy rules support it, but it was not exercised. No replacement or host-setting change follows without a fresh
affirmative answer; no security or user-work-loss consequence was established.

Possible cover: the completed ledger entry `replace-yolo-questions-remain-separate.md` at `16898–16913` authorizes
mutual exclusion but does not expressly accept delayed resurrection. `SPEC.md:849–855` requires shared pending consent
and ends plain Replace identity on cancellation. The ledger's queued-affirmative-answer decision at `6821–6855` has a
different trigger. The future controller refactor is outside TODO's Planned bucket. No exact BUGS, current queue or
filter match was established; the delayed production read prevents assuming a subsecond action window. Proposed drop:
none; this is a residual of the completed class, not a claim that its original decision accepts it.

Provenance: trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`; the Replace reviewer corrected its
first-pass cancellation conclusion during required synthesis, then narrowed the finding to header-first cancellation. No
runtime reproduction was performed.

Original finding IDs: `replace-consent-whole-1/f1`.
