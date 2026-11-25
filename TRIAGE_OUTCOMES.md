# Triage outcomes

## tensorlake-key-discard-lacks-preservation-test.md

- Outcome: `other`.
- Assessment: No test checks that pressing Discard on a typed Tensorlake key replacement leaves the saved key alone (no
  request, field emptied, Test still available, the other form's draft kept). Verified: confirmed; no key-form Discard
  assertion in `credential_tests.rs`/`tensorlake_tests.rs` (grep). Code path is a plain local reset (`editing.rs:220`).
  Coverage gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (or fix code if test gaps are wanted) — no product
  failure; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## ntfy-test-no-longer-witnesses-pending-save-lock.md

- Outcome: `other`.
- Assessment: The Notifications Send test test no longer proves that Save is unavailable on an edited form while a test
  is running; it only checks a clean form, which has no Save anyway. Verified: confirmed. `tests.rs:4145-4155` asserts
  `!drawn(FOOTER_SAVE)` on a just-saved (clean) form; the dirty branch (`:4161-4166`) answers the test before looking at
  the footer. Coverage gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (or fix code) — no product failure; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## sound-preview-regression-misses-separate-mappings.md

- Outcome: `other`.
- Assessment: The sound preview test presses only the "Turn finished" Play button, so a wrong sound on the other three
  Play buttons would go unnoticed. Verified: confirmed (per-row match arms in `choices.rs:395-` `preview`; single-row
  test). Coverage gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (or fix code) — no product failure; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## pi-settings-tests-miss-trust-route.md

- Outcome: `other`.
- Assessment: Pi's Settings tests no longer check a successful trust read showing Pi's stored value, or that Pi's page
  has no hint control. Verified: confirmed per reviewer as a coverage gap; no shipped failure claimed.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## agent-settings-tests-miss-late-trust-replies.md

- Outcome: `other`.
- Assessment: No test checks that a trust read or save started on one agent's page settles on that agent after you
  switch to another agent's page. Verified: confirmed as a coverage gap; the product binds replies to the starting agent
  (per reviewer).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## agent-settings-search-test-omits-focus-target.md

- Outcome: `other`.
- Assessment: The Settings search test for an agent's trust result checks the page and label but not which agent's
  switch the result focuses. Verified: confirmed as a coverage gap; the shipped catalog binds both correctly (per
  reviewer).
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard — coverage only; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-credential-delivery-test-stale-claude-settings-assertion.md

- Outcome: `other`.
- Assessment: A credential-delivery test still asserts that the old Claude settings file was not created, which is
  always true now, and does not check the files that actually fail or the Codex file after retry. Verified: confirmed.
  `credential_delivery_tests.rs:2076` still has the vacuous assertion; no Codex-after-retry assertion (`:2077-2084`).
  Coverage gap only.
- Decision: q-tests, "Gaps that exist only in product tests": the maintainer picked "Re-apply the Nov 21 rule".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix the test if the fix is a few lines (about 20 at most) and the test guards a user-facing
  behavior; otherwise make no change. Either way remove the feedback file and its index entry, and say in the execution
  record which way it went. Recommendation for reference: discard (or fix code) — coverage only; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## agent-settings-search-omits-agent-name.md

- Outcome: `other`.
- Assessment: In Settings search, results for different agents' pages (for example the "no settings" entry of several
  agents) have the same accessible name, so a screen-reader user hears identical options and cannot tell which agent
  each opens. Verified: confirmed. Each result option's accessible name is the entry label alone (`search.rs:789-799`);
  the agent appears only in a separate heading row (`:690-697`). Screen-reader speech not verified. SPEC.md 2585-2590
  describes grouping but not option naming; reading.
- Decision: q-a11y, "Screen-reader and keyboard parity the spec does not name": the maintainer picked "Parity for
  consent, warning and status text". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 20 lines or less; otherwise make no change. Either way remove the
  feedback file and its index entry, and say in the execution record which way it went. Recommendation for reference:
  fix code — include the section name in each option's accessible name; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## sound-preview-overlay-blocks-settings-switch.md

- Outcome: `other`.
- Assessment: Each sound switch in Notifications has a small Play button. The invisible strip above and below that
  button, as tall as the row, swallows clicks, so clicking there neither plays nor toggles the switch. Verified:
  confirmed. The wrapper is `absolute().top_0().bottom_0()` with `block_mouse_except_scroll()` (`choices.rs:723-735`),
  wider in height than the 24 px button. The dead area is about 24 px wide.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — move the blocking hitbox to the button; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## app-settings-shared-upgrade-option-has-local-heading.md

- Outcome: `other`.
- Assessment: Settings → App's heading says its settings apply to "This Mac", but the page also holds "Show what's new
  after updates", which the helm shares. Verified: confirmed per reviewer (frame scope mapping not re-traced). SPEC.md
  2563: the heading "shows its icon, name, description and scope"; SPEC.md 2687-2690 places both an installation setting
  and the helm-shared choice in App. The spec does not say what scope label a mixed page shows, so this is a reading.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: discard or fix spec (say App's scope is the installation's, with the shared switch saying so itself);
  size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-settings-containers-reload-omits-loading-keeps-error.md

- Outcome: `other`.
- Assessment: In Settings → Containers, after a leftovers refresh failed, pressing Refresh again (or deleting the last
  listed leftover) shows no "Reading container leftovers…" while the helm checks (up to 20 s); only the old error stays,
  as if the new check had already failed. Verified: confirmed. Loading text shows only for `(None, None)`
  (`containers.rs:512-520`); `start_round` and the Delete follow-up read leave `leftover_error` set
  (`:174-184,350-358`). Self-corrects when the read answers. Not the rare-glitch filter: a retry after a failed refresh
  is not a rare trigger.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — track a reading flag and show the status; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-host-settings-tailscale-switch-refusal-prefix-and-stuck-error.md

- Outcome: `other`.
- Assessment: In a host's settings, flipping the Tailscale switch while a session launch is starting shows a bare
  lowercase fragment ("not started, a session launch is in progress…") without naming the setting, can show it beneath
  an older Tailscale error, and can leave it on screen after an earlier Tailscale save succeeds. Verified: confirmed.
  `hosts/settings.rs:759-764` sets the flag and returns; the row draws the bare `LAUNCH_IN_FLIGHT` (`:960-966`) beside
  the draft's refusal (`:957-959`); nothing clears the flag when a save settles. Diagnostic/stale display only.
- Decision: q-polish, "Small presentation and robustness defects with no spec rule": the maintainer picked "Fix the
  cheap ones, remove the rest". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Fix it if the fix is about 30 lines or less including a test; otherwise make no change. Either
  way remove the feedback file and its index entry, and say in the execution record which way it went. Recommendation
  for reference: fix code — prefix, suppress the old error, clear on settle; size S.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-settings-trust-help-omits-agent-requested-launches.md

- Outcome: `fix code`.
- Assessment: The workspace-trust switch help on the Codex, Muse, Pi and Antigravity Settings pages says it applies to
  New, Clone, Restart with and Replace with, but not that it also applies to launches an agent requests. On a host where
  agent requests are approved without asking, turning it on lets agents start trusted sessions anywhere, which the help
  never mentions. Claude's help does mention it. Verified: confirmed. `agent_trust.rs:212-224,239` omit agent-requested
  launches; Claude's text at `:205` includes "including launches an agent requests" (dropped for the others by
  4aff3f1b). SPEC.md 1852-1853: "New, Clone, Restart with, Replace with and agent-requested launches apply the current
  setting in any directory." The spec requires the help to explain that hooks and commands may run (1856-1857) but does
  not dictate listing launch routes, so this rests on a reading. Not covered by `drift-workspace-trust.md` (that plan
  adds live drift tests, not Settings copy).
