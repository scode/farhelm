# Host progress hover card

## Since the last review

The existing draft #3272 now implements the recorded answer to its overflow block: long diagnostics become a preview
only when needed to preserve a readable current step, with a visible pointer to full details in Hosts. It was rebased
onto the landed Always update work; the hover change itself survived unchanged.

## What this was about

Following a running setup, update or uninstall used to give different detail in Hosts and the sidebar. The sidebar's bar
gave only a count, and the Hosts hover left out the connection diagnosis. Both now show the same card with the host,
operation, count, elapsed time, all steps and explanations, the current step, and available diagnostics. The sidebar
also names the current step, and keyboard focus on its host button opens the card.

## Things you should know

The card remains non-interactive. An oversized diagnosis can require opening Hosts to read its tail; the card says
`Full details in Hosts` when that happens. Messages that fit stay whole, and the existing host identity opens Hosts for
the full diagnosis. The current step retains its space and follows progress changes while the card is open.

## Open questions and possible follow-ups

None identified.

## PRs

- [#3272: share host progress details on hover](https://github.com/scode/farhelm/pull/3272/changes) — one draft PR
  covering both progress surfaces, the specification, changelog fragment and TODO removal.

## Checks run, reused and skipped

- `node --test crates/farhelm-ui/js-tests/tooltip.test.js`: 23 passed, recorder `fa51e728-d562-4115-bb19-19b3e55786d9`.
  This covers allocation, list following and shared tooltip decisions.
- Focused `cargo nextest run -p farhelm-ui --lib -E '<card, summary and attention cases>'`: 4 passed, 918 selected out,
  recorder `1713d0a7-cb87-4bb9-8d1c-5f31adb2c34d`. This covers peer-safe diagnostics, connection state, summary data and
  progress precedence.
- `npx playwright test 'host-updates\.spec\.ts|tooltip\.spec\.ts'`: 58 passed, 29 per engine, recorder
  `fdb1ab98-3da1-4462-9eaf-84fc30b429ce`. Chromium and WebKit covered both surfaces, keyboard entry, long-diagnostic
  previews, fitting multiline messages and ordinary session cards.
- Selected provisioning and Tensorlake progress cases on both engines: 7 passed and 1 failed, recorder
  `1b9326ce-fe65-44d2-bb29-c327fd0bcb97`. The failed WebKit dark-theme case sampled text geometry once while the elapsed
  clock could replace the card. Its screenshot showed the text readable. The assertion now polls the same complete
  visibility oracle; the exact WebKit case passed in `014556d8-71fd-443b-98b7-e76f8538f4f9`. The original failure
  remains retained. Together these observations cover both engines' inline progress, short-window following and migrated
  Tensorlake hover; they do not turn the failed command into a passing one.
- After rebasing, a focused card/header integration selection reported 6 passed and 2 failed in
  `b6286c3c-c3cf-40bd-9e70-800e73d57512`. Both failures came from assuming the pointer still hovered the same host after
  a resize moved the rows; WebKit's screenshot showed another host's tooltip. The test now re-establishes hover and
  requires the card visible at each size. Both corrected cases passed in `3302ef90-56b7-4280-b5cd-6260cce2a0e9`. The
  original failures remain retained; the six other passing cases cover surface agreement, fitting messages and
  progress-only menu preservation on both engines.
- `cargo build -p farhelm` passed using the CLI inputs at `cb163bb8`; those inputs remain identical at final head
  `37fe8462`. The pinned `dx build --package farhelm-ui --platform web --release` passed again at `41880b8c`, after the
  Always update rebase. The later changes were a test-only pointer correction and TODO/queue bookkeeping, so the web
  build's shipped inputs are unchanged. These build commands were not run through the test recorder.
- Formatting, targeted Markdown formatting, whitespace, changelog format and the isolated test-sleep checker passed; 429
  delay calls had a rationale.

The Rust and JavaScript checks ran at `cb163bb8` with the hover working diff; the initial browser runs ran at
`e91db5b3`, with the narrow retry adding only the test-readiness correction. Their relevant card, tooltip, summary and
provisioning inputs remain unchanged by the final rebase. The incoming Always update work changes the attention header
and its preference handling; the final integration checks cover its interaction with the card and progress-only menu
preservation. The CLI is reused because that incoming work changes no CLI, helm, supervisor or protocol input. No
runtime `SKIPPED` messages occurred in these retained runs.

The blocked round retained `614128e5-e99e-4f37-a79a-0dd0dbb77bd6` (3 passed, 5 failed) and
`a06eeb91-1cc1-46e9-83e6-69b0ee16d95a` (one WebKit case failed). Three failures in the first run and the later WebKit
failure showed a long connection diagnosis crowding the current step out of a short card. The recorded preview decision
and its implementation resolve that product failure; the final inline and long-diagnostic checks above exercise it on
both engines. The first run's other failures were test defects: reading a replaced clone's paint and counting fixture
events from an earlier case. Those corrections are included in this draft and their cases pass in the current evidence.
Both old failed runs remain retained.

The full workspace, remaining browser suite, desktop smoke, installer and real cloud provisioning were skipped: this
change is confined to shared UI rendering and the focused cases cover its integration risks. Hosted CI is manual-only
and was not dispatched.

## Review gate's outcome

The required single fresh gpt-6.1-sol/high reviewer reviewed the complete PR. Its fractional-height sizing finding was
fixed and its correction verified. The same reviewer cleared the later test-readiness and pointer-premise corrections. A
fresh scope reassessment found no unnecessary product mechanism and endorsed the narrow test correction. The resume
audit and commit/PR wording cold read passed. Every touched file received a separate documentation pass. No rejected
finding, unanswered product choice or scope departure remains.
