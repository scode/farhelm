# Normal teardown waits unboundedly on detach

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Closing a terminal tab while the supervisor connection is wedged hangs the cleanup for about a minute, holding server
resources for exactly the broken-connection case the cleanup exists to survive.

## Details

Source: pre-pr-review-swarm, area helm-agent-surface, 2026-09-17. Confidence: possible. Coordinator verified the
unguarded await and the inconsistency with the revoked paths. Complement, not duplicate, of
`detach-timeout-abandons-upstream-detach.md`: that item is about the revoked paths' timeout dropping the send; this one
is about the ordinary path having no timeout at all.

On the ORDINARY exit path (browser closed socket / supervisor ended attachment), `serve_term` calls
`client.detach(channel).await` directly (`crates/farhelm-helm/src/terminal.rs:729`) with NO timeout — while every
revoked path uses `detach_bounded` (5s): attach-race revocation (:491-495), admission-gap revocation (:522), mid-socket
revocation (:698). `detach` parks indefinitely on the full bounded queue (`SUPERVISOR_WRITER_QUEUE = 64`, client.rs:96),
so on a wedged supervisor connection the handler task parks at :729 until the 60s `WRITER_STALL_TIMEOUT` (client.rs:108)
kills the connection and drops `writer_rx` — normal teardown takes ~60s plus the 5s `settle_outbound` grace, retaining
the handler and outbound tasks for exactly the wedged-peer condition the teardown code exists to survive. Not infinite
(the stall timeout is the indirect bound), but a 12x inconsistency with the revoked paths' stated policy ("Give
supervisor cleanup a bounded opportunity").

Possible: needs a wedged connection at teardown; bounded-at-60s consequence.

Suggested fix: use `detach_bounded(&client, channel).await` at :729 too — the local entry is already removed inside
`detach`, and the supervisor-side attachment is reaped by connection death if the `Detach` frame never gets out, the
same tradeoff the revoked paths accept.