- Decision: q-wording, "User-visible text that misstates what will happen": the maintainer picked "Fix them all".
  (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: Correct the user-visible text so it states what will actually happen in the case at hand.
  Recommendation for reference: fix code — restore the agent-requested wording and refresh goldens; size S. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## trailing-tensorlake-key-replacement-keeps-old-discovered-sandboxes.md

- Outcome: `fix code`.
- Assessment: In Settings → Tensorlake, saving a different API key over the saved one keeps showing the old key's
  "Sandboxes not on this helm" while the new list loads, and keeps them (with Delete buttons) under "Could not refresh"
  if the new key cannot list sandboxes. Verified: confirmed. `follow_account` re-reads without clearing cached rows when
  a key stays saved (`tensorlake_sandboxes.rs:159-166`); a failed discovery keeps `last_discovered` (`:241-246`). The
  module docs say "A key change invalidates that evidence too" (`:21-22`). SPEC.md 613-614 ("above previously discovered
  sandboxes") does not distinguish keys, so a reading. Delete still confirms by name and targets the vendor ID
  (reviewer), so no wrong deletion is established.
- Decision: q-staleconf, "Confirmations and kept state whose subject is gone": the maintainer picked "Close or
  invalidate them". (2026-11-24 fewest-questions triage page (main c38b93552).)
- Completion criteria: A confirmation or kept state whose subject is gone (host removed, listing emptied, account key
  changed) closes or is invalidated. Recommendation for reference: fix code — clear cached discovery on a key
  replacement; size S. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-settings-nov24.md` ``.

## ui-channel-displaced-notice-lost-to-reader-race.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md "One GUI at a time" (`5834-5839`): "A UI that connects takes the seat, and the
  one holding it is displaced: the displaced window says that another window took over and goes wholly inert, its
  terminals included. ... Only the helm's own word that another UI took the seat counts as a takeover; a connection that
  merely drops is a reset". SPEC_impl.md "The seat: one UI per helm" (`531-532`): "A new one displaces the old; the
  displaced window shows 'another window took over' and does not reconnect on its own. Only the live channel's
  'displaced' message counts as a takeover." `crates/farhelm-helm/src/ui_channel.rs:1149-1151` itself promises "Its
  writer then drains what it queued, so a displaced channel's last message still reaches the GUI before the socket
  closes." Code: `run`'s displaced arm (`ui_channel.rs:1025-1028`) queues `ServerMessage::Displaced` and returns,
  dropping `incoming_rx`. In `serve_socket` (`:1197-1241`), the outer `tokio::select!` polls `reader` against
  `channel_and_writer`; any frame arriving from the old client makes `incoming_tx.send(message).await` fail (`:1233`),
  `reader` returns, and `select!` drops `channel_and_writer` mid-drain before `Displaced` is written. Verified:
  confirmed by source inspection (race timing not executed). Fix follows directly: stop polling `reader` once `core`
  completes so `writer` always drains `Displaced`, keeping reader-first and write-failure teardowns unchanged. Size
  estimate ~40-60 lines with a regression test.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-25 triage, main `5af40d23e`.) The maintainer can revise it.
- Completion criteria: Make `serve_socket` drain queued outgoing messages (including `ServerMessage::Displaced`) after
  `core` returns without an incoming client frame aborting the drain, preserving `SILENCE_CLOSE`, reader-first, and
  write-failure teardowns, and add a regression test with a concurrent incoming frame during displacement. Size gate:
  about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment);
  over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## helm-agent-create-dispatch-logs-asking-session-unescaped.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md "Session IDs in logs" (`152-160`): "Session IDs must not inject log lines or
  use invisible or direction-changing characters to disguise the ID shown in logs. ... Safe escaping by the configured
  logging formatter also satisfies the requirement without a separate escaping helper at each call site." Code:
  `crates/farhelm-helm/src/agent_requests.rs:3116-3121` (`dispatch_agent_create`) logs `asking = asking_session`
  verbatim, where `asking_session` comes from the untrusted supervisor (`client.rs:2442-2448`) and the configured
  `tracing_subscriber::fmt` formatter (`farhelm-desktop/src/lib.rs:484-486`, `farhelm/src/main.rs:2788-2791`) writes raw
  `&str` unescaped. All 8 sibling tracing sites in `agent_requests.rs` (`:1476,2014,2455,3425,3497,3648,3734,4708`) use
  `escape_for_log(asking_session).as_str()`. No comment defends omitting escaping on `asking_session`. Verified:
  confirmed by source inspection. Fix follows directly: wrap `asking_session` in
  `escape_for_log(asking_session).as_str()`. Size estimate ~15-25 lines including a unit test.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-25 triage, main `5af40d23e`.) The maintainer can revise it.
- Completion criteria: Escape `asking_session` with `escape_for_log` in `dispatch_agent_create`
  (`agent_requests.rs:3116- 3121`) matching its sibling sites, and add a focused regression test. Size gate: about 100
  lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment); over that,
  execution blocks and brings the item back to the maintainer. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-security-and-agent-boundaries.md` ``.

## trailing-templates-stale-save-duplicate-footer.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC.md "One GUI at a time" (`5871-5873`): "By default, a pointer or keyboard action uses
  the target, consent and preconditions from the frame the user was shown, or is ignored when that frame has not yet
  been shown or its target is no longer in view." Exact sibling precedent:
  `template-delete-can-target-unshown-editor.md` (PR #4832) was recorded as clearly settled under the same rule. Code:
  `crates/gui/farhelm-gui/src/templates/editor.rs:1286- 1306` binds Delete (`:1309-1328`) to the drawn template's name,
  but Save (`:1286-1291`) and Duplicate (`:1293-1306`) capture no editor identity and call `dialog.save_editor` /
  `dialog.duplicate` (`dialog.rs:468-506`), acting on whatever editor is currently mounted.
  `dialog.is_open_editor(&self, name_field, cx)` (`dialog.rs:630-637`) already exists to verify the drawn editor
  instance. Verified: confirmed by source inspection (two-click timing unmeasured). Fix follows directly: bind Save and
  Duplicate footer clicks to the drawn editor's `name_focus` and ignore when `!self.is_open_editor(&name_focus, cx)`.
  Size estimate ~30-50 lines with a regression test.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-25 triage, main `5af40d23e`.) The maintainer can revise it.
- Completion criteria: Bind the Templates editor footer's Save and Duplicate buttons to the drawn editor instance (via
  `name_focus` / `is_open_editor`) so stale clicks after switching templates are ignored, and add a regression test.
  Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue removal or a changelog
  fragment); over that, execution blocks and brings the item back to the maintainer. Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-gui-hosts-menus-and-terminal.md` ``.

## trailing-host-refusal-remove-reads-live-skip.md

- Outcome: `fix code`.
- Assessment: Clearly settled. SPEC_impl.md "Questions: the prompt" (`996-1000`): "The same rule starts at the control
  that asks the question or sends the request directly. Its handlers carry the drawn session key, row, skip preference
  and the parts of the environment's state that determine the command. The prompt's `when_shown` signal admits an action
  snapshot after presentation. ... The question and request use the snapshot that admitted the press." SPEC.md "One GUI
  at a time" (`5871-5873`): "By default, a pointer or keyboard action uses the target, consent and preconditions from
  the frame the user was shown, or is ignored when that frame has not yet been shown or its target is no longer in
  view." Exact sibling precedent: `trailing-hosts-menu-remove-reads-live-skip-confirmation.md` (PR #4849) was recorded
  as clearly settled under the same rule. Code: `crates/gui/farhelm-gui/src/hosts/refusal.rs:249-264` calls
  `super::remove::start(host, &mark, window, cx)`, which reads live `listed(host, cx)` and live `remove::asks_first(cx)`
  (`remove.rs:136-141`), unlike `hosts/menu.rs:515-542`, which snapshots `(view, asks)` in an `ActionFrame` and calls
  `remove::start_shown`. Verified: confirmed by source inspection. Fix follows directly: snapshot the drawn host view
  and `remove::asks_first(cx)` in `hosts/refusal.rs` with `ActionFrame` and call `remove::start_shown`. Size estimate
  ~30-60 lines with a GPUI test.
- Decision: Recorded without asking under the clearly-settled rule (`review_feedback_queue/.agents/triage.md`,
  "Preparing each item", maintainer 2026-10-19). (2026-11-25 triage, main `5af40d23e`.) The maintainer can revise it.
- Completion criteria: Snapshot the drawn host view and `remove::asks_first(cx)` via `ActionFrame` in
  `crates/gui/farhelm-gui/src/hosts/refusal.rs` and invoke `remove::start_shown` (matching `hosts/menu.rs:515-542`),
  with a regression test. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the queue
  removal or a changelog fragment); over that, execution blocks and brings the item back to the maintainer. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-gui-hosts-menus-and-terminal.md` ``.

