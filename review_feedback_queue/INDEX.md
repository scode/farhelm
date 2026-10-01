# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

## Highest priority: security or data loss

- `yolo-guard-misses-env-prefix.md` — a command wrapped in `env NAME=value` starts a YOLO agent on a sensitive host with
  no confirmation and no badge.
- `yolo-guard-fails-open-without-row.md` — the sensitive-host YOLO guard lets a launch through if the host's registry
  row disappears mid-request.
- `sighup-skips-orderly-shutdown.md` — SIGHUP (closing the terminal that started the desktop app or a hand-run
  supervisor) skips the orderly tmux shutdown.
- `header-replace-recomputes-alive.md` — header Replace recomputes "nothing is alive" after the YOLO confirmation, so it
  can kill an agent or shell restarted meanwhile.
- `sidebar-replace-recomputes-alive.md` — sidebar Replace recomputes "nothing is alive" after the YOLO confirmation, so
  it can kill an agent or shell restarted meanwhile.
- `yolo-guard-misses-codex-option-form.md` — `codex -a never -s danger-full-access` is not treated as YOLO, so it runs
  on a sensitive host with no confirmation or badge.
- `merged-list-crowded-by-one-host.md` — a remote host reporting 500 future-dated or top-sorting sessions pushes every
  other host's sessions out of the merged list.
- `sessions-changed-hint-unthrottled.md` — the helm does not rate-limit "sessions changed" hints, so a hostile host can
  drive back-to-back refreshes and fleet-wide re-reads.
- `ssh-forwarding-inherited.md` — with ForwardAgent/ForwardX11 in the user's ssh config, the helm's always-on
  connections expose the ssh agent and X display to remote agents.
- `yolo-safe-survives-identity-adoption.md` — a host's "safe for YOLO" mark survives adopting a different machine's
  identity, so YOLO launches there skip the sensitive-host confirmation.
- `confirmed-nothing-alive-prompt-kills-live-agent.md` — a sidebar delete prompt that drifted to "nothing alive", or the
  header Restart confirm, can kill an agent restarted after the prompt was worded.
- `replace-with-kills-running-source-unwarned.md` — Replace with kills a running source session with no warning and no
  liveness precondition.
- `claude-scan-claims-foreign-record.md` — the Claude scan fallback can commit another process's conversation as the
  session's, so Resume appends to the wrong conversation.
- `claude-resume-template-selector-collision.md` — Claude's derived resume command keeps an original `--continue`,
  `--resume` or `--`, so Resume can open the wrong conversation or start fresh.
- `yolo-guard-misses-equivalent-spellings.md` — the sensitive-host YOLO guard misses Cursor's short `-f` flag and Pi
  launches whose program is not named `pi`.
- `yolo-guard-skips-resume-template.md` — the sensitive-host YOLO guard never classifies a profile's separate resume
  command, so the first Resume or Restart can skip approvals unconfirmed.
- `omp-corridor-uncounted-pane-runtime.md` — when the foreground OMP's command line cannot be read, a nested OMP can
  take over the session's Resume target.
- `claude-clear-report-dropped-on-claim-timeout.md` — a Claude `/clear` report refused after a 1 s capture-claim wait is
  never resent, so Resume keeps reopening the cleared conversation.

- `process-snapshot-requires-supervisor-witness.md` — process cleanup accepts an empty process table without proving the
  supervisor was enumerated.
- `probe-cancellation-leaves-helper-processes.md` — cancelling discovery can leave isolated probe helpers and stderr
  readers alive.

## High priority: material UX degradation

- `codex-resume-template-duplicates-selector.md` — restarting a Codex launch that already used `resume` can append a
  second selector and fail.
- `pi-resume-selector-option-boundaries.md` — Pi restart rewriting can delete an unrelated option value and lose the
  verified session selector.
- `provisioning-child-output-drain-deadline.md` — provisioning can outlive its deadline while draining a descendant-held
  output pipe.
- `clipboard-writes-unbounded-blocking-admission.md` — OSC 52 clipboard bursts can fill the shared blocking pool behind
  a stalled native sink.
- `tmux-capture-tail-deadline.md` — pane capture stops applying its deadline after stdout closes, so periodic sampling
  can hang.

- `pi-resume-downgrade-on-read-error.md` — Pi resume check destroys a valid locator when the session file merely fails
  to read.
- `plain-retry-erases-pending-fresh-window.md` — plain retry downgrades a pending fast reconnect to a slow probe.
- `sftp-overall-deadline-fails-slow-links.md` — sftp transfer's 60 s overall deadline fails slow links
  deterministically.
- `event-feed-liveness-postponed-by-revisions.md` — the event feed's liveness check never fires on a busy fleet, so dead
  subscribers hold the 64 seats.
