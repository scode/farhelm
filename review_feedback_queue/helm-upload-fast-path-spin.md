# The helm's upload fast path can spin without a deadline

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A degenerate upload could pin a helm CPU core at 100% forever with the request never completing — if the triggering body
shape is reachable, which is genuinely disputed (see Details).

## Details

Source: pre-pr-review-swarm, area data-transfer, 2026-09-17. Confidence: possible (state-lifecycle p1). Verified facts:
the relay loop's fast path (`now_or_never`, helm uploads.rs:216-218) bypasses the deadline machinery, which is armed
only in the slow arm (230-232); an empty item hits `continue` straight back to the check with no await, no yield, no
deadline (283-285). If the body stays immediately ready with empty items, the relay spins forever, never reaching end,
stall timeout, or error handling.

Two caveats, both recorded honestly. First, reachability through the production HTTP stack is disputed: the finding
lens's premise is unverified, and the edge-inputs lens's second sweep asserts an always-ready empty body is unreachable
through hyper's real body stream — neither side proved the transport behavior. Second, review-quality flag: the
finding's original writeup cited entry-boundary symbol names (`UploadRequest::Pending`, `poll_ready`, `push_chunk`,
`single_request`/`Streaming`) that do not exist anywhere in the checkout — the restater caught this, the coordinator
verified the real loop shape directly (the handler is `upload_attachment`, body via `into_data_stream()` at 189), and
the fabricated story is dropped here. A future agent should re-derive reachability from the real code before acting.

Suggested fix (concrete and harmless either way): when the fast-path item is empty, fall through to the deadline-driven
await instead of `continue`, so an empty item goes through the progress/stall machinery like every other frame.
