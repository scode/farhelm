## What this was about

Honest web links printed by agent terminals could show a prominent mismatch warning when their text continued onto
another row. Farhelm compared only the hovered row's piece with the destination. The maintainer chose to join the
terminal library's tagged hyperlink cells, including Claude's reopened links and Codex's cursor-moved continuations,
while keeping genuine lookalikes loud.

The draft makes those honest links quiet from either row. Split lookalikes still warn. An ambiguous or oversized
continuation cannot count as a match merely because one piece equals the destination.

## Things you should know

The terminal library remains unmodified. The fix reads its undocumented cell link tag; a browser test against the
shipped bundle pins that field and the agents' two layouts. If an upgrade removes it, the old row-only comparison
returns, including its false alarms. The comparison covers tagged text, not unlinked decoys around it, and the quiet
display still names the real destination host. Joins are limited to 128 rows and 8192 characters.

Execution disproved one planner assumption: reusing a cell object retains the preceding link's tag on unlinked cells.
The initial browser run caught both false warnings and a missed forgery. The fix now reads fresh cell snapshots and the
unit fixture reproduces that stale-tag behavior. Native terminal wrapping can also insert an empty padding cell before a
wide character; that synthetic pad is omitted while printed spaces are preserved. Both corrections were independently
reviewed. File-link hover and the vendored bundle were not changed.

The specification now records the authorized change from accepting wrapped-link false alarms. The old assessment TODO
was replaced with the authorized, detailed upstream API consideration. Nothing was filed upstream. This is one draft PR,
ready for the landing monitor; it has not been merged.

## Open questions and possible follow-ups

The maintainer can consider asking xterm.js for a public link-extent or cell-tag API. The new Near term TODO explains
both agent layouts and the possible upstream approaches. Any upstream library change would arrive with a future stable
version. No maintainer decision blocks this plan.

## The PRs

- [#2156: stop false warnings on wrapped web links](https://github.com/scode/farhelm/pull/2156/changes), draft at
  `6b3852197dbb69272d2e180558fd91f5ad6f7010`.

## Checks run, reused and skipped

- Passed the full UI JavaScript harness: `node --test`, 261 tests, recorder `f23e0d4a-b6cb-4df2-85a6-2ba7b86538a6`.
- Passed the focused real-browser comparison cases on Chromium and WebKit:
  `npx playwright test 'terminal-links\.spec\.ts' -g 'OSC 8 tagged rows|hovers quietly|an OSC 8 span wins'`, 16 tests,
  one worker, no retries or skips, recorder `8edb0bce-846a-470b-9205-cf0f5db20f43`. This covers both hover rows for
  Claude, Codex, native wrapping, wide-character padding, split-host lookalikes and ambiguous continuations, plus
  ordinary quiet/forged links.
- The first full link-spec attempt recorded 34 passes and six failures, recorder `b20505f6-d01f-4e23-a063-e4809fb25e95`.
  Those six failures exposed the stale-cell implementation defect; all affected cases pass in the final focused run. The
  failed run is retained and is not claimed as a full pass.
- Passed `cargo build`, the web release build with Dioxus 0.7.10, the isolated test-sleep checker (301 delays, zero
  missing reasons), changed Markdown/TypeScript `dprint check`, `git diff --check`, and the changelog format check. An
  initial build invocation selected an unrelated program named dx; the explicit Dioxus executable then built
  successfully. No application test was involved in that invocation failure.
- Reused the successful final JS, web-build and browser evidence for head `6b385219`: it ran on the working tree based
  on `516d441d`, and the only later source change was one explanatory docstring sentence. Executable code and browser
  cases are identical. Main had no intervening changes at the final fetch.
- Skipped broader Rust and browser batteries: no Rust behavior, file-transfer policy or link activation policy changed;
  the targeted checks cover the changed comparison. JS files have no configured dprint plugin, so no JS formatter pass
  is claimed. No manual agent recapture, upstream filing or live-install change was needed.

## Review gate outcome

Opus 5.5 at high effort reviewed the full diff and rechecked the corrections. No correctness defect remains; the
wide-character correction was also assessed as necessary and narrow. The optional request for extra fake wide-space
coverage was declined: the existing unit preserves a real edge space, the browser proves synthetic padding, and only an
empty cell qualifies for omission. The suggested docstring clarification was applied. The reviewer ran no tests; runtime
results above were executed separately by the executor.

A fresh medium-effort wording reader accepted the commit/PR text. Private review evidence records the actual Opus model
and cumulative counters; native-reader and executor usage are unavailable. A fresh native report reader checks this
report before delivery.
