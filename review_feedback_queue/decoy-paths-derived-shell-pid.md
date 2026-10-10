# Decoy paths derived from the shell PID

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A predictable temporary directory could redirect a test write through a symlink.

## Details

F322 — **possible** — `scripts/test-start-stack-cleanup.sh:79-86` — Decoy paths derived from the shell PID

The fixture derives its directory from the shell's process number, accepts a pre-existing directory, and uses a
truncating marker write. Another account able to prepare that directory could place a symlink to a file writable by the
runner, causing clobber through its authority. The substrate and exploit were not tested. Create a private unique
directory and refuse unowned existing state before writing without following symlinks.

## Evidence and triage context

- scripts/test-start-stack-cleanup.sh:79-81 derives /tmp/fh-e2e.decoy-$$ predictably; 85 accepts an already-existing
  directory with mkdir -p; 86 writes marker with ordinary truncating redirection and no ownership or no-follow check. A
  non-sticky attacker-owned directory containing the symlink is a distinct case from a symlink directly in sticky /tmp.
  SPEC.md:2150-2164 excludes deliberate same-account interference, not interference by another Unix account. No matching
  queue, Planned, BUGS or ledger coverage was found. Preconditions include another local account able to prepare the
  predicted directory, a writable target and successful earlier fixture setup; no exploit was run.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_10_sec:p1:C6`.

- `automation_website_10_sec:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
