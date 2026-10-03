### What this was about

The desktop application used a loopback helm that looked like a normal browser-facing helm: it listened on a fixed port,
served the web UI, offered token sign-in, and persisted the webview credential in WebKit storage. Local processes could
reach that listener, while access depended on credentials that could outlive the desktop launch. The plan makes the
desktop helm private to its own window at the application boundary: every launch gets fresh in-memory credentials, and
the browser UI and token sign-in are removed. The loopback listener remains reachable as a local socket, but it is not
usable without those per-launch credentials.

The desktop helm now binds a kernel-selected loopback port, serves no browser UI, exposes no token exchange, and accepts
only the two credentials minted in memory for that launch. Standalone helms retain their browser behavior but refuse the
desktop webview's custom-scheme origins. The webview keeps its credential in page memory and removes a legacy stored
value when possible. The desktop smoke test uses a debug-only readiness handoff instead of a fixed port or
environment-controlled release seam.

### Things you should know

The chosen design keeps the loopback transport because it preserves streaming WebSockets and upload backpressure without
inventing a second IPC transport. Dioxus's own loopback channel remains a framework implementation detail. A
browser-based workflow uses a standalone helm after the desktop app is quit; the desktop helm cannot be used for browser
takeover or token sign-in.

The three changes are stacked and remain draft PRs for maintainer review. The final code also removes the completed
desktop networking TODO entry and updates the product and implementation specifications, security documentation, desktop
triage guidance, and manual checklist.

### Open questions and possible follow-ups

There are no implementation decisions waiting on the maintainer. The remaining review decision is whether to approve and
land the stack. The accepted follow-up architecture is the loopback framework-style transport; moving the desktop UI to
an in-process transport would be a separate design and implementation effort.

### PRs

- [#1516](https://github.com/scode/farhelm/pull/1516/changes) `feat!: make the desktop helm private` — reviewed head
  `3f8747b4`; random per-launch loopback port, no browser UI or token route, in-memory launch credentials, and the
  debug-only smoke readiness handoff.
- [#1527](https://github.com/scode/farhelm/pull/1527/changes)
  `fix: keep desktop webview origins out of standalone helms` — reviewed head `6ab1c994`; custom-scheme origin admission
  is limited to the embedded desktop mode while the standalone origin and CORS tests cover both modes.
- [#1536](https://github.com/scode/farhelm/pull/1536/changes) `refactor: keep desktop credential in page memory` —
  reviewed head `1cf34b8e`; page-memory credential precedence, browser storage fallback, and best-effort cleanup of the
  legacy key.

### Checks run, reused and skipped

Evidence was collected against the reviewed heads named above, and the final stack contains those unchanged commits:

- PR1 and PR2 targeted Rust checks included `cargo fmt --all -- --check`, `dprint check`, `git diff --check`, the
  isolated source sleep checker (274 delays inspected, zero unannotated), and
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings`.
- Recorded focused helm nextest evidence `8e272b97-ffe9-4e75-8135-c8a3e167f448` (14/14 selected tests) covered PR2 head
  `6ab1c994` and its unchanged PR1 parent, including the embedded authentication, origin, router, and CORS boundaries.
- PR3 head `1cf34b8e` passed the full UI asset-JS harness (190/190), including unavailable-storage authentication and
  reconnect behavior; its cargo formatting, dprint, diff, and source sleep checks were rerun after the final
  corrections.
- Desktop feature compilation `cargo check -p farhelm-ui --features desktop` passed on PR3's final code.
- Recorded desktop smoke evidence `74d430e4-69d7-45b5-add7-6b622717a9f8` passed against PR3's final code, including the
  second-instance refusal and restart credential boundary.
- Recorded Chromium/WebKit feed evidence `f7fb119a-42ba-4910-abba-9f0a15b55454` (2/2) and terminal evidence
  `8a55298f-8c75-46e6-9fe9-6e98151f2977` (4/4) passed against PR3's final code. A broader browser run
  `7dabf89c-9c32-4bbc-9da8-18f25d06fc24` stopped at its recorder breadth deadline after 72 passing cases; the exact
  active feed case was rerun on both engines and passed.

Earlier focused checks were reused where the intervening changes were documentation-only or confined to the already
covered asset/test harness. No full workspace battery, desktop doctest suite, installer suite, or website build was run
because the final changes are confined to the desktop helm/UI behavior and its documentation, and the targeted evidence
covers the material regression risks.

### Review gate outcome

Each PR passed the required fresh-context Opus 5.5 high-effort review at its reviewed head. The final exact-commit
review of PR3 found no blocking correctness, security, or plan-conformance issue; its low-severity documentation and
test-clarity observations were addressed before that final review. PR1 and PR2 likewise completed their correction
passes before clean exact-commit reviews of their final heads. The stack is ready for maintainer review and approval.
