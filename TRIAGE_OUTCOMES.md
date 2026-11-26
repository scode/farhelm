# Triage outcomes

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
