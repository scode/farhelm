# Menu arrows follow settled focus

## What this was about

The session, host, app bar and templates menus remembered the last arrow-key destination even after that keyboard move
finished. If a pointer or screen reader then moved focus to another item, the next arrow could start from the old
destination. In the session menu, that could unexpectedly focus Delete; deleting still required Enter or Space.

The maintainer accepted a narrower fix after the first executor found that independent focus during pending keyboard
moves cannot always be distinguished from a delayed keyboard landing using the existing focus events. This plan forgets
the remembered destination when that destination receives focus. Subsequent independent focus becomes the next arrow's
origin, while stale keyboard landings preserve advancement during rapid arrow presses. All four menus share the rule.

## Things you should know

Independent focus can still lose precedence while keyboard moves are pending. For example, after two rapid Down presses,
moving focus independently to the fourth item before those requests land can make the next Down request the fourth
again, rather than the fifth. The maintainer explicitly accepted this window for this plan.

A request to an item that already has focus can produce no focus-in event, such as Home on the first item or navigation
in a one-item menu. In that case the remembered destination can remain until a later actual focus landing. The plan
allowed retaining this residue; it is documented in the code. No broader focus-source mechanism or queue redesign was
introduced. The feedback item was removed and its triage execution record now names this PR.

## Open questions and possible follow-ups

The broader design for independent focus during pending keyboard moves is already tracked separately as a near-term
TODO. It remains outside this fix; no further decision is needed to deliver the accepted narrower behavior.

## The PRs

