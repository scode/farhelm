# Revocation during admission orphans the terminal attachment

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If your login is revoked or rotated at the exact moment a terminal tab is still connecting, the abandoned connection can
keep owning the session: the real owner is displaced and later reconnects are refused as already owned, until something
else happens to shake it loose.

## Details

Source: pre-pr-review-swarm, area helm-agent-surface, 2026-09-17. Confidence: possible. Coordinator verified the race
window, the dropped-future path, and the map-entry leak.

If auth is revoked while `attach_from_query` is in flight, `serve_term` drops the browser socket and waits only
`WS_TEARDOWN_GRACE` (5s) for admission to complete (`crates/farhelm-helm/src/terminal.rs:486-496`): on timeout the
`if let Ok(Ok(...))` falls through, the handler returns, and the `attached` future is dropped mid-await with NO detach.
`attach_with_policy` (`crates/farhelm-helm/src/client.rs:2893-2952`) registers its `terminals` map entry BEFORE sending
`Attach` (client.rs:2914-2920), and both cleanup paths (the `is_err` remove at :2939, the wrong-variant remove at :2948)
run only if the future is polled past its `request().await` — a dropped future skips them. So the map entry leaks and,
if the supervisor already processed the `Attach`, the supervisor-side attachment is never detached. No helm task owns
the channel afterwards (`TermStream` died with the future): on a healthy connection the orphan persists, holding attach
ownership/lease, able to displace the previous owner and refuse later `if_unowned` reconnects. Self-heals only if
traffic arrives (closed-queue arm of `route_terminal_event`) or the connection dies; a silent session stays pinned.

Possible: the race window needs revocation/rotation racing a >5s attach.

Suggested fix: make attach cancellation-safe (a guard whose drop removes the map entry and enqueues an idempotent
upstream `Detach`, mirroring `begin_upload`'s guard), or do not drop `attached` on timeout — spawn it into a task that
detaches on success.
