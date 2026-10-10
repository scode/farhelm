# Concurrent lockless sweeps can redirect `kill-server` outside test state

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Concurrent cleanup could redirect tmux shutdown beyond owned test state.

## Details

F65 — **possible** — `crates/farhelm-teststate/src/lib.rs:469` — Concurrent lockless sweeps can redirect `kill-server`
outside test state

Two lockless sweeps can both check the same directory before one removes it. A replacement pathname introduced afterward
can direct the other's later socket connection at an unrelated tmux server, using the cleanup account's authority. The
attack requires successful replacement timing, traversal, and an accessible target socket; none was reproduced. Claim
the directory into a private owned cleanup namespace and verify identity before both socket collection and connection,
including the lockless path.

## Evidence and triage context

- crates/farhelm-teststate/src/lib.rs:283-287 calls reap without acquiring a lock when the old directory has no lock
  file.
- crates/farhelm-teststate/src/lib.rs:455-469 checks directory ownership, collects socket pathnames, and later contacts
  those pathnames without preserving directory identity.
- crates/farhelm-teststate/src/lib.rs:471 removes the directory, allowing another account to claim the freed name before
  the second sweep connects.
- crates/farhelm-teststate/src/lib.rs:493-502 executes tmux -S <pathname> kill-server as the cleanup account.
- crates/farhelm-teststate/src/lib.rs:640-652 and e2e/start-stack.sh:121 provide independent process entry points;
  e2e/start-stack.sh:132-143 explicitly permits lockless stack directories.
- https://raw.githubusercontent.com/tmux/tmux/3.7c/client.c:112-126 copies the supplied path directly into sockaddr_un
  and calls connect; it does not reject a replacement symlink before connecting.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2150-2164 accepts deliberate same-account interference. It does not cover another account redirecting this
  account's cleanup command. FILTER.md:38-42 explicitly excludes wrong-target destruction and security consequences.

Caveats:

- Requires an eligible lockless directory, concurrent sweeps, and a successful replacement interleaving.
- The replacement directory must permit traversal and the target tmux socket must be reachable by the cleanup account.
- No exploit reproduction was performed; this is test infrastructure rather than shipped product code.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_01_sec:p2:F1`.

- `test_infrastructure_01_sec:p2:F1`: confidence as filed: possible; suggested bucket as filed: highest.