## agent-environment-launch-skips-place-host-incarnation-pin.md

- Outcome: `fix code`.
- Assessment: Confirmed by source inspection (live retarget not executed). In
  `crates/farhelm-helm/src/agent_requests.rs:4582-4775` (`launch_environment_for_agent`), cached places are checked
  before the approval card (`:4600-4621`), and after Allow only `plan.source_owner` is rechecked (`:4696-4706`); the
  container host's connection incarnation, route, or Podman `PlaceIdentity` is never pinned, so
  `Podman::ensure_prepared` and `Podman::create` (`podman.rs:1230,1318-1329`) call `places.fresh(...)` and can claim a
  replacement machine if the place host was retargeted while the card waited. Contrasts with ordinary agent Create
  (`approve_launch`, `agent_requests.rs:2747,2802-2809`, PR #4860) and `SPEC_impl.md:9825-9826`.
- Decision: Q1 ("Pin container host identity on agent launch"): maintainer picked "Fix code: pin across card & into
  container creation" with note: "if this is easy using existing mechanisms, which it sounds like from the estimate. if
  the executor or planned realizes this iis much more complicated, kick it back to the todo list with an explanation of
  why its complicated." (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Capture the container host's identity before showing the approval card, verify it after Allow,
  and pass it through `start_session_environment` into `Podman::ensure_prepared` and `Podman::create` so a retargeted
  place host is refused rather than claiming a different machine, using existing mechanisms (estimate M, ~120-180
  lines). If planning or execution finds this is much more complicated than reusing existing mechanisms, kick it back to
  `TODO.md` with an explanation of why it is complicated instead of landing a complex change. Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-security-and-agent-boundaries.md` ``.

## uninstall-newline-stripping-defeats-state-inside-lib-guard.md

- Outcome: `fix code`.
- Assessment: Confirmed by source inspection (dual-newline + bind-mount layout traced, not live-executed).
  `crates/farhelm-helm/src/provisioning/backend.rs:3367-3368` runs `readlink -f -- "$1" 2>/dev/null | tr -d '\n'`,
  stripping embedded newlines from resolved paths before the `-ef "$removed"` ancestor walk (`:3392-3401`) and Rust
  containment checks (`plan.rs:972-1021`), letting `rm -rf` on the unstripped `lib_dir` (`backend.rs:3656-3660`) delete
  a state directory reached through a newline-bearing symlink target. Tension between `SPEC.md:5658-5661` (deliberate
  setup) and `SPEC.md:5663-5666,6115-6117` (lasting damage / uninstall refuses layouts it cannot tell apart safely).
- Decision: Q2 ("Uninstall newline-in-path safety guard"): maintainer picked "Fix code: fail closed on embedded
  newlines". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Capture `readlink -f` with a trailing sentinel instead of piping through `tr -d '\n'` in
  `crates/farhelm-helm/src/provisioning/backend.rs`, fail closed when a resolved path contains an embedded newline so
  Uninstall refuses the layout, and add a focused regression test (estimate S, ~20-40 lines). Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## trailing-legacy-claude-settings-v036-hosts-never-cleaned.md

- Outcome: `fix spec`.
- Assessment: Confirmed for the leftover file and direct-to-Cleared path (`v0.36.0` wrote `ANTHROPIC_BASE_URL` and
  `ANTHROPIC_AUTH_TOKEN` into `~/.claude/settings.json`; schema 76->77 added nullable `tensorlake_credential_delivery`
  with no backfill at `store.rs:4957-4967`; `retire_legacy_settings` at `credential_delivery.rs:1229,1260-1261` marks
  `None` as `Cleared` directly). Live Claude Code precedence between `settings.json` and process env is unverified.
- Decision: Q3 ("Legacy v0.36.0 Tensorlake Claude & Codex cleanup"): maintainer picked "Fix spec: accept v0.36.0
  unstamped Tensorlake hosts" with note: "tensorlake is STILL experimental to be clear. Yes. accept this, fix spec."
  (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Update `SPEC_impl.md` ("Named credential catalog", around lines 6255-6259) to state that
  Tensorlake is still experimental and that legacy v0.36.0 Tensorlake hosts that never reconnected under v0.37-v0.43
  (carrying no delivery record) are accepted as not automatically migrated for Claude settings (alongside the existing
  crash acceptance). Combine with `trailing-legacy-codex-config-v036-hosts-never-cleaned.md` if convenient. Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.

## trailing-legacy-codex-config-v036-hosts-never-cleaned.md

- Outcome: `fix spec`.
- Assessment: Confirmed for the leftover file and direct-to-Cleared path (`v0.36.0` wrote `model_provider = "farhelm"`
  and `[model_providers.farhelm]` with bearer token into `~/.codex/config.toml`; `retire_legacy_settings` at
  `credential_delivery.rs:1230,1260-1261` marks `None` as `Cleared` directly). Live Codex `-c` merge vs replace behavior
  is unverified.
- Decision: Q3 ("Legacy v0.36.0 Tensorlake Claude & Codex cleanup"): maintainer picked "Fix spec: accept v0.36.0
  unstamped Tensorlake hosts" with note: "tensorlake is STILL experimental to be clear. Yes. accept this, fix spec."
  (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Update `SPEC_impl.md` ("Named credential catalog", around lines 6254-6259) to state that
  Tensorlake is still experimental and that legacy v0.36.0 Tensorlake hosts that never reconnected under v0.37-v0.43
  (carrying no delivery record) are accepted as not automatically migrated for Codex config as well as Claude settings.
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.

## trailing-replace-with-recent-named-fill-enter-replaces.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/launcher/view/search/start.rs:297-309,341-352,544-564`;
  `search.rs:308-323`; `start.rs:108-163`): in Replace with, a highlighted Recent row has
  `.aria_label("Fill from recent setup: ...")` and a summary-only description while pressing Enter immediately replaces
  and deletes the source session. Whether assistive technology speaks the child `⏎ Replace` label when the parent
  `ListBoxOption` carries an explicit `aria_label` is unverified.
- Decision: Q4 ("Replace with Recent setup Enter & screen-reader wording"): maintainer picked "Fix code: keep
  Enter=Replace, fix screen-reader name". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Keep click=fill and Enter=replace on Recent setups in Replace with, and update the highlighted
  option's accessible name/description so assistive technology explicitly announces that Enter replaces and deletes the
  source session (while clicking fills), with a focused regression test (estimate S, ~20-40 lines). Remove the feedback
  file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-safety.md` ``.

## trailing-sandbox-agent-plants-repository-suggestions.md

- Outcome: `fix spec+code`.
- Assessment: Confirmed by source (`crates/farhelm-helm/src/sessions.rs:2140-2211,2610-2656`;
  `store/repository_history.rs:84-153`): `record_created_history` gates only
  `environment_target.is_some() && origin != CreateOrigin::User`, so agent-origin checkouts (`CreateOrigin::Agent`)
  inside a sandbox/container (or on an ordinary host) promote arbitrary `owner/repo` values to the top of the helm-wide
  repository suggestion list across all hosts, surviving sandbox deletion.
- Decision: Q5 ("Agent-origin checkouts in repository suggestions"): maintainer picked "Fix spec+code: user launches
  only (exclude all agent checkouts)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Update `SPEC.md` ("Managed checkouts", around lines 2146-2148) to state that helm-wide repository
  suggestions come only from user-initiated Managed checkout launches and exclude agent-origin checkouts on every host;
  gate repository-history recording on `origin == CreateOrigin::User` in `crates/farhelm-helm/src/sessions.rs`, and add
  regression tests for both sandbox/container and ordinary-host agent checkouts (estimate S, ~20-40 lines). Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-security-and-agent-boundaries.md` ``.

## trailing-auto-trust-overrides-custom-flag-refusal.md