- `sink-shutdown-retries-forever-after-delete.md` — deleting a busy session can leave its sink client stuck in an
  endless shutdown-retry loop.
- `input-client-notifications-pile-up.md` — idle input clients never read tmux notifications, so tmux server memory
  grows per open terminal.
- `adopt-checks-current-row-not-dialed.md` — adopt checks the manager's current row, so a stale mismatch after a
  retarget adopts the old machine's identity.
- `identity-mismatch-never-becomes-duplicate.md` — a host that reaches another entry's identity offers an adopt that
  always 409s and never shows as duplicate.
- `session-view-leaks-page-lock.md` — the session view's restart/replace claim is not released on unmount, leaving every
  page action disabled until reload.
- `row-menu-drifts-on-row-height-change.md` — an open row menu can float over a different row when a row above gains a
  detail line at the same index.
- `uploads-aborted-silently-on-remount.md` — terminal reconnect or restart remount silently aborts in-flight uploads and
  erases finished uploads' landed path or failure message.
- `pi-reporter-asset-not-renamed.md` — Pi hosts that ran Pi before v0.13.0 permanently lose Resume and the instructions
  pointer for every Pi session after upgrading.
- `replace-drop-skips-source-delete.md` — switching away or reloading during Replace creates the replacement but never
  deletes the original, with no error.
- `checkout-preview-blocks-read-loop.md` — typing a GitHub repo in the create dialog can freeze typing in every terminal
  on that host while the checkout folder is scanned.
- `repo-search-blocking-scan.md` — repository search scans the checkout folder with blocking calls its timeout cannot
  interrupt, so a slow share can stall the whole supervisor.
- `delete-holds-attachments-lock-through-archive.md` — Delete keeps the host-wide terminal lock through checkout
  archiving and database fsyncs, so typing everywhere on the host pauses.
- `restart-can-still-deselect-session.md` — restarting a session can still empty the main pane or jump to another
  session; #1310 removed only one trigger.
- `create-dialog-empty-catalog-refuses.md` — when the model catalog fails to load or is still loading,
  clone/replace-with/recent setups are refused as "no longer supported".
- `restart-cwd-lossy-non-utf8.md` — restart/retry of a session whose folder resolves to a non-UTF-8 path starts the
  agent in $HOME and reports success.
- `non-utf8-farhelm-path-breaks-launch.md` — a farhelm binary or state directory at a non-UTF-8 path makes every launch
  fail, while the log claims only degraded mode.
- `stop-terminalless-records-plain-exit.md` — Stop on an ambiguous, terminal-less launch records a plain exit before
  sweeping, losing the stop note and later Restart's consent check.
- `probe-drops-add-busy-claim.md` — a probe during a rerun of a failed ADD can drop the busy claim, letting a second
  install run back to back.
- `retarget-race-republishes-old-client.md` — a refresh finishing during a retarget can republish the old connection,
  possibly routing an operation to the old machine.
- `probe-register-not-helm-owned.md` — a probe interrupted mid-registration leaves a host that is registered but
  invisible, never dialed, and blocks re-adding it.
- `desktop-start-fails-on-skewed-supervisor.md` — a hand-started supervisor on another protocol version makes the
  desktop app fail at startup with a misleading timeout.
- `terminal-tombstone-never-buried.md` — a terminal's frozen post-takeover screen is never cleared when it leaves the
  view, leaving a blank pane and leaking memory.
- `checkout-retry-raw-device-check.md` — a create retried after a reboot or remount that renumbered the device is
  refused as "folder replaced" and never retried.
- `takeover-latch-misses-attaching-tabs.md` — a tab still attaching when another window takes over evicts the winner, so
  the two windows displace each other.
- `claude-spinner-window-too-short.md` — the Claude spinner is searched only 6 lines above the input box, so a task list
  in between may read as Idle mid-turn.
- `claude-spinner-rejects-multiword.md` — the Claude spinner check rejects multi-word text like "Compacting
  conversation…", so compaction may read Idle.
- `new-tab-mount-displaces-owner-during-recovery.md` — a recovering view mounts a newly appeared tab with a displacing
  attach and silently takes the session from the device in use.
- `update-silently-downgrades-newer-hosts.md` — Update and "update all" downgrade hosts that run a newer Farhelm, which
  can leave the supervisor unable to start.

## Other: correctness, diagnostics, cleanup, or convenience

- `event-feed-cap-refusal-invisible.md` — the event feed's subscriber-cap refusal is a pre-upgrade 503 that browsers
  cannot observe.
- `output-client-shutdown-can-retry-forever.md` — the per-terminal output client can get stuck retrying shutdown when
  its session disappears while paused.
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
