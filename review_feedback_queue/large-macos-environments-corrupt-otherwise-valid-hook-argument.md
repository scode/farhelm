# Large macOS environments corrupt otherwise valid hook-argument evidence

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A large macOS environment could make valid hooks lose argument evidence.

## Details

F79 — **possible** — `crates/farhelm-supervisor/src/procs.rs:1589` — Large macOS environments corrupt otherwise valid
hook-argument evidence

The macOS argument fetch limits a kernel response containing both arguments and environment data, rather than limiting
only parsed arguments. Inspected kernel behavior supports a large environment displacing evidence of an otherwise short,
valid hook invocation, causing report refusal and missed Resume capture. Supported-kernel execution was not
demonstrated; an existing binding is not directly erased. Size the fetch using the kernel's argument maximum, then
enforce the evidence budget on the parsed argument region.

## Evidence and triage context

- crates/farhelm-supervisor/src/procs.rs:1589–1614 allocates MAX_ARGV_BYTES_PER_PROCESS + 1, calls KERN_PROCARGS2, and
  parses a successful response as complete argv.
- Apple XNU f6217f891ac0bb64f3d375211650a4c1ff8ca1ea, bsd/kern/kern_sysctl.c:1561–1619 copies from copy_end - buflen and
  preserves zero-padding behavior when the buffer is undersized.
- crates/farhelm-supervisor/src/procs.rs:407 collects this argv; procs/claude.rs:42–46 and procs/codex.rs:23–26 reject
  missing or incorrectly shaped reporter argv.
- crates/farhelm-supervisor/src/service/core.rs:2874–2897 retains capture across relaunch; core.rs:10049–10150
  subsequently selects and verifies the durable binding.
- crates/farhelm-supervisor/src/procs.rs:1589–1614 uses the argv cap to size the entire KERN_PROCARGS2 response.
- crates/farhelm-supervisor/src/procs.rs:1454–1474 independently documents this kernel's undersized-buffer hazard.
- Apple XNU f6217f891ac0bb64f3d375211650a4c1ff8ca1ea, bsd/kern/kern_sysctl.c:1561–1619 preserves suffix copying and zero
  padding.
- crates/farhelm-supervisor/src/procs.rs:407 feeds the result to ancestry collection; procs/claude.rs:42–46 and
  procs/codex.rs:23–26 refuse unusable reporter argv.
- crates/farhelm-supervisor/src/procs.rs:1589–1614 sizes the combined response at 65,537 bytes and incorrectly assumes
  undersizing necessarily fails.
- Apple XNU f6217f891ac0bb64f3d375211650a4c1ff8ca1ea, bsd/kern/kern_sysctl.c:1561–1619 instead preserves an
  undersized-buffer suffix-copy path.
- crates/farhelm-supervisor/src/procs.rs:407 records the returned argv; procs/codex.rs:23–26 refuses a reporter whose
  arguments no longer identify the hook.
- procs.rs:1589 allocates 64 KiB+1 for KERN_PROCARGS2, which contains executable path, argv and ordinarily environment
  data; :1614 passes the result to the argv parser. The retained XNU source, kern_sysctl.c:1561-1618, explicitly selects
  the buffer tail and compatibility zeroing when the supplied buffer is too small. This contradicts procs.rs:1611-1613's
  assumption that undersizing necessarily produces ENOMEM. procs/claude.rs:42-45 and procs/codex.rs:23-27 require
  recognizable reporter argv.
- Independent decisions are sr_data:p1:F1 KEEP, sr_data:p1:F2 KEEP, and sr_data:p1:F3 DROP as covered by
  review_feedback_queue/omp-bun-pane-proof.md:9–50. No input formal entry was merged or removed.
- Same allocation and parser at procs.rs:1589-1614, with XNU's undersized tail-copy behavior at retained
  kern_sysctl.c:1561-1618.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:1836–1838 accepts missing over-budget or truncated argv evidence. It does not expressly accept an
  environment-size limit on otherwise short argv.
- SPEC_impl.md:1836–1838 covers oversized or truncated argv, not a large environment with short argv.
- SPEC_impl.md:1836–1838 names an argv evidence budget; it does not explicitly accept rejecting short argv because the
  environment exceeded the fetch allocation.
- SPEC_impl.md:1833-1838 accepts unavailable, truncated or over-budget argv as missing, but its limit is an argv limit.
  It does not explicitly accept a large environment making short argv unreadable. No exact Planned, BUGS, queue or
  ledger coverage found.
- Exact duplicate of sr_systems:p1:F1. SPEC_impl.md:1836-1838's argv budget does not cover combined-environment
  undersizing.

Caveats:

- No macOS execution was performed. Applicability of the inspected XNU behavior to every supported kernel remains
  unverified.
- Wrong Resume additionally requires an existing binding, a subsequent conversation switch whose report is rejected, and
  the previous target remaining resumable.
- No transcript deletion is established.
- No macOS runtime reproduction.
- This input reports missing capture; the stronger stale-target variant remains separately represented by sr_data:p1:F1.
- No macOS execution was performed.
- The original high-bucket missing-capture claim is preserved; the highest-possible stale-target variant is separately
  retained in sr_data:p1:F1.
- No live macOS execution. Exact malformed bytes depend on the environment and kernel version. Existing stored
  identities are not directly erased. The demonstrated case is a new report losing valid hook evidence.
- No live macOS reproduction. Exact returned bytes depend on kernel version and environment. This establishes lost new
  hook admission, not erasure of an already captured identity.
- No live macOS reproduction; preserve the large-environment premise on the retained finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_data:p1:F1`, `sr_edges:p1:F2`, `sr_lifecycle:p1:F1`,
`sr_systems:p1:F1`, `sr_data:p1:C13`, `sr_secrets:p1:C7`.

- `sr_data:p1:F1`: confidence as filed: Possible; suggested bucket as filed: highest.
- `sr_edges:p1:F2`: confidence as filed: Possible; likely with supported-kernel behavior unverified; suggested bucket as
  filed: high.
- `sr_lifecycle:p1:F1`: confidence as filed: Definite; confirmed by inspection; suggested bucket as filed: high.
- `sr_systems:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
- `sr_data:p1:C13`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `sr_secrets:p1:C7`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
