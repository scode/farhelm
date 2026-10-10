### What this was about

When an agent asked to start or change a session, the approval card could become a tall strip at the window's right
edge, with its answer buttons below the fold. The chosen layout was suggestion B of the agreed mockup: compact and
centered below the main pane's tabs, leaving the sidebar and terminal prompt clear, with one request expanded at a time.

The change follows that layout. The oldest request opens first; choosing another request keeps it selected across
refreshes. Answering it or its expiry brings up the oldest remaining request. Other requests appear as labelled header
buttons below the expanded card, with a waiting count.

### Things you should know

At executor delivery, #1760 is an open draft PR and has not landed. The landing monitor owns the final integration check
and merge; its later Landing notes should record those outcomes.

All decision data remains labelled and complete. Commands wrap in full, and long content scrolls with the entire
approval region. The 700ms button pause now also applies when the expanded request changes; a stale timer cannot arm a
newer selection early. Approvals remain answerable with a dialog open, and the late-answer notice remains.

The region targets 660px width with 12px pane margins and a height cap of 55% of the pane, further limited by space
below its anchor. It follows the actual pane, tabs, sidebar, header notices and window chrome. Its geometry observation
is confined to the pane and direct surrounding layout; it does not watch terminal output or poll. Sidebar size is
included because a sidebar resize can move a pane that has already reached its 320px width floor.

Common and narrow screenshots from both engines were inspected against the mockup. The common card was about 356 CSS
pixels tall rather than the sketch's roughly 320, with three fact columns, the complete wrapped command and all buttons
visible. The 320px pane gave a 296px card and one fact column, with whole-region vertical scrolling and no horizontal
overflow. A long requesting host wrapped without truncation; dimension and full-text assertions covered the answer
button below that screenshot's viewport.

The source review found one narrow-pane defect: an unbroken requesting-host name could make the permanent-allow button
overflow. The button now wraps that full name within the card. Execution also exposed two new fixture defects: staged
replies lacked the helm's build stamp, and a test tried to click a deliberately hidden quick-switcher trigger. The
fixtures now use the existing stamped-reply helper and the supported keyboard shortcut after establishing terminal
focus.

### Open questions and possible follow-ups

No maintainer decision is pending. This plan does not change the approval protocol or which facts an action supplies. At
executor delivery, the independently running sidebar-resize plan is still in flight, and the combined integration check
remains outstanding for the landing monitor. That check belongs to landing rather than a proposed follow-up feature: the
plans share layout and CSS. This card uses measured pane bounds rather than assuming a fixed sidebar width, and its
browser regression already exercises sidebar movement with an unchanged minimum pane width.

### PRs

