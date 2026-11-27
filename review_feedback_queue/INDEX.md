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

- [`trailing-close-tab-shortcut-acts-on-unshown-tab.md`](trailing-close-tab-shortcut-acts-on-unshown-tab.md) — With Skip
  session confirmations on, Cmd+W (Ctrl+Shift+W on Linux) pressed just as the shown shell tab changes on its own (its
  shell exited, another client closed it) closes the newly shown tab at once, ending its processes.

- [`trailing-delete-guard-ends-unseen-tab.md`](trailing-delete-guard-ends-unseen-tab.md) — Deleting (or Replacing) a
  session whose row shows a running agent and no shell tabs authorizes ending everything, so a tab opened after the row
  was drawn is ended unasked, with or without the Delete question; SPEC.md says such a Delete should be refused.

- [`trailing-claude-fresh-start-keeps-old-capture.md`](trailing-claude-fresh-start-keeps-old-capture.md) — After Restart
  starts Claude fresh, Farhelm keeps the old never-saved conversation; if the new Claude cannot report its own, every
  later Restart silently starts yet another blank Claude and drops the conversation the user worked in.

## High priority: material UX degradation

- [`trailing-recent-setup-wrapper-prefix-elided.md`](trailing-recent-setup-wrapper-prefix-elided.md) — Two Recent setups
  that differ only in the cut-off part of their wrapper prefix (for example a sandboxing wrapper) look identical to a
  keyboard user, so Enter can launch through an unintended wrapper.

- [`trailing-grok-resume-after-custom-double-dash.md`](trailing-grok-resume-after-custom-double-dash.md) — For a Grok
  session whose Custom permission words end in `-- <words>`, Restart and named starts place `--resume <id>` after the
  tail, so Grok reads it as prompt text and starts a fresh conversation instead of resuming.

- [`trailing-copy-late-clipboard-overwrite.md`](trailing-copy-late-clipboard-overwrite.md) — A slow Shift-Cmd-click file
  copy writes the clipboard when its read finishes, over anything the user copied meanwhile, so the next paste is the
  file instead.

- [`trailing-custom-agent-footer-press-across-pages.md`](trailing-custom-agent-footer-press-across-pages.md) — In
  Settings, a press on one custom agent's Save or Discard, released after the keyboard switched to another agent's page,
  saves or discards that other agent's draft.

- [`trailing-owed-hint-try-send-starved.md`](trailing-owed-hint-try-send-starved.md) — Over a slow link kept full by a
  busy terminal, the supervisor's 'sessions changed' hint never wins a queue slot, so the host's session list in the
  helm can lag by up to the 30-second poll.

- [`trailing-activity-hold-marks-watched-unseen.md`](trailing-activity-hold-marks-watched-unseen.md) — Watching a
  working agent finish and switching away before it is recognized idle now leaves its row with the blue new-output dot,
  although the user saw every line.

- [`trailing-close-wait-reason-scrolled-off.md`](trailing-close-wait-reason-scrolled-off.md) — When Settings stays open
  because a close met a refused save or missed hosts, the page it shows can scroll to the top or bottom, leaving the
  reason off-screen, so Close seems to do nothing.

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

- [`trailing-copy-download-status-line-overwrite.md`](trailing-copy-download-status-line-overwrite.md) — A file copy's
  and a file save's status lines replace each other, so a copy failure can vanish when an earlier save finishes and a
  running save can lose its Downloading line.

- [`trailing-skip-confirmations-help-omits-open-tabs.md`](trailing-skip-confirmations-help-omits-open-tabs.md) — The
  Skip session confirmations switch's help still promises Delete on the first click, though a session with open shell
  tabs is now always asked about.

- [`trailing-tailscale-approved-path-untested.md`](trailing-tailscale-approved-path-untested.md) — Test gap: no helm
  test covers an agent's approved Tailscale host add being reported back to the agent as Tailscale.

- [`trailing-claude-refused-start-command-restart-untested.md`](trailing-claude-refused-start-command-restart-untested.md)
  — Test gap: nothing pins that a Claude Restart still resumes when the stored start command fails this build's checks.

- [`trailing-ntfy-disclosure-omits-blocked-reason.md`](trailing-ntfy-disclosure-omits-blocked-reason.md) — SPEC.md's
  outbound-network and non-goal text still says ntfy carries only waiting alerts with titles and host names; blocked
  alerts also send the agent's free-text reason.

- [`trailing-blocked-alert-master-switch-untested.md`](trailing-blocked-alert-master-switch-untested.md) — Test gap:
  nothing pins that a blocked-agent ntfy alert needs ntfy alerts switched on, or that it does not depend on the waiting
  switch.

- [`trailing-ntfy-criteria-refusal-test-unchecked.md`](trailing-ntfy-criteria-refusal-test-unchecked.md) — Test gap: the
  test for refusing over-long blocked criteria never checks that the refused save changed nothing.

