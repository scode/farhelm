# Emergency browser cleanup kills an already-reaped spawner by its old number

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Emergency cleanup could kill a replacement for an already-reaped spawner.

## Details

F76 — **possible** — `scripts/test-start-stack-cleanup.sh:263–264`; `scripts/test-start-stack-cleanup.sh:263` —
Emergency browser cleanup kills an already-reaped spawner by its old number

The harness explicitly reaps its spawner, then retains its process number across a cleanup observation loop and
diagnostics before issuing KILL on failure. That number no longer reserves the original process identity. If reused
during the interval, emergency cleanup could kill unrelated work. Clear signal authority when reaping the spawner and
retain owned process instances for remaining cleanup. The failed-phase and reuse sequence was not reproduced.

## Evidence and triage context

- scripts/test-start-stack-cleanup.sh:309 and :326 reap the spawner. The following await_gone calls poll up to twenty
  iterations, with curl deadlines and sleeps at :194–201. On failure, :314 or :331 invokes emergency_cleanup, whose :264
  unconditionally signals the old spawner number.
- scripts/test-start-stack-cleanup.sh:307-310 and 324-327 kill and reap the spawner before awaiting cleanup.
- scripts/test-start-stack-cleanup.sh:194-203 performs twenty iterations containing one-second sleeps and potentially
  two-second HTTP waits.
- scripts/test-start-stack-cleanup.sh:313-314 and 330-331 run diagnostics before emergency cleanup.
- scripts/test-start-stack-cleanup.sh:263-264 sends SIGKILL to saved script and spawner numbers without checking birth
  identity.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:2194–2205 does not accept retaining a reaped number across this asynchronous polling work.
- SPEC_impl.md:2194-2205 and TRIAGE_OUTCOMES.md:6523-6528 accept only short bounded reuse windows, not numbers stored
  across later work. The watcher-specific ledger decisions concern different, much shorter sequences.

Caveats:

- Requires a failed cleanup phase and PID reuse. This is a distinct signal site from exit cleanup.
- Requires PID reuse; not reproduced.
- The spawner path is directly established as post-reap; the script may or may not have exited.
- This is separate from the command-line-wide kill at line 266.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_10_cor:p1:F6`,
`automation_website_10_sec:p1:F7`.

- `automation_website_10_cor:p1:F6`: confidence as filed: possible; suggested bucket as filed: highest.
- `automation_website_10_sec:p1:F7`: confidence as filed: possible; suggested bucket as filed: highest.
