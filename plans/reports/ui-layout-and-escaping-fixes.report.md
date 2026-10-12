## What this was about

Twelve accepted review findings concerned an open session page retaining stale information, taking control without a
request after a helm update, losing a newer read-state choice, hiding meaningful title characters, and making controls
unreachable or misleading. The maintainer requested contained fixes in one draft PR, with explicit fallbacks if any item
needed a larger mechanism. All twelve were implemented; no complexity gate tripped and no recorded decision was changed.

## Things you should know

After Restart or Replace from the session header, the sidebar now requests fresh information even while the page asks
you to reload for a newer helm. Newly discovered terminals in that outdated page wait for an explicit take-control
action. Restart itself remains an explicit attachment request for the agent terminal; it does not grant the same intent
to a newly discovered tab. Existing attachment readiness behavior remains intact.

A read or unread choice made while an earlier save is outstanding is sent again before reporting completion, including
an identical choice. Another window can change the stored state between those requests, so matching values alone are not
evidence that the newer choice was saved.

Rename exposes hidden characters in the editable title. Leaving the field untouched preserves the original bytes;
editing submits exactly the edited text. Saving a template now refuses hidden characters in its session name, using the
same rule already applied to template commands. This also refuses joiners used in some emoji. Existing templates still
launch, but saving another change requires fixing the affected value. The specification, template guide and changelog
fragment state that restriction. The session launcher's existing untouched-name preservation stays intact.

An unreachable host's menu stays reachable at the supported 240px sidebar width. An ended session's header keeps all six
controls inside a 650px main area. Long status wording can shrink with an ellipsis; its full text remains available in a
tooltip. Settings waits while any host's uninstall confirmation is being prepared, because opening Settings for a
different host could otherwise produce the same two-dialog lockout.

The first failed session detail refresh stays silent. Two consecutive accepted failures show “could not refresh this
session; showing the last state this client read”; the next accepted answer clears it. Late failures from an older
Restart cannot warn over the newer state. Tabs and their live terminals survive failed detail reads and recovery.

An approvals-only template offers every declared approval choice without requiring an agent type. Launch still checks
that the eventual agent supports the selected choice. Template Delete refuses while the saved list is reloading, so Undo
cannot restore its pre-save version. Refused host permission changes directly restore both switches to their stored
values.

## Open questions and possible follow-ups

No maintainer decision is required. The permission-switch browser test proves the refusal outcome, not the particular
schedule where the UI never renders an intermediate busy state. The direct reset for that schedule was checked in the
independent source review; adding scheduling machinery or a new test seam was explicitly outside the agreed scope.
WebKit browser execution covers the desktop renderer's engine family, not the native desktop shell.

## PRs