- Outcome: `fix spec+code`.
- Assessment: Confirmed for Farhelm's code (`crates/farhelm-helm/src/workspace_trust.rs:143-152`,
  `launches.rs:679-699,837-856,899-941`): `workspace_trust_blocked_by_selection` detects only Antigravity `--settings` /
  `--settings=`, so in a session's own sandbox/container Farhelm appends `--approve` / `-c <trusted>` after Custom Pi
  `--no-approve` or Codex untrusted `-c` overrides. Live Pi/Codex CLI flag precedence is unverified.
- Decision: Q6 ("Auto-trust vs Custom flags on Pi & Codex"): maintainer picked "Fix spec+code: honor Pi & Codex Custom
  trust flags too" with note: "But SPEC this as best effort detection ONLY. We do NOT guarantee it and we never document
  it as a guarantee." (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Extend `workspace_trust_blocked_by_selection` to also detect Pi `--no-approve`/`--approve` and
  Codex project-trust `-c` overrides in Custom permission flags (with unit tests, estimate S ~30-50 lines), and update
  `SPEC.md` (and `SPEC_impl.md` if applicable) to specify this explicitly as best-effort detection ONLY — never
  documented or promised as a guarantee. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-trust-settings-and-ui-wording.md` ``.

## trailing-host-setup-frozen-skip-ignores-later-restore.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/hosts/add.rs:948-968,1127,1313`; `menu.rs:419-424,454`;
  `setup.rs:799-824`): PR #4849 froze `asks_before_setup` at press time so turning skip ON mid-probe cannot skip
  confirmation, which caused turning "Set up new hosts without asking" OFF mid-probe to be ignored for that attempt
  (tension between `SPEC.md:2686-2687` and `SPEC_impl.md:996-1000`).
- Decision: Q7 ("Skip-confirmation preference timing during in-flight host probes"): maintainer picked "Fix code (both):
  safety wins in both directions" with note: "only if it stays simple. if this turns complicated it is not worth it -
  kick back into todo list." (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Check `asks_before_setup || setup::asks_first(cx)` when the plan settles (in `hosts/add.rs` and
  `hosts/setup.rs`) so turning skip OFF mid-probe restores the confirmation while turning skip ON mid-probe still cannot
  skip it, with a regression test (estimate S, ~15-35 lines). Only implement if it stays simple; if it turns
  complicated, kick it back into `TODO.md` with an explanation instead. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-gui-hosts-menus-and-terminal.md` ``.

## trailing-tensorlake-rerun-reads-skip-setting-late.md

- Outcome: `fix code`.
- Assessment: Partly correct. Confirmed that Tensorlake Re-run reads `GET /api/tensorlake` only when the probe settles
  (`crates/gui/farhelm-gui/src/hosts/setup.rs:804-812,858-899`), so turning "Create Tensorlake sandboxes without asking"
  ON while waiting in the probe queue skips the billed-creation confirmation. Reviewer misstated that Tensorlake probes
  wait on interactive SSH sign-in (they do not, though they share `plan_slots`).
- Decision: Q7 ("Skip-confirmation preference timing during in-flight host probes"): maintainer picked "Fix code (both):
  safety wins in both directions" with note: "only if it stays simple. if this turns complicated it is not worth it -
  kick back into todo list." (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Snapshot or read `create_without_asking` before probing Tensorlake Re-run and ask for
  confirmation if either the pre-probe or post-probe value says to ask, with a regression test (estimate S-M, ~40-80
  lines). Only implement if it stays simple; if it turns complicated, kick it back into `TODO.md` with an explanation
  instead. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-gui-hosts-menus-and-terminal.md` ``.

## trailing-unread-hook-and-spare-switches-read-as-off.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/settings/hook_settings.rs:47-54`,
  `settings/checkouts.rs:84-95`, `components/checkbox.rs:282-306`): both switches still use `.disabled(...)` instead of
  `.value_unknown(...)` (unlike `agent_trust.rs:457-469`), so screen readers hear `Role::Switch` + `aria_toggled(False)`
  while unread/unreadable.
