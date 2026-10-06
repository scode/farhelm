# Resolve stale session notifications

## What this was about

A session could keep a red, unread warning after Restart could resume its conversation again. This happened when an
agent finally told Farhelm which conversation it was in, or when Codex or Grok's missing conversation record returned.
The approved behavior keeps that history, visibly marks it resolved and counts it as read.

The warning now stays in the list, greyed and marked “resolved”, without making the bell loud or appearing as new. If
the same problem returns before the agent is relaunched, the existing warning moves to the top as new and unread, even
after it was cleared. Repeated checks while the problem persists do not repeat it. The matching TODO entry is removed.

## Things you should know

Only those two recoverable warnings resolve, and only for the current launch. Warnings that Farhelm could not set up
conversation reporting, or that an agent is using an older or changed conversation-reporting integration, keep their
existing behavior. Warnings from earlier launches also stay as they were. Resolution also works for warnings stored
before this upgrade and recovery across a supervisor restart.

The supervisor's stored data upgrades to schema 28. As accepted during planning, an older supervisor refuses that
upgraded database after a downgrade, including the supervisor managed by an older desktop app. The forward upgrade
preserves existing history. There is no protocol-version change: an older helm or UI displays resolved entries as
ordinary warnings, and an older supervisor supplies no resolution flags to a newer helm.

Read and clear marks continue to work as before. Resolution does not move the shared read mark, and a returning warning
gets a newer sequence so previous read or clear marks cannot hide it. The change remains one stored warning per kind per
launch; reopening adds no row and does not trim retained history.

One cohesive draft PR carries storage, transport, display and documentation together instead of the outline's suggested
two. Keeping them together makes the new state visible and keeps the accepted downgrade consequence with its product
behavior. The website's session-list page is updated; no preview server or website deployment was started.

## Open questions and possible follow-ups

No decision is needed to complete this plan. The separate TODO about continuing the silent-agent check after a
supervisor restart remains: this change resolves an already-recorded warning, and does not add that missing check.

The supervisor checks for recovery every two seconds and before replying to a session listing. If a resumable session
still has an earlier launch's warning or an integration warning that cannot resolve this way, those checks continue to
attempt database updates that change nothing. This preserves the agreed simpler design; the extra work is documented,
but its performance impact has not been measured. A cache to avoid these repeated attempts remains a possible follow-up
if measurement shows a need.

## PRs

