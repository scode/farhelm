# Header YOLO answer may cross question openings

Reviewed commit: c1aeb38015b00a8dccc3bfef853f240e512f2e83

## TLDR

An old header confirmation would answer a newer question if its click could reach the handler after reopening. The
controller permits that substitution, but a complete shipped input sequence remains unestablished.

## Details

Suggested bucket: provisionally **highest**, for the unresolved authorization consequence. Confidence: **confirmed local
mechanism; actionable event reachability unestablished**. This is a conditional lead, not a reproduced stale
confirmation or permission bypass.

`crates/farhelm-ui/src/replace.rs:309–326` reads the live question rather than an opening captured by the answer;
`session_view.rs:2666–2690` supplies the header callbacks. The controller test at `replace.rs:796–813` directly invokes
the controller and does not establish UI delivery across Cancel, another admitted request and a later refusal.

Matching checksum-verified Dioxus 0.7.10 source narrows the gap. Browser handlers run synchronously
(`dioxus-web/src/dom.rs:88–110`); desktop event transport uses synchronous XHR and listener dispatch
(`dioxus-interpreter-js/src/ts/native.ts:353–407,566–590`; `dioxus-desktop/src/webview.rs:190–194`). No asynchronously
retained old renderer callback was found. However, cancellation does not prove an empty question renders before a ready
same-scope task reopens it (`dioxus-core/src/scheduler.rs:179–222`). That scheduling fact alone proves neither delivery
of an older trusted answer nor passage through the busy-button constraints.

Missing source premise: a normal input route completing that entire sequence. The focused investigation is stuck on that
premise, separately from runtime frequency. Independent auditor `g125` could not approve dropping the broad concern, so
retain it and keep the affected review incomplete. Possible cover: `SPEC.md:3273–3276` accepts some subsecond input, but
does not clearly accept answering a different opening. No exact other coverage was found. Proposed drop: none for the
conditional lead.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g044/f1`, independently investigated by `g125`.
Neighboring sidebar opening guards and preserved deletion consent were separately source-refuted, without settling this
header route.
