# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

## Highest priority: security or data loss

- `stale-dial-outcome-publishes-over-retarget-nudge.md` — editing a host mid-dial briefly routes operations to the old
  machine.
- `stripped-agent-marker-forges-killable-tab.md` — stripped agent marker lets a forged tab kill the live agent.
- `restart-kills-terminal-less-agent-without-consent.md` — restart of a terminal-less session kills a possibly live
  agent without stop consent.
- `tab-close-kills-rc-started-services.md` — tab close, auto-reap and Delete kill services the tab shell's rc files
  started (ssh-agent, personal tmux).
- `sweep-can-claim-supervisor-or-tmux-server.md` — nothing stops the kill sweep from claiming the supervisor itself or
  its private tmux server.
- `nondumpable-daemons-escape-sweep.md` — non-dumpable daemons such as ssh-agent survive stop and delete on hosts
  without a systemd user manager.
- `offline-rotate-creates-fresh-database.md` — `token rotate` against a wrong state dir creates a fresh DB and reports
  success while the real token stays live.
- `ipv4-only-bind-allows-localhost-squat.md` — the helm binds only 127.0.0.1, so another local user can squat [::1] and
  steal the device secret via localhost.
- `supervisor-error-forges-reauth-401.md` — a remote supervisor can forge the helm's device-auth 401 and force the web
  UI to the token prompt.
- `agent-label-leaks-env-prefix.md` — the fleet-wide `agent` label takes the first shell word, leaking a leading
  `KEY=secret` and showing `env` for env-prefixed launches.
- `helm-answers-resolveprofile-to-any-supervisor.md` — the helm answers `ResolveProfile` with raw profile command lines
  to any supervisor; only the asking supervisor refuses it.
- `tmux-cwd-format-expanded-on-create.md` — session create passes the cwd to tmux `-c` unescaped: `#` paths start the
  agent in $HOME and `#(cmd)` names run commands.
- `tmux-cwd-format-expanded-on-relaunch.md` — restart in place passes the cwd to `respawn-pane -c` unescaped, bypassing
  restart's directory identity check.
- `tmux-cwd-format-expanded-on-tab-open.md` — opening a tab passes the session cwd to `new-window -c` unescaped: wrong
  directory or a hidden command runs.
- `supervisor-stop-closes-clients-output-on.md` — the supervisor has no SIGTERM handler, so a normal stop closes every
  output client with output on (tmux-abort risk).
- `checkout-can-take-archive-dir-name.md` — a fresh checkout can be named `farhelm-archived-working-copies`; later
  deletes move checkouts into it and its session is undeletable.
- `archive-root-accepts-active-checkout.md` — the archive move accepts an active managed checkout (or the source itself)
  as its archive directory.
- `inode-reuse-defeats-ownership-check.md` — the `(dev, ino)` ownership check can't tell a recreated directory from the
  original, so Delete can archive a user's folder.
- `archive-dir-owner-not-checked.md` — the archive directory's owner and mode aren't checked, so another account in a
  shared root can capture archives.
- `superseded-launch-specs-never-removed.md` — launch specs of superseded or unconsumed launches (argv + session token)
  stay on disk until Delete.
- `reload-leaves-unread-launch-spec.md` — startup reconciliation cleans launch specs only after Error, so
  Interrupted/Exited launches keep credentials on disk.
- `observers-leave-unread-launch-spec.md` — the ticker, listing and stop observers clean launch specs only after Error,
  leaving credentials on disk.
- `codex-hook-trust-bypass-runs-repo-hooks.md` — every Codex launch passes `--dangerously-bypass-hook-trust`, so a
  trusted repo's own hooks can run unreviewed.
- `create-reply-foreign-id-misroutes.md` — a create reply naming an id another host caches routes the new session's
  operations to that other host.
- `create-reply-sets-remembered-yolo.md` — a remote supervisor's create reply sets the helm-wide remembered
  permissions/workspace-trust defaults.
- `replace-records-peer-launch-defaults.md` — a plain Replace copies the remote row's launch selection into the
  helm-wide remembered permission/trust defaults.
- `replace-sets-peer-default-profile.md` — a plain Replace makes the remote host's claimed profile the helm-wide
  remembered default profile.
