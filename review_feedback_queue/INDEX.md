# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

## Highest priority: security or data loss

## High priority: material UX degradation

- `tab-cleanup-blocks-status-sampling.md` — Automatic cleanup of an exited terminal tab can leave every session on that
  host showing stale status for seconds while background processes stop.
- `upload-cancellation-drops-final-reply.md` — Delete can discard an upload result during temporary connection
  backpressure, leaving the upload waiting forever even after the connection resumes normal traffic.

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