- [`trailing-restart-extra-instructions-untested.md`](trailing-restart-extra-instructions-untested.md) — Test gap: no
  supervisor test pins that a Restart's blocked-agent instruction reaches the relaunched agent.

- [`trailing-paste-upload-docs-linux-file-copy.md`](trailing-paste-upload-docs-linux-file-copy.md) — The Work in a
  session docs say a Linux file-manager copy pasted with Paste via file upload uploads its path as text; on Wayland it
  now uploads the file itself.

- [`trailing-restart-always-resumes-text-stale.md`](trailing-restart-always-resumes-text-stale.md) — SPEC_impl.md's CLI
  section and the agent CLI's --mode refusal still say every Restart resumes, which a Claude fresh start now
  contradicts.

- [`trailing-drift-runner-budget-too-short.md`](trailing-drift-runner-budget-too-short.md) — Maintainer tooling: the
  drift runner's fixed 720 s per case can kill a slow conversation-rotation or Codex base case before the test reports
  which step stalled.

- [`trailing-credential-delete-refusal-close-untested.md`](trailing-credential-delete-refusal-close-untested.md) — Test
  gap: nothing checks that a credential Delete refused while Settings waits to close keeps Settings open with the
  reason.

- [`trailing-notify-reason-limit-messages-disagree.md`](trailing-notify-reason-limit-messages-disagree.md) — farhelm
  agent notify can refuse a reason as over 2000 bytes and then, shortened, as over 500 characters; a reason the helm
  would clean down to size is refused outright.

- [`trailing-notify-accepts-invalid-asking-id.md`](trailing-notify-accepts-invalid-asking-id.md) — A notify from a
  broken or hostile host is recorded for a session id the helm refuses everywhere else (empty, too long, control
  characters), stored bare where a full id is promised.

- [`trailing-spec-impl-host-refresh-cadence-stale.md`](trailing-spec-impl-host-refresh-cadence-stale.md) — SPEC_impl.md
  still says a connected host's session list refreshes every 3 seconds in one place and every 30 in another.

- [`trailing-custom-agent-delete-close-untested.md`](trailing-custom-agent-delete-close-untested.md) — Test gap: closing
  Settings during a custom agent Delete is tested only when the catalog update beats the reply, and the reworked test
  lost its post-Delete page checks.

- [`trailing-tensorlake-key-clear-close-untested.md`](trailing-tensorlake-key-clear-close-untested.md) — Test gap:
  closing Settings while a Tensorlake key Clear or a failing key Test is out is untested.

- [`trailing-two-tensorlake-keys-close-save-replaces.md`](trailing-two-tensorlake-keys-close-save-replaces.md) — With
  two different Tensorlake keys typed, saving one and closing now offers Save for the other, with no hint that it
  replaces the key just saved.

- [`trailing-spec-impl-codex-clear-test-claim-stale.md`](trailing-spec-impl-codex-clear-test-claim-stale.md) —
  SPEC_impl.md still says a real-agent test pins Codex's transcript availability after /clear; the rotation category
  that replaced that test does not.

- [`trailing-rotation-driver-echo-matches-menu-row.md`](trailing-rotation-driver-echo-matches-menu-row.md) — Test
  tooling: the rotation drift driver accepts the slash-menu row as the typed command's echo, so Enter can fire on a
  partial or different command.

- [`trailing-brick-override-launch-after-quit-fatal.md`](trailing-brick-override-launch-after-quit-fatal.md) —
  Maintainer tooling: in a brick-test override pass, the managed launch right after the harness's own Quit is fatal
  while the old app is still exiting.

- [`trailing-brick-managed-envelope-feedback-guard.md`](trailing-brick-managed-envelope-feedback-guard.md) — Maintainer
  tooling: the brick test's 'do not send feedback yet' guard is only in the plain instructions, not the managed ones
  live runs use, while Welcome now offers Send feedback.

- [`trailing-brick-addendum-checks-after-window-drag.md`](trailing-brick-addendum-checks-after-window-drag.md) —
  Maintainer tooling: a candidate's extra brick-test checks still run after window-drag on the fresh pass, so a VM that
  cannot drag still blocks them.

- [`trailing-cmd-w-double-tap-test-overstates.md`](trailing-cmd-w-double-tap-test-overstates.md) — Test text: the Cmd+W
  double-tap test claims a second press can never reach the session, but it covers only the moment the tab's close is in
  flight (SPEC.md accepts the later Delete).

- [`trailing-second-close-inherits-first-refusal.md`](trailing-second-close-inherits-first-refusal.md) — A second Close
  pressed while Settings still waits on earlier work re-shows the first press's refused page when that work lands,
  instead of asking about unsaved changes.

- [`trailing-docs-shots-marks-left-after-failure.md`](trailing-docs-shots-marks-left-after-failure.md) — Maintainer
  tooling: a failed bulk Restart/Replace release-notes shot leaves sessions marked, so later shots can be captured with
  a stray marked bar.