- [#1932 — keep session controls current and edits faithful](https://github.com/scode/farhelm/pull/1932/changes), draft;
  one change `lzvmtmzv`, bookmark `plan/ui-layout-and-escaping-fixes/01-ui-fixes`.

## Checks run, reused and skipped

- Run after the source-review corrections: `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, and `cargo fmt --all -- --check`; passed. These cover the changed
  shared and desktop renderer code. The fresh dx web build also passed.
- Run: UI asset `node --test`, 238 passes, recorder `0c92894b-48e8-4fbd-a1a8-8a0f68613b12`. This is the final JavaScript
  revision after the attachment-intent corrections.
- Run: focused Chromium and WebKit cases through the recorder, one worker and zero retries. Header Restart/Replace under
  build skew passed both engines in `22447639-f51e-4d0c-a7dd-3303e137ff4f`. Discovered-tab intent, Rename and permission
  refusal passed both engines in `b8e97dc3-3da9-49fd-9550-97a60afa2362`; that run also retained the sidebar fixture
  failure described below. The corrected minimum-sidebar case passed both engines in
  `c346d027-1f8b-4e1a-a3fb-42957fc11646`.
- Run: eighteen cases passed in `8b209bb3-999b-485c-b8e9-f5a262e953a5`: minimum-width ended header, same-host and
  other-host Settings refusal during uninstall planning, all approvals-only choices, Save/Delete/Undo reload boundary,
  the existing template editor lifecycle, displaced-view tab discovery and take control, agent-only Restart remount,
  pending-tab mount cancellation, and repeated detail failures/recovery, on both engines.
- Run: eight existing Settings permissions, dialog lifecycle and remote uninstall cases passed on both engines in
  `3f8614bb-d3c7-4ed5-9ab7-da95a04031af`. The owned sandbox supplied a real SSH fleet; no cases skipped for missing SSH.
  No host account or live Farhelm installation was modified.
- Reused: seven focused Rust cases in `a7861088-274f-4bda-ab31-e725f83d4867`, covering seen-write settlement, partial
  approval options, template session-name save refusal and legacy application, and untouched raw-field seeds. Their pure
  contracts were unchanged by the subsequent attachment and Settings corrections. The earlier CLI build remains
  applicable because those corrections change UI code and assets only.
- Run: source-only sleep checker, 289 delays and zero missing rationales; changed Markdown formatting, changelog format
  lint, whitespace check, and exact bookkeeping verification. Only the twelve intended Execution fields changed; their
  feedback files and complete wrapped index entries were removed. The private bookkeeping helper passed Deno formatting,
  type checks and its ledger-drift and wrapped-item tests. Website `bun install --frozen-lockfile` and `bun run build`
  passed: 32 pages built and every internal link validated; dependencies unchanged.
- Reused after careful rebase onto `66dc1a19`: upstream helm and supervisor edits affect tests only; test-state cleanup
  retains socket directories after unconfirmed shutdown and does not alter this plan's live owned stack. Other changes
  record queue states, reports and disjoint triage outcomes. The index conflict was resolved by preserving both plans'
  removals. The shipped UI, specification and browser tests were unchanged upstream at this rebase.
- Skipped: the full Rust and browser suites, desktop runtime, installer, release and cross-distribution setup gates. The
  focused checks cover the changed behavior; this plan changes neither the native shell nor those installation and
  release paths. No new reason to repeat the broad successful compiler checks was identified.

Three failed browser observations are retained privately. `4c227291-7fa9-4e7d-a384-f4cdc6198499` could not find the
browser cache because the recorder removes its environment override. `bf6925d9-92e4-4597-8d09-a6f2e03a9e71` passed
Chromium but could not launch WebKit until its runtime libraries were installed in the disposable sandbox. Neither is
product evidence. In `b8e97dc3-3da9-49fd-9550-97a60afa2362`, the sidebar test used Home, which the resize handle does
not support. The fixture now reaches 240px through supported arrow keys; its exact retry passed. These same-session
setup and fixture mistakes do not establish a latent flake and did not produce a FLAKES entry.

## Review gate outcome

The required fresh gpt-6.1-sol high source reviewer found three contained corrections: cross-host Settings needed the
global planning guard, withheld attachment needed a working explicit take-control action, and Restart needed to retain
its agent-only attachment intent under build skew. All three were fixed and the same reviewer accepted the follow-up
source. The reviewer also corrected an overclaim in the permission test; the report preserves that evidence limit. The
required both-engine geometry and lifecycle checks subsequently passed. A separate documentation pass inspected every
touched code and test file for the new intent, lifetime and failure contracts.

A second fresh gpt-6.1-sol medium wording reader accepted the revised commit and PR text after the first reader found an
ambiguous description of the template restriction. Implementation and validation stayed with the executor; no
decomposition work was delegated. The required fresh native report reader accepted this account as sufficient for a
maintainer decision and found no public-repository hygiene violation; the executor read the complete findings.

Private state is in `farhelm-plan-ui-layout-and-escaping-fixes-log.md` beside the checkouts. Galaxy-brain evidence
reuses session `9317ea96-7a9d-4c8c-bdc5-c8d8cf7fc943`, referenced there. The native tools expose no independently
reported model attribution, effort or token usage; these remain evidence gaps and support no cost claim.

### Landing

Landed on 2026-10-12 (UTC) as #1932. The rebase met the review queue index, where the plans that landed before it had
removed adjacent entries; all removals were kept and the index still matches its files.

#### Fixes made while landing

All in #1932, before it merged:

- The header's status badge may now shrink in a crowded header, but the live statuses (running, waiting, idle) draw only
  an 8px dot, which shrinking would clip. Those three keep their size; only the worded statuses give up width.
- Rename seeded its field with the display spelling, which turns an empty or whitespace-only title into the phrase
  "(empty)" or "(whitespace only: N characters)". Submitting untouched was still faithful, but typing "foo" into it
  renamed the session to "(empty)foo". The field now starts with an escape-only spelling, which leaves empty and blank
  titles as they are; a unit test covers it.
- An existing browser test, "rename exposes peer title and refusal characters while retaining the raw draft", expected
  the rename field to start with the title's raw characters, which this plan's agreed outcome deliberately changed: the
  field now starts escaped, so a title an agent set cannot hide characters from the person editing it. It failed on both
  engines (run `0125ad28`); the plan had not run that spec. The test now expects the escaped seed and is renamed to say
  so; the stored title still keeps its exact characters.

#### Review before merging

No blockers. Header Replace and Restart refresh the sidebar through its existing read gate, so no older reply can
overwrite a newer one; the template session-name rule holds at every save path while stored templates still launch; and
the terminal changes stay inside the mount decision, leaving the link guards, steady path hover and snapshot copy alone.
Notes left as they are:

- Settings is now disabled on every host while any host has planning in flight, including a one-click update's whole
  run, which is wider than the agreed "while an uninstall plan is pending".
- Restart now always takes the session's terminal, even on an up-to-date helm; this is the user's own request, and it
  only matters in a race with another window's automatic recovery.
- While the reload notice is showing, a tab you open yourself waits for "take control", which then reattaches every
  terminal in the view.
- The changelog fragment is `fixed`, though newly refusing hidden characters in a template's session name is arguably
  `changed`; and the launch templates page does not yet say every approval choice is now offered without an agent type.

#### Checks

This plan was landed after its own checks in a separate workspace, while browser-oracle-and-fixture-fixes and the next
product plans were checked and landed in the main checkout; supervisor-and-test-followups and helm-supervisor-small-
fixes landed on main in between and touch only the supervisor, helm and test stack, which this plan does not.

- Run now, after the first two fixes: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, the test-delay
  checker, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop`; the UI, protocol and helm unit tests through the recorder (run
  `1a63c0f2`, 1639 of 1639); the UI JavaScript tests (238 of 238); the docs website build; and on Chromium and WebKit
  with one worker and no retries the header, provisioning, templates, terminal-tabs, ui-plan-regressions, terminal-links
  and terminal-replay-rename specs (run `0125ad28`: 320 passed, the rename test failed as described).
- After the test fix: the terminal-replay-rename spec on both engines (run `c867c880`, 44 passed).

Nothing in the report above was made untrue by the landing.
