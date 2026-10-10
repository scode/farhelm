# Mouse handlers surviving disposal during a drag

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Disposal during a lost mouse release could leave document handlers attached.

## Details

F303 — **possible** — `crates/farhelm-ui/assets/vendor/xterm.js:1, bindMouse, readable 438–459` — Mouse handlers
surviving disposal during a drag

Drag handlers are added directly to the document and removed on button release or protocol changes, without an inspected
disposal owner. If release is lost and the terminal is disposed, those references could remain even though socket
closure limits stale input delivery. Establish disposal cleanup for the document listeners as well as ordinary release
cleanup; broader availability or work-loss consequences are not established.

## Evidence and triage context

- xterm.js:1, readable 459 directly adds document mouseup/mousemove listeners. Readable 445 removes them on button
  release and 454–457 on protocol changes, not on an explicit disposal guard. terminal.js:5609–5621 closes the socket
  and disposes the terminal, which limits stale input delivery but does not prove those document references are removed.
  No exact coverage basis was found.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_cor:p1:C5`.

- `vendor_01_cor:p1:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
