# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

## Highest priority: security or data loss

- `template-successor.md` — Template installation identity ignored at create dispatch.
- `uninstall-missing-id.md` — Connected uninstall omits missing recorded identity comparison.
- `uninstall-reload.md` — Uninstall can lose process-only kill policy before stopping.
- `shutdown-expiry.md` — Planned shutdown exits before every output client becomes safe.
- `template-clobber.md` — A new template can overwrite saved choices while its catalog is unavailable.
- `yolo-sidebar-cancel.md` — Sidebar YOLO replacement can run after cancellation.
- `yolo-launcher-cancel.md` — Launcher YOLO answer can restore cancelled consent.
- `yolo-restart-cancel.md` — Restart with accepts a cancelled YOLO answer.
- `installer-symlink.md` — App updates can overwrite files outside a symlinked bundle directory.
- `installer-startup-prune.md` — An interrupted update can prune a version whose supervisor is starting.
- `feedback-queued-close.md` — A queued close can discard feedback while sending starts.
- `template-empty-name.md` — Editing a template silently drops its instruction to clear the name.
- `omp-bun-pane-proof.md` — A nested OMP conversation can be accepted for an unreadable Bun foreground.

- `restart-parent-cancel.md` — Restart with parent accepts unrestricted consent after its question is cancelled.

## High priority: material UX degradation

- `tab-cleanup-blocks-status-sampling.md` — Automatic cleanup of an exited terminal tab can leave every session on that
  host showing stale status for seconds while background processes stop.
- `upload-cancellation-drops-final-reply.md` — Delete can discard an upload result during temporary connection
  backpressure, leaving the upload waiting forever even after the connection resumes normal traffic.

- `feedback-cancel.md` — Feedback forwarding can be cancelled with its HTTP request.
- `snapshot-root.md` — Writable snapshot may pass replacement checkout ownership.
- `grok-trust.md` — Choosing Grok preserves incompatible workspace trust.
- `setup-inline-cancel.md` — Cancelled host setup can turn off future setup questions.
- `addhost-cancel.md` — Cancelled Add host can still accept a queued setup answer.
- `font-focus.md` — Text-size buttons leave focus outside the terminal at the limits.
- `installer-directory-target.md` — An update can report success while leaving the app unlaunchable.
- `uninstall-forgotten-success.md` — A failed uninstall can be reported as successful after another client forgets the
  host.

## Other: correctness, diagnostics, cleanup, or convenience

- `hostnotfound-refresh-keeps-serving.md` — an identity-less actor for a deleted host keeps serving, because its
  refreshes never write the store and so never see the row is gone.
- `session-detail-drains-full-list.md` — every fleet-revision bump makes each open session view trigger a full
  ListSessions on its host.
- `probe-reregister-drops-terminals.md` — probing an already-registered healthy host forces a reconnect that drops every
  open terminal on it.
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `tab-reap-budget-starved-by-failures.md` — failed tab closes spend the per-tick reap budget, so a few persistent
  failures stop exited tabs from being reaped host-wide.
- `tmux-build-script-bash32.md` — build-private-tmux.sh's macOS branch aborts under bash 3.2 because of empty arrays
  under `set -u`.
- `provisioning-update-replaces-binary-before-setup-guard.md` — remote UPDATE replaces the farhelm binary before
  checking whether `farhelm helm setup` took the host over.

- `codex-draft-mistaken-for-question.md` — Pasting question-shaped diagnostics into an unsent Codex draft can make the
  sidebar say the agent needs an answer when it is idle.

- `uninstall-cancel.md` — Cancelled uninstall planning leaves SSH helpers.
- `birth-oracle.md` — Birth-time oracle treats execution errors as absent capability.
- `token-recovery-path.md` — Token recovery command silently targets a different path.
- `template-catalog-error.md` — Template discovery failure looks like an empty catalog.
- `uninstall-dryrun-locks.md` — Uninstall preview omits currently held lock blockers.
- `plans-heading-splice.md` — Legal Markdown headings can strand part of a plan question.
- `publisher-copy-race.md` — Screenshot publication may certify pixels different from those it checked.
- `watcher-revision-cache.md` — Plans watcher can cache an ignore verdict against the wrong revision.
- `preview-ready-identity.md` — Preview startup can report an unrelated server as the docs preview.
- `template-dot-name.md` — Dot-only template names pass validation but cannot be saved.
