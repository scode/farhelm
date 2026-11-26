# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

Collection record for the abbreviated review through `00de0b6c`:
[complete findings, candidates, withdrawals, leads and returned proposals](records/review-through-00de0b6c/README.md).
This record is bookkeeping, not an additional feedback item. Provisional buckets preserve unresolved stronger
consequences.

## Highest priority: security or data loss

## High priority: material UX degradation

- [`trailing-replace-with-recent-named-fill-enter-replaces.md`](trailing-replace-with-recent-named-fill-enter-replaces.md)
  — In Replace with, a screen reader announces a highlighted recent setup as "Fill from recent setup" while Enter on it
  replaces and deletes the source session; highest if the "⏎ Replace" key cap is not announced with it.

- [`trailing-auto-trust-overrides-custom-flag-refusal.md`](trailing-auto-trust-overrides-custom-flag-refusal.md) — In a
  session's own sandbox, Farhelm's automatic trust flag follows a user's Custom Pi `--no-approve` or Codex untrusted
  override and may win (vendor flag precedence unverified; highest if it does).

## Other: correctness, diagnostics, cleanup, or convenience

- [`trailing-drift-copyback-runs-while-agent-may-live.md`](trailing-drift-copyback-runs-while-agent-may-live.md) — A
  Grok or Antigravity drift case copies refreshed vendor files back once its terminal hosts end, even when the agent may
  still be running (failed Delete sweep, failed create, detached process), so a later vendor save such as a refreshed
  login can be lost without a note.

- [`trailing-drift-runner-drops-requested-harness.md`](trailing-drift-runner-drops-requested-harness.md) — The drift
  runner silently skips an explicitly requested agent that has no case in the chosen category and can exit successfully
  without checking it.

- [`trailing-drift-panic-skips-copyback.md`](trailing-drift-panic-skips-copyback.md) — A panic inside a Grok or
  Antigravity drift case skips copying a refreshed vendor login back to the maintainer's real config, with no note
  (pre-existing).

- [`trailing-drift-failed-copyback-passes-case.md`](trailing-drift-failed-copyback-passes-case.md) — A failed copy-back
  of a refreshed vendor login is only a console note and the drift case still reports passed, so the maintainer may be
  signed out unnoticed.

- [`trailing-drift-cursor-initial-prompt-never-passes.md`](trailing-drift-cursor-initial-prompt-never-passes.md) —
  Cursor's launch-prompt drift case always ends "could not run" even when Cursor accepts the prompt, so the check can
  never pass.

- [`trailing-drift-capture-timing-check-too-coarse.md`](trailing-drift-capture-timing-check-too-coarse.md) — The
  capture-timing drift check treats prompt-timed and reply-timed agents alike, so a vendor drift between "at the first
  prompt" and "after the first reply" would go unnoticed.

- [`trailing-brick-cleanup-refuses-untested-remote-mac-pair.md`](trailing-brick-cleanup-refuses-untested-remote-mac-pair.md)
  — After a stop-after-P0 request, brick-test cleanup refuses a prepared but untested remote Mac pair and demands a
  retention decision for machines with no evidence.

- [`trailing-drift-claude-trust-real-config-byte-check.md`](trailing-drift-claude-trust-real-config-byte-check.md) — The
  Claude trust drift case fails whenever another Claude session rewrites the real ~/.claude.json during the case,
  reading as if Farhelm wrote it (frequency unmeasured).

- [`trailing-drift-claude-trust-ancestor-vacuous-pass.md`](trailing-drift-claude-trust-ancestor-vacuous-pass.md) — The
  Claude trust drift case can pass without proving Claude honours Farhelm's entry if the maintainer's config already
  trusts /tmp or /.

- [`trailing-launcher-new-clone-enter-after-vanished-pick.md`](trailing-launcher-new-clone-enter-after-vanished-pick.md)
  — In New and Clone, Return on a highlighted template or folder row can launch instead of picking when the highlight
  vanished before the redraw; only Replace with is guarded.

- [`trailing-launcher-held-escape-discards-draft.md`](trailing-launcher-held-escape-discards-draft.md) — Holding Escape
  with typed results or the Save as template panel open can close the whole launcher and discard its draft; the fix
  covered only the highlight case.

- [`trailing-launcher-stale-permanent-yolo-answer-silent.md`](trailing-launcher-stale-permanent-yolo-answer-silent.md) —
  A stale "Start, and don't ask again on this host" answer closes the YOLO question with nothing started and no reason,
  sometimes after the host setting was already saved.

- [`trailing-trust-hint-late-refusal-untested.md`](trailing-trust-hint-late-refusal-untested.md) — No test covers a
  trust hint save refusal that lands while the hint is already dismissed.

- [`trailing-unread-hook-and-spare-switches-read-as-off.md`](trailing-unread-hook-and-spare-switches-read-as-off.md) —
  The hook setup and spare-checkout switches read as a saved "off" to screen readers while their setting is unread.

- [`trailing-stored-trust-chip-credits-settings-switch.md`](trailing-stored-trust-chip-credits-settings-switch.md) —
  Restart and prompt approval cards explain a session's stored trust by a Settings switch that may now be off.

- [`trailing-switch-warning-description-untested.md`](trailing-switch-warning-description-untested.md) — No test checks
  that the trust and hook switches carry their warning for screen readers.

- [`trailing-settings-trust-help-misdescribes-persistent-hosts.md`](trailing-settings-trust-help-misdescribes-persistent-hosts.md)
  — Settings trust help says sandbox and container sessions trust automatically, but persistent recipe hosts follow the
  switch.

- [`trailing-disposable-trust-hint-says-off-by-default.md`](trailing-disposable-trust-hint-says-off-by-default.md) — In
  a session's own sandbox, the trust hint says trust is off by default and points to a switch that does not apply there.

- [`trailing-repository-history-failure-shown-as-no-matches.md`](trailing-repository-history-failure-shown-as-no-matches.md)
  — A failed repository-history read shows "No repository matches" and is not retried while the launcher is open.
