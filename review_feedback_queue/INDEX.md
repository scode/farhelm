# Review feedback queue index

One line per open item. This file must always match the feedback files in this directory.

Each file identifies its reviewed commit, confidence, caveats, and provenance for triage. Findings retained from the
review at `bd8d5d76439f8f36bbc2637b44d7ba4026b65675` have not all been reassessed against later product changes; that
review also had known coverage gaps. An entry records an open finding, not a clean verdict for the surrounding code.

Collection record for the abbreviated review through `00de0b6c`:
[complete findings, candidates, withdrawals, leads and returned proposals](records/review-through-00de0b6c/README.md).
This record is bookkeeping, not an additional feedback item. Provisional buckets preserve unresolved stronger
consequences.

## Highest priority: security or data loss

- [`hook-settings-toggle-uses-unshown-state.md`](hook-settings-toggle-uses-unshown-state.md) — Space can save the
  all-hosts hook opt-in while the switch still shows the on state the user intended to turn off.

- [`agent-environment-launch-skips-place-host-incarnation-pin.md`](agent-environment-launch-skips-place-host-incarnation-pin.md)
  — An approved agent container-environment launch can create the container on a different machine than the card named
  if the place host was retargeted or re-identified while the card waited; the environment-launch path never pins the
  place host's connection (the ordinary-create path does).

- [`uninstall-newline-stripping-defeats-state-inside-lib-guard.md`](uninstall-newline-stripping-defeats-state-inside-lib-guard.md)
  — Uninstall's `readlink -f | tr -d '\n'` strips embedded newlines, corrupting the spellings every containment/identity
  guard consumes; a constructed dual-newline layout can make uninstall delete the user's data directory (traced, not
  run; deliberate-setup drop rejected by independent check).

- [`trailing-legacy-claude-settings-v036-hosts-never-cleaned.md`](trailing-legacy-claude-settings-v036-hosts-never-cleaned.md)
  — A Tensorlake host set up by 0.36.0 and not connected under 0.37-0.43 keeps Farhelm's old Claude endpoint URL and
  token in Claude's settings for good; per SPEC_impl.md they can take precedence and receive a new token (Claude Code
  precedence unverified).

- [`trailing-legacy-codex-config-v036-hosts-never-cleaned.md`](trailing-legacy-codex-config-v036-hosts-never-cleaned.md)
  — The same 0.36.0-era Tensorlake hosts keep Farhelm's old Codex endpoint and token in Codex's config, so a newly
  selected login or endpoint may not take effect, and per SPEC_impl.md the old token can reach a new tokenless endpoint
  (Codex merge behavior unverified).

## High priority: material UX degradation


- [`trailing-settings-trust-help-omits-agent-requested-launches.md`](trailing-settings-trust-help-omits-agent-requested-launches.md)
  — The Settings pages for Codex, Muse, Pi, and Antigravity omit that their workspace-trust switch also applies to
  agent-requested launches, which run without an approval card on Always-allow hosts.

- [`trailing-replace-with-recent-named-fill-enter-replaces.md`](trailing-replace-with-recent-named-fill-enter-replaces.md)
  — In Replace with, a screen reader announces a highlighted recent setup as "Fill from recent setup" while Enter on it
  replaces and deletes the source session; highest if the "⏎ Replace" key cap is not announced with it.

- [`trailing-sandbox-agent-plants-repository-suggestions.md`](trailing-sandbox-agent-plants-repository-suggestions.md) —
  A sandbox-confined agent's fresh checkout puts a repository of its choosing at the top of the checkout suggestions on
  every host, past its environment boundary (highest if a suggestion can be taken without the user reading it).

- [`trailing-auto-trust-overrides-custom-flag-refusal.md`](trailing-auto-trust-overrides-custom-flag-refusal.md) — In a
  session's own sandbox, Farhelm's automatic trust flag follows a user's Custom Pi `--no-approve` or Codex untrusted
  override and may win (vendor flag precedence unverified; highest if it does).

## Other: correctness, diagnostics, cleanup, or convenience

- [`tensorlake-key-discard-lacks-preservation-test.md`](tensorlake-key-discard-lacks-preservation-test.md) — The new
  Tensorlake Discard coverage does not prove that abandoning a replacement keeps the saved key.

- [`ntfy-test-no-longer-witnesses-pending-save-lock.md`](ntfy-test-no-longer-witnesses-pending-save-lock.md) — The
  changed Notifications test can pass if Save wrongly becomes available on an edited form while Send test is pending.

