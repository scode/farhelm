### What this was about

Claude sessions could show Idle while Claude was compacting a conversation. Its compaction spinner uses several words,
which Farhelm previously rejected as a working indicator. The fix recognizes multi-word spinners and shows the session
as Running during compaction, as requested. Finished-turn text without an ellipsis still reads Idle.

### Things you should know

The maintainer chose to recognize multi-word spinners generally, accepting some cosmetic false-positive risk. The
existing position above the input box, glyph, ellipsis and digit-led parenthesis checks remain. Reply text such as
`- Ran the suite… (2 failures)` can still look like work if it falls within that input-box window; a regression case
records this accepted behavior.

A real capture from Claude 2.1.296 produced all 12 scenarios, including compaction. The saved compaction frame has a
one-second timer; a unit case separately covers a minute-duration timer. The older 2.1.285 screens remain covered. No
other status drift requiring a product decision appeared.

### Open questions and possible follow-ups

No blocking questions. The capture tool observes compaction every 100 milliseconds after command submission, but
submission itself retains its 1.5-second polling interval. A compaction that finishes in that interval can be missed by
a future capture. This run captured compaction successfully.

The capture uses the configured Claude interface, with identifying values neutralized; it does not claim a pristine
vendor configuration. Isolating that configuration could be a separate capture-tool improvement.

### PRs

- [PR #1774](https://github.com/scode/farhelm/pull/1774/changes) — Claude compaction reads as Running; includes the real
  captured screens, capture scenario, documentation and removal of the completed TODO. One reviewed draft PR, still
  unmerged.

### Checks run, reused and skipped

Checks cover the working change based on `3c434a81`. A fresh fetch before publication found no newer main changes.

- Real Claude capture through the recorder: `python3 scripts/capture-agent-screens.py --harness claude --keep-work`, run
  `61dfe72c-bb19-428a-bbcc-aa009e0642d6`; all 12 scenarios captured. One private pane using pinned tmux, no other vendor
  capture.
- Capture's fixture selection, run `d9f4cad3-1bea-401a-a007-abd5b350c4e0`: 6 passed. Covers every committed screen,
  including both Claude versions and unchanged vendors.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(screen_reader::)'`, run
  `038d78e6-283c-4730-bd2d-e3c7a49cbd10`: 8 passed. Final shape selection after adding the accepted-prose case, run
  `6b124514-b178-4cfc-a7f2-4aa71d3ed7c0`: 1 passed. Zero retries, four global nextest slots, no runtime substrate skips.
- Corrected capture predicate proof, run `79323b33-6929-4a5b-ab9c-a78b5e5338be`: 14 boundary cases and the real saved
  compaction screen passed without another vendor turn.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, Rust formatting, Python AST syntax, changed
  Markdown formatting and `python3 releasing/check-changelog.py format` passed. These inspect the changed predicate,
  script and documentation.
- Isolated test-delay checker: 276 sources inspected, zero missing rationales. Reused after subsequent comments and
  shape strings only; no delay or module changes altered its coverage.
- Fixture and reader results reused after later explanatory comments; the added prose case received its own final shape
  run. No runtime failures needed retrying.
- Workspace Clippy, browser, desktop, broad runtime suites and doctests skipped: the product change is a pure supervisor
  screen predicate, covered by the focused checks. Other vendor captures would not add evidence for Claude compaction.

### Review gate outcome

The required independent Opus 5.5 high general review and follow-up completed. Its documentation and capture-predicate
findings were corrected, the accepted false-positive risk was made explicit, and the short capture blind window was
documented. An optional glyph blacklist was declined because the maintainer required the other guards to remain
unchanged. Final formatting passed after the follow-up's wrapping finding. The review gate passed with local fixes; the
commit and PR wording cold read passed.

### Landing

Landed on 2026-10-10 (UTC) as #1774 (Claude shows as running while it compacts), one squash commit on main, directly
after host-icons, which touches no supervisor code. Since the plan was based, main also gained the shutdown quiet-down
fix (#1780) and the previous round's supervisor change that made the two-second timer the only place sessions are
sampled. The reviewer checked that interaction: the screen reader is called from the same sampling path as before, and a
compaction now counts as activity, so a long compaction keeps the session's "last seen working" time fresh, which is
what SPEC.md describes.

#### Review before merging

A separate reviewer that had not worked on this round's plans re-ran the new rule by hand over all 50 committed Claude
screens: every 2.1.285 and 2.1.296 screen reads its named state, the only screen whose reading changes is the compaction
screen (Idle before, Running now), and no other line on any screen matches the new rule. Waiting screens cannot read as
Running, because working lines are only looked for when the input box is on screen and every dialog replaces it. Codex
and the other agents' readers are untouched. The new captured screens contain no personal identifiers; they do mention,
in Claude's own words, that a private skill was loaded at session start, as the earlier captures already did, which
isolating the capture's Claude configuration (the report's possible follow-up) would avoid.

#### A change made while landing

SPEC_impl.md now records the false positive you accepted: reply text shaped like a spinner line just above the input box
(`- Ran the suite… (2 failures)`) also reads as working. Without it there, a future review finding about that behavior
would come back to you instead of being closed as already decided. The landing also closed an unbalanced parenthesis in
the spec's compaction example.

#### Checks

- Run now, stacked on host-icons: the supervisor's screen reader and captured-screen tests, 14 of 14, in the recorded
  run `aca47509` (its three failures were host-icons' helm tests, fixed there), with `cargo clippy --all-targets` and
  `cargo clippy -p farhelm --bins` clean.
- Reused from the executor: the real Claude capture and the per-version screen checks; the landing changed only spec
  wording.

Nothing in the report above was made untrue by the landing.
