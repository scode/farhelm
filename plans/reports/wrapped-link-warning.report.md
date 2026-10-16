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

### Landing

Landed on its own in round 68, as #2156. Nothing but queue bookkeeping reached main between this plan's base and its
landing. The base already held the earlier terminal-link safety work and the Rust test-portability changes from
round 67. That link work covered the drag check, the stale-link recheck, the refusal when link details are clipped, and
hidden-character escaping. I read the PR against that code, and a fresh-context reviewer did the same independently.
Neither found an interaction. File-link hover, link clicking and the drag check are unchanged; only the web-link hover
display changed.

#### Fix made during the landing: the spec overstated the new protection

The PR rewrote the specification's hover-warning paragraph to say that "a split lookalike still warns", and it dropped
the older caveat that a program choosing its own line breaks can still keep a lookalike quiet. That caveat still holds.
The fix only joins consecutive rows of one hyperlink. A program can print `https:/` and `/github.com/login` as two
separate hyperlinks, on non-adjacent rows, or with the same link appearing twice on one row, which the fix treats as
ambiguous and does not join. Each piece is then judged alone, neither looks like a web address on its own, and both
hover quietly. I confirmed this against the address pattern the warning uses. The code is not affected, and such a split
lookalike was already quiet before this plan, so nothing got weaker; the quiet display still names the real host. Only
the wording was wrong.

The landing reworded SPEC.md to say that a lookalike split across rows of one hyperlink is judged on its whole text, and
restored the caveat. It also added the same caveat, with the example, to the matching SPEC_impl.md paragraph, so a later
change does not overstate the protection again. No code changed.

One sentence in the executor's report needs the same correction. "Split lookalikes still warn" holds only for a
lookalike split across consecutive rows of one hyperlink, not for one a program prints as separate pieces. The changelog
fragment's "Links whose text names a different destination still warn" was left as is: it is accurate for the wrapped
links this plan is about, and the release curation should not widen it into a general promise.

#### A possible follow-up

The reviewer noted, without verifying it, that a program reusing one link tag for the same address on two consecutive
rows would have the rows joined into a doubled address and see a false warning. That can only add a warning, never
remove one. It is a note for a future look, not something this plan needed to settle.

#### Checks

The executor's last full run of the terminal-links browser spec had six failures. Those failures led to its final fix,
and afterwards it re-ran only the 16 comparison cases. The spec's other cases (clicking, dragging, and activating
wrapped and scrolled links) also pass through the changed web-link hover. For a change to a security warning, they
needed to pass on the final code before it landed.

- Run now, at the rebased tip, after fresh `cargo build` and web release builds: the terminal-links and
  terminal-link-clicks browser specs in full, on Chromium and WebKit, through the test recorder with the pinned tmux,
  one worker and no retries: `npx playwright test terminal-links.spec.ts terminal-link-clicks.spec.ts`. 60 of 60 passed
  with no skips, run `2633fd0c-7362-432b-b985-67b48c86132f`.
- Run now, directly rather than through the recorder: the UI JavaScript harness, `node --test`, 261 of 261 passed. The
  executor's recorded run of the same harness on identical code is in the report.
- Run now: `dprint check` on SPEC.md and SPEC_impl.md after the landing's rewording, clean.
- Reused: the executor's test-sleep, changelog-format, dprint (its changed Markdown and TypeScript) and
  `git diff --check` results, from its head `6b385219`. The landing changed only spec prose on top of identical code,
  and main gained only queue bookkeeping in between.
- Skipped: Rust test runs, since no Rust code changed, and the other browser specs, which do not exercise web-link
  hover.

The rest of the executor's report still holds, including its description of the spec change, with the caveat restored.
