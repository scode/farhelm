# Trash content can become unreachable by keyboard

Reviewed commit: 7891be3cbf4024dcf39550f30785d637c5e4bb28

## TLDR

Keyboard-only users can lose access to offscreen host information when all trash actions are disabled.

## Details

Suggested bucket: `other`. Confidence: likely; the DOM/CSS mechanism is confirmed, but the exact shipped WebKit version
was not exercised. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

crates/farhelm-ui/src/list/trash.rs:517 introduces a scrolling body without tabindex; app.css:7028,7058–7061 moves
scrolling from the dialog into that body. When hosts are unreachable or their listings fail, the body's buttons are
disabled (trash.rs:548,563–564). Close remains focused outside the scroll region (507,514), leaving no keyboard focus
target inside it on browsers without automatically focusable scrollers. Before this commit, Close was inside the
scrolling dialog. Add a keyboard-focusable, appropriately labelled body and exercise the all-disabled case. The new test
uses script-driven scrolling (trash-dialog.spec.ts:101), so it cannot detect this failure. Browser behavior is supported
by the [WebKit issue](https://bugs.webkit.org/show_bug.cgi?id=277290) and
[W3C scrolling rule](https://www.w3.org/WAI/standards-guidelines/act/rules/0ssw9k/).

Possible cover: None found. The pinned trash-listing-exceeds-control-frame.md concerns unavailable listing/deletion, not
keyboard access to rendered content.

Status at frozen endpoint: Persists structurally at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: trash.rs:584 still lacks
tabindex; app.css:7252–7255 retains the nested scroller. Runtime verification remains outstanding.

Review coverage: full commit diff read: true. No reviewed-commit diff content unread. An aggregated tool response
truncated historical trash.rs context; missing lines were reopened in full at 390–589. The 13,089-line triage ledger was
searched rather than read exhaustively; unrelated ledger bodies and queue finding files were not read. Backend deletion
implementation was not rereviewed because this commit leaves target construction and dispatch unchanged. Current-main
test additions beyond the inspected excerpt were outside scope. No working-copy queue finding contents were read.

Runtime evidence: None, as requested by default. No checkout files were created, modified, or deleted. Browser-specific
keyboard behavior and extreme viewport geometry remain runtime caveats.
