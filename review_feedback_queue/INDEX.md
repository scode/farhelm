# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

## Highest priority: security or data loss

- `titles-raw-in-confirm-prompts.md` — titles render unescaped in delete/replace confirmations and rows, so agent-set
  titles can impersonate another session.
- `display-peer-misses-invisible-chars.md` — display_peer's escape list misses invisible characters, so two identities
  can render identically in the adopt prompt.
- `osc8-link-target-never-shown.md` — OSC 8 hyperlinks from terminal output open their hidden target in the browser
  without ever showing it.
- `token-prompt-invites-password-manager.md` — the browser token prompt is a password field, so password managers offer
  to save and sync the master web token.

## High priority: material UX degradation

- `pi-resume-downgrade-on-read-error.md` — Pi resume check destroys a valid locator when the session file merely fails
  to read.
- `plain-retry-erases-pending-fresh-window.md` — plain retry downgrades a pending fast reconnect to a slow probe.
- `refresh-timeout-misses-profile-and-commit-tail.md` — refresh timeout misses the profile and commit tail, freezing a
  host as stale-connected.
- `reload-adopts-stale-pane.md` — reload adopts a stale dead pane as the new generation's terminal.
- `revocation-during-admission-orphans-attachment.md` — revocation racing a slow attach orphans the attachment, pinning
  session ownership.
- `send-upload-ignores-cancellation.md` — the transfer's queue send ignores cancellation, stalling deletes.
- `sftp-overall-deadline-fails-slow-links.md` — sftp transfer's 60 s overall deadline fails slow links
  deterministically.
- `staging-holds-claim-across-unbounded-io.md` — upload staging holds the lifecycle claim across unbounded disk I/O.
- `stop-outcomes-lost-when-degraded.md` — stops silently lose intent and outcome while the supervisor is degraded.
- `superseded-reap-watchers-never-exit.md` — superseded output-reap watchers never exit, leaking a task per churn cycle.
- `tmux-kill-runs-unbounded-under-global-lock.md` — teardown's tmux calls run unbounded under the global lock.
- `untracked-mutations-leak-on-wedged-tmux.md` — untracked mutations leak permit, claim, and fence against wedged tmux.
- `unvalidated-state-dir-on-probe.md` — probed registration with a bad state-dir path permanently bricks the entry.
- `tab-close-skips-scope-on-stale-verdict.md` — tab close skips the tab's cgroup whenever the cached manager verdict is
  negative, contrary to its docs.
- `restart-sweep-on-abortable-connection-task.md` — restart's stop-and-sweep runs on a connection task that a client
  disconnect aborts mid-kill.
- `crash-mid-sweep-leaves-frozen-tree.md` — a supervisor death between SIGSTOP and SIGKILL leaves the tree frozen and
  listed as running.
- `delete-roots-only-agent-pane.md` — Delete roots its process walk only on the agent pane, so tab, split-pane and
  terminal-less processes survive.
- `split-tab-close-reaps-one-pane.md` — closing a hand-split tab reaps only one pane's process tree.
- `title-rewrite-erases-sweep-marker.md` — programs that rewrite their process title erase the session marker and escape
  stop and delete.
- `ticker-retries-unkillable-tab-reaps.md` — the ticker retries unkillable dead-tab reaps with full graces every tick,
  holding the lifecycle claim.
- `delete-serial-scope-kills-under-global-lock.md` — Delete kills scope units serially with multi-second bounds while
  holding the global directory lock.
- `agent-created-tmux-windows-never-reaped.md` — windows the agent opens on the private tmux server escape stop and
  restart; Delete only SIGHUPs them.
- `restart-relaunches-over-unconfirmed-scope.md` — restart relaunches even when the previous run's cgroup could not be
  confirmed empty.
- `same-site-navigation-passes-origin-guard.md` — the navigation guard rejects only cross-site, so a page on another
  localhost port can open the helm and take a terminal.
