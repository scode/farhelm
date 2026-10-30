## What this was about

Hovering over a host widened the rail over the sessions it was meant to filter, while the controls at its foot stayed
narrow. The agreed replacement keeps full names in tooltips and adds a button that deliberately expands the whole rail
beside the session list.

## Things you should know

Each device starts collapsed and remembers its own choice. The button below Help expands the rail and its foot from 56px
to 220px. Expanded names are clickable parts of their host rows; choosing a host keeps the rail open. Tooltips and
accessible names include a distinct remote destination and retain the existing state explanations. Hover and keyboard
focus no longer widen anything.

Expansion adds up to 164px to the sidebar, taking room from the terminal while preserving the session list's width when
space permits. The approved cap limits that growth in smaller windows; the list supplies the remainder. The stored
resize width keeps its old meaning. At the cap, dragging can change that stored width before the visible edge moves, as
the plan accepted. Very narrow windows retain horizontal scrolling. Status choices wrap when the expanded list becomes
too narrow, including at the minimum stored sidebar width.

The specification and changelog describe the change, and the TODO entry is removed. README, video and docs screenshots
remain for their next requested refresh; the fenced media was not changed.

## Open questions and possible follow-ups

No maintainer decision is needed to land this work. The next normal media refresh can pick up the fourth footer button.

## The PRs

