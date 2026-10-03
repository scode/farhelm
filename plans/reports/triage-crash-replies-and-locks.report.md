## What this was about

If a supervisor's internal Stop or Restart task panicked, the request received no reply. The UI or agent command waited
until the connection dropped. Separately, replacing a failed host connection worker gave it a new session-cache write
lock, allowing it to write while another operation still held the old lock. No concrete Stop/Restart panic was
identified; the fix covers that failure boundary.

Two older findings concerned Linux session cleanup when a systemd scope could not be confirmed stopped. Triage had
already deferred those to the TODO entry requiring a working systemd user manager with no fallback. Their PRs only
remove the resolved review-queue records; they do not fix those cleanup behaviors.

## Things you should know

Stop and Restart now answer internal task failures with an error. Restart explicitly says its outcome is unknown,
because the old agent might have stopped or a replacement might have started. Tests inject a panic through the real
request dispatch using a narrow test-only fault hook; production builds retain their existing task ownership and
shutdown behavior.

A host keeps one session-cache write lock across connection-worker replacement. Its lock is removed when the host is
deleted. Provisioning retains its separate lock, so a long installation does not block session write-backs. The original
finding predated #1165 and #1167, which already prevented host edits or removal during provisioning; this fixes the
remaining cache-write boundary.

The stack was rebased onto main after the host dialogs and shared confirmation preferences landed. Those changes
preserve the removal endpoint and do not change the supervisor task or cache-lock contracts. All four PRs remain drafts.

## Open questions and possible follow-ups

No decision is needed for these four outcomes. The deferred systemd cleanup behavior remains wanted work in TODO.md;
approving this plan does not implement it. After an internal Restart failure, the user still needs to check whether the
agent is running; the error deliberately makes no success or rollback promise.

## The PRs

- [#1508](https://github.com/scode/farhelm/pull/1508/changes): remove the stale-scope deletion finding from the review
  queue; its implementation remains deferred.
- [#1511](https://github.com/scode/farhelm/pull/1511/changes): remove the failed-create rollback finding from the review
  queue; its implementation remains deferred.
- [#1512](https://github.com/scode/farhelm/pull/1512/changes): return errors when Stop or Restart tasks panic,
  preserving uncertainty for Restart.
- [#1515](https://github.com/scode/farhelm/pull/1515/changes): preserve session-cache write serialization across
  host-worker replacement.

## Checks run, reused and skipped

- Ran `cargo fmt --all -- --check`, targeted `dprint check`, the isolated test-delay checker (274 delays, zero without
  rationale), and `python3 releasing/check-changelog.py format`: all passed. These inspect the files and fragments
  changed by the stack.
- Ran `cargo clippy -p farhelm-helm --all-targets -- -D warnings` after the final cache-lock correction and rebase:
  passed.
- Ran six focused helm tests through the recorder with pinned nextest and tmux: cache and provisioning locks across
  replacement, provisioning-lock lifetime, removal cleanup, removal racing Retry, and connection retirement. All six
  passed after the final correction and rebase; recorder run `d2832d88-4b47-4563-bb0b-d6900cc95c5d`, nextest run
  `a14c838e-cb78-4e87-82a2-3d92602f617e`. This includes the actual REST removal path, not just a cleanup helper.
- Reused the two passing Stop/Restart panic dispatch tests from revision `1e21aa85`, recorder run
  `07735fef-56ac-4a75-b8ca-fb4881f5500c`, nextest run `a7be5a3a-c7fd-4e45-8f6a-052a8494884a`, and the supervisor Clippy
  pass. Later supervisor edits only clarified constructor documentation; upstream changes and the cache fix do not alter
  the tested supervisor code or its dependencies.
- An earlier Restart test failed because its initial fault injection hit an inner task whose panic was already caught,
  so it did not exercise the unanswered outer task. Evidence was retained as run `aef1fb5a-6de2-4088-9fc5-86c3139d757b`;
  the corrected dispatch test uses the intended boundary. This was a same-session test-design failure, not a latent
  flake.
- Skipped the full Rust and browser batteries, desktop runtime, installer and release checks: this stack changes no
  browser, desktop, installation or release behavior, and the focused dispatch and lock tests cover the changed failure
  boundaries. The first two PRs are Markdown-only and needed no runtime tests.

## Review gate's outcome

Fresh-context Opus 5.5 reviews at high effort passed both code PRs. The final review also reassessed the Stop/Restart
test seam after its corrective rounds and found no unnecessary mechanism. Review corrections added real-dispatch
coverage and unknown-outcome wording, removed duplicate cache-lock pruning, tested the actual removal path, and
clarified lifecycle and lock-order documentation. No unresolved correctness finding remains. The optional suggestion to
rearrange two equivalent rollback checks was left unchanged; both checks express the successful-deletion condition
directly.

The two queue-cleanup PRs intentionally had no code review gate, as agreed in the plan. Commit/PR wording passed cold
reads. The report itself was independently cold-read before delivery.