- `remote-host-contests-foreign-session-ids.md` — any remote host can make other hosts' sessions refuse every operation
  by listing their ids.
- `remote-unit-overwritten-without-ownership-check.md` — ADD/UPDATE replace an existing remote
  farhelm-supervisor.service (setup-managed or hand-written) with no ownership check.
- `provisioning-chmods-shared-directories.md` — provisioning chmods the existing unit dir, and on UPDATE the binary's
  dir (possibly $HOME), exposing files to other accounts.
- `sftp-misparses-ipv6-and-uri-destinations.md` — the sftp upload parses IPv6-literal and ssh:// destinations as a
  different host than ssh, so the payload can go elsewhere.
- `installer-overwrites-user-farhelm-file.md` — install.sh replaces and deletes any existing regular file named
  farhelm/farhelm-desktop without an ownership check.
- `installer-deletes-farhelm-app-on-grep.md` — install.sh rm -rf's any ~/Applications/Farhelm.app whose Info.plist
  mentions farhelm, ignoring its own receipt.
- `installer-mirror-var-drops-https-no-signature.md` — install.sh honours the helm's FARHELM_RELEASE_BASE_URL, dropping
  HTTPS pinning with no minisign check.
- `app-info-plist-uses-caller-umask.md` — install.sh writes Farhelm.app's Info.plist with the caller's umask, so another
  local account may inject LSEnvironment.
- `setup-pins-relative-path-tmux.md` — helm setup can pin a tmux found via a relative PATH entry like "." into the
  boot-time supervisor unit.
- `install-lock-owner-unchecked.md` — the install lock and journal are trusted without checking their owner, so a
  shared-dir co-user can redirect recovery.
- `delete-ended-session-kills-live-tabs.md` — deleting a session whose agent ended skips confirmation and kills its
  still-running terminal tabs.
- `delete-lacks-liveness-precondition.md` — an unconfirmed delete from a stale ended row can kill a freshly restarted
  agent; DELETE carries no precondition.
- `header-replace-confirm-ignores-cancel.md` — the header Replace confirm lacks the prompt-open check, so
  cancel-then-confirm still replaces (deletes) the session.
- `row-menu-drifts-after-own-delete.md` — an open row menu drifts onto another row after this client's own delete, so
  its Delete can hit the wrong session.
- `session-header-raw-peer-text.md` — the session header shows title/cwd/command raw and copies raw bytes, so bidi text
  can make copied commands differ from shown ones.
- `titles-raw-in-confirm-prompts.md` — titles render unescaped in delete/replace confirmations and rows, so agent-set
  titles can impersonate another session.
- `display-peer-misses-invisible-chars.md` — display_peer's escape list misses invisible characters, so two identities
  can render identically in the adopt prompt.
- `osc8-link-target-never-shown.md` — OSC 8 hyperlinks from terminal output open their hidden target in the browser
  without ever showing it.
- `token-prompt-invites-password-manager.md` — the browser token prompt is a password field, so password managers offer
  to save and sync the master web token.

## High priority: material UX degradation

- `duplicate-freeze-clobbers-retarget-nudge.md` — retargeting a duplicate-frozen host loses the edit until a second
  edit.
- `pi-resume-downgrade-on-read-error.md` — Pi resume check destroys a valid locator when the session file merely fails
  to read.
- `plain-retry-erases-pending-fresh-window.md` — plain retry downgrades a pending fast reconnect to a slow probe.
- `reap-competing-sink-waits-unboundedly.md` — losing a sink-install race can hang an attach forever.
- `refresh-timeout-misses-profile-and-commit-tail.md` — refresh timeout misses the profile and commit tail, freezing a
  host as stale-connected.
- `reload-adopts-stale-pane.md` — reload adopts a stale dead pane as the new generation's terminal.
- `restricted-create-holds-lifecycle-claim-across-reply.md` — restricted create holds the lifecycle claim across the
  reply send.
- `revocation-during-admission-orphans-attachment.md` — revocation racing a slow attach orphans the attachment, pinning
  session ownership.
