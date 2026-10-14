# Cancelling download admission leaks pending-request bookkeeping

Reviewed commit: 871499e5b39d52eb4b00b54090f9af1f1d8a108b

## TLDR

Repeatedly cancelling downloads before they start leaves a small amount of helm memory retained until the host
reconnects.

## Details

Suggested bucket: `other`. Confidence: confirmed by code inspection; cancellation frequency and measurable memory impact
are unverified. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: none.

In crates/farhelm-helm/src/client/downloads.rs:87–97, BeginDownload uses the shared request registry. Cancelling that
future drops its guard; lines 135–143 and 208–212 remove the download route and send AbortDownload, but never remove the
pending request. The supervisor handles AbortDownload by dropping the route
(crates/farhelm-supervisor/src/service/connection.rs:616), which aborts its task (service/downloads.rs:28–31). If
cancellation arrives while that task is opening the file or waiting to enqueue DownloadStarted (195–220), no correlated
reply is ever sent. The helm entry inserted at client.rs:2540 is otherwise removed only by a reply (1805–1806) or
connection teardown (1598–1600). Consequently each successful cancellation in this interval permanently retains another
entry on the live connection. This commit exposes the protocol/client API without a GUI action; pinned main subsequently
calls it from the download HTTP handler. Impact is small per cancellation; substantial memory growth was not measured.
Retire the pending admission request when its guard is cancelled, or ensure cancellation completes the correlated
request.

Possible cover: TRIAGE_OUTCOMES.md heading cancelled-request-leaks-pending-entry.md and SPEC_impl.md 'Supervisor
metadata retention and nonresponse' accept accumulation caused by selectively nonresponding malicious or buggy peers.
This case uses the new protocol's ordinary cancellation behavior with an otherwise correctly behaving supervisor; the
specification expressly preserves ordinary-operation boundedness.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. Both download modules are unchanged;
the shared request registry still has no cancellation removal. Current crates/farhelm-helm/src/downloads.rs:83–87
invokes begin_download directly. No fixing commit found.

Review coverage: full commit diff read: true. No commit diff content remains unread. The first combined policy output
was truncated; the missing BUGS tail and the relevant TODO/Planned content were reopened separately. Unrelated TODO
buckets from that truncated output were not relied upon. TRIAGE_OUTCOMES.md is 13,089 lines and was searched throughout,
with relevant sections read rather than every unrelated outcome. Unchanged surrounding modules, full shared e2e harness
internals and later UI implementation were not exhaustively read. Potentially highest refuted candidates and the
specification-based proposed drop are retained for the parent's independent disposition.

Runtime evidence: No runtime tests or reproductions run, as requested by default. Applied the supplied test-authoring
checklist to all changed tests and fixtures by inspection. No checkout files were created, modified or deleted; no
scratch reports, fetches, rebases or nested agents.
