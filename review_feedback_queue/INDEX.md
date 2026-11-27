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

- [`trailing-claude-trust-survives-archive-on-older-supervisors.md`](trailing-claude-trust-survives-archive-on-older-supervisors.md)
  — With an updated helm and a not-yet-updated supervisor, Claude trust added on a fresh managed checkout's first launch
  is never recorded for removal, so it outlives archive and a later checkout at the reused `repo-N` path (possibly
  another owner's same-named repository) starts trusted.

- [`trailing-agent-trust-switch-acts-on-unshown-state.md`](trailing-agent-trust-switch-acts-on-unshown-state.md) — After
  the helm refuses an agent trust save, a Space press in the moment before the reverted switch is drawn can turn
  workspace trust on against the state the user saw; be7cfc065c bound only the hook switch to its shown state.

- [`trailing-bulk-replace-before-first-report-drained.md`](trailing-bulk-replace-before-first-report-drained.md) — A
  marked-bar Restart/Replace pressed within about a second (up to two with the report watcher off) of an agent's first
  report and exit, or reaching a queued row hidden from the list, starts over a session that could have resumed; bucket
  uncertain, SPEC.md's sub-second rule may cover part.

## High priority: material UX degradation

- [`trailing-recent-setup-wrapper-prefix-elided.md`](trailing-recent-setup-wrapper-prefix-elided.md) — Two Recent setups
  that differ only in the cut-off part of their wrapper prefix (for example a sandboxing wrapper) look identical to a
  keyboard user, so Enter can launch through an unintended wrapper.

- [`trailing-grok-resume-after-custom-double-dash.md`](trailing-grok-resume-after-custom-double-dash.md) — For a Grok
  session whose Custom permission words end in `-- <words>`, Restart and named starts place `--resume <id>` after the
  tail, so Grok reads it as prompt text and starts a fresh conversation instead of resuming.

## Other: correctness, diagnostics, cleanup, or convenience

- [`trailing-custom-agents-same-name-recents-indistinguishable.md`](trailing-custom-agents-same-name-recents-indistinguishable.md)
  — If two registered custom agents share a display name, their Recent setups in New show the same agent name, so the
  user cannot tell from the row which program Enter will run.

- [`trailing-withdrawn-start-row-enter-silent.md`](trailing-withdrawn-start-row-enter-silent.md) — An Enter on a
  highlighted recent setup or template in the launcher's side column, pressed as the window narrows and hides the
  column, does nothing and says nothing; the next Enter launches (or in Replace with, replaces and deletes the source
  with) the unfilled draft.

- [`trailing-permanent-yolo-saved-setting-unreported-after-close.md`](trailing-permanent-yolo-saved-setting-unreported-after-close.md)
  — If the user edits a field while "Start, and don't ask again on this host" is still writing the host setting, the
  launcher can close and the saved host-wide YOLO setting is never reported.

- [`trailing-stale-yolo-answer-freezes-template-refusal.md`](trailing-stale-yolo-answer-freezes-template-refusal.md) —
  Answering the YOLO question with "don't ask again" while Save as template is open shows "Finish or cancel saving the
  template", and that note stays after the panel closes until the next Launch press.

- [`trailing-settings-search-selection-reset-on-registration-push.md`](trailing-settings-search-selection-reset-on-registration-push.md)
  — While a Settings search is open, any custom agent registration change (from another window, the CLI or an agent)
  moves the arrowed-to result back to the first one.

- [`trailing-close-question-discard-silent-noop.md`](trailing-close-question-discard-silent-noop.md) — If a draft
  changes while "Save changes before closing?" is open, Discard neither closes Settings nor explains why.

- [`trailing-close-save-closes-over-inline-delete-question.md`](trailing-close-save-closes-over-inline-delete-question.md)
  — When a close-save finishes after the user opened an inline Delete question for a leftover Tensorlake sandbox or
  Podman container, Settings closes and drops the unanswered question.

- [`trailing-agent-docs-trust-scope-installed-hosts.md`](trailing-agent-docs-trust-scope-installed-hosts.md) — The
  Codex, Antigravity, Pi and other agent docs pages still say the trust switches apply "on installed hosts", the wording
  571096872e replaced in Settings and SPEC.md; they also cover persistent hosts created with Add host.

- [`trailing-antigravity-initial-prompt-after-custom-double-dash.md`](trailing-antigravity-initial-prompt-after-custom-double-dash.md)
  — An Antigravity launch whose Custom permission words end in `-- <operands>` and that carries an initial prompt passes
  the prompt after the `--`, so agy treats it as an operand.

- [`trailing-hook-log-parser-drops-identity-pair.md`](trailing-hook-log-parser-drops-identity-pair.md) — Test tooling
  only: the new hook-log reader misreads two kinds of hook line that do name a conversation, and a test now asserts that
  such a line names none.

- [`trailing-screen-fixture-publication-unrecoverable-after-upgrade.md`](trailing-screen-fixture-publication-unrecoverable-after-upgrade.md)
  — Maintainer tooling: if a screen capture is killed mid-publication and the agent is upgraded before the rerun, the
  capture tool never repairs the half-written version directory.

- [`trailing-install-lock-holdable-by-other-accounts.md`](trailing-install-lock-holdable-by-other-accounts.md) — On a
  Mac, another account that can read `~/Applications` can hold the installation lock (the folder flock, or the
  world-openable fallback lock file older releases left behind), so installs, updates and uninstall refuse as busy.

- [`trailing-pinned-uninstall-test-cannot-tell-pins.md`](trailing-pinned-uninstall-test-cannot-tell-pins.md) — Test gap:
  the test meant to prove a confirmed Tailscale Uninstall signs in on the connection it just verified would still pass
  if it used the planning-time connection.

- [`trailing-recipe-launch-combined-budget-checked-after-billing.md`](trailing-recipe-launch-combined-budget-checked-after-billing.md)
  — A new sandbox or container launch whose title, initial prompt and command together exceed 64 KiB passes every helm
  check, so the sandbox is created and billed before the supervisor refuses the launch.

- [`trailing-agent-clone-log-comment-cites-removed-note.md`](trailing-agent-clone-log-comment-cites-removed-note.md) —
  Comment accuracy: the clone dispatch's log comment points to a create note that 443eac14b0 removed, and the two sites
  now log the asking session differently.

- [`trailing-permanent-yolo-answer-suspended-host-silent-close.md`](trailing-permanent-yolo-answer-suspended-host-silent-close.md)
  — When a sandbox or container host suspends while the YOLO confirmation is open, "Start, and don't ask again on this
  host" closes the question without saving the setting or launching, and says the launch changed.

- [`trailing-quick-switcher-hint-strip-screen-reader-fragments.md`](trailing-quick-switcher-hint-strip-screen-reader-fragments.md)
  — Screen reader users opening the quick switcher (Cmd+K) or template list (Cmd+J) hear the empty-field hints as loose
  pieces rather than labelled hints.

- [`trailing-rename-dialog-no-escaped-title-view.md`](trailing-rename-dialog-no-escaped-title-view.md) — Since
  fb80d7dd61 the Rename dialog shows a session title only in the raw editable field, so zero-width or direction-changing
  characters an agent or the CLI put in it are no longer visible there.

- [`trailing-welcome-dialog-advertises-inert-cmd-n.md`](trailing-welcome-dialog-advertises-inert-cmd-n.md) — Help >
  Welcome shows a ⌘N key hint beside Start a session, but ⌘N is inert while the dialog is open.

- [`trailing-stop-and-suspend-promises-closing-absent-tabs.md`](trailing-stop-and-suspend-promises-closing-absent-tabs.md)
  — For a running sandbox or container session with no terminal tabs, the row menu's Stop and suspend item, its hover
  help and its confirmation still say it will close terminal tabs.

- [`trailing-template-editor-controls-edit-live-editor.md`](trailing-template-editor-controls-edit-live-editor.md) — In
  the Templates dialog, a click on the old editor's controls just after another template was opened applies to the newly
  opened template; 446755516c fixed only Save and Duplicate.

- [`trailing-records-tool-git-version-floor.md`](trailing-records-tool-git-version-floor.md) — The records tool needs
  git 2.41 for `records backup` (`fetch --porcelain`) and git 2.31/2.32 for its isolation and token header, and nothing
  checks the version; on older git backup fails every run and path/push lose their isolation.

- [`trailing-records-tool-honors-ssh-askpass.md`](trailing-records-tool-honors-ssh-askpass.md) — Where the environment
  sets SSH_ASKPASS with a display, a rejected records token makes `records path`/`push` open a password dialog and wait,
  and the credential helper's clone test can hang the same way.

- [`trailing-records-backup-false-rewritten-history-alarm.md`](trailing-records-backup-false-rewritten-history-alarm.md)
  — `records backup` treats any `!` line from `git fetch --porcelain` as a refused rewrite, so a held lock, stale
  `.lock` file or full disk raises a false "rewritten history" alarm.

- [`trailing-records-sync-kill-hides-recreated-log.md`](trailing-records-sync-kill-hides-recreated-log.md) — If the
  records tool is killed in a narrow window of a sync, a record file the agent had re-created disappears from the slot's
  working copy and is never restored (it stays under `.git`).

- [`trailing-records-config-parse-error-bypasses-redaction.md`](trailing-records-config-parse-error-bypasses-redaction.md)
  — A small mistake in the records config makes the tool's error quote the offending line, including the repository URL
  or Cloudflare account id it promises never to print into agent logs.

- [`trailing-records-path-follows-symlinked-record-dir.md`](trailing-records-path-follows-symlinked-record-dir.md) — If
  a writer of the shared records repository replaces a record directory with a symlink, every other host that opens that
  record is told to write its log wherever the link points.

- [`trailing-records-pattern-name-test-blind-to-sibling.md`](trailing-records-pattern-name-test-blind-to-sibling.md) —
  Test gap: the test for note names such as `[a]*.md` cannot detect a pattern matching a sibling note, and its `:` case
  is never leading.

- [`trailing-wayland-submit-overtakes-field-paste.md`](trailing-wayland-submit-overtakes-field-paste.md) — On Wayland,
  Enter right after a paste can submit a text field before the pasted text arrives; the SSH sign-in prompt then sends an
  empty or partial password.

- [`trailing-tab-reap-first-sweep-failure-no-backoff.md`](trailing-tab-reap-first-sweep-failure-no-backoff.md) — An
  exited terminal tab whose first cleanup sweep keeps failing is retried every two seconds with no backoff.

- [`trailing-kept-failed-close-tab-listed-live.md`](trailing-kept-failed-close-tab-listed-live.md) — A closed tab whose
  host was ended by signal and whose sweep then failed stays in the tab strip looking live until a retry succeeds.

- [`trailing-pointer-menu-does-not-follow-row.md`](trailing-pointer-menu-does-not-follow-row.md) — A session menu opened
  with a right press stays where the press was when its row moves, unlike the ⋯ menu and SPEC.md.

- [`trailing-marked-menu-close-test-no-stays-open-control.md`](trailing-marked-menu-close-test-no-stays-open-control.md)
  — Test gap: the marked right-press menu's close test would pass if the menu closed on every helm update.

- [`trailing-shift-f10-marked-row-opens-combined-menu.md`](trailing-shift-f10-marked-row-opens-combined-menu.md) —
  Shift+F10 on a marked row opens the combined marked menu, while SPEC.md and the code's overview say it opens the row's
  own menu.

- [`trailing-marked-menu-metadata-steals-terminal-focus.md`](trailing-marked-menu-metadata-steals-terminal-focus.md) —
  Pin, Mark read, Mute or Move to group from the right-press marked menu moves keyboard focus off the terminal to the
  list's sort title.

- [`trailing-existing-environment-checkout-note-dead.md`](trailing-existing-environment-checkout-note-dead.md) — Spec
  and dead code: no launcher path can show the existing-environment checkout note any more, but SPEC.md still promises
  it.
