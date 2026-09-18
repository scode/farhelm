# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

- `abandon-upload-waits-unboundedly.md` — transfer cleanup waits unboundedly, pinning claims and blocking deletes.
- `abandoned-publish-reports-failure.md` — a commit that times out still publishes while reporting failure.
- `adopt-publishes-unconditionally-after-commit.md` — adopt overwrites state after commit, resurrecting a retired actor
  as connecting.
- `adopt-request-silently-ignores-unknown-fields.md` — typo'd extra fields in an adopt request are silently dropped.
- `agent-fence-claimed-before-validation.md` — fence claimed before pure validation, parking the read loop on malformed
  requests.
- `agent-label-empty-cell-on-trailing-slash.md` — trailing-slash invocation renders a blank agent name in the session
  list.
- `agentrequest-refusals-hold-fence-across-reply.md` — AgentRequest refusals hold the per-session fence across the reply
  send.
- `ambiguous-restart-misattributes-exit.md` — ambiguously failed restart records the old run's death as the new
  generation's exit.
- `archive-discards-stopped-agent-exit-code.md` — archive drops a stopped agent's exit code and can misattribute a
  natural exit.
- `archive-drops-permit-before-reply.md` — archive drops its admission permit before the metadata rebuild and reply.
- `archived-retry-resurrects-session.md` — retried create resurrects an archived session and launches a stood-down
  agent.
- `attach-refuses-tombstoned-channel.md` — Attach refuses a channel that only holds a finished upload's tombstone.
- `cached-session-skips-created-at-check.md` — cached session detail shows a row the list hides as poison.
- `cancelled-request-leaks-pending-entry.md` — cancelled request without a reply leaks its pending entry for the
  connection's lifetime.
- `clearing-local-alias-restores-colliding-name.md` — clearing the local alias can restore a colliding display name.
- `clone-audit-log-skips-escape-for-log.md` — clone audit log prints agent-chosen ids raw, letting one id render as
  another.
- `commit-window-reads-unpublished-outcome.md` — a commit in the close-to-publish window gets the generic error.
- `create-mode-message-blames-wrong-caller.md` — the no-selector create error blames the wrong caller.
- `create-runs-inline-on-read-loop.md` — CreateSession runs its full validate-and-launch inline on the read loop.
- `delete-quarantine-waits-unboundedly.md` — session-delete quarantine awaits the disk unboundedly under the claim.
- `detach-timeout-abandons-upstream-detach.md` — detach timeout drops the send, leaving the supervisor-side attachment
  live.
- `directory-source-staging-leak.md` — crash-orphaned extraction staging files are never pruned.
- `discard-quarantined-hangs-response.md` — post-delete quarantine discard hangs the response on a wedged disk.
- `discarded-replacement-logs-spurious-row-gone-retire.md` — a never-started handler logs a spurious row-gone warning
  and wakes all clients.
- `disconnect-publishes-keep-stale-contested-claims.md` — disconnect keeps dead hosts' claims, spuriously refusing
  session actions.
- `duplicate-freeze-clobbers-retarget-nudge.md` — retargeting a duplicate-frozen host loses the edit until a second
  edit.
- `failed-delete-strands-attachments-in-quarantine.md` — failed delete strands attachments in quarantine until startup
  destroys them.
- `failed-forwarder-wedges-delete-until-restart.md` — one failed output-forwarder join wedges delete and archive until
  restart.
- `failed-restart-discards-capture.md` — failed restart permanently discards the session's conversation-capture state.
- `failure-suppressor-never-resets.md` — failure-log suppressor never resets and mixes unrelated failure kinds.
- `folder-history-rename-unique-failure.md` — folder-history rename fails the whole refinement on duplicate spellings.
- `folder-merge-drops-newer-alias-into-proven.md` — folder merge deletes a newer alias without transferring it to a
  proven row.
- `forget-splits-guarded-update-drops-fresh-contested.md` — session delete strips a fresh collision record from the new
  connection.
- `generic-session-accepts-placeholder-template.md` — placeholder resume template silently accepted for generic sessions
  it can never serve.
- `getent-colonless-line-accepted-as-shell.md` — malformed colon-less getent output accepted as the login shell.
- `helm-upload-fast-path-spin.md` — the helm's upload fast path can spin without a deadline.
- `host-views-transiently-pairs-new-identity-with-stale-mismatch.md` — host list briefly pairs a new identity with its
  resolved mismatch.
- `in-memory-seed-skips-id-length-bound.md` — in-memory seed stores an over-long id sibling paths refuse, listing an
  unopenable session.
- `local-identity-conflict-returns-500.md` — local identity conflict during discovery returns 500 instead of 409.
- `local-update-refusal-returns-500.md` — refusing UPDATE on the local row returns 500 instead of a 4xx refusal.
- `manual-update-refusal-returns-500.md` — UPDATE planning for a manual-needs host returns 500 instead of a 4xx refusal.
- `normal-teardown-waits-unboundedly-on-detach.md` — ordinary teardown awaits detach with no timeout, parking ~60s on a
  wedged connection.