- [`sound-preview-overlay-blocks-settings-switch.md`](sound-preview-overlay-blocks-settings-switch.md) — The Play
  overlay leaves parts of its sound switch row unclickable.

- [`sound-preview-regression-misses-separate-mappings.md`](sound-preview-regression-misses-separate-mappings.md) — The
  preview test exercises one sound and can miss a wrong mapping on the three other Play controls.

- [`app-settings-shared-upgrade-option-has-local-heading.md`](app-settings-shared-upgrade-option-has-local-heading.md) —
  The App heading says This Mac even though it contains a helm-shared highlights setting.

- [`agent-settings-search-omits-agent-name.md`](agent-settings-search-omits-agent-name.md) — Settings search can give
  different agents' pages or hook controls the same accessible name.

- [`pi-settings-tests-miss-trust-route.md`](pi-settings-tests-miss-trust-route.md) — Pi's Settings tests no longer check
  a successful trust read or the absence of a hook hint control.

- [`agent-settings-tests-miss-late-trust-replies.md`](agent-settings-tests-miss-late-trust-replies.md) — The per-agent
  Settings tests can miss a trust reply being applied to the page opened after the request.

- [`agent-settings-search-test-omits-focus-target.md`](agent-settings-search-test-omits-focus-target.md) — The Settings
  search test can pass with a trust result that names the right agent and focuses the wrong control.

- [`shortcuts-chord-during-composition.md`](shortcuts-chord-during-composition.md) — The Keyboard shortcuts chord can
  interrupt an input method if native handling clears its mark first or the action cannot query the focused field.

- [`trailing-settings-containers-reload-omits-loading-keeps-error.md`](trailing-settings-containers-reload-omits-loading-keeps-error.md)
  — After a failed refresh in Settings -> Containers, deleting the last leftover or clicking Refresh hides the reading
  indicator and leaves only the old refresh error visible while the new read waits.

- [`trailing-masked-input-test-omits-a11y-value-checks.md`](trailing-masked-input-test-omits-a11y-value-checks.md) — The
  masked-input test checks painted text and accessible names but never inspects accessible values or runs with
  accessibility active, leaving screen-reader secret exposure unguarded.

- [`trailing-credential-delivery-test-stale-claude-settings-assertion.md`](trailing-credential-delivery-test-stale-claude-settings-assertion.md)
  — The backend-failure delivery test still asserts .claude/settings.json is absent (always true now), leaving failed
  agent-credentials and Codex config targets unverified before and after retry.

- [`trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md`](trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md)
  — Toggling Tailscale in Host settings during a launch shows an unprefixed sentence fragment, stacks any earlier
  failure above it, and leaves the launch refusal stuck even after an in-flight toggle succeeds.

- [`trailing-custom-agent-settings-dirty-close-prompt-collapses-multiple-agents.md`](trailing-custom-agent-settings-dirty-close-prompt-collapses-multiple-agents.md)
  — When two or more custom-agent pages have unsaved changes, the 'Save changes before closing?' prompt lists only a
  single 'Custom agent' and jumps Settings to whichever custom-agent draft sorts first rather than staying on the page
  the user was viewing.

- [`trailing-tensorlake-key-replacement-keeps-old-discovered-sandboxes.md`](trailing-tensorlake-key-replacement-keeps-old-discovered-sandboxes.md)
  — When you replace your saved Tensorlake API key with a different key in Settings without clicking Clear first, the
  list of sandboxes not on this helm keeps showing leftover sandboxes from the old key while the new check runs — and
  keeps showing them indefinitely with active Delete buttons if the new key fails to list sandboxes (for example, if the
  new key is invalid or Tensorlake is unreachable).

- [`ui-channel-displaced-notice-lost-to-reader-race.md`](ui-channel-displaced-notice-lost-to-reader-race.md) — A
  displaced UI channel's Displaced notice can be dropped by a reader/writer drain race, so the displaced window
  reconnects and steals the seat back (latent: a second UI cannot arise in the shipped embedded app).

- [`upload-cancel-mid-publication-reported-not-published.md`](upload-cancel-mid-publication-reported-not-published.md) —
  An upload cancelled while the host is publishing it can be reported to the user as "definitely not published" although
  the host's answer says the outcome is unknown (reachability premise unverified; recorded higher).

