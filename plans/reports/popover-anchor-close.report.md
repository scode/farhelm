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
