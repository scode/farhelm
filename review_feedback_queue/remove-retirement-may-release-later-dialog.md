# Old Remove retirement may release a later desktop dialog

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
