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

- [`trailing-legacy-claude-settings-v036-hosts-never-cleaned.md`](trailing-legacy-claude-settings-v036-hosts-never-cleaned.md)
  — A Tensorlake host set up by 0.36.0 and not connected under 0.37-0.43 keeps Farhelm's old Claude endpoint URL and
  token in Claude's settings for good; per SPEC_impl.md they can take precedence and receive a new token (Claude Code
  precedence unverified).

- [`trailing-legacy-codex-config-v036-hosts-never-cleaned.md`](trailing-legacy-codex-config-v036-hosts-never-cleaned.md)
  — The same 0.36.0-era Tensorlake hosts keep Farhelm's old Codex endpoint and token in Codex's config, so a newly
  selected login or endpoint may not take effect, and per SPEC_impl.md the old token can reach a new tokenless endpoint
  (Codex merge behavior unverified).

## High priority: material UX degradation

- [`trailing-replace-with-recent-named-fill-enter-replaces.md`](trailing-replace-with-recent-named-fill-enter-replaces.md)
  — In Replace with, a screen reader announces a highlighted recent setup as "Fill from recent setup" while Enter on it
  replaces and deletes the source session; highest if the "⏎ Replace" key cap is not announced with it.

- [`trailing-auto-trust-overrides-custom-flag-refusal.md`](trailing-auto-trust-overrides-custom-flag-refusal.md) — In a
  session's own sandbox, Farhelm's automatic trust flag follows a user's Custom Pi `--no-approve` or Codex untrusted
  override and may win (vendor flag precedence unverified; highest if it does).

## Other: correctness, diagnostics, cleanup, or convenience

- [`shortcuts-chord-during-composition.md`](shortcuts-chord-during-composition.md) — The Keyboard shortcuts chord can
  interrupt an input method if native handling clears its mark first or the action cannot query the focused field.

- [`provisioning-orphan-sweep-deletes-concurrent-helm-upload.md`](provisioning-orphan-sweep-deletes-concurrent-helm-upload.md)
  — Two helms provisioning the same host concurrently can delete each other's active staging temporary via the orphan
  sweep (no liveness check); the victim run fails visibly and retryably (cross-helm premise unverified).

- [`release-payload-cache-misses-private-dir-hardening.md`](release-payload-cache-misses-private-dir-hardening.md) — The
  release payload cache is created umask-dependent without symlink refusal, unlike the deliberately hardened sibling
  directory cache; protected today only by the 0700 state dir (hardening consistency).

- [`podman-settle-claim-incumbent-state-non-atomic.md`](podman-settle-claim-incumbent-state-non-atomic.md) — A place
  claim transfer reads the incumbent's live identity refusal non-atomically; if it resolves to Connected in the window,
  the claim still moves (benign routing shift between twins on the same machine; resource guards hold).

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

- [`trailing-release-notes-recapture-parks-on-tooltip-field.md`](trailing-release-notes-recapture-parks-on-tooltip-field.md)
  — Recapturing the v0.42.0 Antigravity release-notes picture parks the pointer on the Conversation field, so its
  tooltip would be drawn into the picture (from source; not seen in a capture).

- [`trailing-brick-cleanup-refuses-untested-remote-mac-pair.md`](trailing-brick-cleanup-refuses-untested-remote-mac-pair.md)
  — After a stop-after-P0 request, brick-test cleanup refuses a prepared but untested remote Mac pair and demands a
  retention decision for machines with no evidence.

- [`trailing-spec-file-click-release-wording.md`](trailing-spec-file-click-release-wording.md) — SPEC.md says a file
  click must be pressed and released over the painted card, but the press closes the card and the release is accepted
  without it; SPEC_impl.md has it right.

- [`trailing-grid-wide-glyph-absorbs-foreign-spacer.md`](trailing-grid-wide-glyph-absorbs-foreign-spacer.md) — After a
  program deletes characters from the middle of a wide character, one cell can show the wrong background color
  (pre-existing; cosmetic).

- [`trailing-tab-shell-hangup-delete-wiring-untested.md`](trailing-tab-shell-hangup-delete-wiring-untested.md) — No test
  would notice if Delete stopped giving macOS tab shells their hangup, which would again lose their shell history.

- [`trailing-drift-claude-trust-real-config-byte-check.md`](trailing-drift-claude-trust-real-config-byte-check.md) — The
  Claude trust drift case fails whenever another Claude session rewrites the real ~/.claude.json during the case,
  reading as if Farhelm wrote it (frequency unmeasured).

