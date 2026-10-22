# Popovers close when their anchors may have moved

## What this was about

Sidebar menus are placed from the button's position when they open. A later layout change can leave the menu pointing at
empty space or another row. Session actions, notification lists, host menus, app-bar menus and template shortcut menus
had separate copies of the rule for dismissing those stale placements. Several row changes never reported movement.

The maintainer chose one causal movement owner and fixes for the missed cases. The owner now receives movement reports
and closes the affected registered popovers. It does not measure positions or reposition menus. Action acceptance,
mutual exclusion, disappearing rows and the existing focus-return behavior keep their separate rules.

## Things you should know

The inventory distinguished movement from accepted actions, switching between popovers and withdrawing their source.
Existing scroll, resize, compact-mode, disclosure, row order and removal closes retain their original timing, including
unconditional state writes where those writes could schedule effects. App-bar and template menus retain their existing
layout-epoch admission rules.

The three recorded gaps are covered: changing visible detail text above a session, carrying a refusal line across it,
and reordering or changing the visible contributions of host rows. Other fixes cover session headers, notification-age
and trash-count widths, template marks and shortcut labels, update status, and expanded host progress, diagnostics and
manual instructions. Host-derived session details use the same accepted host snapshot as host-row comparisons.

An accepted listing is compared as one update. Identical displayed contributions exchanging places wholly above an
unchanged anchor can cancel; separate asynchronous host-progress changes remain separate causes. Comparisons use the
actual held display order, rather than the server's order while the pointer or an open menu holds rows in place.

New deferred closes capture the measured opening before rendering and check that same opening after publication.
Reopening a menu, losing its row, a failed listing, recovery or component retirement cannot give an old close authority
over a new opening. Existing notification read-on-close behavior was preserved.

The new rules remain causal: changed visible content that could move an anchor can dismiss a menu even when that
particular width happens to leave its rectangle unchanged. Equal displayed output, equal known character widths and
unrelated hidden metadata do not acquire blanket closes. Browser tests use geometry only to establish test premises.

The final inventory found another missed cause: pinning a session can wrap its title while displayed order stays held,
and entering or leaving pending deletion can move later rows even in Compact mode. These now report movement for later
session anchors. Changing an ordinary unpinned title, pinning in Compact mode, changing hidden text, or changing an age
without changing its column count remains inert. Pinned title and activity-width inputs are conservative: browser font
defaults affect the wrapping threshold, so a change can close a lower popup even when one particular layout stays put.

Review caught a pending-close loss: a change above an open menu followed by an unrelated change below it could erase the
first close before its effect ran. Comparable identity renders now retain the affected openings as one bounded set;
failed-listing and recovery transitions clear it. Older callers keep their original replacement behavior. Regression
tests cover two committed changes, reopening, stale bindings, recovery and duplicate admission. The final inventory
found no further uncovered movement from the original candidate list. The plan's TODO entry is removed in the last PR.

## Open questions and possible follow-ups

Two existing notification-focus items remain queued. Switching from an old popup can let its automatic focus return
interrupt a newly opened notification list and potentially mark entries read. Separately, clearing notifications from
another window can remove the focused bell component before it returns focus. These concern focus ownership and
component teardown; moving the layout rule did not resolve them. Their browser event ordering remains unverified here.

WebKit browser coverage exercises the web UI in the desktop renderer's engine family. It does not prove native webview
acknowledgement timing or desktop pixels. The update provider is unavailable in the browser build; its presentation and
movement path has actual hook tests and desktop compilation, without a fake provider or a real update installation. Host
provisioning browser fixtures simulate remote setup outcomes; they do not install onto a remote host.

## The PRs

All ten are open drafts in one linear stack; delivery does not merge them.

