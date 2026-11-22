# Invalid session IDs in pending_scope_cleanups are logged unescaped

Reviewed commit: 729ed16229e0b2cd660f9898182bac305be679fd

## TLDR

When the supervisor discards a corrupted or malformed session ID from its startup cleanup queue, it prints that raw ID
directly into its log output without escaping line breaks or invisible formatting characters, contrary to the logging
spec.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/host_restart.rs:917`.

Source-verified. `SPEC_impl.md` lines 152-160 ('Session IDs in logs') requires: 'Session IDs must not inject log lines
or use invisible or direction-changing characters to disguise the ID shown in logs. ... Safe escaping by the configured
logging formatter also satisfies the requirement without a separate escaping helper at each call site.' In
`crates/farhelm/src/main.rs:2641-2650`, `init_tracing()` uses the default `tracing_subscriber::fmt()` formatter, which
outputs `Display` fields (`%id`) verbatim without escaping newlines, ANSI escapes, or Unicode bidi/invisible characters.
In `Supervisor::retry_pending_scope_cleanups` (`crates/farhelm-supervisor/src/service/core/host_restart.rs:900-926`),
the `else` branch at line 917 runs specifically when `crate::scope::unit_name(&id, generation)` returned `None` because
`id` is NOT a valid lowercase UUID, yet both line 917
(`warn!(session = %id, generation, "discarding pending restart cleanup with an invalid session id")`) and line 924
(`warn!(session = %id, generation, ..., "could not retire pending restart scope cleanup")`) format `id` with `%id` (raw
`Display`) instead of `?id` (`Debug`, which escapes control and non-printable/bidi characters via `str::escape_debug`).
Fix direction: change `session = %id` to `session = ?id` at
`crates/farhelm-supervisor/src/service/core/host_restart.rs:917` and `:924` (or format via `id.escape_debug()`).

Reviewer metadata (g18-sqlite-and-restart-cleanup/f3): suggested bucket: other (could NOT be in highest on currently
established routes: `pending_scope_cleanups` lives in the 0600 `supervisor.db` inside the 0700 state directory and
shipped code only inserts minted UUID session IDs, so there is no established untrusted injection route in shipped code;
would move to highest if an unprivileged or untrusted caller could insert arbitrary session IDs into
`pending_scope_cleanups` or `sessions`); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g18-sqlite-and-restart-cleanup/f3`.