- `send-upload-ignores-cancellation.md` — the transfer's queue send ignores cancellation, stalling deletes.
- `sftp-overall-deadline-fails-slow-links.md` — sftp transfer's 60 s overall deadline fails slow links
  deterministically.
- `staging-holds-claim-across-unbounded-io.md` — upload staging holds the lifecycle claim across unbounded disk I/O.
- `stale-natural-verdict-kills-replacement-attach.md` — stale natural-end verdict destroys a replacement attachment
  reusing the channel.
- `stale-stall-verdict-kills-replacement-attach.md` — stale stall verdict destroys a replacement attachment reusing the
  channel.
- `stop-outcomes-lost-when-degraded.md` — stops silently lose intent and outcome while the supervisor is degraded.
- `superseded-reap-watchers-never-exit.md` — superseded output-reap watchers never exit, leaking a task per churn cycle.
- `tmux-kill-runs-unbounded-under-global-lock.md` — teardown's tmux calls run unbounded under the global lock.
- `untracked-mutations-leak-on-wedged-tmux.md` — untracked mutations leak permit, claim, and fence against wedged tmux.
- `unvalidated-state-dir-on-add.md` — adding a host with a bad state-dir path permanently bricks the entry.
- `unvalidated-state-dir-on-ensure.md` — a bad state-dir line in the ensure file bricks a host on every boot.
- `unvalidated-state-dir-on-probe.md` — probed registration with a bad state-dir path permanently bricks the entry.
- `delete-lists-scopes-before-reprobe.md` — Delete lists systemd scopes before its re-probe, so a stale verdict skips
  old-run and closed-tab scopes.
- `tab-close-skips-scope-on-stale-verdict.md` — tab close skips the tab's cgroup whenever the cached manager verdict is
  negative, contrary to its docs.
- `cancelled-scope-probe-wedges-verdict.md` — a cancelled systemd probe leaves the verdict stuck at Probing, hanging
  every later lifecycle operation.
- `restart-sweep-on-abortable-connection-task.md` — restart's stop-and-sweep runs on a connection task that a client
  disconnect aborts mid-kill.
- `sigkill-misses-stopped-dropouts.md` — a process SIGSTOPped but missing from the sweep's final enumeration is left
  frozen.
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
- `helm-startup-fails-on-busy-token-lock.md` — the helm (or desktop app) aborts startup if a `token show`/`rotate`
  briefly holds the token-control lock.
- `event-feed-liveness-postponed-by-revisions.md` — the event feed's liveness check never fires on a busy fleet, so dead
  subscribers hold the 64 seats.
- `clipboard-sink-blocks-async-worker.md` — the desktop clipboard endpoint runs blocking clipboard I/O on async workers
  under a global mutex.
- `spawn-reply-returns-raw-profile-invocation.md` — a spawn by profile name replies with the profile's raw command line
  and resume template.
- `agent-fence-wait-unbounded.md` — a mutating agent request can wait up to ten minutes for the delete fence, then run
  after its caller gave up.
- `agent-create-replays-asker-as-child.md` — a keyed `agent create` can report the asking session itself as the newly
  created session.
- `spawn-replays-asker-as-child.md` — a keyed `farhelm spawn` without `--parent` can return the asking session as its
  own new child.
- `helm-link-chosen-by-hashmap-order.md` — agent requests can be routed to a stale half-open helm connection chosen by
  HashMap order.
- `spawn-holds-global-mutex-waiting-parent.md` — a spawn waiting on its busy parent's lifecycle claim holds the
  host-wide create/delete mutex.
- `clone-discloses-source-invocation.md` — cloning a raw-invocation session onto the asker's host discloses the source's
  full command line.
- `tmux-run-bytes-unbounded-under-lock.md` — one-shot tmux commands have no timeout, and resize runs one under the
  supervisor-wide attachments lock.
- `sink-shutdown-retries-forever-after-delete.md` — deleting a busy session can leave its sink client stuck in an
  endless shutdown-retry loop.
- `tab-input-starts-capture-clock.md` — typing into a tab starts the agent's capture clock, so the conversation can go
  uncaptured or ambiguous.
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
- `untitled-checkout-naming-ignores-registry.md` — untitled checkout naming ignores registry-claimed folders, so preview
  proposes a name the nested-checkout rule rejects.
