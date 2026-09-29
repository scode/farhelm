# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

## Highest priority: security or data loss

## High priority: material UX degradation

- `pi-resume-downgrade-on-read-error.md` — Pi resume check destroys a valid locator when the session file merely fails
  to read.
- `plain-retry-erases-pending-fresh-window.md` — plain retry downgrades a pending fast reconnect to a slow probe.
- `send-upload-ignores-cancellation.md` — the transfer's queue send ignores cancellation, stalling deletes.
- `sftp-overall-deadline-fails-slow-links.md` — sftp transfer's 60 s overall deadline fails slow links
  deterministically.
- `unvalidated-state-dir-on-probe.md` — probed registration with a bad state-dir path permanently bricks the entry.
- `tab-close-skips-scope-on-stale-verdict.md` — tab close skips the tab's cgroup whenever the cached manager verdict is
  negative, contrary to its docs.
- `restart-sweep-on-abortable-connection-task.md` — restart's stop-and-sweep runs on a connection task that a client
  disconnect aborts mid-kill.
- `delete-roots-only-agent-pane.md` — Delete roots its process walk only on the agent pane, so tab, split-pane and
  terminal-less processes survive.
- `restart-relaunches-over-unconfirmed-scope.md` — restart relaunches even when the previous run's cgroup could not be
  confirmed empty.
- `event-feed-liveness-postponed-by-revisions.md` — the event feed's liveness check never fires on a busy fleet, so dead
  subscribers hold the 64 seats.
- `agent-create-replays-asker-as-child.md` — a keyed `agent create` can report the asking session itself as the newly
  created session.
- `spawn-replays-asker-as-child.md` — a keyed `farhelm spawn` reusing another session's key can be handed that session's
  child as its own new child.
- `sink-shutdown-retries-forever-after-delete.md` — deleting a busy session can leave its sink client stuck in an
  endless shutdown-retry loop.
- `input-client-notifications-pile-up.md` — idle input clients never read tmux notifications, so tmux server memory
  grows per open terminal.
- `missing-checkout-root-blocks-delete.md` — a removed, recreated or unmounted checkout root makes its sessions
  permanently undeletable.
- `unstable-device-number-blocks-delete.md` — the ownership check treats `st_dev` as stable, so a remount can make
  checkout sessions undeletable and unrestartable.
- `noreplace-rename-unsupported-strands-archive.md` — on filesystems without no-replace rename, the last Delete leaves
  the checkout stuck `archive_pending` forever.
- `checkout-name-scan-case-sensitive.md` — the checkout name scan is case-sensitive, so on macOS a case-variant folder
  makes every launch conflict.
- `ambiguous-restart-republishes-old-terminal.md` — an ambiguous restart failure republishes the pre-restart terminal,
  so a live agent is unopenable and killable without consent.
- `idempotency-fingerprint-keeps-raw-cmdline.md` — idempotency fingerprints keep the raw command line (with any keys) in
  the database after Delete.
- `adopt-checks-current-row-not-dialed.md` — adopt checks the manager's current row, so a stale mismatch after a
  retarget adopts the old machine's identity.
- `identity-mismatch-never-becomes-duplicate.md` — a host that reaches another entry's identity offers an adopt that
  always 409s and never shows as duplicate.
- `ssh-controlpath-too-long.md` — the ssh ControlPath under the state dir overflows sun_path for common long usernames,
  so every ssh host fails.
- `update-reports-success-on-hand-started-supervisor.md` — UPDATE of a hand-started supervisor crash-loops the new unit
  and can report success while the old build keeps serving.
- `payload-dir-must-be-writable.md` — --payload-dir writes .extracted/ into the operator's directory, so read-only or
  shared release dirs fail every run.
- `linger-failure-blocks-update-restart.md` — the optional linger step runs before restart/attach and most loginctl
  failures are fatal, so UPDATE never restarts.
- `embedded-payload-cleanup-blocks-helm-start.md` — a failure deleting the retired embedded-payloads cache aborts helm
  startup.
- `installer-stale-lock-recovery-not-exclusive.md` — two installers recovering the same stale lock can both replay the
  journal, deleting the restored binary.
- `installer-bundle-swap-unlocked.md` — the app-bundle step runs after the lock is released with rm -rf then mv, so
  overlaps or interrupts leave a mixed, nested or half-deleted bundle.
- `leftover-uninstall-receipt-blocks-uninstall.md` — a stale .Farhelm.app.uninstall-receipt blocks every later uninstall
  after a reinstall; the advised rerun never clears it.
- `setup-partial-unit-write-mismatch.md` — helm setup can fail after rewriting only the supervisor unit, leaving a
  mismatched helm/supervisor pair.
- `no-supervisor-setup-splits-state-dir.md` — setup --no-supervisor leaves setup's own supervisor on the old state dir
  while the helm is pinned to the new one.
- `receiptless-app-bundle-blocks-uninstall.md` — on macOS a Farhelm.app without a receipt refuses the whole uninstall,
  and the advice cannot work under the no-bundle opt-out.