- `event-feed-liveness-postponed-by-revisions.md` — the event feed's liveness check never fires on a busy fleet, so dead
  subscribers hold the 64 seats.
- `clipboard-sink-blocks-async-worker.md` — the desktop clipboard endpoint runs blocking clipboard I/O on async workers
  under a global mutex.
- `spawn-reply-returns-raw-profile-invocation.md` — a spawn by profile name replies with the profile's raw command line
  and resume template.
- `agent-create-replays-asker-as-child.md` — a keyed `agent create` can report the asking session itself as the newly
  created session.
- `spawn-replays-asker-as-child.md` — a keyed `farhelm spawn` reusing another session's key can be handed that session's
  child as its own new child.
- `spawn-holds-global-mutex-waiting-parent.md` — a spawn waiting on its busy parent's lifecycle claim holds the
  host-wide create/delete mutex.
- `clone-discloses-source-invocation.md` — cloning a raw-invocation session onto the asker's host discloses the source's
  full command line.
- `tmux-run-bytes-unbounded-under-lock.md` — one-shot tmux commands other than resize have no timeout, so an
  unresponsive tmux server can hang startup and per-connection requests.
- `sink-shutdown-retries-forever-after-delete.md` — deleting a busy session can leave its sink client stuck in an
  endless shutdown-retry loop.
- `input-client-notifications-pile-up.md` — idle input clients never read tmux notifications, so tmux server memory
  grows per open terminal.
- `csh-login-shell-breaks-agent-launch.md` — the agent launch form `$SHELL -l -i -c …` may fail under csh/tcsh login
  shells.
- `csh-login-shell-breaks-tab-launch.md` — the tab launch form `<shell> -l -i` may fail under csh/tcsh login shells.
- `missing-checkout-root-blocks-delete.md` — a removed, recreated or unmounted checkout root makes its sessions
  permanently undeletable.
- `unstable-device-number-blocks-delete.md` — the ownership check treats `st_dev` as stable, so a remount can make
  checkout sessions undeletable and unrestartable.
- `noreplace-rename-unsupported-strands-archive.md` — on filesystems without no-replace rename, the last Delete leaves
  the checkout stuck `archive_pending` forever.
- `failed-plan-does-not-reserve-checkout-name.md` — a failed clone's plan doesn't reserve its folder name, so a later
  clone is silently co-owned and archived early.
- `checkout-name-scan-case-sensitive.md` — the checkout name scan is case-sensitive, so on macOS a case-variant folder
  makes every launch conflict.
- `delete-holds-directory-lock-whole-teardown.md` — every Delete holds the host-wide directory lock through its whole
  teardown, stalling creates and restarts.
- `restart-refused-after-stopping-agent.md` — a consented restart can stop the agent and then refuse to relaunch because
  the offer changed during the stop.
- `create-retry-duplicates-agent-without-manager.md` — without a systemd user manager, a create retry can launch beside
  the first attempt's surviving processes.
- `ambiguous-restart-republishes-old-terminal.md` — an ambiguous restart failure republishes the pre-restart terminal,
  so a live agent is unopenable and killable without consent.
- `second-restart-holds-directory-lock.md` — a second restart of the same session holds the host-wide directory lock
  while waiting for the first.
- `ticker-waits-on-lifecycle-claim.md` — the ticker waits on a session's lifecycle claim, so one stop/restart/delete
  freezes status for every session.
- `launch-path-prepends-binary-directory.md` — the launch shim prepends Farhelm's whole binary directory to PATH,
  shadowing the user's claude/tmux.
- `idempotency-fingerprint-keeps-raw-cmdline.md` — idempotency fingerprints keep the raw command line (with any keys) in
  the database after Delete.
- `provisioning-holds-host-cache-lock.md` — a provisioning run holds the host's cache-write lock throughout, freezing
  its list and hanging session-action replies.
