# token show fails misleadingly when a non-serving process holds the lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Right after first setup, `farhelm helm token show` can fail once with a confusing "serving helm has no token" or
schema-version error.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F5 / COR-SHOW-BUSY`, tagged **possible**. Anchors and title: `token_control.rs:145-148`, `auth.rs:302-307`,
`crates/farhelm-helm/src/lib.rs:1637-1641` — `token show` assumes any lock holder is a serving helm with a token and
fails misleadingly

When `farhelm helm token show` finds the token-control lock held (see F3), it assumes a running helm owns the directory.
It then opens the database without migrating (`open_without_migration`) and calls `show_existing_token`, which fails
with "the serving helm has no token" if no token row exists yet (lines 145–148; `auth.rs:302–307`). Two real situations
reach this path without a helm that has a token:

1. A helm on its first start, between taking the lock (`lib.rs:1637`) and minting the token (`state.auth.token()` at
   `lib.rs:1641`). The helm exists but has not minted yet.
2. Another CLI holding the lock, such as a concurrent first-run `token show` or an offline `rotate`. No helm is involved
   at all.

On a fresh directory it gets more confusing. `open_without_migration` still passes `SQLITE_OPEN_CREATE`, so if the lock
holder has not created `helm.db` yet, `show` creates an empty file itself. It then fails with "helm.db schema version 0
is older than the version 7 required to read the web token", which reports a schema problem that does not exist.

Right after first setup, the documented first command can therefore fail once with an error that blames a helm that may
not exist, or a schema that is not really wrong. Both windows are narrow.

Suggested fix: on the busy path, retry briefly when no token exists yet. Do not create the database on this path, which
is meant to be read-only. Word the error neutrally, for example "token not initialized yet; retry".

Restater note: the finding also says this path "can leave an empty version-0 helm.db behind". In every sequence I could
trace, that empty file is transient. A serving helm opens and migrates `helm.db` before it takes the lock (`lib.rs:1595`
then `:1637`), and both CLI lock holders run the migrating `HelmStore::open` after taking the lock, which migrates an
empty file created meanwhile. A version-0 file would persist only if the lock holder failed between taking the lock and
opening the database. The misleading error message is the real effect.
