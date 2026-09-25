# token rotate can report a timeout after the rotation committed

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

`token rotate` can print an error while every browser is in fact logged out and the old token dead, with no new token
shown.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F2 / COR-ROTATE-TIMEOUT`, tagged **possible**. Anchors and title: `token_control.rs:25`, `token_control.rs:241-256`,
`token_control.rs:300-325`, `store.rs:162` — `token rotate` can report a timeout while the helm completes the rotation

When a helm is running, `farhelm helm token rotate` does not touch the database itself. It connects to a private Unix
socket in the helm's state directory (the "token-control" socket), sends `rotate\n`, and waits for `OK <new token>`. It
does it this way so the running helm can close its own live WebSockets right after deleting their credentials. On the
CLI side, connect, write, and reply read all share one 2-second budget (`CONTROL_DEADLINE`, line 25; used in
`rotate_through_helm`, lines 241–256).

On the helm side, `serve_client` (lines 300–325) runs `AuthState::rotate` to completion and never checks whether the CLI
is still there. Rotation can wait on three things in turn:

- the rotation mutex;
- the store's single database-connection mutex, which every API request and host-cache refresh also takes, with no
  timeout;
- an immediate SQLite write transaction, which can wait up to the store's 5-second `BUSY_TIMEOUT` (`store.rs:162`) if
  another process, such as the checkout-config CLI, holds the database write lock.

If the total goes past 2 seconds, the CLI exits non-zero with "timed out reading token rotation reply" and prints no
token. The helm keeps going regardless: it commits the new token, deletes every device secret, and closes every
authenticated socket. Its reply then fails to write, which is only logged.

So the command reports failure for an operation that succeeded. People usually rotate because they suspect the token
leaked. This user now cannot tell "not rotated" from "rotated, but the answer was lost". Every browser has been logged
out, and the new token was never shown to them (`token show` would reveal it, but nothing tells them to run it). How
often rotation takes more than 2 seconds in practice was not measured. That the CLI's 2-second wait is shorter than the
helm's possible 5-second-plus wait is certain.

Suggested fix: give the reply read its own deadline, longer than the helm's worst case (busy timeout plus margin), and
keep the connect deadline short. If the reply still times out after `rotate` was sent, say the outcome is unknown and
point the user to `farhelm helm token show`. Alternatively, put a helm-side deadline around `rotate()` that is shorter
than the CLI's.