- Decision: Q8 ("User-visible wording & screen-reader accuracy"): maintainer picked "Fix all 5 as recommended".
  (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Switch the hook-setup and spare-checkout Settings switches to `.value_unknown(...)` matching
  `agent_trust.rs`, and add regression tests verifying screen-reader state while unread/unreadable (estimate S, ~25-35
  lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-trust-settings-and-ui-wording.md` ``.

## trailing-stored-trust-chip-credits-settings-switch.md

- Outcome: `fix spec+code`.
- Assessment: Partly correct. Confirmed for agent-requested Restart approval cards
  (`approvals/model.rs:545-563,975- 1001`, `card.rs:559-562,601-607`), which show `TRUSTED_WORKSPACE` crediting the
  Settings switch even if turned off after launch; incorrect for Prompt cards (`model.rs:526,982-984`), which never show
  the trust chip. `SPEC.md:4721- 4722` currently prescribes the shared sentence.
- Decision: Q8 ("User-visible wording & screen-reader accuracy"): maintainer picked "Fix all 5 as recommended".
  (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Give agent-requested Restart approval cards stored-launch trust-chip wording (e.g. "Saved with
  this session when it started. Repository hooks, settings and commands can run without asking."), update
  `SPEC.md:4721-4722` to match, and add/update tests (and goldens if affected) (estimate S, ~20-35 lines). Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-trust-settings-and-ui-wording.md` ``.

## trailing-settings-trust-help-misdescribes-persistent-hosts.md

- Outcome: `fix spec+code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/settings/agent_trust.rs:235-237,267-269`;
  `SPEC.md:1885,1899,1907` vs `SPEC.md:1861-1863` and `workspace_trust.rs:29-37`): Settings help on all five agent pages
  (and those three `SPEC.md` lines) say "installed hosts" / "Sandbox and container sessions apply workspace trust
  automatically", omitting that persistent recipe hosts created with Add host follow Settings rather than trusting
  automatically.
- Decision: Q8 ("User-visible wording & screen-reader accuracy"): maintainer picked "Fix all 5 as recommended".
  (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Align the workspace-trust Settings help copy in `agent_trust.rs` and `SPEC.md:1885,1899,1907`
  with `SPEC.md:1861-1863` (distinguishing a session's own disposable sandbox/container from installed and persistent
  hosts created with Add host), and update affected tests/goldens (estimate S, ~15-25 lines). Remove the feedback file
  and its index entry.
- Execution: ``planned in `plans/queue/triage-trust-settings-and-ui-wording.md` ``.

## trailing-disposable-trust-hint-says-off-by-default.md

- Outcome: `fix spec+code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/session_pane/trust_band.rs:90-96,340-360`,
  `settings/agent_trust.rs:348-384`): the workspace-trust hint band does not check `HostRow::is_session_owned`, so when
  a trust question appears in a session's own sandbox/container, the band says trust is "off by default" and points to a
  Settings switch that does not apply there. (Reviewer cited plain Replace at `sessions.rs:4829` as plain Restart; both
  retain stored trust.)
- Decision: Q8 ("User-visible wording & screen-reader accuracy"): maintainer picked "Fix all 5 as recommended".
  (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Suppress the workspace-trust hint band when the session's host has `is_session_owned == true`,
  update `SPEC.md:1887-1917` to state that the hint applies only where the Settings switch applies, and add a regression
  test (estimate S, ~25-40 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-trust-settings-and-ui-wording.md` ``.

## trailing-menu-stop-and-suspend-exited-shell-wording.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/session_list/menu.rs:1318,1365-1387`;
  `session_list/environment.rs:457,540-564`; `environment_ui.rs:797-847`): unlike the ordinary Stop confirmation fixed
  in PR #4931 (`stop.rs:128-176`), the row menu's Stop and suspend ignores `status.has_ended()` and still offers to stop
  and keep an already-exited shell when only terminal tabs remain open.
- Decision: Q8 ("User-visible wording & screen-reader accuracy"): maintainer picked "Fix all 5 as recommended".
  (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Give the row menu's Stop and suspend item (tooltip, description, and confirmation/Cancel copy) a
  tab-only variant when the session's main program has already ended while tabs remain, matching PR #4931's wording, and
  add a regression test/golden (estimate S, ~40-70 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-gui-hosts-menus-and-terminal.md` ``.

## trailing-spec-file-click-release-wording.md

- Outcome: `fix spec`.
- Assessment: Confirmed by source (`SPEC.md:3753-3754` vs `SPEC_impl.md:3484-3485,3498-3499`,
  `attached.rs:2805-2816, 2887-2890,4092-4100`, `files.rs:209-238`): pressing closes the hover disclosure popover, so
  release cannot happen over a still-painted popover; `SPEC_impl.md` already states the real contract accurately.
- Decision: Q9 ("Stale or inaccurate spec prose & comments"): maintainer picked "Update spec & comments on all 4 to
  match code". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Reword `SPEC.md:3753-3754` to match shipped behavior and `SPEC_impl.md:3484-3499` (pressed while
  that hover's disclosure is painted and fully visible, and released before the disclosure is replaced, with link text
  unchanged). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.

## trailing-spec-impl-hook-answer-preview-wait.md

- Outcome: `fix spec`.
- Assessment: Confirmed by source (`SPEC_impl.md:1005-1006` vs `launcher/view/hook_question.rs:228-230,245-266,308-310`,
  `view.rs:1405-1436,1511-1536`, `checkout.rs:402-428`): only the YOLO answer routes through
  `submit_or_wait_for_preview`; a hook-setup answer saves any setting and stops safely with `WAIT_FOR_PREVIEW`.
- Decision: Q9 ("Stale or inaccurate spec prose & comments"): maintainer picked "Update spec & comments on all 4 to
  match code". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Narrow `SPEC_impl.md:1005-1006` so it accurately distinguishes the YOLO answer (carries
  permission through a host-setting write or checkout preview) from the hook-setup answer (carries permission through
  its setting write and refuses safely if a checkout preview is still loading). Remove the feedback file and its index
  entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.

## trailing-spec-impl-custom-agent-id-letter.md

- Outcome: `fix spec`.
- Assessment: Confirmed by source (`SPEC_impl.md:7613` says registered IDs "start with a letter", contradicting
  `SPEC.md:493`, `custom_agents.rs:37-54,95-98`, and `main.rs:910-922`, which allow leading digits).
- Decision: Q9 ("Stale or inaccurate spec prose & comments"): maintainer picked "Update spec & comments on all 4 to
  match code". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Change `SPEC_impl.md:7613` from "start with a letter" to "start with a lowercase letter or
  digit". Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.

## trailing-stale-references-to-removed-repository-search.md

- Outcome: `fix spec+code`.
- Assessment: Confirmed across all 6 sites on `5af40d23e`: `SPEC.md:1854`, `service/connection.rs:1190`,
  `working_copies.rs:86,92`, `farhelm-proto/src/lib.rs:445,2597-2599`, and `ui_channel_tests.rs:1299-1305` still refer
  to the removed host-side repository discovery/search.
- Decision: Q9 ("Stale or inaccurate spec prose & comments"): maintainer picked "Update spec & comments on all 4 to
  match code". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Clean up all 6 stale references to removed repository discovery/search in `SPEC.md`, code
  comments, and the `ui_channel_tests.rs` test docstring (docs/comments only, ~15 lines). Remove the feedback file and
  its index entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.

## trailing-file-prepaint-click-test-gap.md

- Outcome: `other`.
- Assessment: Confirmed as a test-only coverage gap (`terminal/files/tests.rs:720-764` never triggers render/placement,
  so `self.disclosure` is `None` rather than exercising `shown.drawn.get().is_some()` at `attached.rs:4097`; GPUI
  mid-frame mouse dispatch timing unverified). Shipped code obeys `SPEC.md:3753`.
- Decision: Q10 ("Pure product-test coverage gaps"): maintainer picked "Re-apply Nov 21 rule + fix security-binding test
  (#21)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Apply the Nov 21 rule: fix the test if the fix is a few lines (about 20 at most) and the test
  guards a user-facing behavior; otherwise make no change. Either way remove the feedback file and its index entry, and
  record in Execution which way it went.
- Execution: ``planned in `plans/queue/triage-gui-hosts-menus-and-terminal.md` ``.

## trailing-tab-shell-hangup-delete-wiring-untested.md

- Outcome: `other`.
- Assessment: Confirmed as a test-only coverage gap (`service/session_hosts.rs:518-562` `sweep_roots` and
  `teardown.rs:529-534` tab-shell wiring into `SweepTarget::WholeSession` are not exercised by any test; only
  `sweep.rs:3576` constructs `WholeSession` directly). Shipped code is correct.
- Decision: Q10 ("Pure product-test coverage gaps"): maintainer picked "Re-apply Nov 21 rule + fix security-binding test
  (#21)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Apply the Nov 21 rule: fix the test if the fix is a few lines (about 20 at most) and the test
  guards a user-facing behavior; otherwise make no change. Either way remove the feedback file and its index entry, and
  record in Execution which way it went.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## trailing-agent-create-connection-binding-untested.md

- Outcome: `fix code`.
- Assessment: Confirmed as a test coverage gap guarding a security-relevant host-binding path (`agent_requests.rs:2217`
  forwards `approval.target_incarnation` into `dispatch_agent_create`, but the reconnect test at `:11451-11466` calls
  `dispatch_agent_create` directly and never exercises `create_for_agent`'s forwarding). Shipped code obeys
  `SPEC_impl.md:9825-9826`.
- Decision: Q10 ("Pure product-test coverage gaps"): maintainer picked "Re-apply Nov 21 rule + fix security-binding test
  (#21)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Add a focused helm test pinning `approval.target_incarnation` forwarding through
  `create_for_agent` into `dispatch_agent_create` (estimate S-M, ~35-60 lines). Remove the feedback file and its index
  entry.
- Execution: ``planned in `plans/queue/triage-security-and-agent-boundaries.md` ``.

## trailing-trust-hint-late-refusal-untested.md

- Outcome: `other`.
- Assessment: Confirmed as a test-only coverage gap (`trust_band_tests.rs:666-731` covers refusal-then-dismissal and
  dismissal-then-204, not dismissal-then-refusal against `trust_band.rs:316-327`). Shipped code obeys `SPEC.md:1897`.
- Decision: Q10 ("Pure product-test coverage gaps"): maintainer picked "Re-apply Nov 21 rule + fix security-binding test
  (#21)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Apply the Nov 21 rule: fix the test if the fix is a few lines (about 20 at most) and the test
  guards a user-facing behavior; otherwise make no change (estimate ~12-15 lines). Either way remove the feedback file
  and its index entry, and record in Execution which way it went.
- Execution: ``planned in `plans/queue/triage-trust-settings-and-ui-wording.md` ``.

## trailing-switch-warning-description-untested.md

- Outcome: `other`.
- Assessment: Confirmed as a test-only coverage gap (`settings/tests.rs:2403-2412` checks separate `Role::Label` help
  text rather than `node.description` on the workspace-trust and hook-setup switches; `agent_trust.rs:465` and
  `hook_settings.rs:50` wire `aria_description` correctly).
- Decision: Q10 ("Pure product-test coverage gaps"): maintainer picked "Re-apply Nov 21 rule + fix security-binding test
  (#21)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Apply the Nov 21 rule: fix the test if the fix is a few lines (about 20 at most) and the test
  guards a user-facing behavior; otherwise make no change (estimate ~15-20 lines). Either way remove the feedback file
  and its index entry, and record in Execution which way it went.
- Execution: ``planned in `plans/queue/triage-trust-settings-and-ui-wording.md` ``.

## trailing-replace-in-environment-history-check-vacuous.md

- Outcome: `other`.
- Assessment: Confirmed as a vacuous test check (`sessions_tests.rs:862-884` checks `launch_history` is empty after a
  command launch, which never writes `launch_history` at `store.rs:210-217`, so `HistoryScope::RepositoryOnly` at
  `sessions.rs:2202-2208` is unguarded at the route level). Shipped code obeys `SPEC_impl.md:5326-5328`.
- Decision: Q10 ("Pure product-test coverage gaps"): maintainer picked "Re-apply Nov 21 rule + fix security-binding test
  (#21)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Apply the Nov 21 rule: fix the test if the fix is a few lines (about 20 at most) and the test
  guards a user-facing behavior; otherwise make no change (estimate ~10-20 lines). Either way remove the feedback file
  and its index entry, and record in Execution which way it went.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## trailing-session-owned-setup-flag-untested.md

- Outcome: `other`.
- Assessment: Confirmed as a test-only coverage gap (`sessions_tests.rs:12339-12368` tests `needs_checkout_root` only on
  the local installed host; `assert_new_environment_checkout` at `:575-690` never queries `/api/launch-history` for
  `owned`). Shipped code obeys `SPEC.md:1844-1846`.
- Decision: Q10 ("Pure product-test coverage gaps"): maintainer picked "Re-apply Nov 21 rule + fix security-binding test
  (#21)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Apply the Nov 21 rule: fix the test if the fix is a few lines (about 20 at most) and the test
  guards a user-facing behavior; otherwise make no change (estimate ~8-12 lines). Either way remove the feedback file
  and its index entry, and record in Execution which way it went.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## trailing-drift-copyback-runs-while-agent-may-live.md

- Outcome: `fix code`.
- Assessment: Partly correct. Confirmed control flow in
  `crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs:741- 760,873-880` and `private_config.rs:18-22,589-595`:
  copy-back gates only on `state.close().is_ok()` (terminal host shutdown), even when session create failed partway
  without deleting or session delete failed its process-tree sweep. Vendor rename-save during those windows is
  unverified.
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Track confirmed session deletion and absence of failed-create leftovers in `DriftEnv` so
  `PrivateConfig::finish` gates copy-back accurately (and emits notes when skipped), update the misleading comments in
  `private_config.rs`, and add unit tests (estimate S-M, ~40-65 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## trailing-drift-runner-drops-requested-harness.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`scripts/run-harness-drift.py:279-296`): `--harness` validates against all
  categories' cases rather than the selected categories, so passing multiple `--harness` values where one has no case in
  the selected category silently drops it and can exit 0 (`PASSED`).
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: In `scripts/run-harness-drift.py`, refuse with a clear error when any explicitly requested
  `--harness` has no matching case in the selected categories, and cover this in `scripts/test-run-harness-drift.py`
  (estimate S, ~20-35 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## trailing-drift-panic-skips-copyback.md

- Outcome: `fix code`.
- Assessment: Confirmed for control flow (`crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs:856-880`): a panic
  inside `run(...).await` unwinds past `config.finish(...)`, dropping the private mirror without copying back refreshed
  vendor logins or printing a note.
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Catch unwinds around `run(...).await` in `with_drift_supervisor_using`, run normal teardown and
  `config.finish(...)`, then resume unwind on panic, with a unit test (estimate S, ~20-35 lines). Remove the feedback
  file and its index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## trailing-drift-failed-copyback-passes-case.md

- Outcome: `fix code`.
- Assessment: Confirmed by source
  (`crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:294-326,595-623`;
  `drift/mod.rs:874-880,906-925`): a failed `replace_real` copy-back appends a note and still returns `Ok`, letting the
  drift case report `PASSED` (commit `39f269bc0b` avoided early-aborting later files, which can be preserved while still
  failing the case after the loop).
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Continue copying all remaining files and running `real_leaves_unchanged()`, then fail the drift
  case (after printing notes) when any copy-back I/O replacement failed, update `docs/harness-drift-tests.md`, and add a
  unit test (estimate S, ~20-35 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## trailing-drift-cursor-initial-prompt-never-passes.md

- Outcome: `fix code`.
- Assessment: Confirmed by source
  (`crates/farhelm/tests/e2e/real_agent_capture/drift/initial_prompt.rs:58-70,101-108, 302-310`): `untracked_selector`
  returns `Ok(None)` unconditionally for Cursor, so the named second step always ends `COULD NOT RUN`. Explicitly left
  unfixed by `plans/queue/agent-catalog-cli-and-drift-cleanups.md` PR 4; also noted in `TODO.md:480-485` (Plan
  follow-ups).
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Seed Cursor's `initial_prompt` drift case using `cursor-agent create-chat` (reusing the pattern
  in `named_conversation.rs:258-264` and shared `vendor_lookup` helper) so both fresh and named steps can pass, remove
  the matching `TODO.md:480-485` entry if fully addressed, and add unit coverage (estimate S, ~25-45 lines). Remove the
  feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## trailing-drift-capture-timing-check-too-coarse.md

- Outcome: `fix code`.
- Assessment: Confirmed by source
  (`crates/farhelm/tests/e2e/real_agent_capture/drift/capture_timing_hooks.rs:291-340, 365-398`): `FirstPrompt`,
  `FirstPromptWithHooks`, and `FirstReply` share one branch checking only no capture before prompt and expected lines
  present after the turn, without checking hook-log ordering for prompt-timed agents.
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Tighten `capture_timing_hooks` to assert hook-log ordering (prompt/start capture line precedes
  `event=Stop`) for `FirstPrompt` / `FirstPromptWithHooks` agents, with unit tests (estimate S, ~25-40 lines). Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## trailing-release-notes-recapture-parks-on-tooltip-field.md

- Outcome: `discard`.
- Assessment: Confirmed by source geometry and timing (`scripts/docs-shots/release-notes.py:157-166`,
  `gui-agent.py: 1858-1861,1877`, `launcher/view.rs:1642-1665`, `layers/tooltip.rs:15,74`); not observed visually.
  Published release shots are frozen and human-reviewed if ever recaptured.
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Remove the feedback file and its index entry without changing code or spec.
- Execution: ``planned in `plans/queue/triage-queue-discards.md` ``.

## trailing-brick-cleanup-refuses-untested-remote-mac-pair.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`releasing/mac-vm-test/brick_test.py:280-286,936-947,1399-1418,1675-1680,1882-1900`):
  after `stop-after-p0` narrows scope before any test runs on a prepared remote Mac P1 pair, `cleanup` still requires
  `native_preflight_retained(entry)` and refuses cleanup, contrary to `BRICK-TEST.md:715-716`.
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: In `releasing/mac-vm-test/brick_test.py`, require `native_preflight_retained(entry)` during
  cleanup only when the pass is in scope or already has results/preflight/tester evidence, and add a unit test in
  `test_native_coordinator.py` (estimate S, ~15-25 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## trailing-drift-claude-trust-real-config-byte-check.md

- Outcome: `fix code`.
- Assessment: Confirmed by source
  (`crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:255-273,478-481, 584,617`): byte-for-byte
  equality on real `~/.claude.json` fails whenever another active Claude Code session on the shared host rewrites it
  during the drift case (frequency unmeasured).
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: For copied Claude config files in `private_config.rs`, fail only if the real file gained a trust
  entry (`hasTrustDialogAccepted`) for the scratch workspace key (or emit a `drift:` note on unrelated byte changes),
  update `docs/harness-drift-tests.md`, and add unit tests (estimate S, ~25-40 lines). Remove the feedback file and its
  index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## trailing-drift-claude-trust-ancestor-vacuous-pass.md

- Outcome: `fix code`.
- Assessment: Confirmed by source
  (`crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:228-243,465-547, 652-656`): copying real
  `~/.claude.json` verbatim preserves ancestor `projects` entries, so if the host's config already trusts `/tmp` or `/`,
  Claude inherits trust (`SPEC_impl.md:3637-3638`) and the drift case passes without proving Farhelm's entry took
  effect. Local config state unverified.
- Decision: Q11 ("Maintainer drift, docs-shot & brick-test tooling"): maintainer picked "Fix the 9 drift & brick-test
  items; discard frozen shot (#32)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Strip `hasTrustDialogAccepted` / ancestor `projects` entries for `work.ancestors()` from the
  private mirror copy of Claude config files before launching the Claude trust drift case (so the test proves Farhelm's
  entry works even if `/tmp` or `/` is trusted in real config), and add a unit test (estimate S, ~20-35 lines). Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-drift-and-brick-tooling.md` ``.

## upload-cancel-mid-publication-reported-not-published.md

- Outcome: `fix code`.
- Assessment: Partly correct. Confirmed in source (`crates/farhelm-supervisor/src/service/uploads.rs:1224-1264` vs
  `:1265-1289`, `helm-client/src/transfers.rs:134-144`, `gui/.../attachments.rs:876-885`): `DiskStage::Cancelled` says
  `"publication completion is unknown"` but sends `ErrorKind::NotFound`/`InvalidRequest` (4xx -> `NotPublished`) instead
  of `ErrorKind::Internal` (5xx -> `Unknown`). Live reachability to a waiting GUI requires an unverified half-closed
  helm->supervisor pipe (`service/connection.rs:774`).
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Return `ErrorKind::Internal` for `DiskStage::Cancelled` in `commit_upload`
  (`crates/farhelm-supervisor/src/service/uploads.rs:1254-1258`, matching `DiskStage::Failed` and `SPEC_impl.md:5050`),
  and add a focused unit test (estimate S, ~15-30 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## preapproved-agent-requests-skip-deleting-session-refusal.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/farhelm-helm/src/approvals.rs:453-505`): preapproved native-link and
  standing-permission returns (`:468-480`) run before the `inner.deleting.contains_key(&asking_key)` refusal
  (`:499-505`), so cardless preapproved requests from a deleting session are carried out instead of refused
  (`SPEC_impl.md:9797-9799`, `approvals.rs:248`).
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Check `state.approvals.session_is_being_deleted(&asking_key)` before preapproval fast paths in
  `ask_with_answer` (`crates/farhelm-helm/src/approvals.rs`), and add a regression test (estimate S, ~25-35 lines).
  Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-security-and-agent-boundaries.md` ``.

## update-identity-unverified-misleading-plan-again-remedy.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/farhelm-helm/src/provisioning/service.rs:1788-1799,3021-3028,3220-3240`):
  Update planning accepts a host with a recorded identity when the supervisor reports none (`IdentityUnverified`), then
  execution fails with `"the supervisor identity changed after UPDATE planning ... plan again"` in a futile loop
  omitting the real remedies already written in `attach_refusal` (`:392-397`).
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Refuse `recorded = Some` / `reported = None` during Update planning with the `IdentityUnverified`
  remedies from `attach_refusal`, distinguish missing vs changed identity in `revalidate` (`service.rs:3021-3028`), and
  add regression tests (estimate S, ~25-45 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## installer-fallback-lock-world-openable.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/farhelm-supervisor/src/installation/mod.rs:208-215`): `FALLBACK_LOCK_FILE`
  omits `.mode(0o600)`, creating a `0644` file that another local user who can traverse `~/Applications` can open and
  flock-lock to block installs. (The umask-0 substitution variant is out of scope per `SPEC.md:5994-6004`.)
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Create `FALLBACK_LOCK_FILE` with `.mode(0o600)` at
  `crates/farhelm-supervisor/src/installation/ mod.rs:208-215` (leaving `~/Applications` folder creation standard) and
  add a unit test (estimate S, ~15-30 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## trailing-file-disclosure-holdback-silent-click.md

- Outcome: `fix code`.
- Assessment: Confirmed by source
  (`crates/gui/farhelm-gui/src/terminal/attached.rs:521-523,2429-2433,2693,2802-2841, 2901-2932,4092-4100`): when a
  terminal file hover card would cover the pointer and is held back via `refusal_waits_for_hover`, `link_released`
  returns silently instead of emitting `Hidden::NotVisible.file_words()`, unlike web links (`:2833-2841`) and contrary
  to `SPEC_impl.md:3555-3562`.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Show the `"not visible"` refusal (`Hidden::NotVisible.file_words()`) when clicking a terminal
  file path whose disclosure was held back over the pointer (matching web links and `SPEC_impl.md:3555-3562`), and add a
  regression test (estimate S-M, ~35-60 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-gui-hosts-menus-and-terminal.md` ``.

## trailing-reposted-notice-not-revealed.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/notices.rs:177-186`, `main_view.rs:203-207,260,463-474`):
  `notices::post` deduplicates in place, so re-posting an earlier notice in an overflowing banner stack leaves it
  scrolled out of view instead of moving it into view.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Move a re-posted notice to the bottom of the notice stack in `notices::post` and scroll it into
  view in `main_view.rs` (tracking a monotonic post sequence or position change), with a regression test (estimate S,
  ~20-35 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-gui-hosts-menus-and-terminal.md` ``.

## trailing-launcher-new-clone-enter-after-vanished-pick.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/launcher/view/search.rs:308-326,662-685`,
  `folder_list.rs:175-182,221-249`, `history.rs:110-139`): PR #4812 scoped the presented-frame `ActionFrame` Enter
  guards for vanished empty-pane highlights and folder dropdown highlights to `Mode::ReplaceWith`, letting Return in New
  and Clone fall through and launch the form draft. Live folder timing unverified.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Extend the existing presented-frame `ActionFrame` Enter guards for empty-pane highlights and
  folder dropdown highlights to New and Clone (not just Replace with), matching `SPEC.md:1235-1237` and `5871-5873`, and
  add regression tests (estimate S, ~30-60 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-safety.md` ``.

## trailing-launcher-held-escape-discards-draft.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/launcher/view/search.rs:644-659`, `view.rs:230-248`,
  `save.rs:95-104`, `focus.rs:237-269,306-328`): `focus::note_closing(true, ...)` is called only when clearing
  `start_highlight`, not when dismissing wide-pane `inline_results` or `save_panel`, so auto-repeat Escape closes the
  launcher and discards the draft. Platform auto-repeat delivery unverified.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Call `focus::note_closing(true, window, cx)` whenever `Launcher::on_dismiss` closes an inner
  launcher region (`inline_results` or `save_panel`) and returns `Dismissal::Stay`, preserving fresh second Escape to
  close the launcher, and add tests for both regions (estimate S, ~10-25 lines). Remove the feedback file and its index
  entry.
- Execution: ``planned in `plans/queue/triage-launcher-safety.md` ``.

## trailing-launcher-stale-permanent-yolo-answer-silent.md

- Outcome: `fix code`.
- Assessment: Confirmed by source
  (`crates/gui/farhelm-gui/src/launcher/view/yolo_question.rs:241-243,318-330,422-426, 460-470`, `save.rs:38-91`,
  `view.rs:1252-1254,2396-2398,2847-2876`): clicking "Start, and don't ask again on this host" when the launch is
  refused (including simply opening Save as template) or changed silently closes the question with no reason, and
  post-write staleness saves the host setting silently with no notice; a closed `Inline` also suppresses `alert` in
  `question.or(alert)`.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Check `inline.is_open(cx)` in `yolo_inline` so a closed inline slot never hides refusal alerts;
  give pre-write stale permanent YOLO answers visible refusal feedback consistent with the one-off answer (including
  Save-panel-open); show a clear launcher note on post-write staleness stating that the host now starts YOLO sessions
  without asking and the launch did not start because it changed; update/add tests (estimate S-M, ~40-80 lines). Remove
  the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-safety.md` ``.

## trailing-podman-image-inspect-hits-output-cap.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/farhelm-helm/src/environment/podman/build.rs:821-839,984-1000`,
  `provisioning/backend.rs:106,2123-2156`): both `podman image inspect` calls are unprojected and pass through the 64
  KiB stdout cap, unlike `container inspect` (fixed in #4880). Real-image exceedance threshold unverified.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Project both `podman image inspect` calls in `podman/build.rs` with `--format` extracting only
  `Id` and the needed `farhelm.*` labels (matching #4880's `container inspect` projection), and update/add unit tests
  (estimate S, ~35-55 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## trailing-podman-leftover-listing-hits-output-cap.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/farhelm-helm/src/environment/podman.rs:891-893,917-922,1601-1644`,
  `provisioning/backend.rs:106,2123-2156`): `list_owned` runs unprojected `podman ps --all --no-trunc --format=json`
  through the 64 KiB cap even though it keeps only container IDs, and one oversized host fails the listing for all hosts
  (note `try_join_all` all-or-error semantics at `:891-893` are deliberate). Container-count threshold unverified.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Switch `list_owned` in `crates/farhelm-helm/src/environment/podman.rs` to
  `--no-trunc --format {{.ID}}` (preserving deliberate `try_join_all` all-or-error semantics), and update/add unit tests
  (estimate S, ~25-40 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## trailing-recipe-home-launch-skips-early-title-check.md

- Outcome: `fix code`.
- Assessment: Confirmed end-to-end by source (`crates/farhelm-helm/src/sessions.rs:1196-1247`,
  `main.rs:730-759,1121- 1165`, `agent_requests.rs:4415-4440,4653`, `launcher.rs:1230-1233`):
  `CreateReq::prepare_sandbox_launch` validates `self.title` only inside
  `if let Some(checkout) = &self.github_checkout`, so home-folder recipe launches with an invalid `--title` (control
  chars or >64 KiB) create and briefly bill a disposable environment before the supervisor refuses. GUI paste filtering
  unverified.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Validate `self.title` unconditionally in `CreateReq::prepare_sandbox_launch` (printable check +
  64 KiB cap with a neutral non-checkout error message) so invalid titles are refused before provisioning for
  home-folder recipe launches as well as managed checkouts, and add tests for both GUI/REST and agent-request routes
  (estimate S, ~25-45 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-provisioning-podman-and-runtime.md` ``.

## trailing-repository-history-failure-shown-as-no-matches.md

- Outcome: `fix code`.
- Assessment: Confirmed by source (`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:538-559`, `search.rs:885-892`,
  `checkout_tests.rs:648-658`): `follow_repository_history` collapses `GET /api/github-repositories` errors via
  `unwrap_or_default()`, so the launcher displays `"No repository matches; enter owner/repo"` instead of a load failure.
- Decision: Q12 ("Straightforward small product fixes"): maintainer picked "Fix all 14". (2026-11-25 fewest-questions
  triage page, main `5af40d23e`.)
- Completion criteria: Preserve repository-history load errors in `checkout.repositories` and show
  `"Couldn't load recent repositories; enter owner/repo"` in `repository_empty_message` while keeping manual
  `owner/repo` entry working and keeping the once-per-opening read (`SPEC_impl.md:5121`), with a regression test
  (estimate S, ~25-45 lines). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-launcher-safety.md` ``.

## shortcuts-chord-during-composition.md

- Outcome: `fix spec`.
- Assessment: Partly correct. Confirmed in source for Linux XKB compose cancellation
  (`vendor/gpui-pre-linux/src/linux/ wayland/client.rs:2218-2263`, `x11/client.rs:1165-1214`, `attached.rs:4401-4409`,
  `main_view.rs:309-313`), which clears `composition.marked` before dispatching `KeyDown` (live Wayland/X11 unverified);
  fixing requires patching vendored GPUI. Part 2 (focused text fields outside focus layers) has no shipped route
  (`shortcuts.rs:268-270`).
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Document the Linux XKB compose-cancellation limitation for `Ctrl+Shift+/` in `SPEC_impl.md` (no
  vendored GPUI patch; Part 2 has no shipped non-layer text field). Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.

## provisioning-orphan-sweep-deletes-concurrent-helm-upload.md

- Outcome: `discard`.
- Assessment: Confirmed in source (`crates/farhelm-helm/src/provisioning/backend.rs:735-814,1235,1430,1764-1770`;
  cross-helm overlap not live-executed): two separate helms provisioning the same host simultaneously can delete each
  other's active staging temp file, causing a fail-closed, visible, retryable failure with existing binary/data intact.
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Remove the feedback file and its index entry without changing code or spec.
- Execution: ``planned in `plans/queue/triage-queue-discards.md` ``.

## release-payload-cache-misses-private-dir-hardening.md

- Outcome: `discard`.
- Assessment: Confirmed in source (`crates/farhelm-helm/src/provisioning/release_payloads.rs:822,1364` vs
  `payloads.rs:455-475,1212-1215`): the release payload cache uses plain `create_dir_all` inside `helm_state_dir`, which
  is `0700` on all production callers (`supervisor/src/lib.rs:119-126`). Zero user-facing impact today.
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Remove the feedback file and its index entry without changing code or spec.
- Execution: ``planned in `plans/queue/triage-queue-discards.md` ``.

## podman-settle-claim-incumbent-state-non-atomic.md

- Outcome: `discard`.
- Assessment: Confirmed in source (`crates/farhelm-helm/src/environment/podman_probe.rs:606-642`,
  `store/podman.rs:425-501`): if an unused twin host entry finishes verifying identity between reading live actor state
  and committing `claim_podman_place`, the unused place claim still transfers — a benign routing-preference shift
  between twins on the same machine+UID with zero containers/images/helpers.
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Remove the feedback file and its index entry without changing code or spec.
- Execution: ``planned in `plans/queue/triage-queue-discards.md` ``.

## trailing-grid-wide-glyph-absorbs-foreign-spacer.md

- Outcome: `fix spec`.
- Assessment: Confirmed in source (`vendor/alacritty_terminal/src/term/mod.rs:2021-2047`,
  `crates/farhelm-supervisor/src/grid_output.rs:231-238,272-289`): a non-wide-aware program deleting starting on the
  second column of a wide character can leave one cell with the wrong background color; `grid_output.rs:272-273`
  deliberately avoids style comparison so variation-selector widening keeps working.
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Add mid-wide-character DCH/ICH edits (deleting/inserting starting on a wide character's spacer
  column) to `SPEC.md:3396-3399`'s 2026-11-21 terminal fidelity exclusions. Remove the feedback file and its index
  entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.

## trailing-hook-hint-keeps-stale-save-error.md

- Outcome: `discard`.
- Assessment: Confirmed in source
  (`crates/gui/farhelm-gui/src/session_pane/hook_band.rs:39-44,66-83,96-117,143-149, 194-204`). Exact re-report of
  `hook-hint-can-repaint-old-dismissal-error.md`, already closed as no-change in PR #4863 under the 2026-11-24
  cheap-polish threshold (`TRIAGE_OUTCOMES_ARCHIVE.md:45849-45872`).
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Remove the feedback file and its index entry without changing code or spec.
- Execution: ``planned in `plans/queue/triage-queue-discards.md` ``.

## trailing-trust-hint-hide-survives-reconnect.md

- Outcome: `discard`.
- Assessment: Confirmed in desktop source
  (`crates/gui/farhelm-gui/src/session_pane/trust_band.rs:145-151,213-217, 495-500`; gap-during-disconnect unverified):
  local × hides intentionally survive transient reconnects so brief disconnects do not re-show dismissed hints.
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Remove the feedback file and its index entry without changing code or spec.
- Execution: ``planned in `plans/queue/triage-queue-discards.md` ``.

## trailing-relative-claude-lock-in-old-cleanup-records.md

- Outcome: `discard`.
- Assessment: Confirmed in source (`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:49-60,120-139,264-281`;
  `launch.rs:1986-1999`): pre-#4902 checkout cleanup records written under a relative `CLAUDE_CONFIG_DIR` retain a
  relative `lock_path`. Practically unpopulated in the wild, and `TODO.md:125-129` (Immediate) already plans to drop
  post-clone Claude trust edits.
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Remove the feedback file and its index entry without changing code or spec.
- Execution: ``planned in `plans/queue/triage-queue-discards.md` ``.

## trailing-symlinked-claude-json-permanent-warning.md

- Outcome: `fix spec`.
- Assessment: Confirmed in source
  (`crates/farhelm-helm/src/provisioning/credential_delivery.rs:835-858,935-938,1293- 1342`,
  `tensorlake_backend.rs:147-159`, `backend.rs:348-354,380-386,1302-1305,1345-1357`): a recipe image shipping a
  symlinked `~/.claude.json` lacking `hasCompletedOnboarding: true` is deliberately left untouched, retries once, and
  emits a generic `"could not install the agent settings"` warning on every reconnect.
- Decision: Q13 ("Rare / low-value edge cases & tradeoffs"): maintainer picked "Accept / discard as listed (3 fix spec,
  6 discard)". (2026-11-25 fewest-questions triage page, main `5af40d23e`.)
- Completion criteria: Update `SPEC_impl.md:6836-6840` to explicitly accept the generic persistent warning and retry on
  symlinked/non-regular `~/.claude.json` alongside the 1 MiB trade-off. Remove the feedback file and its index entry.
- Execution: ``planned in `plans/queue/triage-spec-and-comment-updates.md` ``.