- `checkout-name-scan-case-sensitive.md` — the checkout name scan is case-sensitive, so on macOS a case-variant folder
  makes every launch conflict.
- `delete-holds-directory-lock-whole-teardown.md` — every Delete holds the host-wide directory lock through its whole
  teardown, stalling creates and restarts.
- `conversation-program-bricks-supervisor.md` — a create whose program is `{conversation}` stores a resume template the
  loader refuses, so the supervisor can't start.
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
- `startup-sweep-keeps-staged-spec-copies.md` — the startup sweep keeps staged `.tmp-` copies of launch specs for live
  sessions.
- `idempotency-fingerprint-keeps-raw-cmdline.md` — idempotency fingerprints keep the raw command line (with any keys) in
  the database after Delete.
- `second-helm-migrates-before-owner-lock.md` — a second helm on another port migrates and writes the live helm's
  database before it discovers the owner lock is taken.
- `provisioning-holds-host-cache-lock.md` — a provisioning run holds the host's cache-write lock throughout, freezing
  its list and hanging session-action replies.
- `adopt-checks-current-row-not-dialed.md` — adopt checks the manager's current row, so a stale mismatch after a
  retarget adopts the old machine's identity.
- `identity-mismatch-never-becomes-duplicate.md` — a host that reaches another entry's identity offers an adopt that
  always 409s and never shows as duplicate.
- `ssh-controlpath-too-long.md` — the ssh ControlPath under the state dir overflows sun_path for common long usernames,
  so every ssh host fails.
- `update-installs-tmux-into-shared-bin.md` — UPDATE installs Farhelm's private tmux next to the registered binary (e.g.
  ~/.local/bin), overwriting or shadowing the user's tmux.
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
- `probe-misses-install-sh-supervisor.md` — the probe misses a farhelm in ~/.local/bin off the ssh PATH and offers to
  install a second, crash-looping copy.
- `linger-failure-blocks-update-restart.md` — the optional linger step runs before restart/attach and most loginctl
  failures are fatal, so UPDATE never restarts.
- `remote-mode-check-reads-symlink-mode.md` — the remote metadata check hashes a symlink's target but reads the link's
  mode, chmodding through links or replacing them.
- `embedded-payload-cleanup-blocks-helm-start.md` — a failure deleting the retired embedded-payloads cache aborts helm
  startup.
- `installer-lock-flag-rolls-back-other-install.md` — install.sh keeps LOCK_ACQUIRED=1 after releasing the lock, so its
  exit handler can roll back a concurrent installer's live transaction.
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
- `other-install-bundle-refusal-advice.md` — when the app bundle belongs to another install, uninstall advises rerunning
  the installer, which takes the bundle from that install.
- `desktop-401-retry-sends-revoked-secret.md` — the desktop 401 retry appends a second Authorization header, so the helm
  reads the revoked one and the retry fails.
- `desktop-reauth-remount-loses-action.md` — the desktop credential refresh remounts the app before the retry runs,
  silently losing the action that hit the 401.
- `desktop-reauth-failure-dead-end.md` — a transient webview re-auth failure after rotation leaves only an error line
  with no retry until relaunch.
- `session-view-leaks-page-lock.md` — the session view's restart/replace claim is not released on unmount, leaving every
  page action disabled until reload.
- `remembered-lookup-overrides-selection.md` — a late remembered-selection lookup replaces the session the user clicked
  and retires the id on transient errors.
- `header-replace-prompt-omits-kill-warning.md` — the header Replace confirmation never says a running agent and its
  tabs will be killed.
- `row-menu-drifts-on-row-height-change.md` — an open row menu can float over a different row when a row above gains a
  detail line at the same index.
- `uploads-aborted-silently-on-remount.md` — terminal reconnect or restart remount silently aborts in-flight uploads
  with no message.
- `clipboard-facts-label-stuck.md` — any paste leaves an unclickable "clipboard facts" label over the terminal's
  bottom-right corner.
- `desktop-bootstrap-token-always-pushed.md` — the desktop webview receives the master web token on every (re)auth, even
  when its stored secret is valid.

## Other: correctness, diagnostics, cleanup, or convenience

