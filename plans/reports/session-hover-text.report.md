## What this was about

Hovering a session in the sidebar repeated its title or gave different hints depending on where the pointer landed. The
agreed behavior is one useful summary card from the title and unmarked open area in either sidebar density, while status
and other meaningful marks keep their own explanations.

The card now shows the full title, status, agent summary, host with its chosen mark, managed checkout when present, full
directory, start and last-activity times, and the row's open and actions cues. Keyboard focus on the open button shows
the same card. The full launch command remains on the agent mark, and ended-status detail remains on its status mark.

## Things you should know

The card reuses the existing tooltip's timing, placement and dismissal behavior. Its hidden source is rendered by the UI
and copied as DOM nodes, so peer text is escaped and isolated before the tooltip sees it. A stale session's status is
dim and still; a session with unknown locality gets no invented host mark. Old listings that lack a host or time omit
those facts.

The app currently ships only a dark palette. I inspected the normal and compact cards in Chromium and WebKit under that
palette; both were legible. The plan's proposed light-theme inspection could not be performed because there is no light
theme. The docs sentence is on the Session list page; this unattended code plan started no docs preview.

## Open questions and possible follow-ups

No decision is needed for this plan. A future light theme would need its own visual check of the card.

## PRs

- [#2293 — support summary cards in hover help](https://github.com/scode/farhelm/pull/2293/changes): shared tooltip card
  support, draft.
- [#2300 — show sidebar session summaries on hover](https://github.com/scode/farhelm/pull/2300/changes): row card,
  tests, specs, docs and changelog, draft on #2293.

## Checks run, reused and skipped

Run now: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, `dprint check`, the isolated test-delay checker (304 inspected, none
missing a rationale), changelog format, and the website production build with internal links valid. UI test-only
compilation and the pinned web build passed. These inspect the changed Rust and web code, the desktop compile path, the
edited browser tests and prose, and the published docs links. The focused runtime checks below cover the card's
rendering and hover behavior.

Recorded Rust runs used pinned nextest 0.9.148 and tmux 3.7c, four global slots and zero retries.
`cargo nextest run -p farhelm-ui --lib -E 'test(=list::summary_card::tests::card_rows_follow_available_session_facts)'`
passed 1/1 in 88f0d0b7-0407-4cd0-91b1-817766505b48. This was the exact rerun after 714a438f-476c-49d1-b2d7-57a540a7ed19
failed: that first test assertion sought a static Dioxus template label among dynamic text mutations. The corrected test
passed, and the failed record remains retained.
`cargo nextest run -p farhelm-ui --lib -E 'test(=list::summary_card::tests::card_status_and_peer_title_keep_their_distinct_contracts) | test(=list::row::tests::titles_and_folders_render_escaped_without_filling_an_empty_folder)'`
passed 2/2 in 9115d3af-1086-471b-b269-fdfa8c78ec83. `cargo nextest run -p farhelm-ui --lib -E 'test(activity::)'` passed
9/9 in 42fc91d6-cc23-4d32-8c93-a65f16d95f20.

Recorded Playwright runs used one worker, zero retries, required pinned tmux, and Chromium and WebKit. The redacted
child selections and passing run IDs were `npx playwright test 'tooltip\.spec\.ts' -g 'a card target clones'`
(cab623ab-6b0c-468a-b2c3-60f148366b7c, 2), `npx playwright test 'tooltip\.spec\.ts' -g 'summary card|stale card'`
(6f22e08f-95e5-4a88-97c5-52ded7a0a982, 4), `npx playwright test 'tooltip\.spec\.ts'`
(c3cee50b-06b1-45ef-8520-431d8c5e208d, 26), `npx playwright test 'tooltip-coverage\.spec\.ts'`
(9b0c0de5-8aaf-49d6-9388-cee92eb2b381, 4),
`npx playwright test 'sidebar\.spec\.ts' -g 'compact hides the second line|hostile identity and host text|[directory hover case]'`
(12991325-c1d9-4765-b615-ac408be486c1, 6), and
`npx playwright test 'tooltip\.spec\.ts' -g 'a legacy session card|an unplaced host keeps'`
(b2a3c86e-7e37-4a9f-87c1-0e6b33bcbb54, 4). The sidebar selector is redacted because one test title includes a local
path. The first run covered the tooltip code at #2293's head, f7388a12. The other runs covered product code at #2300's
head, 1d4e74e1, while it was still in the working copy above f7388a12. Their results remain applicable to that pushed
revision: the last edit added only two test cases and adjusted their fixture wait. The dark-palette screenshot
inspection came from the new-card run.

Skipped: the full Rust and browser batteries were broader than the affected hover behavior; the focused cases and full
tooltip spec cover it. The JS unit harness was not rerun because the tooltip asset's exported functions did not change.
The desktop app itself was not exercised; WebKit checked its browser engine family, and the desktop feature compiled.

## Review gate

The required fresh-context gpt-6.1-sol high review found no issue in #2293. The same gate for #2300 found one test gap:
missing omission assertions for legacy sessions and unknown locality. Two browser cases now cover those facts and passed
on both engines. It found no product-code defect.

### Landing

Landed in round 97, as #2293 and #2300, merged bottom-up, second of the round's two plans, right after the typed
launcher choices (launcher-typed-guarded, #2297 and #2301), and unchanged from what the report describes.

#### What landed since the stack was based, and whether it interacted

Since this stack's base, main gained the desktop file-types setting (json-download-open, #2296 and #2299), the fixes
that close the launcher's host list on an outside click (#2294) and fit a filled-in launcher in a 900-pixel window
(#2295), TODO.md and plan bookkeeping, and, just before it in this round, the typed launcher choices. The landing
monitor read the stack against all of them and against the host colors, the session details bar and the earlier
hover-help and link-refusal changes; a fresh-context reviewer did the same independently. Neither found an interaction:

- The card's host mark copies the host's chosen icon and color, including the local host's new choices, and a host whose
  locality is unknown gets no mark.
- Every value from a host or agent on the card (title, host, checkout, directory, agent line) goes through the existing
  escaping, and the hover help copies the card as page elements rather than reading it as markup, so nothing can be
  injected. The card shows nothing sensitive that the row hides: its only new fact is the session's start time, and the
  full launch command stays on the agent mark.
- The session details bar and the covered-link refusal are unaffected, and only this stack edits the hover-help coverage
  test.

The only conflict was in TODO.md, where each side had removed its own entry; both removals stay.

#### Fixes made during the landing

None.

#### Things to keep in mind

The card's agent line (for example "codex · guarded · …") shows the permission the session was launched with, the same
as the row's actions-menu header. When the sidebar shield has switched to YOLO because a no-checks mode was seen on the
agent's status line, the card can still say "guarded". The shield's own hover still says YOLO was observed, so the
information is on the row, just not on the card. Options: leave it, which keeps the card consistent with the menu
header; or have the card follow the shield, for example "guarded, YOLO observed", since people may read the card as the
full summary. The landing monitor recommends leaving it unless you read the card as authoritative; either way it is a
small follow-up, not a landing issue.

#### Checks

- Run now, on main with this stack and the typed launcher choices merged together locally (the landed diffs are
  identical to that tree's): `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, the isolated test-sleep check (304 delays, none without a reason);
  `cargo nextest run -p farhelm-ui --lib` through the test recorder, four slots and no retries: 534 passed, run
  `5ed60b24-5901-40ee-89e2-0b8ce2eb8ade`; `node --test` in the UI's JavaScript test directory, run directly: 294 passed
  (the executor had not run it); `bun run build` for the docs site; then fresh `cargo build` and web builds and
  `npx playwright test` through the recorder on `tooltip.spec.ts`, `tooltip-coverage.spec.ts`, `sidebar.spec.ts` (all of
  it, where the executor ran a selection), the launcher files of the other plan, `terminal-create-idempotency.spec.ts`
  and `launcher-enter.spec.ts`, on both engines, one worker, no retries: 340 passed, run
  `62f8f0a1-3047-4fa1-8851-31f0a2431993`.
- Reused: the executor's static checks (dprint, the changelog format check).
- Skipped: everything else the report skipped, for the reasons it gives, including a light theme, which the app does not
  have.

Nothing in the executor's report became untrue at landing.
