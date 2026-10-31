# Physical Tab can cross an unresolved readiness boundary

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A physical Tab press might be handled after catalog readiness changes, rather than under the advisory state present when
it was pressed.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Actual Dioxus/Wry/input and synchronous XHR paths were checked, refuting the simple queued-replay explanation. The
missing premise is native physical-input delivery relative to handler readiness, together with the intended
keypress-versus-handler-time contract. No native input trace or runtime experiment was produced.

Source locations:
`crates/farhelm-ui/src/list/view.rs:4009-4015,4023-4030; crates/farhelm-ui/src/launch_sequence.rs:674-680; crates/farhelm-ui/assets/template-shortcuts.js:269-277`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d, separate discovery pass. Source basis: Tab's relay payload has no
readiness/catalog snapshot; the Rust input callback reads the current catalog, independently populated by prefix fetch
completion. Suspected consequence: Tab pressed before readiness could complete after readiness and create a different
queue/word boundary; subsequent text could be a new entry rather than an extension. No actual wrong launch, security or
user-work-loss consequence established. Actual checks: source-only trace of fetch_templates (:4219-4237), prefix spawn
and synchronous input (:4009-4030), SequenceState::Tab (:674-680), JS relay, Rust no-replay test (:866-901) and
held-web-catalog browser test (:317-365). Existing tests keep the read held until after Tab has been handled, so do not
answer cross-bridge ordering. Locked dioxus-interpreter-js 0.7.10 native.ts:353-410,561-591 uses a synchronous XHR, not
a queued asynchronous IPC input; dioxus-desktop webview.rs:43-100,102-195 immediately invokes the Rust event handler,
with no await inside it. This refutes the initial simple queued-event/replay explanation. Native host polling is
separately driven by app.rs:413-425, launch.rs:25-41 and webview.rs:537-600; Wry's custom-protocol callbacks are in
wkwebview/class/url_scheme_handler.rs:48-112,276-330 and webkitgtk/web_context.rs:126-152,234-253. Exact missing
premise: a supported native engine must deliver a physical Tab's synchronous request to Rust only after the independent
catalog task has been polled, although the physical Tab preceded that readiness. Those source callbacks do not specify
the OS/WebKit delivery order versus the Rust wake event, so no actual compatible schedule has been established. A second
interpretation question is whether SPEC.md:918-919's pending no-op binds the physical page event or the synchronous Rust
handler; SPEC_impl.md:6128-6147 deliberately locates word/completion ownership in Rust, while the browser test's :317
wording says catalog available at keypress. Tab is not a destructive confirmation, and no universal
render-bound-completion requirement is asserted from SPEC_impl.md:963. Available source cannot establish closed native
engine scheduling or choose that contract interpretation. Resolving evidence: a native trace showing physical Tab,
catalog publication and Rust handling in that order, plus an observable unexpected completion/word boundary; or a
specified event-order guarantee ruling that order out. No native or runtime experiment was authorized or run. Source
provenance: dioxus-desktop, dioxus-interpreter-js and Wry archive checksums match pinned Cargo.lock; native.ts,
webview.rs, protocol.rs, app.rs, launch.rs and both Wry callback files individually match those archives. Coverage
searches of SPEC.md, SPEC_impl.md, empty TODO Planned, plans/queue, BUGS.md, index and ledger found no exact acceptance
or existing item. proposed_drop none pending parent assessment of this narrow uncertainty. possible_cover: handler-time
semantics of the Rust-owned synchronous completion contract; no filter is asserted. Suggested bucket other by
established scope, confidence unverified after investigation with the precise scheduling and contract premises above.
This is not a confirmed shipped defect.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g153/f6`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
