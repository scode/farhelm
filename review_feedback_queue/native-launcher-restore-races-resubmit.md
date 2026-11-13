# Native launcher restoration may race a later resubmit

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

Unverified after investigation: an old restoration frame could make a resubmitted, hidden desktop launcher block page
input again. The required native evaluation/render ordering has not been established; no generic web failure is claimed.

## Details

Suggested and recorder bucket: **other**. Confidence: **unverified after investigation**. A proved sequence causing
persistent page blockage could justify high; that consequence is not established here. Reviewed change:
`112fd9d812e95df7f69f66c85820d76faaf89582`; references below use the reviewed tip.

`crates/farhelm-ui/src/list/create_form.rs:2055-2072` queues restoration in `requestAnimationFrame`, guarded by a
connected root, the same opening generation and `aria-modal=true`. A later same-opening submit immediately releases
isolation (`1914-1918`) but changes that ARIA marker through a rendered `submitted_wait` update (`7521-7524`). Isolation
release clears its observers, listener and inert ownership without clearing the marker (`modal_isolation.rs:225-234`);
install has no submitted-state guard (`168-170`). The hidden-presentation effect only blurs
(`create_form.rs:5447-5472`).

Matching Dioxus 0.7.10 source establishes different native transports: eval uses `WebView::evaluate_script`, while DOM
edits use the websocket edit queue and acknowledgement. It does not establish a supported platform order putting an
accepted resubmit's release before the old restoration frame, then the nonmodal DOM edit, with no subsequent release.
That is the exact missing premise. Matching web source evaluates and flushes synchronously, refuting the generic web
physical-event gap. The native split alone is not proof of harmful scheduling.

Actual dependency checks used locked Dioxus 0.7.10: desktop `document.rs:26-33,89-94`, `query.rs:45-112`,
`webview.rs:537-591` and `edits.rs:45-74`; web `document.rs:205-251` and `lib.rs:178-264`. The edit-flush
acknowledgement fences later VDOM work, not an already queued frame. These results narrow the missing premise rather
than establish the platform order.

SPEC.md:1322-1335,3257-3260 and SPEC_impl.md:209-214 require the hidden submitted launcher to release page isolation.
The old Remove-retirement queue item concerns releasing a replacement owner's isolation, and moved-on failure return
concerns a different admission transition. No exact acceptance, Planned item, pending repair, BUGS, queue,
current-ledger or clear filter cover was established. The earlier YOLO-focus proposal was abandoned; it did not address
this restoration guard, and its abandonment establishes neither repair nor blanket acceptance.

A supported native trace of input, eval, frame execution and DOM-edit application, or a cross-transport ordering
guarantee, would settle it. Accessible matching transport source cannot resolve the platform event order; repeating
those reads would not prove it. No native/browser experiment ran. Keep the source gap and practical consequence
separate.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g018/d3`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Proposed drop: none for the investigated uncertainty.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): create_form.rs:2175-2194 still admits its
restoration frame using a connected root, the same opening and aria-modal=true. release_submitted_composer at :2034-2038
releases isolation without synchronously clearing that marker; the hidden transition at :5615-5625 still only blurs
before restoration, and aria_modal is rendered at :7738. The modal_isolation.rs change removes an unrelated HostModal
activity gate. The supported native release → old frame → nonmodal edits ordering remains unverified after
investigation; no generic web bug is established.
