# Directory-report polling can accept an unfinished write

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The launch-directory report can be read while still empty.

## Details

F202 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:27284` — Directory-report polling can accept an
unfinished write

Shell redirection creates the report before writing its directory record. Polling for a readable file can therefore
succeed in that gap, after which an empty comparison falsely rejects correct launch behavior. Publish the witness
atomically or wait for a complete newline-terminated record before treating it as evidence.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:27169 writes a shim using pwd -P with direct output redirection.
- crates/farhelm-supervisor/src/service/core.rs:27284 returns the first readable content, including the empty file
  created before pwd writes.
- crates/farhelm-supervisor/src/service/core.rs:27267 immediately compares that content against a nonempty canonical
  path.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No matching accepted test race found. The rare self-correcting product-operation filter is not a clear match for an
  incorrect test verdict, so it cannot justify dropping this finding.

Caveats:

- The scheduling interval is narrow and its observed frequency is unknown.
- No runtime reproduction.
- Narrow scheduling window; frequency unmeasured.
- Readability is the wrong readiness oracle; atomic publication or a complete-record check would address this specific
  race.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_05:p1:F2`.

- `gap_supervisor_state_sec_05:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
