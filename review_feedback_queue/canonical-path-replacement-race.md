# Canonical-path replacement race

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Replacing a validated directory could redirect a later launch; supported reachability is unresolved.

## Details

F19 — **possible** — `crates/farhelm-supervisor/src/service/core.rs:2993` — Canonical-path replacement race

Directory validation returns a canonical pathname rather than retaining directory identity through launch. Replacing a
component of that canonical path during later asynchronous work could make the eventual launch open another directory.
Changing only the original symlink does not have this effect. No supported accidental replacement workflow or
outside-account exploit has been established. First settle that scope question; if such a workflow exists, define and
enforce directory identity through launch.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:2993–3012: identity validation returns canonical path text, with no
  retained directory handle.
- crates/farhelm-supervisor/src/service/core.rs:10152–10160: Restart retains that text before further asynchronous work.
- crates/farhelm-supervisor/src/service/core.rs:10234–10307: probing and cleanup intervene before relaunch.
- crates/farhelm-supervisor/src/service/core.rs:2993–3012: the guard returns canonical path text.
- crates/farhelm-supervisor/src/service/core.rs:10158–10335: later restart work consumes that path across asynchronous
  operations.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- The deliberate same-account attack branch is covered by SPEC.md:2152–2164; full coverage of an accidental
  restart-directory replacement is not established.
- {"matched": "Duplicate of retained input gap_supervisor_state_cor_02:p1:F5, at the same single editable guard
  region.", "possible": ["SPEC.md:2152–2164 covers deliberate same-account filesystem interference.", "SPEC.md:882–884
  separates accepted terminal-tab path behavior from agent restart.", "SPEC.md:2384–2389 addresses filesystem aliasing
  rather than replacement during this gap."]}

Caveats:

- No concrete supported accidental actor or outside-account exploit was established.
- Ordinary retargeting of the original symlink alone does not defeat the returned canonical path.
- Retained for scope clarification, not presented as a confirmed exploitable vulnerability.
- No concrete supported accidental replacement workflow or outside-account exploit was established.
- No confirmed vulnerability or user-work loss is claimed.
- No runtime reproduction was performed.
- The referenced C12 would duplicate this guard region if collected separately.
- No concrete supported accidental actor or outside-account exploit has been established.
- Retargeting only the original symlink does not defeat the returned canonical path.
- The retained F5 must preserve these caveats.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_02:p1:F5`,
`gap_supervisor_state_cor_02:p1:C12`.

- `gap_supervisor_state_cor_02:p1:F5`: confidence as filed: possible; requires a concrete in-scope replacement actor and
  sequence; suggested bucket as filed: highest.
- `gap_supervisor_state_cor_02:p1:C12`: confidence as filed: not separately tagged in candidate list; suggested bucket
  as filed: not separately tagged in candidate list.
