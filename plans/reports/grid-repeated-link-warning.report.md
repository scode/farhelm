## What this was about

An agent could print one web address through consecutive hyperlinks with the same destination but no explicit link ID.
Hovering the later piece compared only its tail with the destination, showed a false "Link text does not match where it
goes" warning, and underlined only that piece. The maintainer chose a browser-only fix for grid terminals, joining only
direct continuations with the same target.

The fix joins those pieces on a row and across soft wraps. The hover compares their whole text and underlines the whole
piece on each row. Different targets, explicit IDs and real gaps keep their existing boundaries. The addressed TODO and
review finding were removed; the separate finding about snapshot comparison remains.

## Things you should know

The browser recognizes the emulator's numeric generated-ID spelling. A program can imitate that spelling, but joining
still requires equal targets and direct continuation; it gives no quieter comparison than the existing explicit-ID rule
already permits. The specification records that rationale and the producer regression pins the spelling.

As accepted in the plan, a later piece that inherits the chain's first identity no longer joins another row merely
because cursor movement placed that piece's original generated ID there. That other row is compared separately and can
show a warning for its fragment instead of being compared with the earlier pieces. Repeated complete addresses that each
match their target remain quiet. Joining still compares at most 128 rows and 8,192 characters; an extent that cannot be
joined within those limits warns on address-like fragments. No stored snapshot or wire format changed.

## Open questions and possible follow-ups

No unresolved decision or required follow-up. The warning remains an aid rather than a guarantee, as already specified.

## The PRs

- [#2949: avoid false link warnings in grid terminals](https://github.com/scode/farhelm/pull/2949/changes) — draft,
  commit `92d6787beca3c0ccb2adf400987686c3e2916c56`.

## Checks run, reused and skipped

Run for this change:

- `node --test crates/farhelm-ui/js-tests/grid-buffer.test.js`: 35 passed, recorder
  `33fb95cd-3f30-4566-b9a2-6e7338ebe0c0`.
- Asset JavaScript harness (`node --test` in the UI JS-test directory): 508 passed, recorder
  `415c9b42-4a9e-4203-a6fa-d28dde9d703c`.
- `npx playwright test --config grid-client/playwright.config.ts grid-attach.spec.ts --workers=3 --retries=0` from
  `e2e/`, Chromium and WebKit: 68 passed, recorder `1e4adace-17d1-4573-93cf-02a14e1ba1d0`. This used generic recording
  because the controlled browser mode does not accept a separate config; counts come from the retained Playwright
  output. These specs load shipped assets through a fake socket and require no real stack.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^grid_output::/) | test(/^service::grid_stream::/)'`: 51
  passed, recorder `118494a7-20af-42e3-9d5d-e67c370ae224`, pinned nextest, four slots and zero retries, no tmux
  required. The 1,682 skipped tests were outside the selection; retained output contains no runtime `SKIPPED` marker.
- Changed-file dprint, `cargo fmt --all -- --check`, changelog format and the test-delay checker passed. The latter
  inspected 397 delays with none missing a rationale. The instruction-link checker passed with zero errors and three
  existing stale citations outside this change.
- `cargo clippy -p farhelm-supervisor --lib --tests -- -D warnings` passed, covering the new producer regression.

No earlier-session evidence was reused. Runtime checks exercised the implementation and tests in the PR; subsequent
edits only clarified documentation. A fetch found only plan-claim bookkeeping on main, with no functional interaction.
Full workspace, real-stack browser, vendor and desktop suites were skipped because the production change is confined to
grid-buffer link identity and the selected unit and browser checks exercise that boundary directly.

## The review gate's outcome

The demanded fresh-context gpt-6.1-sol high adversarial review covered all changed files and the full test-authoring
checklist and reported no findings. The separate commit-message cold read passed after one rewrite for clarity.
