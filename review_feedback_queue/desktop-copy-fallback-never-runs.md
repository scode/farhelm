# Header copy fallback never runs on desktop

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On desktop, copying the directory or command from the session header can silently fail while the button says "copied".

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F20 / COR-NATIVE-COPY-FALLBACK-DEAD`, tagged **definite**. Anchors and title:
`crates/farhelm-ui/src/session_view.rs:1508`, `crates/farhelm-ui/src/auth.rs:297` — The header copy buttons'
browser-clipboard fallback can never run on desktop

The header's directory and command buttons copy their value with a small script (`copy_value`, line 1508). It calls
`window.__farhelmNativeClipboardWrite(v)` if that function exists, and falls back to `navigator.clipboard.writeText`
only if the native call throws or returns a promise that rejects. SPEC's session-header bullet specifies the same order:
"the native bridge first and `navigator.clipboard` second".

On desktop, `window.__farhelmNativeClipboardWrite` is installed by `arm_native_clipboard_script` (`auth.rs:297`). It
POSTs to the embedded helm's `/api/clipboard`, attaches `.catch(function () {})` to the fetch, wraps the whole thing in
`try {} catch {}`, and returns `undefined`. It never throws, never returns a promise, and `fetch` does not reject on an
HTTP error status in any case. The fallback can therefore never run: a failed native write (a 401 during
re-authentication, a sink error) is simply lost, and the button still shows "✓ copied". The fallback only matters on
Linux desktop, where webkit2gtk exposes `navigator.clipboard`; on macOS the WKWebView page has no `navigator.clipboard`,
so a fallback could not help there.

The suggested fix is to have the installed writer return a promise that rejects on `!response.ok` and on a fetch
failure.

Restater note: SPEC also says clipboard writes have "silent failures", so showing "✓ copied" after a failure is within
spec. The defect is only that the specified second attempt never happens.
