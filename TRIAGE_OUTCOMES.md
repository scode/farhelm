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