- `adopt-checks-current-row-not-dialed.md` — adopt checks the manager's current row, so a stale mismatch after a
  retarget adopts the old machine's identity.
- `identity-mismatch-never-becomes-duplicate.md` — a host that reaches another entry's identity offers an adopt that
  always 409s and never shows as duplicate.
- `ssh-controlpath-too-long.md` — the ssh ControlPath under the state dir overflows sun_path for common long usernames,
  so every ssh host fails.
- `update-installs-tmux-into-shared-bin.md` — provisioning replaces an existing file at the private tmux destination
  without checking whether Farhelm installed it.
- `sftp-upload-unbounded-before-temp-appears.md` — the sftp upload has no deadline until the remote temporary appears,
  holding the host lock so even Remove hangs.
- `update-reports-success-on-hand-started-supervisor.md` — UPDATE of a hand-started supervisor crash-loops the new unit
  and can report success while the old build keeps serving.
- `update-ignores-remote-xdg-state-home.md` — UPDATE with no recorded state dir pins ~/.local/state/farhelm, so on
  XDG_STATE_HOME hosts the supervisor restarts where the helm cannot reach it.
- `add-ignores-requested-and-recorded-paths.md` — ADD plans the default layout and rewrites the row, ignoring typed or
  recorded remote_farhelm/remote_state_dir.
- `payload-dir-must-be-writable.md` — --payload-dir writes .extracted/ into the operator's directory, so read-only or
  shared release dirs fail every run.
- `probe-misses-install-sh-supervisor.md` — the probe misses a supervisor whose farhelm is in a custom install directory
  and offers to install a second, crash-looping copy.
- `linger-failure-blocks-update-restart.md` — the optional linger step runs before restart/attach and most loginctl
  failures are fatal, so UPDATE never restarts.
- `remote-mode-check-reads-symlink-mode.md` — installing over a symlinked binary or unit destination replaces the
  symlink with a regular file.
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
- `clipboard-facts-label-stuck.md` — any paste leaves an unclickable "clipboard facts" label over the terminal's
  bottom-right corner.
- `desktop-bootstrap-token-always-pushed.md` — the desktop webview receives the master web token on every (re)auth, even
  when its stored secret is valid.

## Other: correctness, diagnostics, cleanup, or convenience

- `refresh-publish-races-retarget-in-check-then-act-gap.md` — retarget in the check-then-publish gap is briefly
  overwritten by the old connection.
- `same-version-cache-generations-never-pruned.md` — same-version payload cache generations from other base URLs are
  never pruned.
- `seed-write-validates-handle-outside-publish.md` — session seed validates the handle outside the publish, briefly
  404ing new sessions.
- `failed-stop-leaves-stale-stop-intent.md` — a failed stop leaves StopRequested, so the agent's later natural exit is
  labelled a user stop.
- `stop-terminal-less-records-exit-before-kill.md` — stopping a terminal-less session records a plain exit before
  killing a possibly live agent.
- `sweep-drops-unreadable-root-silently.md` — the sweep silently drops a pane root whose identity read fails and can
  still report success.
- `token-show-busy-lock-misleading-failure.md` — `token show` assumes any lock holder is a serving helm with a token and
  fails with a misleading error.
- `event-feed-cap-refusal-invisible.md` — the event feed's subscriber-cap refusal is a pre-upgrade 503 that browsers
  cannot observe.
- `revoked-attach-timeout-leaves-attachment.md` — cancelling a revoked terminal attach can leave the supervisor-side
  attachment with no owner.
- `any-dioxus-webview-passes-origin-guard.md` — any Dioxus or wry app's webview origin passes the helm's origin guard
  and CORS, not just Farhelm's.
- `terminal-query-refused-before-upgrade.md` — a bad `?cols=`/`?rows=` is refused with HTTP 400 before the upgrade,
  contrary to the on-socket refusal promise.
- `unauthenticated-bearer-contends-sqlite.md` — any unauthenticated Bearer value is hashed and looked up in SQLite,
  letting a local flood contend the DB lock.
