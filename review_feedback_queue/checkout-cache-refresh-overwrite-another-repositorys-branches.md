# Checkout cache refresh can overwrite another repository’s branches

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Checkout preparation can force-update branches in another repository.

## Details

F83 — **definite** — `crates/farhelm-supervisor/src/launch.rs:1515` — Checkout cache refresh can overwrite another
repository’s branches

Git children used to refresh the checkout cache inherit repository-selection overrides. Such an override can redirect
forced ref updates and pruning away from the cache into an unrelated repository, making branch tips inaccessible through
their original references. The cache lock then guards the wrong resource. Remove repository-context overrides from
preparation children while retaining ordinary Git configuration and credential support.

## Evidence and triage context

- launch.rs:649-654 invokes the user's interactive login shell; :996-1035 constructs preparation children without
  removing GIT_DIR. :1515-1533 performs git -C <cache> fetch --prune with forced heads and tags refspecs. An absolute
  inherited GIT_DIR controls Git's selected repository despite -C. The retained probe uses this exact fetch shape
  against an unrelated repository.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:1386-1396 requires the user's shell environment; SPEC_impl.md:1571-1592 describes cache refresh commands.
  Neither authorizes mutations of unrelated repositories. No exact Planned, BUGS, queue or ledger coverage found.

Caveats:

- Requires an absolute inherited GIT_DIR naming another repository.
- The retained reproduction updated a non-checked-out branch; Git protections can refuse some checked-out-branch cases.
- I inspected the retained script and result but ran no reproduction.
- Requires an absolute inherited GIT_DIR naming another repository. Git may refuse updates to a checked-out branch;
  other branch/tag refs remain exposed. I inspected the retained probe but ran no reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ss_edges:p1:F1`.

- `ss_edges:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