- `quarantine-sweep-silently-aborts.md` — the quarantine sweep silently stops at the first unreadable entry.
- `redirect-hop-bound-off-by-one.md` — redirect hop bound enforces 4 hops while the policy documents 5.
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
- `token-rotate-timeout-after-commit.md` — `token rotate` can report a timeout while the helm completes the rotation and
  logs out every browser.
- `token-show-busy-lock-misleading-failure.md` — `token show` assumes any lock holder is a serving helm with a token and
  fails with a misleading error.
- `event-feed-cap-refusal-invisible.md` — the event feed's subscriber-cap refusal is a pre-upgrade 503 that browsers
  cannot observe.
- `revoked-attach-timeout-leaves-attachment.md` — cancelling a revoked terminal attach can leave the supervisor-side
  attachment with no owner.
- `origin-check-ignores-scheme-on-port-80.md` — the origin check ignores the scheme, so a port-80 helm accepts
  `https://127.0.0.1` as its own origin.
- `any-dioxus-webview-passes-origin-guard.md` — any Dioxus or wry app's webview origin passes the helm's origin guard
  and CORS, not just Farhelm's.
- `terminal-query-refused-before-upgrade.md` — a bad `?cols=`/`?rows=` is refused with HTTP 400 before the upgrade,
  contrary to the on-socket refusal promise.
- `client-log-drops-claimed-counted.md` — the client-log drop warning says later drops are counted, but nothing counts
  them.
- `web-token-docstring-spliced.md` — `HelmStore::web_token`'s docstring opens with a spliced fragment claiming it
  inserts a token.
- `unauthenticated-bearer-contends-sqlite.md` — any unauthenticated Bearer value is hashed and looked up in SQLite,
  letting a local flood contend the DB lock.
- `no-helm-spawn-refusal-wrong-remedy.md` — the no-helm refusal for a named spawn advises omitting --agent, which also
  fails; the remedy is --inherit-agent.
- `clone-empty-cwd-passes-supervisor-check.md` — `agent clone --cwd ""` passes the supervisor's relay check and can burn
  an idempotency key.
- `clone-empty-cwd-passes-helm-check.md` — the helm's own clone validation accepts an empty cwd.
- `clone-refusal-names-wrong-flag.md` — clone refusals for a bad source id name `--session` and tell the agent to name
  itself.
- `clone-source-missing-from-truncated-list.md` — `agent clone` reports the source gone when the source host's list was
  truncated at 500.
- `credential-refusal-surfaces-as-broken-pipe.md` — a session-credential refusal can reach the CLI as "Broken pipe"
  instead of the real error.
- `uncredentialed-agent-request-never-answered.md` — an `AgentRequest` sent without a session credential gets no reply
  and hangs its sender.
- `spawn-lookup-timeout-message-misleads.md` — a timed-out spawn profile lookup says a retry may repeat the request,
  though nothing was created.
- `conversation-id-logged-raw-on-store-error.md` — an unchecked reported conversation id is written raw into a log line
  when the session read fails.
- `pi-locator-newlines-reach-logs.md` — Pi conversation locators are logged as sent and can carry raw newlines into
  supervisor logs.
- `output-client-shutdown-can-retry-forever.md` — the per-terminal output client can get stuck retrying shutdown when
  its session disappears while paused.
- `output-client-name-wrong-behind-wrapper.md` — output-client shutdown targets `client-<spawned pid>`, which never
  matches behind a non-exec tmux wrapper.
- `sink-client-name-wrong-behind-wrapper.md` — the session sink's shutdown has the same `client-<pid>` assumption behind
  a non-exec tmux wrapper.
- `startup-reap-misnames-terminal-clients.md` — the startup reap addresses leftover control clients as `client-<pid>`,
  missing terminal-backed ones and failing startup.
- `input-client-attaches-by-bare-pane.md` — the input client attaches by bare pane id, making a tab the session's
  current window.
- `tab-close-kills-before-detach.md` — closing a tab kills its window before detaching its viewer, producing a spurious
  "terminal input failed".
- `pane-states-skips-markers-single-window.md` — `pane_states` skips tab markers when no session has two windows, hiding
  a surviving tab.
