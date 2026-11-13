# Old host-dialog release may affect a later desktop opening

Reviewed commit: a03f32543bdaed2cbfea9cf33d43335611e071ff

## TLDR

Desktop cleanup after removing one host might remove a later Remove dialog's modal protection. The required native event
ordering remains unverified. No unintended operation or user-work loss is established.

## Details

Suggested bucket: **other**, for conditional modal correctness. Confidence: **unverified after investigation**.

`crates/farhelm-ui/src/hosts.rs:1925–1929` retires the static, unversioned `REMOVE_DIALOG` after the request clears its
busy state and page claim (`1970–1981`). Another Remove can then be admitted (`2096–2119`). The nested task performs two
releases: one inside `close_and_wait`, followed after its reply by `close_immediately_if_document_focused`
(`src/modal_dialog.rs:178–230`). The second lookup has no opening identity and releases isolation before its document
focus check. `src/modal_isolation.rs:149–153,223–232` removes the root's inert ownership and keyboard listener. The
mechanism remains at tip `9b19f8d754bf7658761ef246cfda0ea8397f4f21`.

The installed version-matched Dioxus 0.7.10 sources establish queued, initially unpolled tasks and same-scope task
priority before rerender, without a guarantee preceding ancestor-driven child renders. Browser eval and rendering are
synchronous, so the ordinary web next-person-event route is no longer alleged. Desktop eval instead uses native Wry
evaluation, while DOM edits arrive through a WebSocket, animation-frame application and acknowledgement. Query replies
return separately through IPC. Wry 0.53.5 forwards to WKWebView/WebKitGTK without ordering those paths together.

The exact missing premise is a supported desktop schedule mounting and installing a replacement Remove root between the
old native evaluation/query-return path and its second release. Version-matched task, scheduler, component-diff, desktop
query/edit/event and Wry implementations were inspected; they do not settle that platform ordering. A native ordering
guarantee or an ordered trace would settle it. No browser/native runtime experiment ran. This does not allege a retained
old Rust click callback, which the separate framework investigation refuted.

Possible cover: the rare-glitch or subsecond-action filters could apply, but their timing and complete-consequence
premises remain unestablished. SPEC_impl.md:670–680 requires guarded asynchronous retirement. No exact Planned,
unlanded-plan, BUGS, queue or ledger coverage was found. Proposed drop: none. If reachability is established, bind
retirement to its opening or prevent reopening until cleanup finishes.

Provenance: trailing-review run `bfd30ff4-8bd3-46a2-9101-56314a921992`, `g010/g010-f2`, found during synthesis and
narrowed through targeted version-matched source clarification.

## Additional Suspend dismissal producer at 6e423e3

Additional reviewed commit: `6e423e30068b62c6cfa276c6d720ee35bb70387a`.

Current-main recheck: `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`. Extend the existing Remove item, other and unverified
after investigation. The correctly token-consumed single Suspend close/release still uses unversioned handles at
`hosts.rs:3567–3570,3585–3588,3614–3617,3630`. `modal_dialog.rs:94–98,121–148` still resolves whichever selector root
exists when release runs. Preserve Remove's separate nested second release and its distinct native ordering. No exact
current coverage or later fix was found, and no new item or harmful native action is established.

Reviewed source change: `fa518784fc3773bfdc5fc5cdf01269d3858f6b78`. This extends the existing unversioned host-dialog
release concern; it does not replace or establish the earlier Remove route.

A correctly accepted Suspend dismissal could release a later Suspend opening's modal protection if its native bridge
script runs only after that later opening has installed isolation. This single-release ordering remains **unverified
after investigation**. Bucket stays **other**, matching the reviewer; no unintended suspension, security consequence or
user-work loss is established.

At this reviewed tip, `crates/farhelm-ui/src/hosts.rs:3431-3434,3449-3452,3478-3481` correctly consumes the drawn
Cancel, Close or Escape token, then calls `DialogHandle::new(".host-suspend-dialog").close(...)`. The confirming button
at 3491-3493 likewise consumes its answer before releasing the same unversioned handle. This is not a
stale-answer-listener claim. `modal_dialog.rs:94-98` sets no generation; 121-134 leaves the release unbound; 144-148
evaluates release and schedules focus return. `modal_isolation.rs:151-155` resolves whichever root currently matches the
selector, and 225-235 removes that root's inert ownership and keyboard listener.

This is the same missing opening fence as the existing Remove report, with another producer. Remove's original concern
needs a replacement root between its nested `close_and_wait` and second release. Suspend has one accepted answer's
close/release and instead needs that single native evaluation to resolve a newer, already-isolated root. A fence on the
resolved opening at the shared release boundary addresses both; binding the Rust answer token alone does not. Preserve
the earlier Remove source revisions, investigation and native-ordering qualification separately.

The reviewer inspected locked Dioxus 0.7.10 and Wry 0.53.5. Browser `WebEvaluator::create` executes JavaScript
synchronously, refuting a generic lazy-eval browser inversion. Native `DesktopEvaluator::create` immediately submits
`QueryEngine::new_query` (`query.rs:45-112`); prior DOM-edit acknowledgment gates rendering (`webview.rs:537-589`),
while edits use a separately acknowledged WebSocket (`edits.rs:41-76`). Wry's WKWebView and WebKitGTK implementations
(`wkwebview/mod.rs:669-731`, `webkitgtk/mod.rs:686-721`) submit asynchronous native evaluation. These sources do not
demonstrate the old single release overtaking later opening/install work. With ordinary FIFO evaluation the release
precedes installation and this route does not occur; an intact newer modal still blocks input behind it.

The missing premise is a supported desktop execution order where the old release resolves the newer isolated
`.host-suspend-dialog` root. An ordering guarantee or controlled trace identifying release and replacement roots would
settle it. No runtime experiment ran, and the accessible source did not resolve native evaluation versus DOM-delivery
ordering. Do not present new-dialog exposure or focus theft as observed or source-confirmed consequences.

`SPEC.md:3276-3278` forbids an owner closing another surface; `SPEC_impl.md:820-826` requires isolation to last until
its dialog closes. Its rendered-token contract at 1067-1088 does not excuse delayed DOM release aimed at another
opening. No exact unlanded-plan repair, Planned TODO, current ledger acceptance or BUGS entry was found. Recording-time
main `cb03def00870e18a98a1fd9046073d2849be3253` removes the broad focus TODO and abandons yolo-focus-origin in
anticipation of a GUI rewrite. That earlier plan excluded Hosts Suspend and addressed question focus admission, not
delayed DOM release. Existing rare-glitch/subsecond filter caveats remain unresolved rather than newly accepted.

Provenance: run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, canonical `suspend-close-later-modal`; `g001/g001-l6`, event23
and its lead history in `prep/groups/g001/findings.jsonl`. The existing Remove provenance stays unchanged.
