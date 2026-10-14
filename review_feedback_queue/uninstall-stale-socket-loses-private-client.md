# A stale socket strands uninstall after its private client is removed

Reviewed commit: 2ef29e756c983e9a0600865845ef25b4bea13dd1

## TLDR

An interrupted uninstall can become impossible to finish through Retry even though the supervisor and tmux server are
gone.

## Details

Suggested bucket: `high`. Confidence: likely; the combined stale-socket/interrupted-uninstall sequence was not
reproduced. Stale sockets are explicitly acknowledged by the new test.. Source: dedicated `gpt-6.1-sol` high adversarial
review. Proposed drop: none.

backend.rs:2548–2557 executes the frozen tmux client whenever the socket pathname exists. plan.rs:703–714 selects
lib/tmux when no acceptable host client exists, then removes that lib directory before forgetting the host. If tmux
leaves a stale socket and removal completes but host deletion fails or the helm stops before deletion, every retry
executes the now-missing client and fails. service.rs:1578–1579 supplies a fallible registry-deletion boundary. The new
test explicitly acknowledges stale sockets but removes the socket before its missing-client check
(provisioning.rs:8886–8900), masking this combination. Preserve a usable absence probe or remove a socket only after
proving its server absent, before deleting the client.

Possible cover: none found; SPEC_impl.md:3367–3374 promises lib-gone retry and recovery from partial removal.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: backend.rs:2548–2557 and
plan.rs:703–714 are unchanged; provisioning.rs:8987–9001 still removes the socket before the missing-client check.

Review coverage: full commit diff read: true. No commit diff content remains unread. Initial aggregated output was
truncated; the full diff and queue index were subsequently reopened in bounded chunks. The 13,089-line triage ledger was
searched throughout, with relevant sections read, rather than read exhaustively; TODO outside the specified Planned
bucket and cited relevant entries was likewise searched. Unchanged implementation outside the listed ranges, full
browser fixture setup, and remote-container server-disappearance behavior were not exhaustively traced. Covered highest
findings await the parent’s independent drop review; no nested agents were used.

Runtime evidence: None run. No checkout or scratch files created, modified, or deleted. uutils confirmation relies on
retained repository evidence; stale-socket interruption, native-macOS fixture failure, ambient-service killing, and
deliberate socket redirection were not reproduced. Applied the supplied test-authoring checklist to changed tests and
fixtures.
