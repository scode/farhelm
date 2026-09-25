# A second helm migrates helm.db before it checks the owner lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Starting a second (e.g. newer) helm on another port against the same state directory can silently upgrade the running
helm's database before it refuses to start, leaving the running helm erroring and unable to open its own data after a
restart.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F1 / COR-OWNER-LOCK-AFTER-MIGRATE`, tagged **definite**. Anchors and title: `lib.rs:1596`, `lib.rs:1598`,
`lib.rs:1600`, `lib.rs:1637`, `store.rs:29-31` — A second helm migrates helm.db, ingests hosts and dials the fleet
before it tries the single-owner lock

Only one helm process is supposed to serve a given state directory at a time. That rule is enforced by an exclusive
`flock` on `helm-token.lock`, taken inside `token_control::serve`. The module docs in `store.rs:29-31` say this lock
"prevents two serving helms … from claiming the state directory together". The startup sequence in `run_with_ready`
(`lib.rs:1567` onward) only reaches that lock at the very end. It runs in this order:

1. Bind the HTTP port (`lib.rs:1586`).
2. Open `helm.db` with `HelmStore::open` (`lib.rs:1596`). This is the _migrating_ entry point: it runs the full
   schema-upgrade ladder in `apply_schema`. That ladder includes destructive steps such as v28→29's
   `ALTER TABLE session_cache DROP COLUMN archived` (`store.rs:2573`) and several table rebuilds.
3. Run `ensure::ingest` for `--ensure-hosts`, which writes registry rows (`lib.rs:1598`).
4. Start the `ConnectionManager`, which spawns an actor per host (`lib.rs:1600`). Those actors immediately dial every
   supervisor and can write to `helm.db` (`record_first_contact`, `replace_host_sessions`).
5. Only now call `token_control::serve` (`lib.rs:1637`), which takes the flock non-blocking (`LOCK_EX | LOCK_NB`,
   `token_control.rs:345`) and fails with `OwnershipBusy` if another helm holds it.

The port bind in step 1 only protects against a second helm on the _same_ port. A second helm on a different port gets
through steps 2–4 before it fails at step 5. Ways to get there include `--port N`, the desktop app's
`FARHELM_DESKTOP_PORT` (`crates/farhelm-ui/src/desktop.rs:1343`), or the desktop app started beside the service helm,
both using the default state directory. The code's own documentation contradicts this order in three places:

- `lib.rs:1517` claims the early bind makes an already-running helm fail "before anything else has been set up or
  written".
- `HelmStore::open`'s doc (`store.rs:2709`) says callers that may run beside a live helm must use
  `open_without_migration`.
- `token_control::show` (`token_control.rs:137-151`) gets the order right. It takes the flock first and migrates only if
  it wins; otherwise it opens without migrating.

The dangerous case is a newer binary started beside an older, still-running helm. The newer binary upgrades `helm.db`
underneath the incumbent. The incumbent's SQL can then fail against the changed schema, and on its next restart it
refuses the database because `user_version` is newer than it understands. SPEC.md "Upgrade compatibility" accepts that
there is no downgrade hardening yet, but says this is not "blanket permission for destructive migrations". No
pre-upgrade backup exists. The migration also runs inside one `BEGIN IMMEDIATE` transaction, which can hold the live
helm's writers past their busy timeout. Rows the losing process wrote through `--ensure-hosts` have no actor in the live
helm. They do not appear in `/api/hosts`, which is built from the actor set, and re-adding the same destination is
refused as a duplicate.

Suggested fix: take the ownership flock right after `ensure_private_dir`, before `HelmStore::open`, hold it for the life
of the process, and hand it to `token_control::serve`. The early port bind can stay. Fail with a clear message such as
"a helm already owns state directory X". Correct the docs at `store.rs:29-31` and `lib.rs:1517`. The queue item
`helm-startup-fails-on-busy-token-lock` is related but different: it covers the token CLI holding the lock, not this
ordering.

User-visible consequence: starting a second helm (for example a newer one) on another port against the same state
directory can silently upgrade the running helm's database before the second helm refuses to start. The running helm
then errors, and after a restart it cannot open its own data.

Restater note: the claim that the losing process's `--ensure-hosts` rows are "never reconciled" by the live helm is too
strong. The live helm's `sync_registry` reads every registry row and spawns actors for rows without one
(`manager.rs:1409-1575`). So the stray rows get an actor at the live helm's next reconcile, which happens on any host
add, edit or remove, or at its next restart. The ordering defect itself is confirmed against the code.