- [`helm-agent-create-dispatch-logs-asking-session-unescaped.md`](helm-agent-create-dispatch-logs-asking-session-unescaped.md)
  — The helm's agent-create audit log writes the supervisor-reported asking session id unescaped (every sibling site
  escapes it), allowing forged log lines or a disguised asker.

- [`preapproved-agent-requests-skip-deleting-session-refusal.md`](preapproved-agent-requests-skip-deleting-session-refusal.md)
  — While a session is being deleted, a new request from it that the host already allows without asking is carried out
  instead of refused, deviating from the deny-until-delete-finishes contract (no new authority granted).

- [`provisioning-orphan-sweep-deletes-concurrent-helm-upload.md`](provisioning-orphan-sweep-deletes-concurrent-helm-upload.md)
  — Two helms provisioning the same host concurrently can delete each other's active staging temporary via the orphan
  sweep (no liveness check); the victim run fails visibly and retryably (cross-helm premise unverified).

- [`update-identity-unverified-misleading-plan-again-remedy.md`](update-identity-unverified-misleading-plan-again-remedy.md)
  — Update on a host whose supervisor reports no identity (record has one) is accepted then fails with "identity changed
  / plan again", a futile remedy; the real remedies are not named (rare state, safe refusal).

- [`installer-fallback-lock-world-openable.md`](installer-fallback-lock-world-openable.md) — On a lock-unsupported
  volume the installer's fallback lock file is created 0644, so another local account can hold it and block the owner's
  installs (DoS); umask 0 also allows substituting it (premises named).

- [`release-payload-cache-misses-private-dir-hardening.md`](release-payload-cache-misses-private-dir-hardening.md) — The
  release payload cache is created umask-dependent without symlink refusal, unlike the deliberately hardened sibling
  directory cache; protected today only by the 0700 state dir (hardening consistency).

- [`podman-settle-claim-incumbent-state-non-atomic.md`](podman-settle-claim-incumbent-state-non-atomic.md) — A place
  claim transfer reads the incumbent's live identity refusal non-atomically; if it resolves to Connected in the window,
  the claim still moves (benign routing shift between twins on the same machine; resource guards hold).

- [`trailing-drift-copyback-runs-while-agent-may-live.md`](trailing-drift-copyback-runs-while-agent-may-live.md) — A
  Grok or Antigravity drift case copies refreshed vendor files back once its terminal hosts end, even when the agent may
  still be running (failed Delete sweep, failed create, detached process), so a later vendor save such as a refreshed
  login can be lost without a note.

- [`trailing-drift-runner-drops-requested-harness.md`](trailing-drift-runner-drops-requested-harness.md) — The drift
  runner silently skips an explicitly requested agent that has no case in the chosen category and can exit successfully
  without checking it.

- [`trailing-drift-panic-skips-copyback.md`](trailing-drift-panic-skips-copyback.md) — A panic inside a Grok or
  Antigravity drift case skips copying a refreshed vendor login back to the maintainer's real config, with no note
  (pre-existing).

- [`trailing-drift-failed-copyback-passes-case.md`](trailing-drift-failed-copyback-passes-case.md) — A failed copy-back
  of a refreshed vendor login is only a console note and the drift case still reports passed, so the maintainer may be
  signed out unnoticed.

- [`trailing-drift-cursor-initial-prompt-never-passes.md`](trailing-drift-cursor-initial-prompt-never-passes.md) —
  Cursor's launch-prompt drift case always ends "could not run" even when Cursor accepts the prompt, so the check can
  never pass.

- [`trailing-drift-capture-timing-check-too-coarse.md`](trailing-drift-capture-timing-check-too-coarse.md) — The
  capture-timing drift check treats prompt-timed and reply-timed agents alike, so a vendor drift between "at the first
  prompt" and "after the first reply" would go unnoticed.

- [`trailing-release-notes-recapture-parks-on-tooltip-field.md`](trailing-release-notes-recapture-parks-on-tooltip-field.md)
  — Recapturing the v0.42.0 Antigravity release-notes picture parks the pointer on the Conversation field, so its
  tooltip would be drawn into the picture (from source; not seen in a capture).

- [`trailing-brick-cleanup-refuses-untested-remote-mac-pair.md`](trailing-brick-cleanup-refuses-untested-remote-mac-pair.md)
  — After a stop-after-P0 request, brick-test cleanup refuses a prepared but untested remote Mac pair and demands a
  retention decision for machines with no evidence.