- `clone-source-missing-from-truncated-list.md` — `agent clone` reports the source gone when the source host's list was
  truncated at 500.
- `credential-refusal-surfaces-as-broken-pipe.md` — a session-credential refusal can reach the CLI as "Broken pipe"
  instead of the real error.
- `spawn-lookup-timeout-message-misleads.md` — a timed-out spawn profile lookup says a retry may repeat the request,
  though nothing was created.
- `output-client-shutdown-can-retry-forever.md` — the per-terminal output client can get stuck retrying shutdown when
  its session disappears while paused.
- `output-client-name-wrong-behind-wrapper.md` — output-client shutdown targets `client-<spawned pid>`, which never
  matches behind a non-exec tmux wrapper.
- `sink-client-name-wrong-behind-wrapper.md` — the session sink's shutdown has the same `client-<pid>` assumption behind
  a non-exec tmux wrapper.
- `startup-reap-misnames-terminal-clients.md` — the startup reap addresses leftover control clients as `client-<pid>`,
  missing terminal-backed ones and failing startup.
- `tab-close-kills-before-detach.md` — closing a tab kills its window before detaching its viewer, producing a spurious
  "terminal input failed".
- `pane-states-skips-markers-single-window.md` — `pane_states` skips tab markers when no session has two windows, hiding
  a surviving tab.
- `startup-tmux-version-check-unbounded.md` — the supervisor's startup `tmux -V` check has no time or output limit.
- `tab-session-token-in-tmux-argv.md` — opening a tab puts the session token on the tmux client's command line, briefly
  visible to other accounts.
- `pre-mkdir-rollback-leaves-phantom-membership.md` — a pre-mkdir create rollback leaves a phantom membership in another
  checkout, so it is never archived.
- `checkout-membership-misses-bind-mounts.md` — checkout membership is path-text based, so a session reaching the
  checkout via a bind mount doesn't keep it.
- `attachment-discard-under-global-lock.md` — Delete removes a session's attachments recursively while holding the
  supervisor-wide attachments lock.
- `preserved-plan-diagnostic-log-only.md` — the "unresolved plan, path preserved" diagnostic goes only to the log; the
  Delete reply is plain success.
- `checkout-path-too-long-for-archive.md` — admitted checkout paths near PATH_MAX are too long for the archive
  destination, blocking Delete.
- `reload-stale-pane-records-old-exit.md` — after a crash mid-restart, reload attaches the old dead pane and the ticker
  records its exit against the new run.
- `reload-false-never-started-error.md` — after a supervisor exit during restart, reload records a false "agent was
  never started" error.
- `creates-accepted-while-boot-id-unreadable.md` — creates are accepted while the boot id is unreadable and are later
  misclassified as interrupted.
- `create-replay-codex-offer-conflict.md` — a create replay can fail with an unrelated "Codex restart offer changed"
  Conflict.
- `reused-pane-dead-treated-definitive.md` — a failed relaunch into a reused pane treats a dead pane as "respawn never
  happened".
- `claude-scan-budget-never-settles.md` — Claude record-scan capture can never complete in a large project directory and
  rescans forever.
- `env-wrapper-hides-command-not-found.md` — Farhelm's own `env` wrapper turns "command not found" for Goose, Pi and OMP
  into exited (127) instead of error.
- `host-write-lock-split-on-actor-respawn.md` — the per-host write lock lives on the actor handle, so a respawn lets
  edits run during provisioning.
- `alias-edit-waits-on-provisioning.md` — renaming a host alias blocks behind a provisioning run although the alias
  needs no registry lock.
- `identityless-refresh-publishes-outside-lock.md` — an identity-less host's refresh publishes after releasing the cache
  lock, so a concurrent create or delete can be undone.
