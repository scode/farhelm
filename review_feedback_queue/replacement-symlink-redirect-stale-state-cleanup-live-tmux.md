# a replacement symlink can redirect stale-state cleanup at a live tmux server

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A replaced test-state path could redirect cleanup to another live tmux server.

## Details

F58 — **possible** — `crates/farhelm-teststate/src/lib.rs:544` — a replacement symlink can redirect stale-state cleanup
at a live tmux server

Stale-state cleanup verifies a directory's ownership but later discovers and contacts sockets through its pathname.
Concurrent lockless removal can free that name for another account to replace after the check, potentially directing the
sweeper's `kill-server` authority at unrelated sessions. Replacement timing, socket access, and platform symlink
protections remain unverified premises. Bind discovery and connection to verified directory identity using non-following
directory handles or an ownership-safe private claiming protocol. This concerns test infrastructure.

## Evidence and triage context

- crates/farhelm-teststate/src/lib.rs:65 fixes the shared root at /tmp. Lines 239–255 check directory type and owner by
  pathname; lines 283–287 admit sufficiently old lockless candidates without a serialization lock.
- crates/farhelm-teststate/src/lib.rs:455–460 rechecks type and owner, then separately calls socket discovery. Another
  sweep can remove the checked directory during that gap.
- crates/farhelm-teststate/src/lib.rs:544 opens the directory by pathname. Lines 560–574 inspect and retain socket
  pathnames; symlink_metadata protects the final component at that instant, not replaced ancestors or subsequent
  resolution.
- crates/farhelm-teststate/src/lib.rs:469 and 493–506 invoke tmux -S <discovered-path> kill-server without revalidating
  directory identity. Environment clearing limits credential disclosure but does not constrain which server receives the
  destructive command.
- crates/farhelm-teststate/src/lib.rs:640–652 invokes the sweep once per test process.
  crates/farhelm-fixtures/src/main.rs:157–162 exposes the same sweep to e2e/start-stack.sh:121. Concurrent processes
  therefore have actual independent entry points.
- crates/farhelm-supervisor/src/tmux.rs:1954 names live product sockets tmux.sock, satisfying the collector's filename
  requirement if redirected into their state directory.
- crates/farhelm-teststate/src/tmux.rs:495–508 uses an opened directory and fchdir for a separate teardown
  implementation. The stale-state sweep calls its own pathname-based kill_tmux_server and does not receive that
  protection.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- {"basis": "SPEC_impl.md:2231–2237", "comparison": "The accepted race requires write access beneath the user's private
  home. This finding instead requires another account to claim a freed name beneath shared /tmp. The specification
  expressly calls for reconsideration with a writable shared parent."}
- {"basis": "SPEC.md:2150–2164; TRIAGE_OUTCOMES.md:6751–6769, installer-malformed-record.md", "comparison": "These
  exclude deliberate same-account interference. The finding concerns a different account redirecting a
  privileged-in-relation-to-that-account cleanup operation; the actor and trust boundary differ."}
- {"basis": "crates/farhelm-teststate/src/lib.rs:169–180", "comparison": "The comment accepts residual pathname
  replacement based on safe unlinking. That rationale does not address kill-server issued before unlinking against a
  followed replacement."}
- {"basis": "BUGS.md:8–49; review_feedback_queue/shutdown-expiry.md:14–24", "comparison": "These concern tmux crashes
  caused by abrupt or insufficiently drained supervisor shutdown. An explicit kill-server redirected through stale-test
  cleanup has a different trigger and mechanism."}
- {"basis": "TODO.md:33–43; review_feedback_queue/snapshot-root.md:14–24;
  review_feedback_queue/installer-startup-prune.md:14–23", "comparison": "The Planned item concerns connection-loop
  scheduling. The queue items concern snapshot ownership and version pruning. None matches the trigger, destructive
  consequence, and cleanup scope here."}

Caveats:

- No runtime reproduction was performed.
- The reported interleaving requires an eligible lockless directory, concurrent removal, a replacement after the final
  ownership check, an accessible target socket, and remaining kill budget.
- Platform protections against following foreign symlinks in sticky directories, including Linux protected_symlinks when
  enabled, can block the specific top-level-symlink sequence. The source does not establish equivalent protection across
  all supported platforms.
- This is test-infrastructure exposure on a machine running the harness, not an assertion that the shipped application
  invokes this sweep.
- Queue filters exclude security consequences, destructive wrong-target actions, and user-process loss; rarity alone
  cannot justify dropping this finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_01_cor:p1:F1`.

- `test_infrastructure_01_cor:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
