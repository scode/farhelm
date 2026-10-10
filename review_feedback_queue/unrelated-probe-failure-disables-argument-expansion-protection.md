# An unrelated probe failure disables argument-expansion protection

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A transient probe error could disable protection against argument expansion.

## Details

F117 — **possible** — `crates/farhelm-supervisor/src/scope.rs:1405`; `crates/farhelm-supervisor/src/scope.rs:1414` — An
unrelated probe failure disables argument-expansion protection

The scope-manager probe treats an unrelated non-timeout failure as evidence that the no-environment-expansion flag is
unsupported if an unflagged retry succeeds. On modern systemd, later launches can then expand dollar references in
literal wrapper paths and persistently fail or select an unintended path. That probe sequence and wrong-target execution
were not reproduced. Separate flag support from manager health, or retry unflagged only with evidence of unsupported
syntax.

## Evidence and triage context

- scope.rs:1364-1365 retries every non-timeout failure. :1405-1414 converts an unflagged retry's success into
  expand_environment_flag=false, even though :1465-1495 permits the first failure to occur after successful scope
  creation, during visibility, kill or collection. :1238-1243 records that answer; :920-922 subsequently omits
  --expand-environment=no. core.rs:13316-13331 and launch.rs:639-654 pass literal launch paths through the resulting
  wrapper.
- scope.rs:1405-1414 is the same fallback branch identified by ss_general:p1:F2; :920-922 consumes the cached false
  capability answer.

Retained confidence: **possible**. Suggested bucket: **high**.

Possible cover:

- SPEC_impl.md:2116-2138 accepts cached manager-health probing, not this false flag-support inference. TODO.md:471-499
  concerns unavailable-manager fallback and cleanup. FILTER.md's hung-dependency filter does not cover persistent
  incorrect configuration after successful manager responses.
- Exact duplicate of ss_general:p1:F2, not coverage by an existing Planned item or acceptance. SPEC_impl.md:2116-2138
  and TODO.md:471-499 do not settle false flag-support inference.

Caveats:

- A modern flagged probe failing non-timeout and its retry succeeding was not reproduced.
- Persistent launch failure is the supported consequence; execution through another existing path remains unverified.
- Highest is conservative because the wrong-target consequence is unresolved; the source report suggested high for
  launch failures.
- Requires modern systemd, a non-timeout first failure, successful retry and a path containing an expandable dollar
  reference. No runtime reproduction or wrong-target execution demonstrated.
- Requires a modern systemd, a non-timeout failure on the first probe, success on retry and a path containing an
  expandable dollar reference.
- No runtime reproduction or unauthorized-execution consequence was established.
- The claim is persistent launch correctness, not a demonstrated security exploit.
- No reproduction or unauthorized-execution consequence established. Highest severity is not supported by the current
  evidence.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ss_general:p1:F2`, `ss_trust:p2:F1`.

- `ss_general:p1:F2`: confidence as filed: possible; suggested bucket as filed: highest.
- `ss_trust:p2:F1`: confidence as filed: possible; suggested bucket as filed: high.