- [`trailing-drift-claude-trust-ancestor-vacuous-pass.md`](trailing-drift-claude-trust-ancestor-vacuous-pass.md) — The
  Claude trust drift case can pass without proving Claude honours Farhelm's entry if the maintainer's config already
  trusts /tmp or /.

- [`trailing-launcher-new-clone-enter-after-vanished-pick.md`](trailing-launcher-new-clone-enter-after-vanished-pick.md)
  — In New and Clone, Return on a highlighted template or folder row can launch instead of picking when the highlight
  vanished before the redraw; only Replace with is guarded.

- [`trailing-spec-impl-hook-answer-preview-wait.md`](trailing-spec-impl-hook-answer-preview-wait.md) — SPEC_impl.md says
  a hook-setup answer waits for a checkout preview, but the launch is refused instead and the user has to press Launch
  again.

- [`trailing-launcher-held-escape-discards-draft.md`](trailing-launcher-held-escape-discards-draft.md) — Holding Escape
  with typed results or the Save as template panel open can close the whole launcher and discard its draft; the fix
  covered only the highlight case.

- [`trailing-launcher-stale-permanent-yolo-answer-silent.md`](trailing-launcher-stale-permanent-yolo-answer-silent.md) —
  A stale "Start, and don't ask again on this host" answer closes the YOLO question with nothing started and no reason,
  sometimes after the host setting was already saved.

- [`trailing-spec-impl-custom-agent-id-letter.md`](trailing-spec-impl-custom-agent-id-letter.md) — SPEC_impl.md says
  registered custom agent IDs start with a letter, but they may start with a digit.

- [`trailing-hook-hint-keeps-stale-save-error.md`](trailing-hook-hint-keeps-stale-save-error.md) — The hook setup hint
  brings back an old Don't show again error after a later dismissal and re-enable (previously triaged as no change under
  the cheap-fix rule).

- [`trailing-trust-hint-hide-survives-reconnect.md`](trailing-trust-hint-hide-survives-reconnect.md) — A trust hint
  hidden with × can stay hidden for a new trust question that began while the app was disconnected (premise unverified).

- [`trailing-trust-hint-late-refusal-untested.md`](trailing-trust-hint-late-refusal-untested.md) — No test covers a
  trust hint save refusal that lands while the hint is already dismissed.

- [`trailing-unread-hook-and-spare-switches-read-as-off.md`](trailing-unread-hook-and-spare-switches-read-as-off.md) —
  The hook setup and spare-checkout switches read as a saved "off" to screen readers while their setting is unread.

- [`trailing-stored-trust-chip-credits-settings-switch.md`](trailing-stored-trust-chip-credits-settings-switch.md) —
  Restart and prompt approval cards explain a session's stored trust by a Settings switch that may now be off.

- [`trailing-switch-warning-description-untested.md`](trailing-switch-warning-description-untested.md) — No test checks
  that the trust and hook switches carry their warning for screen readers.

- [`trailing-relative-claude-lock-in-old-cleanup-records.md`](trailing-relative-claude-lock-in-old-cleanup-records.md) —
  Claude trust cleanup records written with a relative `CLAUDE_CONFIG_DIR` before the fix keep a relative lock path
  (likely unpopulated).

- [`trailing-settings-trust-help-misdescribes-persistent-hosts.md`](trailing-settings-trust-help-misdescribes-persistent-hosts.md)
  — Settings trust help says sandbox and container sessions trust automatically, but persistent recipe hosts follow the
  switch.

- [`trailing-disposable-trust-hint-says-off-by-default.md`](trailing-disposable-trust-hint-says-off-by-default.md) — In
  a session's own sandbox, the trust hint says trust is off by default and points to a switch that does not apply there.

- [`trailing-stale-references-to-removed-repository-search.md`](trailing-stale-references-to-removed-repository-search.md)
  — SPEC.md, code comments and a test docstring still describe the removed repository search.

- [`trailing-replace-in-environment-history-check-vacuous.md`](trailing-replace-in-environment-history-check-vacuous.md)
  — The replace-in-environment test's check that host history is unchanged can no longer fail.

- [`trailing-session-owned-setup-flag-untested.md`](trailing-session-owned-setup-flag-untested.md) — No helm test still
  checks that sandboxes and containers are never offered checkout-folder setup.

- [`trailing-repository-history-failure-shown-as-no-matches.md`](trailing-repository-history-failure-shown-as-no-matches.md)
  — A failed repository-history read shows "No repository matches" and is not retried while the launcher is open.

- [`trailing-symlinked-claude-json-permanent-warning.md`](trailing-symlinked-claude-json-permanent-warning.md) — A
  symlinked `~/.claude.json` leaves a permanent amber warning that misnames the cause.
