# Offline token rotate against the wrong state directory creates a new database

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

The user believes they revoked a leaked token, but the old token and every browser session keep working.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0328-2b597e9-9c17` (audit of Area 2, helm HTTP/WS authentication boundary) as
`F4 / COR-OFFLINE-ROTATE-CREATES`, tagged **definite**. Anchors and title: `token_control.rs:155-157`,
`token_control.rs:210-218`, `auth.rs:311-314` — `token rotate` against the wrong state directory creates a fresh
database and reports success

`farhelm helm token rotate` finds the helm's state directory from `--state-dir` or from the default
(`$XDG_STATE_HOME/farhelm`, else `~/.local/state/farhelm`). If no helm answers on that directory's control socket, it
rotates offline. Offline rotation creates what it needs and never checks that a helm ever lived there:

- `ensure_private_dir` creates the directory if it is missing (lines 155–157);
- `HelmStore::open` (line 215), which opens with `SQLITE_OPEN_CREATE` and runs migrations, creates a brand-new
  `helm.db`;
- `rotate_offline` (`auth.rs:311–314`) writes a token into it.

The CLI then prints that token and exits 0.

Rotation is what a user runs after a leak. The user may have mistyped `--state-dir`, or be in a shell whose
`XDG_STATE_HOME` differs from the one `farhelm helm setup` baked into the systemd unit (the unit passes an explicit
`--state-dir`). Either way they get a success message and a new token, while the real helm keeps the old token and every
enrolled browser. The store already knows about this hazard. `open_existing_current_schema` exists for the
checkout-config CLI because, in its docstring's words, with the creating open "a mistyped state dir would be silently
answered with a freshly created, local-row-minting helm.db" (`store.rs` ~2733–2758). Token control deliberately keeps
using the creating open.

Suggested fix: on the offline path, refuse to create anything. Open only an existing database (still accepting older
schemas, as token control does today), and fail with "no helm database at …" if it is missing. Rotating a token that
never existed has no legitimate use.
