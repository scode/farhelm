# A timed-out revoked attach can leave the supervisor attachment in place

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Rarely, right after a token rotation, a session can stay held by an attachment no browser owns until the next reconnect
takes it over.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F8 / COR-ATTACH-CANCEL`, tagged **possible**. Anchors and title: `terminal.rs:476-493`, `client.rs:2983-3036`,
`client.rs:1882-1891` — Timing out a revoked terminal attach can leave the supervisor-side attachment in place

A terminal WebSocket (`serve_term`) first asks the owning supervisor to attach the browser to the session's tmux
terminal. If a token rotation revokes the socket while that request is still in flight (`terminal.rs:476–493`),
`serve_term` drops the browser socket and gives the attach request up to 5 seconds (`WS_TEARDOWN_GRACE`) to finish. If
it finishes, the helm sends an explicit detach. If it does not, `serve_term` returns, which cancels the request.

`attach_with_policy` (`client.rs:2983–3036`) registers the new channel in the connection's terminal map before sending
`Attach`. It removes that entry only on an error reply or an unexpected reply, never on cancellation. If the supervisor
accepts the attach after the helm gave up, no Detach is sent, and the supervisor keeps the attachment: a tmux client
pinned to this channel on behalf of a browser that is gone. Cleanup happens only when the next terminal event for that
channel arrives. `route_terminal_event` (`client.rs:1882–1891`) then finds the channel's receiver closed, removes the
entry, and sends Detach upstream (`release_upstream`).

Replay data normally arrives right after an attach, so this probably heals itself. The finding rests on the unverified
premise that some successful attach produces no later event for its channel. If one does, the session stays attached to
a browser nobody owns until another client's attach takes it over.

Suggested fix: make `attach_with_policy` safe to cancel, using a drop guard that removes the channel and calls
`release_upstream` if the future is dropped before completion. Alternatively, keep awaiting the attach in a detached
task rather than cancelling it.

Restater note: the self-healing path looks close to guaranteed. `TermEvent::ReplayComplete`'s docs (`client.rs`
~449–454) say the supervisor sends that marker "exactly once per attach that completes its catch-up", and it travels
through the same per-channel queue, so any attach that completes normally produces at least one later event for the
channel. The remaining gap is an attach ended before catch-up, for example by a takeover, and in that case another
client already holds the session.
