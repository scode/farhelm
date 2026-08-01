# M2 supervisor SQLite: restart-gap semantics and schema versioning

Decisions made while adding session persistence (M2 step 3), recorded because each could plausibly have gone another
way.

## Restart gap: keep rows, reattach the live ones, exited-unknown for the rest

When the supervisor restarts, SQLite has session rows and tmux may or may not still have the sessions. Options
considered:

- **Drop rows with no live tmux session at boot.** Simplest, rejected: it silently loses sessions from the list, which
  is exactly the failure persistence exists to prevent, and it fights M3's direction (interrupted classification wants
  those rows).
- **Mark every reloaded row terminal-less, even when tmux still has the session.** This was the literal reading of
  "don't pull M3's rediscovery forward". Rejected: the liveness probe (`has-session`) is needed anyway to distinguish
  the cases, and once you have it, refusing to hand back a live terminal means telling the user a running agent is
  exited — the "never guess" rule in SPEC.md forbids fabricating state in both directions.
- **Chosen: probe tmux per row.** Live sessions come back as ordinary entries (attach works — supervisor restarts with
  a surviving tmux server are non-events); dead ones stay listed but terminal-less, where attach fails `not_found`
  with a message naming the restart (input and resize never reach a terminal-less session at all: input has no
  attachment to route through, resize is ignored). M3 replaces the crude "exited, unknown code" story with
  boot-id-based interrupted classification; nothing here needs undoing for that.

The DB insert happens after tmux creation, and an insert failure tears the tmux session back down and fails the
create. The alternative — let the session live and log the persistence failure — was rejected because such a session
silently vanishes on the next restart; a loud create failure is recoverable, a quietly unlisted agent is not.

## Schema versioning: `PRAGMA user_version`, not a version table

A `schema_version` table was the initial sketch. Rejected in favor of the pragma: it is atomic with the database file,
has no bootstrap ordering problem, and cannot itself be absent from an otherwise-valid database. A table earns its
place only when migrations need per-step metadata (timestamps, checksums); nothing in the M2–M3 horizon does. An
unknown `user_version` is refused at open rather than guessed at.

## rusqlite `bundled`

Compiles SQLite from source instead of linking the host's libsqlite3. Required by the musl-static distribution plan in
SPEC_impl.md (audited there under cargo-zigbuild), and it makes dev boxes and CI independent of system SQLite
versions. Note: libsqlite3-sys 0.38 requires rustc ≥ 1.96; toolchains tracking stable are fine, stale pinned ones are
not.