- [#2841 — replace host rail hover with an expand button](https://github.com/scode/farhelm/pull/2841/changes), draft.

## Checks run, reused and skipped

Browser commands ran from `e2e` through this recorder wrapper, with pinned tmux on PATH:

```sh
python3 ../scripts/record-test-run.py --runner playwright --kind development --tmux required --timeout <seconds> --selection '<described selection>' --concurrency '3 browser workers, one stack each; retries 0' -- npx playwright test <arguments below>
```

Each selection used both configured engines, Chromium and WebKit. The portable child arguments and command deadlines
were:

- Initial rail/filter selection (900 seconds): `'host-short-names\.spec\.ts' 'filters\.spec\.ts'`.
- Broader rail/footer regression selection (1800 seconds):
  `'sort\.spec\.ts' 'hosts-panel\.spec\.ts' 'sidebar\.spec\.ts' 'host-updates\.spec\.ts' 'host-rail-warning\.spec\.ts' 'settings\.spec\.ts' 'tooltip-coverage\.spec\.ts'`.
  The settings suffix also selected `ntfy-settings.spec.ts`.
- Exact narrow-window case, both the failure and passing rerun (300 seconds):
  `'filters\.spec\.ts$' --grep 'the rail keeps keyboard focus and fits a narrow window$'`.
- Final continuation (1200 seconds):
  `'sidebar\.spec\.ts$' 'host-short-names\.spec\.ts$' 'filters\.spec\.ts$' 'status-filter\.spec\.ts$' --grep '<generated expression selecting the 40 unfinished sidebar titles or any case in the other three specs>'`.
  The generated expression is redacted; its exact selected cases are retained with recorder
  `137fce39-32a5-4135-8828-1213d3b153dc`.

- Final browser continuation, recorder `137fce39-32a5-4135-8828-1213d3b153dc`: all 130 passed, 65 per engine, zero
  failed, skipped, flaky, interrupted or unstarted cases. Complete source, reports, output EOF and normal cleanup were
  confirmed. It ran the complete host-name, host-filter and status-filter specs plus the 40 sidebar titles left
  unfinished by the earlier command deadline, on both Chromium and WebKit, three workers, zero retries, including
  intentional repeats of the already completed Chromium sidebar cases. The final roomy expanded and collapsed captures
  were also inspected.
- The preceding rail/filter selection, `40d4c28f-394d-4961-b86a-8ee994d70329`, passed 38 cases. Its captures exposed
  overlapping narrow status labels that its original assertions missed. Stronger bounds checks then caught selected
  Running extending outside its well in WebKit at the minimum width: `58557e12-8d0c-4ab9-9a42-5c17aab600f6`, one pass
  and one failure. Expanded-only wrapping and reduced label padding address those observations. The final exact case,
  `37402946-51d9-4cd6-9a21-289e67a5527d`, passed on both engines, including selected-font checks at 800, 750, 718 and
  360px windows and the 296px stored minimum. All six final constrained captures were inspected.
- The broader rail/footer regression selection, `7bb43f5a-2500-49ee-9622-199effaacf7c`, reached its 30-minute command
  deadline with 368 passes, no failed assertions, one interrupted WebKit case and 39 unstarted cases. It was not a
  passing selection. It covered sorting, host management, sidebar, host updates and warnings, settings and notification
  settings, and hover help; the continuation above selects precisely its unfinished titles as well as the final rail
  checks. The timeout record and interrupted trace remain retained.
- Two attempts, `37609094-43f3-4176-9af3-d8fd4189c263` and `ee6e947b-14fb-4dc6-8860-51429f1a0860`, refused before
  browser startup because the Playwright version probe did not finish. Direct installed-version probes succeeded and
  unchanged retries ran. These contain no test results. Their records and the same-session WebKit failure remain
  retained; no latent flake is claimed.
- From `crates/farhelm-ui/js-tests`,
  `python3 ../../../scripts/record-test-run.py --kind development --selection 'UI asset JavaScript with final expanded label padding' --concurrency 'node default discovery' --tmux none --timeout 60 -- node --test`,
  recorder `dd35a0c8-5457-4dd2-9ac9-2e7a346f8303`: 383 passed, zero skipped. This includes the expansion math, device
  storage, mount/remount ownership and stylesheet checks. Earlier iterations passed 381 cases, then 383 after upstream
  added two shortcut tests. The final UI release web build passed with the pinned Dioxus 0.7.10 builder:
  `dx build --package farhelm-ui --platform web --release` from `crates/farhelm-ui`, with the pinned builder selected on
  PATH. Two initial invocations selected an unrelated executable and stopped before building; the correct builder was
  used thereafter.
- Focused UI Rust tests, `cargo nextest run -p farhelm-ui --lib -E 'test(/rail_|footer_menu/)'`, recorder
  `0145f3fd-50ac-4371-b7c6-4dee5bde4944`: three rail identity/presentation/update tests passed, 823 outside the
  selection. No footer-named unit matched; footer geometry is browser evidence. Pinned nextest/tmux, four slots, zero
  retries; retained output contains no runtime SKIPPED notices.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check SPEC.md SPEC_impl.md releasing/changelog.d/host-rail-toggle.md`,
  `python3 releasing/check-changelog.py format` and `<isolated-python> -B scripts/check-test-sleeps.py` passed. The
  isolated interpreter's private location is redacted; the delay checker inspected 327 delays with zero missing
  explanations. `python3 scripts/check-agents-md.py` reported zero errors and three pre-existing citation warnings.
- Final JS and focused browser evidence covers the uncommitted UI on base `cac2d3bb`, fingerprint
  `5c19210957b0b8dbf65231e50367853b80013c6fc5385c320b7ddae2d70db45c`. The same rail changes were committed as `3501c33e`
  after rebasing onto `35d8321f`. The intervening change adds cleanup diagnostics with unchanged operations, locks and
  replies. The earlier Rust tests and UI lint cover the feature's unchanged Rust on base `fcad8cbb`; the Rust run
  fingerprint is `b12c3aff260a9183338cc9e721cfe48326f759c0623a5fe78f8aee9b2f448ca4`. The browser regressions before the
  correction cover `fcad8cbb` with fingerprint `cd486848e761fecbdbaaed314a13bf9f5124deef18e77e68b6e7980bfc3b6fe0` and
  are reused only where behavior stayed the same. The rebased UI was rebuilt and keyboard activation of the rail toggle
  is in the final selection. The server build (`cargo build -p farhelm`) and fixture build were reused because the rail
  APIs and fixture contracts are unchanged; upstream template-name validation and cleanup logging were inspected
  separately, not claimed as newly tested backend work.
- Whole-workspace, installer, provisioning, desktop smoke and native macOS runs were skipped: this change concerns
  shared rail markup, CSS and page-owned storage, covered by the targeted checks and both browser engines. No website
  page or generated media changed, so no website or media build was needed. No CI workflow was dispatched.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idioms and the full
test-authoring checklist. Its findings caught footer icons and menu anchors shifting during expansion, and text-overflow
measurements that could miss a failure while the intended font was still loading. Both were corrected and verified; the
final padding correction was also inspected. No source findings remain. A fresh own-model scope reassessment kept the
narrow fix to the existing wrapping facility rather than a new layout mechanism. Commit and PR wording passed a separate
cold read, and the documentation pass is complete.
