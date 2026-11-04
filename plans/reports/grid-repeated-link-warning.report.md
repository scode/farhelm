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

### Landing

Landed with one code fix and two wording fixes at landing. In new terminals, a hyperlink that a program prints in
several adjacent pieces with the same target and no id of its own is now one link for the hover: the hover underlines
all of it on the row under the pointer and checks its whole text, so the false "Link text does not match where it goes"
warning is gone, also when the link wraps onto the next row.

- #2949: the joining in the browser's grid terminal code, with its unit tests and a grid-client browser test; a
  supervisor test that pins the id spelling the joining relies on; the specification and implementation notes, and
  comments in the protocol crate; the changelog entry; and the removed TODO entry and review finding.

#### What the landing fixed, and why

The landing's review found that the change made hovering over links much slower on output a program can choose to print.
Every check of whether a piece of a row belongs to the hovered link built a text key as long as the link's target,
inside loops that visit up to 128 rows either way and every link on the hovered row. On a synthetic screen of 300 rows,
each carrying the same 128 one-character links with 2,000-character targets, one hover lookup took about 11.3 seconds
with the plan against 85 milliseconds on main (measured again at landing; the review measured about the same). While the
pointer was on a link, that froze the page each time it moved onto another row or the screen changed. The landing
compares the link's id and target directly instead, which changes no behavior and brought the same lookup back to 103
milliseconds.

The specification's new clause said "grid terminals", a term it uses nowhere else; it now says new terminals, in a
terminal host, as the rest of the specification does. The changelog entry said the fix "also works when the link wraps
onto another row", which read as if the underline spans both rows; the entry now says the underline covers the row under
the pointer, as for any wrapped hyperlink, while the warning checks the text across the wrap.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping landed on main between the plan's base and the landing's checks; while they ran, three
documentation PRs (#2955, #2957, #2958) added TODO entries, and nothing else landed in this round. A fresh-context
review checked the contracts the change relies on and found them intact: the terminal emulator still gives every link
without an id of its own an id of the generated form the browser recognizes, the wire format is unchanged (the protocol
crate's edits are comments), the rules for which link targets may open are untouched, and every other caller of the
changed link lookup works with a joined link now carrying its first piece's id.

#### Things you should know

- The specification still says, in general, that consecutive rows of one hyperlink are compared together, including
  links an agent continues after indentation with cursor movement. The plan accepted one exception, and only the
  implementation notes record it: a later piece of a joined link that cursor movement placed on another, unwrapped row
  is now judged on its own and can warn for its fragment. A lookalike gets no quieter that way than separate explicit
  link ids already allow. A one-clause caveat in the specification would make it literally true again; that would be a
  follow-up.
- Two separate links to the same target printed back to back on one row with no gap between them, with no ids of their
  own, now join into one link whose doubled text warns, where each used to be quiet. Any separator breaks the join, so
  this should be rare; nothing is planned for it.
- No browser test prints the original report's output through a live terminal. The fix end to end rests on the
  supervisor test (the emulator gives the pieces the generated ids the browser joins) together with the grid-client test
  (the browser joins pieces with those ids and does not warn). A live-stack test of the report's exact output would be a
  follow-up.

The executor's report names its own commit, `92d6787b`; what landed is that commit with the landing's fixes added.
Nothing else in the report became untrue at landing.

#### Checks

This round landed only this plan, stacked on main `57a258b4`. The checks below ran on `d9ef4c08`, the plan's commit with
the landing's code fix; what landed differs from it only in the changelog entry's final wording.

- `dprint check` on the changed files and `python3 releasing/check-changelog.py format` (also on the final wording):
  passed. The workspace debug build and the UI's release web build passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`, including the grid buffer's), run
  `ef05b24a-f140-4252-9a8e-77c191f22051`: all 508 passed.
- The whole grid-client browser config on Chromium and WebKit, which drives the shipped grid assets against recorded
  streams with no stack (`npx playwright test --config grid-client/playwright.config.ts --workers=3 --retries=0` in
  `e2e/`, through the recorder's generic mode), run `57f824d5-d221-4bfd-8669-4fe5906f51ee`: all 152 passed. The executor
  ran only its grid-attach spec; this includes the grid terminal spec, which exercises the changed link lookup through
  the hover underline and its currency.
- The browser suite's host-terminal run (every stack's terminals in terminal hosts, drawn by the grid component) of the
  link-handling specs, `terminal-links` and `terminal-files` on both engines (`FARHELM_E2E_SUITE_TERMINALS=host`, kept
  through the recorder with `--keep-farhelm-env`, and `npx playwright test --project=<engine>-<spec> ... --retries=0`
  through its generic mode), locally, run `ecbd1640-ec9a-4685-9a7d-82700290729c`: 42 passed, 24 skipped by the specs
  themselves on terminal hosts. It first ran on four Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; Node 24.16 there, against 24.20 locally; runs `bea78b27-bf8e-4ed0-9ea0-500fb6dd3fdd`,
  `2b5c23c6-85e0-49a6-a06c-f147e827d058`, `9fd8c5e2-9d3d-4e38-8f22-2f300e427517`,
  `7e2527d7-cc0d-4a1e-8f49-43bae920c141`, and a rerun on the same build, `80322a7f-c280-4c82-86b0-a053bd74e033`,
  `58154ac2-9072-4e68-9811-02ad2a642d5f`, `5627a636-c300-4fd0-a90d-2e6fd612a05e`,
  `5cbf65c4-f247-4122-8941-cbb120eacfc7`), where the same 40 other tests passed with the same skips, but "file hover and
  complete browser download from ssh host" failed on both engines, both times, with "ssh fixture must be connected": the
  suite's second host is reached over ssh to the machine itself, and each sandbox's own setup check had already reported
  that ssh to itself failed. That is a problem in the sandbox setup, not yet fixed; the local run above, where ssh to
  the machine itself works, passed the test on both engines.
- Reused from the executor's report, because the landing's fix changed only the browser's grid code, a specification
  sentence and the changelog entry: `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-supervisor --lib --tests -- -D warnings`, the test-delay check, and the supervisor's grid
  stream tests
  (`cargo nextest run -p farhelm-supervisor --lib -E 'test(/^grid_output::/) | test(/^service::grid_stream::/)'`, run
  `118494a7-20af-42e3-9d5d-e67c370ae224`, all 51 passed), all on the executor's commit `92d6787b`, whose Rust is the
  landed Rust.
- Not run: the whole Rust workspace and the full browser suite (the change is confined to the browser's grid link
  lookup, which the checks above cover), and the desktop checks (no desktop code changed).