- [#2176: honor menu focus after arrow navigation settles](https://github.com/scode/farhelm/pull/2176/changes) — draft,
  based on main; delivered head `7c4e021ef5afe2f529e36f0558c979c3192e9ed3`.

## Checks run, reused and skipped

Run during this execution, before the final rebase:

- `cargo nextest run -p farhelm-ui --lib -E 'test(menu_panel::)'` through the recorder, run
  `d6265463-62c1-4ca2-b616-8a920c182f2e`: all 20 selected tests passed, with 486 tests deselected, four slots and zero
  retries, nextest 0.9.148, tmux mode none because these are pure unit tests. The three new regressions cover a stale
  landing, acknowledgement of the latest request, and independent focus after acknowledgement. Selected output was
  inspected; no selected test reported a missing runtime substrate.
- The recorded Playwright subset in `sidebar.spec.ts`, `templates.spec.ts`, `feedback.spec.ts` and
  `terminal-multihost.spec.ts`, run `cf7036ff-d842-4fe4-8ad6-725f209eac06`: 34 passed across Chromium and WebKit, zero
  failures or skips, one worker and zero retries, using pinned tmux 3.7c. The child command was `npx playwright test`
  with those four file selectors and the exact grep
  `actions menu|opening the actions menu|reopened busy menu|confirm prompt ignores|mark-seen item|number (keys|popup)|help menu sits|host-menu-keyboard-contract|automatic-setup-menu-order|host-menu-busy-guards`.
  This exercises existing menu keyboard contracts and integration; it is not an end-to-end proof of screen-reader focus
  provenance. The new settled-focus decision is covered by the pure regressions.
- `cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, `cargo build`, and the
  web release build `dx build --package farhelm-ui --platform web --release` passed. These cover both renderer
  compilation and provide the browser harness's matching builds.
- `cargo fmt --all -- --check`, the isolated `python -B scripts/check-test-sleeps.py` (302 files, zero violations),
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and targeted `dprint check`
  passed.

The runtime recorder identified Git HEAD `263e4c8cbc41cf68552925fe23cffa378af5e405` plus the dirty menu patch, rather
than a clean committed fix. Its complete source fingerprints are
`8bb8298dd4d7ea7202526be8a5f0813ab821614c23e910c1f5662ecd83175374` for the unit run and
`486c41fd9ec896e63344fbb4901847d315a1100e9e7c195f75c59cc62db4cae7` for the browser run. The latter includes the feedback
and ledger bookkeeping added in between; the product code did not change.

Reused after rebasing onto main `e6724d81deffd3fb08743a5ba65fc192d5c48a9c`: each runtime, build and static result above.
The five changed product files and changelog fragment are byte-identical to reviewed revision
`cbc750c90c05bd8bf89da5bb18886ca3c7d548aa`. Upstream changes concerned the separate trash confirmation dialog, helm
retry approvals and their reports; their complete diffs do not change menu handlers, focus ordering or keyboard
contracts. The only rebase conflicts were adjacent feedback-index removals, resolved by keeping both removals. Targeted
dprint, the agent-instruction checker and Git whitespace checking passed again after that resolution.

Skipped: another runtime run after the mechanical rebase, the full browser/workspace suites, desktop runtime, installer
and provisioning checks. The inspected upstream changes introduce no menu interaction, the selected tests cover the
affected integration paths, and the other subsystems were unchanged. No CI run was requested or used.

## The review gate's outcome

The required fresh-context gpt-6.1-sol source review at high effort passed with no findings. It inspected the shared
focus queue, all four menu users, the keyboard specification, renderer focus implementations and the full test-authoring
checklist. It confirmed the agreed settled-focus scope and both disclosed limitations. That was source inspection; the
executor independently checked the runtime results above. A first source-review launch received an empty task because of
a private prompt-file path error; it performed no review and was not counted as a pass. The corrected fresh launch
supplied the complete task and passed.

The fresh wording reader passed after replacing vague rapid-arrow wording with a concrete explanation. Implementation
stayed local; only the required reviews were delegated. Private accounting retains external per-turn counters, with
exact model attribution from those counters, native-review usage and executor usage unavailable.

### Landing

Landed on its own in round 73, as #2176, unchanged from what the report describes.

#### What landed since the PR was based, and whether it interacted

Nothing outside the plan queue reached main between this PR's base and its landing, so the landing rebase was clean and
the PR's files are byte-identical to the ones the executor tested and reviewed. The plans landed earlier on 2026-10-16
were already in its base: the trash confirmation fix, a set of fixes to the new-session dialog, and the helm retry
change. I read the PR against them, and a fresh-context reviewer did the same independently. None of them touches menu
focus, and neither review found an interaction.

The reviewer also confirmed that all four menus (session row, host row, app bar, templates) now go through the new focus
rule, with no menu left on the old one. Fast arrow presses stay protected, because only the latest pending arrow
destination is remembered, and the fix forgets it only once focus has landed there. The fix matches the narrower one the
maintainer accepted and claims no more: the code, the commit message and the changelog entry all scope it to after
keyboard navigation has settled.

#### Fixes made during the landing

None.

#### Something the report states that is not yet true

The report says the broader design, for focus moved while arrow presses are still landing, "is already tracked
separately as a near-term TODO". The maintainer asked for that entry when answering this plan's question, and it was
written, but at this landing it is still a draft PR, [#2175](https://github.com/scode/farhelm/pull/2175/changes), which
adds only that TODO entry and lands once the maintainer approves it. Until then the remaining case is recorded in the
plan's Decisions, the code comments and this report, since this PR removed the review feedback item.

#### Checks

- Run now: none. Main gained only plan-queue bookkeeping since the base.
- Reused: the menu unit run `d6265463-62c1-4ca2-b616-8a920c182f2e` and the browser run of the menu keyboard cases on
  Chromium and WebKit `cf7036ff-d842-4fe4-8ad6-725f209eac06` (34 passed), plus the executor's clippy, desktop compile,
  builds, formatting, test-sleep, changelog, agent-instruction and Markdown checks. The runtime runs were recorded
  before the executor rebased onto the trash confirmation and helm retry changes. The menu files have not changed since,
  and neither review found that those changes touch menu focus, so the runs still cover this change.
- Skipped: everything the report skipped (the full browser and workspace suites, desktop runtime, installer and
  provisioning checks, CI), for the reasons it gives.

Apart from the TODO statement above, nothing in the executor's report became untrue at landing.
