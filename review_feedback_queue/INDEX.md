# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

## Highest priority: security or data loss

- `merged-list-crowded-by-one-host.md` — a remote host reporting 500 future-dated or top-sorting sessions pushes every
  other host's sessions out of the merged list.
- `sessions-changed-hint-unthrottled.md` — the helm does not rate-limit "sessions changed" hints, so a hostile host can
  drive back-to-back refreshes and fleet-wide re-reads.
- `ssh-forwarding-inherited.md` — with ForwardAgent/ForwardX11 in the user's ssh config, the helm's always-on
  connections expose the ssh agent and X display to remote agents.
- `claude-resume-template-selector-collision.md` — Claude's derived resume command keeps an original `--continue`,
  `--resume` or `--`, so Resume can open the wrong conversation or start fresh.
- `omp-corridor-uncounted-pane-runtime.md` — when the foreground OMP's command line cannot be read, a nested OMP can
  take over the session's Resume target.
- `claude-clear-report-dropped-on-claim-timeout.md` — a Claude `/clear` report refused after a 1 s capture-claim wait is
  never resent, so Resume keeps reopening the cleared conversation.

- `process-snapshot-requires-supervisor-witness.md` — process cleanup accepts an empty process table without proving the
  supervisor was enumerated.
- `probe-cancellation-leaves-helper-processes.md` — cancelling discovery can leave isolated probe helpers and stderr
  readers alive.

- `desktop-clipboard-fetch-backlog.md` — A remote terminal can submit clipboard updates faster than the desktop consumes
  them, accumulating pending requests and old clipboard text without a bound.
- `desktop-protocol-filesystem-fallback.md` — The desktop page can request files outside the embedded assets through the
  framework protocol. No script-injection exploit was found; this is a native hardening concern.
- `terminal-output-queue-missing-byte-budget.md` — Opening one hostile remote terminal can make the helm retain almost 2
  GiB of output before its message-count limit trips, affecting the process that serves all hosts.

## High priority: material UX degradation

- `opencode-bare-model-rejected.md` — Some documented bare OpenCode model names fail to launch, even though the same
  model works with an opencode/ prefix.
- `opencode-bare-model-switches-harness.md` — Pressing Enter on a supported bare OpenCode model can silently select
  Codex, so the session starts with a different agent and configuration.
- `tab-cleanup-blocks-status-sampling.md` — Automatic cleanup of an exited terminal tab can leave every session on that
  host showing stale status for seconds while background processes stop.
- `desktop-reauth-failure-loses-action-outcomes.md` — If desktop sign-in recovery fails while an action is pending, the
  action can still finish on the server while its result disappears without an unknown-outcome notice.
- `browser-signin-loses-action-outcomes.md` — A browser token prompt can silently lose the results of actions already
  running, including Delete, even though the server continues the work.
- `desktop-auth-ready-with-stale-webview-credential.md` — Desktop sign-in can appear successful while terminals, uploads
  and the event feed remain unusable because the window kept a revoked or missing credential.
- `create-directory-wait-blocks-terminal-reader.md` — Creating a session during Delete can freeze input and other
  requests for every session on that host. This finding is already covered by the Planned creation-dispatch work.
- `checkout-reconciliation-blocks-terminal-reader.md` — Starting a fresh checkout during Delete can freeze unrelated
  terminal input before the actual create request is even sent.
- `list-admission-blocks-terminal-reader.md` — When eight management operations occupy the host, a session-list request
  can stop later keystrokes and terminal control messages from being dispatched.
- `stop-admission-blocks-terminal-reader.md` — A Stop waiting for management capacity can freeze input to unrelated
  sessions on the same host.
- `restart-admission-blocks-terminal-reader.md` — A Restart waiting for management capacity can freeze input to
  unrelated sessions on the same host.
- `rename-admission-blocks-terminal-reader.md` — Even a title change can freeze typing across the host when other
  management operations occupy its request slots.
- `upload-cancellation-drops-final-reply.md` — Delete can discard an upload result during temporary connection
  backpressure, leaving the upload waiting forever even after the connection resumes normal traffic.

## Other: correctness, diagnostics, cleanup, or convenience

- `event-feed-cap-refusal-invisible.md` — the event feed's subscriber-cap refusal is a pre-upgrade 503 that browsers
  cannot observe.
- `claude-scan-budget-never-settles.md` — Claude record-scan capture can never complete in a large project directory and
  rescans forever.
- `env-wrapper-hides-command-not-found.md` — Farhelm's own `env` wrapper turns "command not found" for Goose, Pi and OMP
  into exited (127) instead of error.