- [`trailing-file-disclosure-holdback-silent-click.md`](trailing-file-disclosure-holdback-silent-click.md) — When a
  terminal file's details card would cover the pointer, it flashes and vanishes and a click on the path does nothing,
  with no "not visible" explanation (pre-existing).

- [`trailing-file-prepaint-click-test-gap.md`](trailing-file-prepaint-click-test-gap.md) — The test for clicking a
  terminal file before its card paints misses the placed-but-unpainted window, so dropping that guard would go unnoticed
  (likely; premise on GPUI event timing).

- [`trailing-spec-file-click-release-wording.md`](trailing-spec-file-click-release-wording.md) — SPEC.md says a file
  click must be pressed and released over the painted card, but the press closes the card and the release is accepted
  without it; SPEC_impl.md has it right.

- [`trailing-grid-wide-glyph-absorbs-foreign-spacer.md`](trailing-grid-wide-glyph-absorbs-foreign-spacer.md) — After a
  program deletes characters from the middle of a wide character, one cell can show the wrong background color
  (pre-existing; cosmetic).

- [`trailing-tab-shell-hangup-delete-wiring-untested.md`](trailing-tab-shell-hangup-delete-wiring-untested.md) — No test
  would notice if Delete stopped giving macOS tab shells their hangup, which would again lose their shell history.

- [`trailing-agent-create-connection-binding-untested.md`](trailing-agent-create-connection-binding-untested.md) — No
  test covers binding an agent-requested session to the host connection the user approved, so the fix could regress
  silently.

- [`trailing-reposted-notice-not-revealed.md`](trailing-reposted-notice-not-revealed.md) — In an overflowing notice
  stack, a repeated connection-lost or opt-out-refused notice stays scrolled out of view instead of being revealed
  (pre-existing).

- [`trailing-drift-claude-trust-real-config-byte-check.md`](trailing-drift-claude-trust-real-config-byte-check.md) — The
  Claude trust drift case fails whenever another Claude session rewrites the real ~/.claude.json during the case,
  reading as if Farhelm wrote it (frequency unmeasured).

- [`trailing-drift-claude-trust-ancestor-vacuous-pass.md`](trailing-drift-claude-trust-ancestor-vacuous-pass.md) — The
  Claude trust drift case can pass without proving Claude honours Farhelm's entry if the maintainer's config already
  trusts /tmp or /.

- [`trailing-launcher-new-clone-enter-after-vanished-pick.md`](trailing-launcher-new-clone-enter-after-vanished-pick.md)
  — In New and Clone, Return on a highlighted template or folder row can launch instead of picking when the highlight
  vanished before the redraw; only Replace with is guarded.

- [`trailing-spec-impl-hook-answer-preview-wait.md`](trailing-spec-impl-hook-answer-preview-wait.md) — SPEC_impl.md says
  a hook-setup answer waits for a checkout preview, but the launch is refused instead and the user has to press Launch
  again.

- [`trailing-launcher-held-escape-discards-draft.md`](trailing-launcher-held-escape-discards-draft.md) — Holding Escape
  with typed results or the Save as template panel open can close the whole launcher and discard its draft; the fix
  covered only the highlight case.

- [`trailing-launcher-stale-permanent-yolo-answer-silent.md`](trailing-launcher-stale-permanent-yolo-answer-silent.md) —
  A stale "Start, and don't ask again on this host" answer closes the YOLO question with nothing started and no reason,
  sometimes after the host setting was already saved.

- [`trailing-templates-stale-save-duplicate-footer.md`](trailing-templates-stale-save-duplicate-footer.md) — Save or
  Duplicate clicked just after switching templates can act on the newly opened template, writing automatic updates into
  an older template the user never saw (pre-existing).

- [`trailing-host-setup-frozen-skip-ignores-later-restore.md`](trailing-host-setup-frozen-skip-ignores-later-restore.md)
  — Turning "Set up new hosts without asking" off while a host check is running no longer brings back the setup
  confirmation for that attempt.

- [`trailing-tensorlake-rerun-reads-skip-setting-late.md`](trailing-tensorlake-rerun-reads-skip-setting-late.md) —
  Tensorlake Re-run reads "Create Tensorlake sandboxes without asking" after the plan arrives, so turning it on during
  the check skips the billed-creation confirmation.

