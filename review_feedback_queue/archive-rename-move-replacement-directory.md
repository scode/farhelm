# Archive rename can move a replacement directory

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Checkout archiving could move a foreign replacement directory.

## Details

F118 — **possible** — `crates/farhelm-supervisor/src/working_copies.rs:1648` — Archive rename can move a replacement
directory

The archive helper validates ownership through a pathname, then renames that pathname later. If the checked checkout is
replaced in the gap, the rename can move the replacement directory instead. The consequence is a recoverable
wrong-directory move, not recursive deletion or cross-account compromise; ordinary supported reachability remains
untested. Use an identity-preserving archive protocol with collision retries and a defined response to post-move
mismatch. Another pre-rename check alone only narrows the gap.

## Evidence and triage context

- working_copies.rs:1464-1473 validates source identity, then journals and enters rename_exclusive_into. :1647-1652
  renames the pathname's current occupant and accepts success without post-move identity verification. Recovery
  separately checks identity at :1841-1845 before later rename calls at :1863-1874 and :1907-1918. teardown.rs:1012-1025
  accepts successful archival.

Retained confidence: **possible**. Suggested bucket: **high**.

Possible cover:

- SPEC.md:708-713 requires foreign replacements to remain untouched. SPEC.md:2152-2164 excludes deliberate same-account
  interference, but does not clearly exclude ordinary concurrent checkout replacement. snapshot-root.md covers preserved
  snapshot identities, not a replacement after validation. TRIAGE_OUTCOMES.md:1574-1584 establishes high severity for a
  recoverable wrong archive move.

Caveats:

- No ordinary-concurrency reproduction was performed.
- The supported-versus-deliberate trigger boundary remains open.
- The demonstrated consequence is a recoverable wrong-directory move, not recursive deletion or cross-account
  compromise.
- Adding another pre-rename check alone narrows the race; it does not make pathname mutation atomic with identity
  validation.
- Ordinary-concurrency trigger untested; intentional race manipulation is outside the threat model. Both fresh archive
  and recovery use the affected helper. An extra pre-rename check alone does not close the race.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ss_trust:p1:F1`.

- `ss_trust:p1:F1`: confidence as filed: possible; suggested bucket as filed: high.