[#1690 — resolve recovered session warnings](https://github.com/scode/farhelm/pull/1690/changes), one draft PR based on
main.

## Checks run, reused and skipped

Run on the final combined revision based on `4a548d88`:

- `cargo build` and `dx build --package farhelm-ui --platform web --release`: passed, providing matching CLI and browser
  assets after the careful rebase.
- Recorder `3c3cd956-fa4b-4d49-9cb1-8ba2fac2ec80`, `npx playwright test 'notifications\.spec\.ts'`: eight executed
  passes, four on Chromium and four on WebKit, one worker and zero retries. Complete reports show no failures, skipped
  or unstarted cases. This covers resolution at an unchanged sequence, quiet display, recurrence after clear, bell
  placement, read marking and clearing.
- `cargo fmt --all -- --check`, `dprint check SPEC.md SPEC_impl.md TODO.md`, `python -B scripts/check-test-sleeps.py` in
  the isolated pinned parser environment, and `python3 releasing/check-changelog.py format`: passed. The delay checker
  found 269 explained delays and zero without a rationale; seven changelog fragments were valid. These inspect the
  changed source, docs, tests and fragment.

Reused focused Rust evidence from the working tree based on `b9128921`:

- Recorder `a7649a35-da89-41d7-827b-7d32c719a4e0`,
  `cargo nextest run -p farhelm-supervisor -p farhelm-proto --lib -E 'test(notification_resolution) | test(notification_snapshots) | test(a_late_report_resolves) | test(grok_prepublication) | test(codex_record_restoration) | test(session_notifications_cap) | (test(store::tests::) & test(migration))'`:
  17 selected passes. Covers stored history, migration, stale-resolution guard, recurrence without trimming, wire
  compatibility, snapshot order, late reporting live and across supervisor restart, and restored Codex/Grok records.
- Recorder `4b92d73c-e230-4937-a2db-38ac5ed2da69`,
  `cargo nextest run -p farhelm-ui -p farhelm-helm --lib -E 'test(http_contract) | test(resolved_notifications) | test(the_bell_name)'`:
  six selected passes. Covers helm/UI wire fixtures, the bell's accessible name and resolved-row rendering.
- Both retained nextest reports are complete, with pinned nextest and tmux, four global slots and zero retries. Retained
  output contained no runtime substrate skips. Filtered-out tests are not claimed as coverage. An earlier server run
  passed the same 17 selections, but edits overlapped compilation; the final server run above replaces it as evidence
  for those bodies.
- `cargo clippy -p farhelm-supervisor -p farhelm-proto -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings`: passed for the changed crates and shipped binary configuration.
- Website frozen dependency install and `bun run build`: passed, 31 pages and valid internal links. Later wording
  corrections changed no links, frontmatter or generated structure.

The careful rebase read all upstream template-dialog, launcher inference, CSS, specification, documentation and queue
changes through `4a548d88`. They use distinct controls, functions and spec sections and change no notification
lifecycle, storage, protocol or read-mark contract. The rebase was clean; later corrections were prose, comments and a
test name. The focused Rust and lint evidence therefore still applies. Matching final browser builds cover the combined
revision. The subsequent main change through `342f04ad` is only feedback delivery bookkeeping and its report, with no
product interaction.

Skipped the full Rust and browser suites, desktop compilation/runtime smoke, installer and release checks: the selected
storage/capture/wire/UI tests and both browser engines cover the changed contracts, and the change adds no desktop-only,
installer or release-automation behavior. No hosted CI or website deployment was requested.

## Review gate outcome

One fresh-context GPT-6 Astra high review found no findings. One fresh-context Claude Opus 5.5 high review found no
correctness bugs and identified stale code comments, a misleading test name and reporter jargon in the docs; those were
corrected. Its performance observation is now documented. Proposed extra caching, a new status helper and dynamic SQL
were declined because the agreed static guarded design meets the requirements without those additions. No review swarm
ran, and no unresolved behavior or test findings remain.

The executor implemented and validated the change. Only the required source reviews, resume check, wording cold reads
and report cold read were delegated. Reviewers did not reproduce runtime results. Private orchestration evidence is
retained; native reviewer and orchestrator usage counters were unavailable.

### Landing

Landed on 2026-10-06 (UTC) as #1690 (recovered session warnings are marked resolved), one squash commit on main.

#### What else was on main

Between the commit the change was built on and the landing, the remember-feedback-contact plan landed (#1689, the
feedback dialog remembering its contact). The two edit different sections of SPEC.md and SPEC_impl.md, remove different
TODO entries and share no code, and the rebase applied without conflict. Their database changes are in different
databases: #1689 takes the helm's to version 43, this change takes the supervisor's to version 28. Otherwise main gained
only the planning queue's own bookkeeping.

#### Review before merging

A separate reviewer that had not worked on the plan checked the change against #1689 and the rest of main before it
merged and found nothing that breaks. It confirmed:

- The helm's read and cleared marks work on sequence numbers only. A warning that comes back gets a sequence above any
  earlier mark, so an earlier read or clear cannot hide it; resolving keeps the sequence and moves no mark.
- `farhelm agent sessions` never shows notifications, so it is unaffected; the README image's staging passes the new
  field through unchanged; and no other browser test reads notifications.
- The compatibility claims hold: the new flag is left out when false, so an unresolved warning looks exactly as before
  to an older helm or app, and no protocol version change is needed.
- The repeated checks the report mentions are bounded and harmless. Only a session whose Restart can resume and that
  still has an unresolved warning is checked, at a cost of two database updates that match nothing (no write to disk),
  every two seconds and before each session listing, until the session is relaunched or the warning drops out.

#### Checks

- Run now, on the change after the rebase: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
  `dprint check` on SPEC.md, SPEC_impl.md and TODO.md, all clean.
- Reused: the report's checks, including its browser run. The rebase brought in only #1689, which shares no code with
  this change.
- Skipped: running the Rust and browser tests again, for the same reason.

Nothing in the report above was made untrue by the landing.