- `orphaned-install-temps-on-managed-hosts.md` — interrupted installs orphan payload-sized hidden files on managed
  hosts.
- `oversized-profile-relayed-before-size-check.md` — oversized spawn profile selector relayed before any size check.
- `pane-pid-recycled-before-sweep-binds-identity.md` — recycled pane pid can bind teardown's kill to an unrelated
  process tree.
- `pi-resume-downgrade-on-read-error.md` — Pi resume check destroys a valid locator when the session file merely fails
  to read.
- `plain-retry-erases-pending-fresh-window.md` — plain retry downgrades a pending fast reconnect to a slow probe.
- `quarantine-sweep-silently-aborts.md` — the quarantine sweep silently stops at the first unreadable entry.
- `reap-competing-sink-waits-unboundedly.md` — losing a sink-install race can hang an attach forever.
- `redirect-hop-bound-off-by-one.md` — redirect hop bound enforces 4 hops while the policy documents 5.
- `refresh-arm-busy-drains-on-dropped-sender.md` — orphaned connection actor drains refreshes with no backoff.
- `refresh-publish-races-retarget-in-check-then-act-gap.md` — retarget in the check-then-publish gap is briefly
  overwritten by the old connection.
- `refresh-timeout-misses-profile-and-commit-tail.md` — refresh timeout misses the profile and commit tail, freezing a
  host as stale-connected.
- `refused-delete-discards-in-flight-upload.md` — refused delete still destroys an in-flight upload.
- `refused-retry-strands-credential-spec.md` — refused create retry strands a credential-bearing launch spec on disk.
- `reload-adopts-stale-pane.md` — reload adopts a stale dead pane as the new generation's terminal.
- `resolve-owner-compares-first-claimant-only.md` — ambiguous owner check compares only the first claimant, misrouting
  stops.
- `restart-failure-says-restarted.md` — failed restart tells the client "session restarted" although nothing restarted.
- `restart-kills-tabs-reports-present.md` — fresh-terminal restart kills the session's tabs, then reports them as alive.
- `restricted-create-holds-lifecycle-claim-across-reply.md` — restricted create holds the lifecycle claim across the
  reply send.
- `reverify-stamp-refresh-never-lands.md` — reverify stamp never lands, so appended sessions re-read every pass.
- `revocation-during-admission-orphans-attachment.md` — revocation racing a slow attach orphans the attachment, pinning
  session ownership.
- `same-version-cache-generations-never-pruned.md` — same-version payload cache generations from other base URLs are
  never pruned.
- `seed-eviction-evicts-just-recorded-row.md` — at-capacity eviction can evict the session just recorded.
- `seed-write-validates-handle-outside-publish.md` — session seed validates the handle outside the publish, briefly
  404ing new sessions.
- `send-upload-ignores-cancellation.md` — the transfer's queue send ignores cancellation, stalling deletes.
- `sftp-overall-deadline-fails-slow-links.md` — sftp transfer's 60 s overall deadline fails slow links
  deterministically.
- `ssh-destination-collides-with-local-display-name.md` — ssh destination "this machine" collides with the local host's
  display name.
- `staging-holds-claim-across-unbounded-io.md` — upload staging holds the lifecycle claim across unbounded disk I/O.
- `stale-dial-outcome-publishes-over-retarget-nudge.md` — editing a host mid-dial briefly routes operations to the old
  machine.
- `stale-natural-verdict-kills-replacement-attach.md` — stale natural-end verdict destroys a replacement attachment
  reusing the channel.
- `stale-stall-verdict-kills-replacement-attach.md` — stale stall verdict destroys a replacement attachment reusing the
  channel.
- `stop-actor-skips-client-retirement.md` — removed host's connection lingers instead of tearing down.
- `stop-outcomes-lost-when-degraded.md` — stops silently lose intent and outcome while the supervisor is degraded.
- `stripped-agent-marker-forges-killable-tab.md` — stripped agent marker lets a forged tab kill the live agent.
- `superseded-reap-watchers-never-exit.md` — superseded output-reap watchers never exit, leaking a task per churn cycle.
- `supervisor-discards-actor-panic-cause.md` — connection actor panic loses its cause in the visible record.
- `sweep-deletes-live-staged-sentinel.md` — startup sweep can delete a live shim's staged sentinel file.
- `terminal-less-delete-no-server-guards-never-match.md` — terminal-less delete while the server is down is always
  refused.
- `tmux-kill-runs-unbounded-under-global-lock.md` — teardown's tmux calls run unbounded under the global lock.
- `tombstone-eviction-counts-live-transfers.md` — tombstone eviction counts live transfers, deleting too many
  tombstones.
- `untracked-mutations-leak-on-wedged-tmux.md` — untracked mutations leak permit, claim, and fence against wedged tmux.
- `unvalidated-state-dir-on-add.md` — adding a host with a bad state-dir path permanently bricks the entry.
- `unvalidated-state-dir-on-ensure.md` — a bad state-dir line in the ensure file bricks a host on every boot.
- `unvalidated-state-dir-on-probe.md` — probed registration with a bad state-dir path permanently bricks the entry.
