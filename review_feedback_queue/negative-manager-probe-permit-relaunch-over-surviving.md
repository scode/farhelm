# A negative manager probe can permit relaunch over a surviving scope

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A retry could launch duplicate work after losing evidence of a surviving scope.

## Details

F18 — **possible** — `crates/farhelm-supervisor/src/service/core.rs:8156` — A negative manager probe can permit relaunch
over a surviving scope

An agent launched in a systemd process group, called a scope, can start before its terminal pane is durably recorded. If
the supervisor crashes, descendants survive, other evidence disappears, and a later scope-manager probe fails, recovery
skips the recorded scope. Relaunch can then replace the original cleanup obligation. The full sequence was not
reproduced. Preserve recorded scope selection until that group is confirmed absent and use it during recovery evidence
checks.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:8122–8183: reserved-launch evidence ignores row.launch_scoped and skips
  scope existence when available() is false.
- crates/farhelm-supervisor/src/service/core.rs:8020–8035: Absent authorizes Resolution::Relaunch.
- crates/farhelm-supervisor/src/service/core.rs:8678,8720–8750: replacement launch selection supplies the replacement
  row's launch_scoped.
- crates/farhelm-supervisor/src/store.rs:3818–3822,3871–3917: an empty-pane launching/interrupted/error row can be
  replaced without preserving launch_scoped.
- crates/farhelm-supervisor/src/service/core.rs:9493–9519: external launch precedes durable pane confirmation, leaving a
  concrete crash window.
- crates/farhelm-supervisor/src/launch.rs:941–949: successful agent exec does not create an exec-failure sentinel.
- crates/farhelm-supervisor/src/scope.rs:703–723: the functional availability probe can fail independently of an
  existing scope's absence.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:854–866 and TRIAGE_OUTCOMES.md:2926–2938 accept duplicate detached survivors on hosts without containment;
  they do not fully cover a launch recorded as scoped.
- SPEC.md:870–878 requires honoring recorded scopes despite current manager availability.
- TRIAGE_OUTCOMES.md:3448–3467 covers Stop/Restart cleanup, while this site resolves pending Create evidence.
- TRIAGE_OUTCOMES.md:6119–6140 covers failed-create rollback.
- TODO.md:472–505 discusses eliminating Linux fallback in Maybe later, not the Planned bucket.

Caveats:

- The combined crash state, surviving scope, missing other evidence, and negative new probe were not reproduced.
- Branch behavior is confirmed; reachability of the complete sequence remains the open premise.
- The complete crash/surviving-scope/negative-probe sequence was not reproduced.
- No confirmed loss of production work is claimed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_02:p1:F1`.

- `gap_supervisor_state_cor_02:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
