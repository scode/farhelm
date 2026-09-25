# Helm startup fails if a token CLI briefly holds the token-control lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Running `farhelm helm token show`/`rotate` while the helm or desktop app starts can make it fail to launch with a
confusing "another process owns token control".

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F3 / COR-STARTUP-LOCK`, tagged **possible**. Anchors and title: `token_control.rs:92-94`, `token_control.rs:334-361`,
`crates/farhelm-helm/src/lib.rs:1596-1641` — The helm aborts startup if a `token show`/offline `token rotate` briefly
holds the token-control lock

A file lock (`flock` on `helm-token.lock` in the state directory) decides which process owns token decisions for that
directory. A running helm holds it for as long as it runs. When no helm is running, the token CLI commands take it for a
moment. `token show` holds it while it opens the database (including any schema migration) and, on first use, mints the
token. An offline `token rotate` holds it for its rotation transaction. Every attempt uses a non-blocking exclusive lock
(`acquire_ownership`, lines 334–361). A caller that finds the lock held gets an `OwnershipBusy` error.

The CLI copes with that error: `show` falls back to a read-only path, and `rotate` retries until its deadline. The helm
does not cope. It takes the lock exactly once, inside `token_control::serve` (lines 92–94, called at `lib.rs:1637`). An
`OwnershipBusy` there propagates with `?`, and the helm exits with "another process owns token control". By then,
startup (`lib.rs:1596–1641`) has already bound the HTTP port, opened and migrated `helm.db`, applied any
`--ensure-hosts` file (a flag that registers hosts at startup), and started the connection manager's per-host actors.

This window is easy to hit. `farhelm helm setup` restarts the units and then prints
`farhelm helm token show   # the
browser sign-in token` as the next step (`crates/farhelm/src/setup.rs:723`), so a user
who runs it immediately is racing the helm's startup. Under systemd the unit has `Restart=on-failure`, so the helm comes
back. `farhelm helm run` in a terminal just exits, and the desktop app's embedded helm failing at startup ends the app
with a fatal refusal. In every case the message suggests a rival helm, which is not the real cause.

How often the narrow window is hit in practice is unmeasured.

Suggested fix: on `OwnershipBusy` in `serve`, retry for a short bounded time (or take a blocking lock on a blocking
thread with a timeout), and fail only if the lock is still held, naming the lock file in the error. Also consider taking
the lock before the startup steps that have side effects.