- `tmux-probe-can-hang-on-setsid-child.md` — `probe_tmux` can hang forever on a setsid'd descendant holding its pipes,
  despite its bounded-probe promise.
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
- `failed-create-rollback-row-unpublished.md` — when a failed create's rollback fails, the kept row is never published
  and can't be listed or deleted.
- `abort-relaunch-failure-status-desync.md` — a failed restore after a failed restart leaves displayed and stored status
  out of sync.
- `create-replay-codex-offer-conflict.md` — a create replay can fail with an unrelated "Codex restart offer changed"
  Conflict.
- `create-mark-window-failure-drops-pane.md` — when marking the agent window fails at create, the pane is dropped and a
  running agent has no terminal.
- `canonicalize-failure-stores-literal-cwd.md` — a failed canonicalization at create stores the literal path as
  verified, blocking later restarts.
- `reused-pane-dead-treated-definitive.md` — a failed relaunch into a reused pane treats a dead pane as "respawn never
  happened".
- `claude-scan-budget-never-settles.md` — Claude record-scan capture can never complete in a large project directory and
  rescans forever.
- `capture-ambiguity-flags-report-only-kinds.md` — the capture pass marks report-only agents ambiguous and logs false
  "can't capture" warnings.
- `env-wrapper-hides-command-not-found.md` — Farhelm's own `env` wrapper turns "command not found" for Goose, Pi and OMP
  into exited (127) instead of error.
- `conversation-id-misses-codex-placeholders.md` — the conversation-id plausibility check misses
  `{codex:trusted-cwd}`/`{codex:untrusted-cwd}` placeholders.
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
- `identityless-early-create-unroutable.md` — a session created on a just-connected identity-less host is not recorded
  and 404s until the first refresh.
- `host-edits-not-cancellation-safe.md` — a client disconnect between a host edit's commit and its reconcile leaves the
  actor out of sync until restart.
- `add-host-rollback-leaves-actor.md` — a failed add-host rollback deletes the row but not an actor a concurrent
  reconcile already started.
- `hostnotfound-refresh-keeps-serving.md` — a HostNotFound refresh does not end the connection, so an actor for a
  deleted host keeps serving.
- `actor-respawn-skips-client-retire.md` — respawn/revive aborts the old supervisor task without retiring a
  still-published client.
- `nudge-break-skips-cache-bump.md` — a cache change committed while a nudge is pending is never announced on the event
  feed.
- `alias-change-may-not-bump-feed.md` — an alias-only registry change can skip the fleet-revision bump, leaving other
  clients on the old name.
- `session-detail-drains-full-list.md` — every fleet-revision bump makes each open session view trigger a full
  ListSessions on its host.
- `incarnation-counter-restarts-per-process.md` — incarnation numbers restart each helm process, so expected_incarnation
  can match a different install after restart.
- `replace-keeps-session-seen-row.md` — replace deletes the source without dropping its session_seen row, unlike delete.
- `remote-state-dir-unvalidated.md` — remote_state_dir is stored unchecked (empty/NUL), so the host registers but never
  connects.
- `aggregate-duplicates-identityless-id.md` — an identity-less host's live rows are not deduplicated against cached
  claims, listing one session twice.
- `replace-leaks-claimed-profile-invocation.md` — Replace launches whichever catalog profile a remote host claims,
  sending its full command line to that host.
- `restart-rename-reply-id-unchecked.md` — restart and rename replies are recorded and returned without checking they
  describe the requested session.
- `list-ingress-id-validation-gap.md` — session-list ingress admits empty, control-character and dot-segment ids, and
  the UI's %2E guard does not hold.
- `reconciled-checkout-skips-created-session.md` — a reconciled fresh-checkout reply bypasses created_session's id
  validation.
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
- `install-step-error-leaks-uploaded-temp.md` — install_uploaded_source returns before removing the uploaded temporary
  when its metadata read or mode repair fails.
- `tilde-remote-paths-never-expand.md` — a ~/ remote_farhelm or remote_state_dir is single-quoted and never expands, so
  the host never connects.
- `reach-misreads-escaped-xdg-config-home.md` — an escaped XDG_CONFIG_HOME (path with a space) is misclassified as
  relative and refused.
