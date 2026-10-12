# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

The collected correctness and security review findings refer to commit `bd8d5d76439f8f36bbc2637b44d7ba4026b65675`. That
review stopped with known coverage gaps; the collection is not a whole-repository pass. Later product changes have not
been used to reassess these findings. Each file retains its reviewed commit, confidence, caveats, and provenance for
triage.

## Highest priority: security or data loss

None open.

## High priority: material UX degradation

- `uninstall-forgotten-success.md` — A failed uninstall can be reported as successful after another client forgets the
  host.

- `uninstall-race-first-time-service-setup.md` — Concurrent setup and uninstall can leave services pointing at a deleted
  executable.

## Other: correctness, diagnostics, cleanup, or convenience

- `template-discovery-fails-permanently-once-reply-exceeds.md` — A large valid template catalog permanently disables
  agent discovery.
