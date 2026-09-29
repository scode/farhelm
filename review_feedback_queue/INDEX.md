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
- `no-supervisor-setup-splits-state-dir.md` — setup --no-supervisor leaves setup's own supervisor on the old state dir
  while the helm is pinned to the new one.
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

## Other: correctness, diagnostics, cleanup, or convenience

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
- `update-identity-none-plan-confirm-disagree.md` — UPDATE planning accepts a supervisor reporting no identity but
  confirmation always refuses it.
- `reach-misreads-escaped-xdg-config-home.md` — an escaped XDG_CONFIG_HOME (path with a space) is misclassified as
  relative and refused.
- `uninstall-creates-setup-lock-file.md` — uninstall and setup --uninstall create .farhelm-setup.lock (and possibly the
  unit dir) and never report it.
- `setup-build-tree-heuristic-misfires.md` — setup refuses installed binaries under an empty TMPDIR or any path
  containing a directory named target.
- `header-actions-skip-listing-read.md` — header replace/restart request no listing read, so under build mismatch the
  sidebar keeps deleted/stale rows.
- `desktop-copy-fallback-never-runs.md` — the native clipboard writer never rejects, so the header copy fallback never
  runs and failures show "copied".
- `window-maximize-fence-never-clears.md` — if the window manager ignores the restored maximize, window geometry is
  never tracked for the run.
