# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

## Highest priority: security or data loss

- `claude-clear-report-dropped-on-claim-timeout.md` — a Claude `/clear` report refused after a 1 s capture-claim wait is
  never resent, so Resume keeps reopening the cleared conversation.

## High priority: material UX degradation

- `tab-cleanup-blocks-status-sampling.md` — Automatic cleanup of an exited terminal tab can leave every session on that
  host showing stale status for seconds while background processes stop.
- `upload-cancellation-drops-final-reply.md` — Delete can discard an upload result during temporary connection
  backpressure, leaving the upload waiting forever even after the connection resumes normal traffic.

## Other: correctness, diagnostics, cleanup, or convenience

- `host-write-lock-split-on-actor-respawn.md` — the per-host write lock lives on the actor handle, so a respawn lets
  edits run during provisioning.
- `hostnotfound-refresh-keeps-serving.md` — an identity-less actor for a deleted host keeps serving, because its
  refreshes never write the store and so never see the row is gone.
- `session-detail-drains-full-list.md` — every fleet-revision bump makes each open session view trigger a full
  ListSessions on its host.
- `probe-reregister-drops-terminals.md` — probing an already-registered healthy host forces a reconnect that drops every
  open terminal on it.
- `attach-reports-generic-timeout.md` — the attach step spins 30 s on skew/identity states and reports only "timed out".
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `stop-restart-panic-no-reply.md` — a panicking stop or restart task sends no reply, leaving the UI or `farhelm agent`
  waiting until the connection drops.
- `tab-reap-budget-starved-by-failures.md` — failed tab closes spend the per-tick reap budget, so a few persistent
  failures stop exited tabs from being reaped host-wide.
- `tmux-build-script-bash32.md` — build-private-tmux.sh's macOS branch aborts under bash 3.2 because of empty arrays
  under `set -u`.
- `delete-skips-scoped-tab-on-stale-verdict.md` — Delete skips a scoped tab's systemd scope on a stale "no user manager"
  verdict when the agent launch itself was unscoped.
- `create-rollback-orphans-unconfirmed-scope.md` — a failed create whose scope kill is unconfirmed still deletes the
  row, leaving processes nothing can reach.
- `provisioning-update-replaces-binary-before-setup-guard.md` — remote UPDATE replaces the farhelm binary before
  checking whether `farhelm helm setup` took the host over.

- `codex-draft-mistaken-for-question.md` — Pasting question-shaped diagnostics into an unsent Codex draft can make the
  sidebar say the agent needs an answer when it is idle.
- `build-metadata-false-old-version.md` — A host running the same release can incorrectly show an old-version warning
  when the helm or supervisor build includes metadata.
