# Idempotency records keep the raw command line after Delete

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Deleting a session doesn't remove its command line, keys included, from that host's database.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F25 / SEC-FINGERPRINT-RAW-CMDLINE`, tagged **possible**. Anchors and title: `service/core.rs:1382-1480`,
`service/handlers.rs:59-63`, `store.rs:3335-3347`, `store.rs:4375-4386` — Idempotency records keep the raw command line
in the database after Delete

A request to create a session can carry an "intent key", a client-chosen id that makes retries safe. If the same key
arrives again, the supervisor returns the original answer instead of starting a second agent. To tell a true retry from
a different request that reuses the key, the supervisor stores a "reservation" row in the `create_reservations` table of
`supervisor.db`. The row holds a "fingerprint" of the request. `create_fingerprint` (`service/core.rs:1382`) builds the
fingerprint as plain JSON. It includes the full raw `invocation` (the agent command line, where users sometimes put API
keys) and the resume template. For a fresh GitHub checkout it includes the whole create mode. The web UI's create form
mints an intent key (`farhelm-ui/src/list/create_form.rs`), and interactive creates get the `Permanent` dedup scope
(`service/handlers.rs:59-63`).

When the session is deleted, both delete transactions (`store.rs:3335-3347` and `store.rs:4375-4386`) remove the
`sessions` row. Reservations with `session_lifetime` scope are deleted too. `permanent` reservations are only re-settled
to `created` and kept, fingerprint included. The raw command line therefore stays in the database after every other
trace of the session is gone. The code knows about this. The `create_fingerprint` docstring says a digest would end "the
`invocation` — which may embed credentials — being retained past its session's deletion in a tombstone", and adds that
"neither is owned here". Elsewhere, Delete fails closed if it cannot remove launch spec files, precisely because those
files hold command lines with credentials in them.

This matters because SPEC.md "Lifecycle operations" says "**Delete** removes the session and its stored state". Only the
same Unix account can read the leftover copy (the database is 0600 inside a 0700 directory), so the issue is how long
the copy is kept, not who can read it. The open premise is whether the maintainer counts the retry record as the
session's "stored state". SPEC.md's retry-retention paragraph ("Local authority and trust between hosts") says only that
permanent retention of agent-originated retry records is not required, and it says nothing either way about credentials.
The options:

- Store a digest (such as SHA-256) of the fingerprint instead of the raw strings. Plain text would stay only where
  recovery needs it: the fresh-checkout variant needs the create mode while its session exists.
- Replace the fingerprint with a digest or blank value at Delete, keeping a marker that still refuses reuse of the key.
- Document the retention in SPEC_impl.md.

Restater note: The mechanism is confirmed in code. One caveat for the fix: SPEC_impl.md and the code comments say
existing fingerprint encodings are frozen byte-for-byte, so that rows written by older versions keep matching their
retries. Switching to a digest therefore needs a migration or dual comparison for existing rows. Scrubbing only at
Delete avoids that problem.
