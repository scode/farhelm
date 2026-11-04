## What this was about

In new terminals, emoji such as ❤️ took one column even when a program counted two, shifting the rest of the row. The
maintainer chose Ghostty's default emoji and text presentation rules, including narrowing normally wide characters,
while preserving existing terminal snapshots. Old terminals keep tmux's behavior.

New terminals now change width only for eligible presentation sequences, with the cursor, wrapping, overwrites and
character repetition following the chosen rules. Old snapshots retain their saved cells and repetition state. New
snapshots round-trip changed widths through the existing format; the encoder, width fingerprint and frozen fixture bytes
are unchanged. Three golden-test scripts explicitly reconstruct their old repetition states because current live input
no longer creates those states.

## Things you should know

The reference is Ghostty commit `c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed`, read from source, not built or run. Its
default configuration enables Unicode grapheme widths; the deciding selector rules are in
[grapheme.zig](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/unicode/grapheme.zig#L56-L80)
and
[Terminal.zig](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L1929-L2187).
An already stored combining mark or selector prevents another selector from changing the base. Widening overwrites
rather than shifts text in insert mode; a base moved to the next row takes the current pen. The bounded cell storage can
drop a selector while its requested layout still takes effect.

Two differences are explicitly approved and pinned:

- With wrapping disabled at the right edge, Farhelm treats a plain space as empty when choosing which character a
  selector affects. Ghostty distinguishes a printed space from an erased cell; the saved state cannot. Consequently
  Farhelm can widen a character that Ghostty leaves narrow.
  [Reference](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L1939-L1967).
- A one-column terminal keeps dropping ordinary wide characters before consuming a pending wrap. Ghostty consumes the
  wrap first, so its cursor can end on another row.
  [Reference](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L2189-L2262).

Review also exposed formatting inputs that Ghostty discards before the next selector. A bounded, stateless property
subset handles those inputs and incoming Hangul/Kirat continuations, using existing cell suffix storage and the existing
repetition register. Fresh scope assessments accepted that narrower design. This adds no general grapheme engine or new
saved state. The vendored parser hook preserves upstream behavior by default; Farhelm enables the terminal's repetition
policy, and standalone alacritty tests still use the upstream parser. Unicode property data carries its license notice.

The changed user page is `website/src/content/docs/docs/using/new-terminals.md`. No preview was started during this
unattended run. No live installation was changed.

## Open questions and possible follow-ups

The existing near-term TODO for exact Ghostty matching of the two approved exceptions is preserved. Regional-indicator
pairs, skin-tone modifiers and ZWJ sequence merging remain possible separate work; this plan does not implement them or
add TODO entries for them. No unresolved decision blocks this PR.

## The PRs

- [#2971](https://github.com/scode/farhelm/pull/2971/changes): emoji and text presentation widths in new terminals, with
  frozen-snapshot compatibility; one commit, one open draft. This executor has not marked it ready or merged it.

## Checks run, reused and skipped

All runtime checks ran on Linux through `python3 scripts/record-test-run.py`. Nextest used four slots and zero retries;
tmux comparisons and browser stacks required the pinned tmux 3.7c. Output was checked for runtime `SKIPPED` messages;
none occurred. Ignored manual tests and cases outside a selection are not counted as executed.

- `cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml`: 165 passed, run
  `f95d62ec-7fbd-450c-b90b-3cf16b39dba2`. The exact two-fixture correction passed first in
  `e498b3aa-f3e6-4112-ae4d-bbaa77c112cd`. Later changes only expanded a docstring and regenerated the matching patch.
- Reused `cargo test --manifest-path vendor/vte/Cargo.toml --features ansi`: 62 passed, run
  `96f62b56-dc93-40d0-9539-c1cba68dbc14`, because its source and tests did not change afterwards. Reused
  `cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml`: three passed, run
  `802d97a8-0aaf-4783-a93c-69d63a653ac6`, covering final formatting, continuation and repetition policies; later vendor
  edits changed standalone test assertions and documentation only.
- Both vendor patches were regenerated from checksum-verified published crates, applied to fresh unpacks, and compared
  recursively with the vendored sources: no differences. Numeric patch-hunk citations were refreshed. No warning was
  emitted by the final vendor commands.
- `cargo nextest run -p farhelm-terminal-emulator`: 118 passed, 19 intentionally ignored/manual tests excluded, run
  `90ec10a1-e13d-4536-9c10-4a82d5332f00`. This includes old-style hand-built snapshots, immutable golden bytes,
  generated boundary/round-trip/delegation checks, the changed-width regressions and screen capture.
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(emulator_parity::) | test(/^grid_output::/) | test(/^service::grid_stream::/)'`:
  61 passed, run `2da0c533-b34e-4222-bd31-c24be65966d5`, covering live tmux comparisons, documented differences and grid
  streams. The exact known-differences retry passed first in `e3f91fc5-13ee-4cde-b555-ae19ea5b581f`.
- With `FARHELM_E2E_SUITE_TERMINALS=host` explicitly retained,
  `cd e2e && npx playwright test 'terminal-unicode\.spec\.ts' --grep 'a narrowed text-presentation glyph|tmux reconnect replay'`:
  four passed, two per engine, run `05ea3719-bd78-4243-b8ed-216c93652669`. Chromium and WebKit used the configured three
  worker slots and three owned stacks; this selection used two workers. The suite established the host-terminal premise
  and completed cleanup. Checks cover replayed rows/cursor and adjacent one-cell DOM boxes for a narrowed glyph and
  colored following character; they are geometry checks, not a color-emoji-font assessment.
- `cargo build -p farhelm` passed at the final emoji implementation before the runtime-policy rebase below. The
  successful Dioxus web build from the colored-mark PR at `a67d4bec` was reused: the intervening UI diff contains
  mark-color CSS, comments and contrast tests, with no terminal assets, layout, theme or Rust-rendered terminal change.
  The browser run used that web build and the refreshed binary; its result is reused across the later rebase because the
  landed recovery/stop policies change neither the attached terminal's input/painting nor the viewer's reconnect path.
- `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-terminal-emulator -p farhelm-supervisor --all-targets -- -D warnings`, selected Markdown/TOML
  `dprint check`, changelog format and source whitespace checks passed. Clippy evidence is reused from before the final
  rebase: the emoji diff is identical afterwards, and narrow integration tests compile the combined supervisor. The
  isolated interpreter's `-B scripts/check-test-sleeps.py` inspected 397 delays with zero missing reasons. The
  instruction checker passed with zero errors and three pre-existing citation warnings. Generated patch context is
  verified by fresh apply/comparison, rather than treating its blank context lines as source whitespace.

Earlier retained development failures are not pass evidence: emulator runs `a49d61d7-731d-40e3-83d1-e6b19f4cff57` and
`345bab0d-f424-4145-b67d-284f362f5682` exposed incorrect new assertions and legacy golden-script repetition states;
parity run `0a95732f-13e6-4d3e-938d-6bfbf0620cd3` exposed two tentative tmux expectations corrected from live results;
vendor run `e56bdfd8-2808-4449-9b31-54863d60bbaf` exposed a stale pending-wrap assertion and an incorrect scalar-width
fixture premise. Each was retained before its focused retry. These were same-session corrections, not latent flakes.

Main through `25f85df8` was audited and the change rebased cleanly, with an empty interdiff. It adds recovery retries
for agent/tab terminals and shutdown checkpoint scheduling that skips small tails. A separate integration reader read
the entire landed diff; the executor inspected its production and specification changes. Recovery still completes
snapshot restore before journal replay, and shutdown admission counts bytes rather than cell widths. No functional
conflict was found. The following focused integration check supplements the unchanged emoji-specific evidence:

`cargo nextest run -p farhelm-supervisor --lib -E 'test(a_snapshot_record_is_restored_once) | test(a_recovered_adoption_answers_only_what_no_supervisor_saw) | test(checkpoint_now_takes_a_clean_snapshot_or_keeps_the_previous) | test(checkpoint_now_skips_small_tails_and_encodes_at_the_threshold) | test(readoption_replays_an_exited_runtime_without_recording_twice)'`:
five passed, run `ba752e30-44e2-41f1-91e4-3cfd169a2e49`, with no tmux required. These ASCII fixtures prove the combined
restore/replay/checkpoint composition; the emoji-specific behavior is established by the earlier focused tests.

Formatting and source whitespace checks passed again after the rebase. Other command results are reused per their
unchanged coverage above; subsequent documentation corrections change no tested behavior.

The ordinary xterm.js run was skipped as explicitly out of scope. Grid-client/JS suites were skipped because no grid
asset or fixture changed. Full-workspace, native desktop, installer, provisioning, ignored performance/private-recording
suites and hosted CI were skipped because they add no identified coverage for these input and snapshot changes beyond
the focused checks. The docs edit is ordinary Markdown, so formatting and source review suffice without a site build.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found two product issues: widening at the right
edge must retain a pending wrap even while wrapping is disabled, and ignored formatting inputs must not block a later
selector. Both were fixed. The same reviewer checked the bounded continuation design and corrected test fixtures, then
passed the gate with no findings. Reviews were static; runtime evidence is above. A separate documentation pass covered
all 25 touched files and resolved three contract/record wording issues. Commit and PR wording passed fresh-context cold
reads. The fresh report reader passed this report for product context and public-repository hygiene before delivery.