- `planning-refusal-returns-502.md` — planning refusals (relative remote_farhelm) surface as 502 Bad Gateway instead of
  the typed 409.
- `redirect-limit-off-by-one.md` — the release download refuses the fifth redirect though five are documented.
- `release-download-unbounded-under-host-lock.md` — release downloads have no overall deadline and run under the host
  lock, blocking removal while throttled.
- `e2e-backend-gate-lexical-prefix.md` — the shipped E2E provisioning gate checks the state-dir prefix lexically, so ..
  or a symlink escapes it.
- `uninstall-creates-setup-lock-file.md` — uninstall and setup --uninstall create .farhelm-setup.lock (and possibly the
  unit dir) and never report it.
- `installer-stale-lock-own-pid-live.md` — a stale lock carrying the current shell's pid is treated as live, so every
  later run refuses.
- `interrupt-before-install-record-publish.md` — an interrupt between commit and record publish leaves new binaries with
  the old receipt; uninstall's refusal reads like tampering.
- `setup-build-tree-heuristic-misfires.md` — setup refuses installed binaries under an empty TMPDIR or any path
  containing a directory named target.
- `relative-install-dir-installs-under-cwd.md` — a relative or quoted-~ FARHELM_INSTALL_DIR installs under the current
  directory with misleading messages.
- `setup-pins-relative-xdg-state-home.md` — helm setup writes a relative XDG_STATE_HOME into both units, so services and
  token show use different state dirs.
- `setup-restart-marker-read-error-ignored.md` — setup treats an unreadable restart marker as no restart owed and
  reports success.
- `installer-extract-exits-silently.md` — extract_sole_member exits silently under set -e instead of printing its
  refusal.
- `hook-log-truncation-erases-concurrent-line.md` — hook-log truncation races a concurrent append and can erase another
  hook's line.
- `grok-double-null-field-refused.md` — a Grok callback with both spellings of an optional field set to null is refused
  outright.
- `goose-missing-session-id-not-logged.md` — the Goose reporter neither reports nor logs when AGENT_SESSION_ID is
  missing.
- `grok-prompt-hooks-exceed-payload-cap.md` — the hook's small-payload assumption may not hold for Grok's prompt/stop
  callbacks.
- `desktop-auth-check-no-deadline.md` — desktop-auth.js's credential check has no timeout, so startup can hang on
  "Starting Farhelm…".
- `row-ops-counter-leaks-on-unmount.md` — row_ops leaks when the browser token prompt unmounts the list mid-operation,
  disabling header actions.
- `restart-prompt-claims-running-when-unknown.md` — the header restart prompt says "still running" for an Unknown
  status.
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `sidebar-rename-not-reflected-in-header.md` — renaming the open session never updates its header, which reads a
  mount-time copy.
- `deleted-row-reappears-without-read-fence.md` — delete doesn't fence older listing reads, so an in-flight read can
  briefly restore the deleted row.
- `desktop-copy-fallback-never-runs.md` — the native clipboard writer never rejects, so the header copy fallback never
  runs and failures show "copied".
- `replace-reply-missing-host-fields.md` — plain Replace stores the bare helm reply without host fields, so New defaults
  to the local machine.
- `seen-queue-drops-manual-report.md` — the seen-state write queue drops the newer caller's report when the same value
  is re-queued.
- `tab-close-errors-linger.md` — tab-close errors for tabs that have since disappeared stay on screen for the view's
  life.
- `desktop-startup-misleading-helm-refusal.md` — late desktop startup failures can be reported as "embedded helm stopped
  unexpectedly".
- `window-maximize-fence-never-clears.md` — if the window manager ignores the restored maximize, window geometry is
  never tracked for the run.
- `copy-feedback-timer-cut-short.md` — a second header copy click cuts the "copied" feedback short.
- `session-view-errors-raw-peer-text.md` — session-view restart/replace/tab error lines render supervisor refusal text
  without peer escaping.
- `row-tooltips-raw-peer-text.md` — the sidebar row's directory and host tooltips are raw while the title tooltip is
  escaped.
- `csp-limits-only-framing.md` — the helm's CSP only sets frame-ancestors, so an injection could exfiltrate the device
  secret anywhere.