- `cancelled-refresh-overwrites-seed.md` — a refresh cancelled by a nudge still commits its pre-create snapshot over a
  seed that landed meanwhile.
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
- `replace-leaks-claimed-profile-invocation.md` — Replace launches whichever catalog profile a remote host claims,
  sending its full command line to that host.
- `list-ingress-id-validation-gap.md` — session-list ingress admits empty, control-character and dot-segment ids, and
  the UI's %2E guard does not hold.
- `add-confirm-rewrites-row-before-busy-check.md` — confirming ADD rewrites an existing row and reconnects before the
  busy check, moving an in-flight UPDATE's host.
- `cancelled-start-run-leaves-host-busy.md` — dropping the confirm request inside start_run leaves the host busy (409)
  until the helm restarts.
- `update-reenables-unit-and-linger.md` — every UPDATE re-runs enable --now and enable-linger, silently undoing a user's
  choice to disable them.
- `update-breaks-install-sh-receipt.md` — UPDATE replaces an install.sh-owned binary, so uninstall on that host refuses
  on a digest mismatch.
- `provisioning-ignores-unit-drop-ins.md` — provisioning never reads unit drop-ins, so an override can keep a different
  binary/tmux/state dir running.
- `probe-reregister-drops-terminals.md` — probing an already-registered healthy host forces a reconnect that drops every
  open terminal on it.
- `attach-reports-generic-timeout.md` — the attach step spins 30 s on skew/identity states and reports only "timed out".
- `update-identity-none-plan-confirm-disagree.md` — UPDATE planning accepts a supervisor reporting no identity but
  confirmation always refuses it.
- `sha256sums-part-repair-race.md` — concurrent cache-manifest repairs race on the fixed SHA256SUMS.part name and fail a
  run.
- `failed-download-leaves-part-file.md` — a failed asset download leaves up to 1 GiB of .part in helm state until
  restart.
- `tilde-remote-paths-never-expand.md` — a ~/ remote_farhelm or remote_state_dir is single-quoted and never expands, so
  the host never connects.
- `reach-misreads-escaped-xdg-config-home.md` — an escaped XDG_CONFIG_HOME (path with a space) is misclassified as
  relative and refused.
- `release-download-unbounded-under-host-lock.md` — release downloads have no overall deadline and run under the host
  lock, blocking removal while throttled.
- `e2e-backend-gate-lexical-prefix.md` — the shipped E2E provisioning gate checks neither the directory's ownership nor
  the build type.
- `uninstall-creates-setup-lock-file.md` — uninstall and setup --uninstall create .farhelm-setup.lock (and possibly the
  unit dir) and never report it.
- `interrupt-before-install-record-publish.md` — an interrupt between commit and record publish leaves new binaries with
  the old receipt; uninstall's refusal reads like tampering.
- `setup-build-tree-heuristic-misfires.md` — setup refuses installed binaries under an empty TMPDIR or any path
  containing a directory named target.
- `relative-install-dir-installs-under-cwd.md` — a relative or quoted-~ FARHELM_INSTALL_DIR installs under the current
  directory with misleading messages.
- `hook-log-truncation-erases-concurrent-line.md` — hook-log truncation races a concurrent append and can erase another
  hook's line.
- `grok-double-null-field-refused.md` — a Grok callback with both spellings of an optional field set to null is refused
  outright.
- `grok-prompt-hooks-exceed-payload-cap.md` — the hook's small-payload assumption may not hold for Grok's prompt/stop
  callbacks.
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `desktop-copy-fallback-never-runs.md` — the native clipboard writer never rejects, so the header copy fallback never
  runs and failures show "copied".
- `tab-close-errors-linger.md` — tab-close errors for tabs that have since disappeared stay on screen for the view's
  life.
- `window-maximize-fence-never-clears.md` — if the window manager ignores the restored maximize, window geometry is
  never tracked for the run.
- `csp-limits-only-framing.md` — the helm's CSP only sets frame-ancestors, so an injection could exfiltrate the device
  secret anywhere.
