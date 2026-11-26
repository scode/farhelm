# Triage outcomes

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
