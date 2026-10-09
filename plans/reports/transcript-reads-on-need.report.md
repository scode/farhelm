## What this was about

The supervisor repeatedly read unchanged Codex transcripts and Grok saved conversations while checking session state,
including requests to list sessions. The same passes tried to take absent hook reports, reparsed database queries, and
opened notification-resolution transactions with nothing to resolve. The original ten-session measurements showed these
costs consuming a substantial share of a core.

The three draft PRs remove that redundant work. Transcript headers stop at the first line. Accepted reports establish
which conversation can be resumed; lists, session information, replayed creates and periodic checks use the saved
decision. Restart checks the exact saved conversation before launching anything. Database query statements are reused,
empty report directories avoid unnecessary file operations, and notification resolution runs only when it could resolve
a warning. How often the supervisor checks session state is unchanged; no new CPU-percentage claim is made.

## Things you should know

Relaunch Codex sessions started before this update before relying on Resume for a conversation begun with `/clear`. If
the replacement transcript did not exist when clear began, those processes still have the old hooks and cannot confirm
its later arrival. New Codex launches use the end-of-turn Stop callback. Only a callback for the selected conversation
can confirm it; another conversation cannot replace it. Conversation-start and confirmation reports survive a supervisor
outage separately and are applied in that order.

A deleted saved file can leave Resume available until you choose Restart. That click refuses without starting another
agent, withdraws Resume and adds the existing notification. A file holding a different conversation withdraws Resume
silently. A temporary read failure refuses that attempt with a retry message and leaves Resume available.

Actual Codex 0.162.0 completed two short turns around ordinary `/clear` in an owned test session using the existing API
authentication. Stop named an existing matching root transcript at callback time in both conversations. Clear changed
the runtime and saved-file path, and preserved the old header. The deliberate real-agent regression also passed. No live
Farhelm sessions or installation were changed.

The implementation follows the maintainer's decisions: confirmation through a later hook, file checks when Restart needs
them, retryable read errors, and the explicit breaking relaunch caveat. No new polling, file cache, restart-offer state
or upgrade workaround was added. Both covered TODO entries are removed.

## Open questions and possible follow-ups

None. The separate TODO about how often the supervisor checks session state remains outside this plan.

## PRs

- [#1734: read less data to verify conversations](https://github.com/scode/farhelm/pull/1734/changes), head
  `a979c86c591f272dd144cbb0d470f9d9411f14d2`, based on main.
- [#1736: reduce idle supervisor overhead](https://github.com/scode/farhelm/pull/1736/changes), head
  `581142075cab92c7f5e52d3120d9e8a08e9879b6`, based on #1734.
- [#1738: stop supervisor transcript polling](https://github.com/scode/farhelm/pull/1738/changes), head
  `9767488d8b6ccefbf8e024ec13e831f18b64cc99`, based on #1736.

All three are drafts, unmerged and not marked ready.

## Checks run, reused and skipped

Run now, through the recorder with pinned nextest, four slots, zero retries and pinned tmux 3.7c where applicable:

- `cargo nextest run -p farhelm-supervisor --lib -E 'test(codex) | test(grok) | test(hook_report) | test(report_files) | test(hook_argv)'`:
  59 passed. Run `466ab402-6084-4b5d-907b-8266e857fed7`; 995 selected-out cases. Covers saved-only projections, actual
  Restart read errors, missing-file notifications and callback admission.
- `cargo nextest run -p farhelm --test e2e -E 'test(codex_identity::) | test(=hook_identity::grok_selection_survives_enrichments_made_while_no_supervisor_runs)'`:
  four tests passed, plus fixture setup. Run `59e44cad-21b8-40ce-825b-b0814cf94b1c`; 357 selected-out cases. The
  corrected exact Codex journey also passed in `c3bca93f-9a9d-4a34-b414-b05cb1b43dca`.
- `cargo nextest run -p farhelm --test e2e --run-ignored only -E 'test(=real_agent_capture::real_codex_stop_names_the_persisted_transcript_after_clear)'`:
  one test passed, plus fixture setup. Run `117ae621-7b3a-4631-95d3-bd897bca31f2`; 360 other cases were not selected.
  The preceding actual callback audit passed in `0808c0c6-49c7-4c25-9ed9-addf96cb81f9`. These commands have complete
  source, lifecycle, output and runner evidence; no runtime skip was reported.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  and `cargo clippy -p farhelm --test e2e -- -D warnings` passed, covering test seams and the shipped configuration.
- Rust formatting, targeted dprint, changelog-format lint, the isolated source-delay check (276 delays, zero missing
  rationales), and the website build with internal-link validation passed. The final build includes the corrected Codex
  report-retention explanation. No preview server was started in this unattended run.

Reused from the earlier review units: first-line reader tests in `736f2da6-7dc1-46ad-880a-3331e417ca4a` (six passed),
unchanged parser/safety coverage from `61137769-70fe-4ac4-8597-4293dce5faac` (66 passed), and unchanged
database/report/notification filtering coverage from `f4de7862-7765-4bff-b17d-6861d3ccf239` (179 passed). These covered
the original pushed header and overhead revisions `f379c1540ce278c909282967a1d12d626277f148` and
`d68a206bfd31b2e8d56c2ab7bec041779e438f07`. Careful rebase preserved their product patches; intervening main changes
concerned host-upload progress and plan bookkeeping. Changed Codex/Grok and report-slot tests were rerun above.

Two same-session development failures remain privately retained: `e0d33cba-59c2-4a78-80ef-a3164f4dfefa` exposed the
obsolete single-slot Codex expectation; `b2fac904-728c-4312-a440-052eb9841d42` exposed a test comparing the new
conversation's timestamp with the old one. Both assumptions were corrected and affected cases passed in later distinct
runs. Failed commands are not counted as successful validation.

Skipped the workspace runtime battery, browser, desktop, installer, provisioning and hosted CI runs: the affected
behavior has targeted supervisor, native-session and real-vendor evidence. No UI or installer behavior changed. No
uncovered material risk identified in this diff required broader testing.

## Review gate outcome

Each code PR passed the required fresh-context gpt-6.1-sol high general review; no swarm was used. The final reviewer
received the full test-authoring contract, found the wrong-runtime Stop test's missing premise/admission wait and a
stale website statement, and verified both corrections. The timestamp test correction was also reviewed. No findings
remain. A separate documentation pass covered every touched file. The final valid blind wording reader understood the
performance motivation and breaking caveat without contradictions or convention violations. Exact native model telemetry
and token usage were not exposed; requested routing and evidence gaps are retained privately.