- `host-write-lock-split-on-actor-respawn.md` — the per-host write lock lives on the actor handle, so a respawn lets
  edits run during provisioning.
- `refresh-starved-by-seeds.md` — steady creates/renames through the helm can discard every refresh while the host
  reports healthy.
- `hostnotfound-refresh-keeps-serving.md` — an identity-less actor for a deleted host keeps serving, because its
  refreshes never write the store and so never see the row is gone.
- `session-detail-drains-full-list.md` — every fleet-revision bump makes each open session view trigger a full
  ListSessions on its host.
- `incarnation-counter-restarts-per-process.md` — incarnation numbers restart each helm process, so expected_incarnation
  can match a different install after restart.
- `list-ingress-id-validation-gap.md` — session-list ingress admits empty, control-character and dot-segment ids, and
  the UI's %2E guard does not hold.
- `probe-reregister-drops-terminals.md` — probing an already-registered healthy host forces a reconnect that drops every
  open terminal on it.
- `attach-reports-generic-timeout.md` — the attach step spins 30 s on skew/identity states and reports only "timed out".
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `desktop-copy-fallback-never-runs.md` — the native clipboard writer never rejects, so the header copy fallback never
  runs and failures show "copied".
- `dropped-create-skips-bookkeeping.md` — reloading during a create starts the agent but never records launch history or
  the remembered default profile.
- `agent-create-aborted-on-retire.md` — an agent's create/clone aborted by its host's reconnect still creates the
  session but skips launch history and gets no answer.
- `stop-restart-panic-no-reply.md` — a panicking stop or restart task sends no reply, leaving the UI or `farhelm agent`
  waiting until the connection drops.
- `claude-capture-warns-forever.md` — an exited Claude session whose transcript was deleted logs a WARN every capture
  pass, indefinitely and across restarts.
- `tab-reap-budget-starved-by-failures.md` — failed tab closes spend the per-tick reap budget, so a few persistent
  failures stop exited tabs from being reaped host-wide.
- `profile-body-accepts-unknown-fields.md` — a profile create/update with a misspelled or stray field is accepted and
  silently clears the stored resume template.
- `folder-picker-skips-symlinks.md` — the create dialog's folder picker never lists symlinked folders, and one
  unreadable entry fails the whole listing.
- `tilde-in-remote-path-fields.md` — `~` in the remote farhelm or remote state dir fields is not expanded, giving a
  false "not installed" or a folder named `~`.
- `escape-token-clamp-too-short.md` — the 64-character host label clamp can cut a `<U+E00xx>` escape token in half.
- `partial-release-download-left-behind.md` — a release download that fails mid-stream (including on a full disk) leaves
  its `.part` file in helm state.
- `tmux-build-script-bash32.md` — build-private-tmux.sh's macOS branch aborts under bash 3.2 because of empty arrays
  under `set -u`.
- `pi-pointer-overrides-user-prompt.md` — Pi injection always adds `--append-system-prompt`, which may silently replace
  the user's own (OMP already yields).
- `terminal-font-promise-leak.md` — per-mount callbacks on a never-settling font promise retain every terminal instance,
  so overnight reconnect loops grow without bound.
- `delete-skips-scoped-tab-on-stale-verdict.md` — Delete skips a scoped tab's systemd scope on a stale "no user manager"
  verdict when the agent launch itself was unscoped.
- `restart-with-skips-create-validation.md` — restart-with stores argv and resume templates without create's checks, so
  a bad bundle can stop the supervisor from starting.
- `ssh-config-remotecommand-blocks-host.md` — a `RemoteCommand` in the user's ssh config makes every connection to that
  host fail, with a misleading error.
- `provision-lock-map-grows-per-requested-id.md` — the host provisioning lock map gains a never-freed entry for every
  host id a request names, registered or not.
- `create-rollback-orphans-unconfirmed-scope.md` — a failed create whose scope kill is unconfirmed still deletes the
  row, leaving processes nothing can reach.
- `provisioning-update-replaces-binary-before-setup-guard.md` — remote UPDATE replaces the farhelm binary before
  checking whether `farhelm helm setup` took the host over.
- `drop-on-hidden-terminal-navigates-away.md` — a file dropped on a terminal that is catching up or reconnecting can
  navigate the browser page away.

- `codex-draft-mistaken-for-question.md` — Pasting question-shaped diagnostics into an unsent Codex draft can make the
  sidebar say the agent needs an answer when it is idle.
- `build-metadata-false-old-version.md` — A host running the same release can incorrectly show an old-version warning
  when the helm or supervisor build includes metadata.
- `seen-write-cancellation-skips-notification.md` — Closing the browser during a read/unread update can save the change
  without notifying other windows, leaving their dots stale until another event or refresh.