- [`trailing-host-refusal-remove-reads-live-skip.md`](trailing-host-refusal-remove-reads-live-skip.md) — Remove in the
  host refusal dialog reads "Remove hosts without asking" at the press, so a just-changed setting can forget the host
  without the confirmation the dialog implied.

- [`trailing-spec-impl-custom-agent-id-letter.md`](trailing-spec-impl-custom-agent-id-letter.md) — SPEC_impl.md says
  registered custom agent IDs start with a letter, but they may start with a digit.

- [`trailing-hook-hint-keeps-stale-save-error.md`](trailing-hook-hint-keeps-stale-save-error.md) — The hook setup hint
  brings back an old Don't show again error after a later dismissal and re-enable (previously triaged as no change under
  the cheap-fix rule).

- [`trailing-trust-hint-hide-survives-reconnect.md`](trailing-trust-hint-hide-survives-reconnect.md) — A trust hint
  hidden with × can stay hidden for a new trust question that began while the app was disconnected (premise unverified).

- [`trailing-trust-hint-late-refusal-untested.md`](trailing-trust-hint-late-refusal-untested.md) — No test covers a
  trust hint save refusal that lands while the hint is already dismissed.

- [`trailing-unread-hook-and-spare-switches-read-as-off.md`](trailing-unread-hook-and-spare-switches-read-as-off.md) —
  The hook setup and spare-checkout switches read as a saved "off" to screen readers while their setting is unread.

- [`trailing-stored-trust-chip-credits-settings-switch.md`](trailing-stored-trust-chip-credits-settings-switch.md) —
  Restart and prompt approval cards explain a session's stored trust by a Settings switch that may now be off.

- [`trailing-switch-warning-description-untested.md`](trailing-switch-warning-description-untested.md) — No test checks
  that the trust and hook switches carry their warning for screen readers.

- [`trailing-relative-claude-lock-in-old-cleanup-records.md`](trailing-relative-claude-lock-in-old-cleanup-records.md) —
  Claude trust cleanup records written with a relative `CLAUDE_CONFIG_DIR` before the fix keep a relative lock path
  (likely unpopulated).

- [`trailing-settings-trust-help-misdescribes-persistent-hosts.md`](trailing-settings-trust-help-misdescribes-persistent-hosts.md)
  — Settings trust help says sandbox and container sessions trust automatically, but persistent recipe hosts follow the
  switch.

- [`trailing-disposable-trust-hint-says-off-by-default.md`](trailing-disposable-trust-hint-says-off-by-default.md) — In
  a session's own sandbox, the trust hint says trust is off by default and points to a switch that does not apply there.

- [`trailing-podman-image-inspect-hits-output-cap.md`](trailing-podman-image-inspect-hits-output-cap.md) — Recipe image
  retirement still reads Podman's full image report, which can exceed the output cap and block cleanup (size
  unverified).

- [`trailing-podman-leftover-listing-hits-output-cap.md`](trailing-podman-leftover-listing-hits-output-cap.md) — The
  leftover container listing still reads Podman's full ps report, so one oversized host hides leftovers on every host
  (size unverified).

- [`trailing-recipe-home-launch-skips-early-title-check.md`](trailing-recipe-home-launch-skips-early-title-check.md) — A
  recipe launch into the new home folder creates the environment before refusing an invalid session name.

- [`trailing-stale-references-to-removed-repository-search.md`](trailing-stale-references-to-removed-repository-search.md)
  — SPEC.md, code comments and a test docstring still describe the removed repository search.

- [`trailing-replace-in-environment-history-check-vacuous.md`](trailing-replace-in-environment-history-check-vacuous.md)
  — The replace-in-environment test's check that host history is unchanged can no longer fail.

- [`trailing-session-owned-setup-flag-untested.md`](trailing-session-owned-setup-flag-untested.md) — No helm test still
  checks that sandboxes and containers are never offered checkout-folder setup.

- [`trailing-repository-history-failure-shown-as-no-matches.md`](trailing-repository-history-failure-shown-as-no-matches.md)
  — A failed repository-history read shows "No repository matches" and is not retried while the launcher is open.

- [`trailing-symlinked-claude-json-permanent-warning.md`](trailing-symlinked-claude-json-permanent-warning.md) — A
  symlinked `~/.claude.json` leaves a permanent amber warning that misnames the cause.

- [`trailing-menu-stop-and-suspend-exited-shell-wording.md`](trailing-menu-stop-and-suspend-exited-shell-wording.md) —
  The row menu's Stop and suspend still offers to stop a shell that has already exited.
