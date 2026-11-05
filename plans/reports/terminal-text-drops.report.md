## Since the earlier blocks

The maintainer chose to retain every unattached vowel or trailing sign, including signs stored as combining marks, and
accepted that an emoji selector following a retained sign cannot reach an earlier emoji. The later answer authorized the
measured repair up to 400 physical changed lines. The repair is now implemented in 335 lines under that counting,
including its mirrored vendor patch, patch hash, vendor documentation, specification and tests.

## What this was about

Since v0.35.0's emoji-width change, new terminals dropped some invisible formatting characters and also deleted ordinary
Kirat Rai vowel signs, including within words. Deleted characters were absent from the screen, copied text, scrollback
and saved terminal contents. The maintainer accepted dropping the invisible formatting characters and asked to preserve
the visible signs.

The first draft documents the accepted formatting loss in the implementation specification and the Move sessions to new
terminals guide. The second restores ordinary printing for every vowel or trailing sign that cannot attach to the
preceding character. Existing legal Hangul attachments remain intact. Both triage outcomes are recorded as complete, and
their feedback files and index entries are removed.

## Things you should know

A retained sign becomes the immediately preceding stored character, so an emoji selector after it is ignored. For
example, a heart followed by a Kirat Rai vowel sign and an emoji selector keeps both the narrow heart and the sign.
Selectors do not reach past the sign. This is the behavior the maintainer selected, replacing the original reach-back
requirement.

The repeat command uses the same attachment decision: an unattached printable sign becomes the repeated character;
attached signs and zero-width combining signs keep the previous repeat character. Repeated vowel signs can themselves
attach under the existing legal-pair rules. The repair adds no remembered input state, selector layout policy or
snapshot format. Old saved terminal layouts remain readable without reinterpretation. Invisible formatting characters
still disappear as documented; this repair does not recover text already omitted from stored terminal contents.

## Open questions and possible follow-ups

None required for these outcomes. Both PRs remain drafts for the landing monitor; this executor has not merged them.

## The PRs

- [#3029: document discarded terminal formatting characters](https://github.com/scode/farhelm/pull/3029/changes).
- [#3045: retain standalone terminal vowel signs](https://github.com/scode/farhelm/pull/3045/changes), based on #3029.

## Checks run, reused and skipped

All runtime checks ran through the test recorder on Linux against the same repair working tree. Nextest used the pinned
0.9.148 runner, four slots and zero retries.

- `cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml`: 165 passed, run
  `d963a4d4-3052-4b3c-8cd7-6638d27a4c3a`.
- `cargo test --manifest-path vendor/vte/Cargo.toml --features ansi`: 62 passed, run
  `e9d7ae4e-d696-41c6-ad67-3d4b18af646f`.
- `cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml`: 3 passed, run
  `1c0c24b0-8422-43cd-8a5e-cdf77ad62a73`.
- `cargo nextest run -p farhelm-terminal-emulator`: 121 passed and 20 explicitly ignored measurements, extended runs and
  fixture-writing tests, run `150e8cfe-ebb6-4135-b6dc-c0758ce02fb3`. This includes the changed snapshot regression,
  retained legal continuations, generated round trips and unchanged golden snapshot bytes.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(emulator_parity::)'`: 10 passed against the checked pinned
  tmux 3.7c, run `18929bf0-4e59-41e6-a7ac-105b21ef165e`. The other 1,743 cases were outside the selection or ignored;
  retained output contains no runtime `SKIPPED` notice. Total across the five runs: 361 executed tests passed.
- Markdown and scoped Rust formatting, whitespace, changelog format and the test-delay checker passed. The delay checker
  inspected 412 calls with no missing rationale. The instruction checker found zero errors and three existing unrelated
  warnings.
- The published vendor archive matched its pinned checksum; applying the updated patch reproduced the checked-in vendor
  tree. All changed patch-hunk references were checked against their current headers.
- The documentation PR's earlier independent review and wording check were reused: its prose is unchanged, and its
  careful rebase resolved only shared feedback-index removals mechanically. Current formatting and instruction checks
  passed again. The full intervening main diff was inspected for interactions.
- Website builds and browser tests were skipped: the guide adds no links or headings, and the repair changes terminal
  interpretation with focused vendor, snapshot and parity coverage rather than browser code. Workspace-wide tests,
  release checks and ignored measurements were unnecessary for this bounded repair. No CI was dispatched.

## Review gate outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues in the repair. The documentation
PR's same required review remains applicable after the mechanical rebase. The repair's scope checkpoint was reused
because the implementation follows the approved measured design. Commit and PR wording passed the cold-reader gate. The
report receives its own independent cold read before delivery.