- [#2540](https://github.com/scode/farhelm/pull/2540/changes): session movement ownership and existing close timing.
- [#2542](https://github.com/scode/farhelm/pull/2542/changes): host, app-bar and template registrations.
- [#2544](https://github.com/scode/farhelm/pull/2544/changes): visible session-detail and refusal movement.
- [#2545](https://github.com/scode/farhelm/pull/2545/changes): host order and visible host-detail movement.
- [#2547](https://github.com/scode/farhelm/pull/2547/changes): session count and delete-outcome headers.
- [#2548](https://github.com/scode/farhelm/pull/2548/changes): notification-age and trash-count widths.
- [#2550](https://github.com/scode/farhelm/pull/2550/changes): displaced template shortcut menus.
- [#2552](https://github.com/scode/farhelm/pull/2552/changes): visible update-status movement.
- [#2555](https://github.com/scode/farhelm/pull/2555/changes): expanded host-body and diagnostic movement.
- [#2556](https://github.com/scode/farhelm/pull/2556/changes): session title-line and pending-delete movement.

## Checks run, reused and skipped

All runtime commands went through `scripts/record-test-run.py`. Rust used pinned nextest 0.9.148, four global slots,
zero retries and failure on empty selection. Browser runs used Playwright 1.64.0, Chromium and WebKit, a three-worker
budget with one stack per worker, zero retries, one execution per case, forbid-only and fail-on-flaky. Pinned tmux 3.7c
matched the required binary. Retained complete output and runner reports were read; the successful selections below had
no runtime skips, unstarted cases or flaky results. Counts overlap across selections and must not be added as a count of
unique tests.

For the Rust table, the command is `cargo nextest run -p farhelm-ui --lib -E '<filter>'`. Each row records the actual
filter, recorder run, passed count and tested source identity. Source identities are Git HEAD plus dirty-tree recorder
fingerprint prefixes; they identify the tested work rather than pretending that HEAD alone contained it.

| PR                              | Filter                                                                                                                                    | Run / passed                                 | HEAD + fingerprint prefix       |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | ------------------------------- |
| 2540                            | `test(anchored_popovers::) \| test(rows::)`                                                                                               | `71282cf3-8fb3-4bab-b975-c12b6cd0a9f6` / 31  | `0fb0699a` + `ed94f804c01fc25b` |
| 2542                            | `test(anchored_popovers::) \| test(app_bar::) \| test(list::templates::)`                                                                 | `6716e63c-4227-4693-94be-041149b21c1d` / 27  | `50de49e4` + `c2074459f5d898d7` |
| 2544                            | `test(anchored_popovers::) \| test(rows::)`                                                                                               | `13a61342-5c90-4702-8091-135020cf1992` / 39  | `1ab37ff2` + `60f071d06ab1211c` |
| 2545                            | `test(anchored_popovers::) \| test(hosts::)`                                                                                              | `0bc4b7db-9276-4a5c-9fbd-1e345a741e99` / 59  | `20bf0564` + `c520d867744e772c` |
| 2547                            | `test(anchored_popovers::) \| test(rows::) \| test(list::row::) \| test(delete_notice_reports_actual_display_changes_before_publication)` | `36fc6d12-1fbb-4255-bb98-2e9f9ad546a5` / 67  | `abc4a89c` + `5def74f9534c13f8` |
| 2548                            | `test(anchored_popovers::) \| test(list::bell::) \| test(list::trash::)`                                                                  | `10f4ebb7-768f-4bc8-8ae4-319cc7250e63` / 27  | `8fb336a5` + `31af11f1069e028e` |
| 2550                            | `test(anchored_popovers::) \| test(list::templates::)`                                                                                    | `1d438078-5c95-47c0-ae03-cecaef282a8e` / 43  | `a7235248` + `cacc63d398aac6bf` |
| 2552                            | `test(anchored_popovers::) \| test(app_bar::) \| test(app_updater::)`                                                                     | `8e388330-9942-4c4d-9b12-9f5486f74399` / 35  | `1717420f` + `599819d578cf2f0d` |
| 2555                            | `test(anchored_popovers::) \| test(hosts::) \| test(provisioning::) \| test(peer::)`                                                      | `fd3fc646-bd15-4988-866e-86b21b686058` / 124 | `b7beece5` + `3377fc8920f93f89` |
| 2555                            | `test(=provisioning::tests::provisioning_producer_reports_diagnostic_manual_pending_and_plan_changes)`                                    | `33f3ddae-8ac2-4812-9d9f-3895e1c2f3cf` / 1   | `b7beece5` + `88e30b87c71474a7` |
| 2556, before pending correction | `test(anchored_popovers::)\|test(list::row::)\|test(list::view::)\|test(rows::)`                                                          | `c8b5dac6-772b-473c-8324-1cf3f3adc345` / 95  | `763f4ea9` + `51daa593316c3b1a` |
| 2556, final                     | `test(anchored_popovers::)\|test(list::view::tests::identity_)`                                                                           | `8e2ad356-c03a-44ab-962e-99cb211d3fd2` / 32  | `763f4ea9` + `1e7f84b297ba89bb` |

The last run supersedes the earlier owner/pending coverage. The unchanged row, markup and row-prefix tests from the
95-test run remain applicable; that earlier run did not detect the pending-close bug. Before the final selection, its
exact regression also passed alone in `2c3893df-39e9-4a8d-8f38-e2a916420af3` on the final fingerprint. The corrected
PeerBlock application-scope regression passed alone in `abd1d786-ba07-4bde-97e3-e5b4f023b148`.

Browser child commands below start with `npx playwright test` from `e2e`; backslashes in quoted file selectors are
literal regex escapes. The two broad early selections are specified afterward to keep this table readable.

| PR / selection                        | File selector or selection label                                                                                                                                       | Run / passed                                                 | HEAD + fingerprint prefix       |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------- |
| 2540                                  | Session adapter selection A                                                                                                                                            | `13402514-3cc1-45a5-bd86-2d239441f33f` / 32                  | `0fb0699a` + `ed94f804c01fc25b` |
| 2542                                  | Other adapter selection B                                                                                                                                              | `bad93c3f-9134-440e-bef0-74871e58f644` / 22                  | `50de49e4` + `c2074459f5d898d7` |
| 2544                                  | `'popover-movement\.spec\.ts'`                                                                                                                                         | `e0dc98d6-141b-4aaa-9e7f-9b4a5b8f48f0` / 32                  | `1ab37ff2` + `60f071d06ab1211c` |
| 2544, host-only extension             | `'popover-movement\.spec\.ts' -g 'a host-only stale cause rewrap'`                                                                                                     | `b7e7d506-458d-4828-a701-b3869aad7de0` / 4                   | `1ab37ff2` + `45cdebdca63d22a9` |
| 2545                                  | `'popover-host-movement\.spec\.ts\|popover-movement\.spec\.ts' -g 'host reorder\|host permutation\|balanced detail\|same detail contributions\|host-only stale cause'` | `89049b6c-ebb2-4913-83c2-fc1f09586fe1` / 20                  | `20bf0564` + `c520d867744e772c` |
| 2547                                  | `'popover-header-movement\.spec\.ts'`                                                                                                                                  | `f12d4d6f-14e2-44ee-b418-2f6451a09c73` / 16                  | `abc4a89c` + `5def74f9534c13f8` |
| 2548                                  | `'popover-width-movement\.spec\.ts'`                                                                                                                                   | `ad0f6293-4439-4096-96f6-1b13fcab374f` / 16                  | `8fb336a5` + `31af11f1069e028e` |
| 2550                                  | `popover-template-movement.spec.ts`                                                                                                                                    | `9b71e647-7345-452d-b734-24e621d87dd0` / 12                  | `a7235248` + `cacc63d398aac6bf` |
| 2555, rebased integration selection C | Expanded host, template, width and modal seams                                                                                                                         | `eec71b70-95c7-4fbb-845f-8d54de3d0027` / 42                  | `06d13303` + `9a23ddeb0d61f868` |
| 2556, final                           | `tests/popover-identity-movement.spec.ts`                                                                                                                              | `c22e4226-a67c-48a3-a243-ff3ea2abb19f` / 18, nine per engine | `763f4ea9` + `1e7f84b297ba89bb` |

Selection A used file selector `'popover-movement\.spec\.ts|sidebar\.spec\.ts|notifications\.spec\.ts|sort\.spec\.ts'`
and
`-g 'compact rows move the anchor|a refusal above moves the anchor|a menu dismissed by a layout change|a row moving under a pointer-opened menu|a row above an open menu growing a detail line|scrolling the sidebar closes an open row menu|opening host details closes an open|opening the create-session form closes an open row menu|a listing refresh with nothing layout-relevant|automatic notification dismissal returns only|a local delete closes the notification list on a row it moves|an open menu defers the release without losing it|a held delete that moves the open menu'`.

Selection B used file selector
`'sidebar\.spec\.ts|terminal-multihost\.spec\.ts|provisioning\.spec\.ts|feedback\.spec\.ts|templates\.spec\.ts'` and
`-g 'opening host details closes an open|add-host-form-closes-a-stray|setup disclosure transitions invalidate|expanded setup failure closes|the help menu sits right of the gear|number popup closes when the dialog moves by|number popup delayed close keeps another'`.

Selection C used files
`tests/popover-expanded-host-movement.spec.ts tests/popover-template-movement.spec.ts tests/popover-width-movement.spec.ts tests/sidebar.spec.ts tests/templates.spec.ts`
and
`-g 'expanded progress|local manual remedy growth|real late settings refusal|unchanged expanded output|number popup below|number popup centered|number popup above|rename (closes a displaced|keeps an unchanged) template number popup|trash count|same-width trash counts|opening the create-session form closes an open row menu|number popup closes when the dialog moves|number popup delayed close'`.
It passed 21 cases per engine: six expanded-host cases and fifteen modal/popover integration cases. The expanded-host
browser positives allow the normal aggregate panel resize to participate; causal isolation comes from actual producer
hook tests. The browser results do not establish coalescence of independent HTTP completions.

Source-only and compilation checks passed at each relevant step: `cargo fmt --all -- --check`, isolated pinned-parser
`python -B scripts/check-test-sleeps.py` (final 314 annotated calls, zero missing reasons),
`python3 releasing/check-changelog.py format` (final 89 fragments),
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and
`cargo clippy -p farhelm-ui --lib --target wasm32-unknown-unknown --features web -- -D warnings`. The final two Clippy
commands and source checks ran on final snapshot `3538840f`; the final web bundle was built with
`dx build --package farhelm-ui --platform web --release` from the UI crate on that same source. A matching
`cargo build -p farhelm` ran after the uninstall backend landed on `a8afdfba` and was reused for the final browser run.

`cargo clippy -p farhelm-ui --lib --features desktop -- -D warnings` passed on steps 1–5, 8–10; the last run preceded
the final pending-set correction, on initial identity snapshot `a54495d2`. Desktop checks for steps 6–7 and another
desktop build after the pending correction were skipped because those deltas use shared renderer-neutral code already
compiled by default and web checks; they add no desktop-specific caller. The final corrected shared implementation was
checked by default all-target Clippy and web Clippy. This is compilation evidence, not a native runtime verdict.
Workspace-wide Clippy was skipped because executable changes are confined to the UI crate; the matching backend build is
not described as a backend test pass.

Reuse is per selection: the original adapter checks cover preserved scroll, resize, listing, local-delete, disclosure,
epoch and focus/read boundaries; later checks add each new producer without changing those legacy close sites. Final
32-test coverage rechecks the shared owner and old replacement semantics after the pending-set correction. The
42-browser integration selection rechecks the landed modal-shell seams, Templates and trash. The final 18 cases cover
actual title wrapping, subpixel delete entry in both densities, activity-track growth and harmless own/earlier/balanced
prefix cases. Reopening and unavailable/recovery are also checked at the actual hook boundary. Other earlier selections
remain evidence for their unchanged producers, rather than a claim that every earlier case was rerun on the final tip.

The retained compile failures were test fixture mistakes fixed during this run: an unqualified Dioxus scope helper, a
mutable signal handle captured by an `Fn` callback, and a resolver closure borrowing its returned data. A later Rust
failure inspected Dioxus's framework root rather than the application's scope; its structural assertions were kept and
the scope corrected. No failed run is counted as a pass or recorded as a latent flake.

The first expanded-host browser experiment failed its isolation premises: two independent HTTP completions did not
guarantee one rendering batch, and WebKit observed an intermediate panel resize. Other failures exposed a mismatched
counterweight, a second inactive editor and an initial-read hold that expired. That experiment is retained. The final
tests distinguish causal hook/owner evidence from browser integration evidence that permits an aggregate resize. An
empty browser selection caused by an anchored bare-title filter ran no tests; the corrected selector was listed before
execution. An initial pin diagnostic discarded its successful in-memory attachment; a separate recorded run retained the
actual measurements. These are separate observations, not retroactive corrections to earlier results.

The stack was integrated with the landed modal shell and uninstall changes. Modal roots, scroll regions and explicit
focus-ownership release before clearing a plan remain intact. Nine review-unit patches were unchanged by the last rebase
onto `a8afdfba`; the uninstall change required a new matching backend build. Main's unrelated uninstall alias acceptance
and its recorded backend follow-ups were not changed or revalidated by this UI plan.

The final base is `73f2bb90`. Its only change after `a8afdfba` adds an unrelated TODO item, which was preserved. All ten
review-unit interdiffs were empty after this final rebase, so each command's retained coverage above was reused without
another runtime run. The delivered top is `e45fa5d6`. No code, spec or test changed in that final integration.

Whole-workspace runtime tests, the complete browser suite, installer/provisioning platforms, real remote setup, native
pixel tests and CI were skipped: they do not add evidence for the identified movement contracts beyond the selected
checks. CI is manually dispatched and was not requested. No live installation was changed.

## The review gate's outcome

Every PR passed exactly two fresh, independent gpt-6-astra reviews at high effort: general adversarial correctness,
design and idioms, and a dedicated check for observable behavior changes. The first two PRs preserve behavior. The
remaining eight are separate `fix:` PRs with changelog fragments; the behavior reviewer accepted their new movement
closes as the maintainer's aligned-change exemption and found no unrelated change. Corrections were rechecked by the
same two agents for their PR. No substitute reviewer or review swarm was used.

The final general review found the pending-close loss described above. It was fixed before publication, and both
reviewers passed the corrected source and browser additions. Fresh own-model scope assessments accepted the bounded
mechanisms where needed, and the final inventory reconciled all original candidates. Commit and PR wording passed
separate cold reads. The two notification focus items remain explicitly outside the accepted fixes.

### Landing

Landed as #2540, #2542, #2544, #2545, #2547, #2548, #2550, #2552, #2555 and #2556, merged bottom-up, with the content
the report describes plus one browser-test update described below, rebased onto this round's main with one TODO.md
conflict resolution. It was the only plan in this landing round.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained launcher-draft-model (12 PRs moving the session launcher's choices into one draft
value with one set of editing rules) and #2557, which records review findings and changes no code. During this round
main also gained documentation only: triage records, review-queue records, plan files and a TODO.md entry (#2558 to
#2568); the stack rebased over them without conflicts. The two plans share no code files. A fresh-context reviewer and I
read this plan against launcher-draft-model and against the recent dialog work already in its base (modal-dialog-shell,
ui-session-actions):

- launcher-draft-model's change to the lock that keeps a second launch from starting while one is in flight only adds a
  read, and this plan adds no new use of that lock.
- The notice shown while a session is being deleted now tells the new owner when it moves the rows below it; every place
  that shows it, including the launcher's rewritten New and Replace with completion, goes through the one path that
  reports.
- The template shortcut menus this plan closes share no state with the launcher's template shortcuts; they only read the
  shortcut bindings to size their labels.
- Opening the launcher still closes open menus, in the same order. The new owner never moves focus, so it cannot disturb
  the order in which a closing dialog makes the page usable again and then returns focus.
- Replace and Delete confirmations that are waiting for an answer survive a menu closing because its row moved, as
  SPEC_impl.md requires.

#### Fixes made during the landing

- The full browser suite and repeats found that "two provisioning-busy rows reconcile independently", an existing host
  setup test that passed on both engines in the previous landing round's full run
  (`c731cfe8-d581-46ff-ae8d-58b90ca2346e`), now failed about one time in three on both engines. It sets up two hosts at
  fake addresses the helm can never reach, holds both setups, then opens each host's menu and checks its Settings item
  while that host's setup is busy and after it completes. With this plan a menu closes when a row above it changes or a
  setup changes status, and the test opened menus at such moments: just after the API reported a completion the page had
  not shown yet, and while the helm's connection retries to the unreachable first host rewrote that row (see the first
  follow-up below). The landing kept what the test proves and changed how it observes: it now checks on the page that
  each setup is running, waits for the page to show the first host's completion, and retries each menu-open-and-check
  for up to 8 seconds. A Settings item in the wrong state still fails every attempt. What the retry gives up is failing
  when a menu closes by itself soon after opening, which is exactly the behavior the first follow-up asks about; if that
  follow-up is fixed, it should come with a test that a menu below an unreachable host stays open. The updated test
  passed ten times out of ten on both engines. The change is in #2545, the PR that started closing host menus when rows
  change. An independent reviewer checked an earlier version against the repository's test-authoring checklist and
  accepted it with changes; the landed version applies those changes except one, turning Details off for this test, and
  was not re-reviewed.
- Its TODO.md removal conflicted with launcher-draft-model's, landed just before it; the landing kept both removals.

#### Possible follow-ups

- Found at landing, a consequence of this plan's rules the report does not mention: with Details shown, a host that is
  connecting or unreachable shows a line describing its current connection attempt and the last failure. That line
  changes with every attempt (the waits between attempts grow from about one second to tens of seconds) and as each
  attempt moves through its steps, and each change closes any open menu on a host row below it. Before this plan, a
  changed line of the same height did not close those menus. The report accepts conservative closes on changed text, but
  a menu that closes on a timer under an unreachable host may be more than intended. Possible follow-up, if not: stop
  treating a rewrite of that connection-attempt line as movement.
- Found at landing, low severity, a gap the plan's inventory missed rather than one it introduced: the sidebar's YOLO
  question for Replace sits directly above the session list, and it appears, disappears and shows or clears its error
  line whenever the helm's answers arrive. Each change shifts every session row, but none is reported to the new owner,
  so menus on those rows stay open. For example, choose Replace on one row, open another row's menu while the request is
  in flight, and when the YOLO question appears that menu is left beside a different row. Possible follow-up: report the
  question's appearance and error line as row movement, the way the deletion notice now does.

#### Checks

The plan adds about 10,500 lines, all in the UI and its browser tests, and changes how menus behave on every session
row, host row and the app bar, while its own runs were per-PR selections; that is why this round ran the full browser
suite. No helm, supervisor or asset JavaScript changed, so the rest of the Rust workspace and the JavaScript harness did
not run. All of the following ran on the stack before the landing's test update, which changes no product code, except
where noted:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, `dprint check` on the
  changed Markdown files, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the
  workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh` (which also builds the desktop
  app): all passed. The isolated test-sleep check passed on the stack as it landed.
- `cargo nextest run -p farhelm-ui` through the test recorder with pinned nextest and tmux, four slots, no retries, run
  `93107480-4749-47e9-8644-2be305108785`: 737 passed. `cargo nextest run -p farhelm-ui --features desktop`, run
  `25b14ca2-1c16-41ee-a737-e2fc0db4e09d`: 898 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `3e6c5c80-5120-4a7f-9b0d-af205ded9491`, retained: 2468 passed, 14 skipped by design
  (WebKit clipboard cases and tests that need a real signed-in agent or run on one engine only), 8 failed. The run took
  2.5 hours against the usual 1.5, which points to heavy machine load. Each failure was on one engine only; three
  repeats of all eight on both engines (run `47725046-13a1-4b60-92e6-361519c6812f`) passed every case except "two
  provisioning-busy rows reconcile independently", which failed once on each engine and is the test the landing updated
  above. After the update it passed ten times out of ten on both engines (run `543f2062-e186-4c06-ab40-f4f298d4db1e`).
  The other seven, all passing three times on both engines in the repeats, were two terminal flood tests, a delete
  refusal test, a launcher test on two hosts, a sidebar flyout stacking test and two host setup menu tests. One of those
  two, "expanded setup failure closes the menu below it", also opens a menu just below an unreachable test host with
  Details on, so the connection-retry closes in the first follow-up may explain it as well as the load. I did not add
  any of the seven to FLAKES.md on one observation.
- Reused: none. Skipped: the rest of the Rust workspace and the JavaScript harness (no helm, supervisor or asset
  JavaScript changed), the website build (no docs page changed), the doctests (no documentation example changed), the
  desktop smoke test (the desktop app crate is unchanged), the native Mac app (not runnable here) and the installer
  (unchanged).

The report's checks were per-PR unit-test and browser selections; this round ran the full browser suite and the full UI
unit tests on the plan as it landed (before the test update, which changes no product code), on top of
launcher-draft-model. The report's statement that its final inventory found no further uncovered movement no longer
holds in full: the second follow-up above is a cause that inventory missed. Nothing else in the executor's report became
untrue at landing.