- `desktop-reauth-remount-loses-action.md` — the desktop credential refresh remounts the app before the retry runs,
  silently losing the action that hit the 401.
- `desktop-reauth-failure-dead-end.md` — a transient webview re-auth failure after rotation leaves only an error line
  with no retry until relaunch.
- `session-view-leaks-page-lock.md` — the session view's restart/replace claim is not released on unmount, leaving every
  page action disabled until reload.
- `row-menu-drifts-on-row-height-change.md` — an open row menu can float over a different row when a row above gains a
  detail line at the same index.
- `uploads-aborted-silently-on-remount.md` — terminal reconnect or restart remount silently aborts in-flight uploads
  with no message.
- `desktop-bootstrap-token-always-pushed.md` — the desktop webview receives the master web token on every (re)auth, even
  when its stored secret is valid.

## Other: correctness, diagnostics, cleanup, or convenience

- `sweep-drops-unreadable-root-silently.md` — the sweep silently drops a pane root whose identity read fails and can
  still report success.
- `event-feed-cap-refusal-invisible.md` — the event feed's subscriber-cap refusal is a pre-upgrade 503 that browsers
  cannot observe.
- `any-dioxus-webview-passes-origin-guard.md` — any Dioxus or wry app's webview origin passes the helm's origin guard
  and CORS, not just Farhelm's.
- `output-client-shutdown-can-retry-forever.md` — the per-terminal output client can get stuck retrying shutdown when
  its session disappears while paused.
- `tab-session-token-in-tmux-argv.md` — opening a tab puts the session token on the tmux client's command line, briefly
  visible to other accounts.
- `pre-mkdir-rollback-leaves-phantom-membership.md` — a pre-mkdir create rollback leaves a phantom membership in another
  checkout, so it is never archived.
- `preserved-plan-diagnostic-log-only.md` — the "unresolved plan, path preserved" diagnostic goes only to the log; the
  Delete reply is plain success.
- `checkout-path-too-long-for-archive.md` — admitted checkout paths near PATH_MAX are too long for the archive
  destination, blocking Delete.
- `claude-scan-budget-never-settles.md` — Claude record-scan capture can never complete in a large project directory and
  rescans forever.
- `env-wrapper-hides-command-not-found.md` — Farhelm's own `env` wrapper turns "command not found" for Goose, Pi and OMP
  into exited (127) instead of error.
- `host-write-lock-split-on-actor-respawn.md` — the per-host write lock lives on the actor handle, so a respawn lets
  edits run during provisioning.
- `refresh-starved-by-seeds.md` — steady creates/renames through the helm can discard every refresh while the host
  reports healthy.
- `host-edits-not-cancellation-safe.md` — a client disconnect between a host edit's commit and its reconcile leaves the
  actor out of sync until restart.
- `hostnotfound-refresh-keeps-serving.md` — an identity-less actor for a deleted host keeps serving, because its
  refreshes never write the store and so never see the row is gone.
- `session-detail-drains-full-list.md` — every fleet-revision bump makes each open session view trigger a full
  ListSessions on its host.
- `incarnation-counter-restarts-per-process.md` — incarnation numbers restart each helm process, so expected_incarnation
  can match a different install after restart.
- `list-ingress-id-validation-gap.md` — session-list ingress admits empty, control-character and dot-segment ids, and
  the UI's %2E guard does not hold.
- `add-confirm-rewrites-row-before-busy-check.md` — confirming ADD rewrites an existing row and reconnects before the
  busy check, moving an in-flight UPDATE's host.
- `cancelled-start-run-leaves-host-busy.md` — dropping the confirm request inside start_run leaves the host busy (409)
  until the helm restarts.
- `probe-reregister-drops-terminals.md` — probing an already-registered healthy host forces a reconnect that drops every
  open terminal on it.
- `attach-reports-generic-timeout.md` — the attach step spins 30 s on skew/identity states and reports only "timed out".
- `update-identity-none-plan-confirm-disagree.md` — UPDATE planning accepts a supervisor reporting no identity but
  confirmation always refuses it.
- `reach-misreads-escaped-xdg-config-home.md` — an escaped XDG_CONFIG_HOME (path with a space) is misclassified as
  relative and refused.
- `uninstall-creates-setup-lock-file.md` — uninstall and setup --uninstall create .farhelm-setup.lock (and possibly the
  unit dir) and never report it.
- `interrupt-before-install-record-publish.md` — an interrupt between commit and record publish leaves new binaries with
  the old receipt; uninstall's refusal reads like tampering.
- `setup-build-tree-heuristic-misfires.md` — setup refuses installed binaries under an empty TMPDIR or any path
  containing a directory named target.
- `relative-install-dir-installs-under-cwd.md` — a relative or quoted-~ FARHELM_INSTALL_DIR installs under the current
  directory with misleading messages.
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `desktop-copy-fallback-never-runs.md` — the native clipboard writer never rejects, so the header copy fallback never
  runs and failures show "copied".
- `window-maximize-fence-never-clears.md` — if the window manager ignores the restored maximize, window geometry is
  never tracked for the run.