- [#1760 — compact agent approval requests](https://github.com/scode/farhelm/pull/1760/changes), draft, based on main;
  commit `cb5302818da0efddeeb607d6d88b73f414ab9d55`.

### Checks run, reused and skipped

Runtime evidence cited below was retained through `scripts/record-test-run.py`. The JS harness was first run directly by
mistake, then repeated through the recorder to establish the required evidence. Browser execution used one worker, zero
retries, Chromium and WebKit, and the matching pinned tmux 3.7c substrate. The published commit preserves the validated
source tree; the final browser run covered the uncommitted nine-file change on main `b9eddc63`, before its commit
description was assigned.

- `cargo build` and `dx build --package farhelm-ui --platform web --release` passed to supply the actual native and web
  products driven by the browser tests. The final web build included the latest sidebar geometry correction.
- `cargo check -p farhelm-ui --features desktop` and `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed
  for the changed UI crate and desktop rendering configuration. Their Rust behavior remains current; later changes were
  geometry JS, fixtures and explanatory comments, covered separately below.
- `cargo nextest run -p farhelm-ui --lib -E 'test(approvals::)'` passed four selected content tests in run
  `751533c5-1add-4639-ad88-0af9b1018275`; 437 tests were outside the selection, with no selected runtime skip. This
  evidence remains applicable because later edits did not change the row-generation contract.
- `cd crates/farhelm-ui/js-tests && node --test` passed 203 tests in run `d50e9874-cdd2-4ae3-aa50-19a84dc8de8c`. This
  covers the existing asset-JS seams; the new geometry asset's execution proof is the browser run, not this harness.
  Later changes did not alter the harness's covered behavior.
- `cd e2e && npx playwright test 'approval-layout\.spec\.ts' 'spawn\.spec\.ts'` completed with 12 passed and two skipped
  in run `29032ad5-e508-4462-9e30-45d263de7e07`, six executed scenarios per engine. There were zero failures, flaky
  outcomes, interruptions, unstarted cases or retries. The eight new layout cases cover placement, visible buttons,
  narrow and long-host wrapping, sidebar movement, empty/session/remount lifecycle, selection persistence and fallback,
  keyboard switching, stale arm timers, and answering with the quick switcher open. Four existing fake-agent spawn cases
  cover Allow, Always allow and Deny against the real helm. The two real-Claude cases require deliberately enabled
  vendor credentials and network and were skipped; they are not claimed as coverage.
- An earlier run, `ae9e795b-0865-4421-b040-3df0c8010e4f`, had four Chromium fixture failures and ten unstarted cases and
  was interrupted after diagnosis. Its evidence is retained privately. Three failures showed build-skew withdrawal from
  the missing reply stamp; the fourth attempted the hidden switcher trigger. The exact compact-card regression then
  passed both engines in run `d0dfdab0-a3fa-4540-9560-3189043587b6`, before the final expanded selection above. These
  were new fixture defects corrected in this session, not a demonstrated latent flake.
- `cargo fmt --all -- --check`, targeted `dprint check` on the changed Markdown, and
  `python3 releasing/check-changelog.py format` passed. The repository's dprint configuration has no CSS, JS or TS
  formatter; those files were inspected manually. A CSS/TS-only dprint invocation returned no-files rather than
  formatting them.
- The isolated-interpreter `python -B scripts/check-test-sleeps.py` check inspected 276 files with zero missing delay
  rationales, covering the changed browser tests.
- `cd website && bun install --frozen-lockfile && bun run build` passed, including internal-link checks, because the
  approval documentation changed. No deployment was requested or performed.
- Successful checks were reused after the careful rebase: the intervening main changes were queue bookkeeping and
  unrelated Overview prose, with the full diff inspected and no functional interaction. No full workspace, full browser,
  installer, release or desktop-runtime battery was run: the changed behavior has targeted content, compile and
  two-engine browser evidence, and those broader gates address no identified remaining regression risk here. Hosted CI
  was not dispatched; this repository does not run CI on PR creation.

### Review gate outcome

The required fresh-context source review was requested as GPT-6.1-sol at high effort with the complete plan, general
charter and verbatim changed-test checklist. It found the long-host button overflow; after the local fix, the follow-up
reported no remaining findings. The final two-engine runtime evidence was then inspected independently by the executor.
Scope reassessment accepted the bounded geometry observation, and a separate documentation pass covered every touched
file. The commit and PR wording passed the required cold read, and the report receives its own cold read before
delivery.

Implementation and investigation stayed local under the plan's no-workhorse requirement. Scope and wording reviews were
requested as native GPT-6.1-sol at medium effort; the required resume check used the inherited native model. Exact
runtime model attribution and usage counters were unavailable, so the requested model names are not proof of runtime
attribution. Private delegation evidence is retained under session UUID `a63f6b2c-45f7-4830-bf84-ea76de63472d`.

### Landing

Landed on 2026-10-10 (UTC) as #1760 (compact agent approval requests), one squash commit on main, first of three plans
in this round (then resizable-sidebar and sweep-on-timer). Main had gained only plan bookkeeping and TODO entries since
the plan was built, so the rebase was clean.

#### Review before merging

A separate reviewer that had not worked on any of this round's plans read #1760, then #1759 and the two together,
against main. It confirmed the report's claims against the code, including that the new placement script is compiled
into the page rather than loaded as a separate asset, so the desktop and web builds both carry it without an asset-list
entry. It also confirmed that the card follows a live sidebar drag: the card watches the sidebar's size as well as the
pane's, so a drag that moves the pane, even one already at its minimum width, moves the card with it.

#### A fix made while landing

The reviewer found that the compact card shortened the asking session's labels to "session" and "session id", which are
exactly the labels the rename, stop and restart cards use for the session being acted on. A stop request from one
session about another then showed two "session" rows on one card, separated only by a border, and a user could misread
which session was about to be stopped. The landing renamed the asking session's rows to "from session" and "from session
id", matching the "from host" row beside them, and said why in the code, in #1760 before it merged. The approval content
tests and the approval-layout and spawn browser specs passed with the change (below).

#### Things the landing did not change

Two points from the review are left for the maintainer rather than changed while landing:

- The card sits above every dialog (it did before, in the corner), and in its new place, centred below the tabs, it
  covers most of the quick switcher's search field and results while a request waits. The quick switcher stays usable
  only once the request is answered. The plan's test proves the card can be answered over the switcher, not that the
  switcher stays usable beside it, and SPEC.md still says the rest of the app stays usable. Whether the card should
  collapse to its header while a dialog is open, or the overlap is acceptable, is a design call.
- The waiting count reads "1 of N waiting" whichever request is expanded, so it can read like a position.

The reviewer also noted that in a window too narrow for both panes, the card follows the main pane and can sit partly
past the window's right edge until the page is scrolled; a wider sidebar (resizable-sidebar, landed next) makes that
narrow case start at a wider window.

#### Checks

- Run now, on the two UI plans stacked in landing order and before the landing fixes, through the recorder on Chromium
  and WebKit with pinned tmux 3.7c, one worker and no retries: the approval-layout, spawn, sidebar-resize, sidebar,
  header, shell-scroll and terminal specs (run `8ef60593`, 364 passed; the two skipped are the real-Claude spawn cases,
  which need vendor credentials).
- Run now, after the landing fixes: `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, the approval content
  tests (run `0fbf5ce6`, 4 of 4), and the approval-layout, spawn and sidebar-resize specs on both engines (run
  `4343dfea`, 16 passed, the same two real-Claude cases skipped); `cargo fmt --all -- --check` and the test-delay check
  (276 delays, none without a reason).
- Run now, on all three plans stacked together: `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and the supervisor's unit tests in full (run `75969832`, 1059 of
  1059).
- Reused from the executors: the UI JavaScript tests, the desktop compile check, the desktop asset comparison, the
  changelog lint and the website build. The landing changed no JavaScript, desktop code, asset list, changelog fragment
  or documentation page, and main gained only plan bookkeeping and TODO entries in between.

Nothing in the report above was made untrue by the landing.
