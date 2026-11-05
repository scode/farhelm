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

### Landing

Landed with no fixes at landing. New terminals no longer delete a vowel or trailing sign that cannot attach to the
character before it (as in ordinary Kirat Rai text): it prints like any other character, and an emoji selector right
after it is ignored, as you chose on 2026-11-05. The specification and the "Move sessions to new terminals" guide now
also say which invisible formatting characters new terminals drop.

- #3029: the specification and guide text on dropped invisible formatting characters.
- #3045: the printing change in the vendored terminal code, mirrored in its patch file and records, with its tests (335
  counted lines, under the 400-line ceiling you allowed).

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained round 176's four plans (terminal-host race records, durable permission revocations,
Delete's checks of unreadable and surviving tabs, and the template approval card and attachment-log fixes) and two
browser-test fixes (#3042, #3043). None touches the vendored terminal code or the terminal emulator; round 176's
specification edits sit in other sections. In this round it landed first, below host-attention-list, which changes only
the UI and hosts documentation. A fresh-context review read them against each other and found no interaction. It also
unpacked the published alacritty_terminal crate, applied the patch file to it and got exactly the vendored tree, so the
patch and the tree agree.

#### Things you should know

- The "Move sessions to new terminals" guide still says that two edge cases differ from Ghostty. Keeping these signs is
  now a third, deliberate difference: Ghostty's default mode discards them. Adding a sentence there is a small docs
  follow-up.
- A Kirat Rai vowel sign right after another one attaches to it, because the existing pairing rules allow that, so the
  two share one column where a program may count two. Before this change both disappeared.

#### Checks

This round landed two plans, stacked in queue order on main `5065e022`: terminal-text-drops, then host-attention-list
with the landing's fix. The checks ran once, on the whole stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 412 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- The vendored terminal crates' own tests, which the workspace run does not reach:
  `cargo test --manifest-path vendor/vte/Cargo.toml --features ansi`, run `82853607-026d-4030-830f-aec8413ba56e`: all 62
  passed; `cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml`, run
  `931aa4d3-f93b-41dc-922e-d6d1aa568b27`: all 165 passed;
  `cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml`, run `bb116fd9-9df3-4b0b-b567-f33693c4f92b`: all 3
  passed.
- The whole Rust workspace with the pinned tmux, `cargo nextest run --workspace --exclude farhelm-desktop`, which
  includes the terminal emulator's snapshot tests and its parity tests against tmux, run
  `3ae3f8ee-2143-4c6e-84bf-e5b9ac48362a`: 5608 passed, 70 skipped (ignored by design), 1 failed. The failure is
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, a recorded flake that sweeps already exclude.
  The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `a7bf68d7-e73f-402c-9ac1-a43bfb6ea92b`: all 862 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `5fc3d8d7-98f6-4625-b09b-b9d26d8fd812`: all 1051 passed. The UI's JavaScript tests, including the check that the
  stylesheet uses only design tokens (`node --test` in `crates/farhelm-ui/js-tests`), run
  `43f3b957-e103-4be3-a85e-2b589d7ffd6e`: all 516 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack), runs `1575baf5-5085-499e-a5a6-c00ff6ba569d`, `4ee46d8a-93af-4c23-8707-74304f887034`,
  `8438552a-ef03-4b89-aa29-333460b7f03c`, `7ce4c8ee-3e11-4c6e-b145-a8a1c3bf4995`,
  `bb8b35cb-8801-4859-a7a3-2170b2df5799`, `6d40c18f-2965-466d-98d4-6d6e0e71d877`,
  `bd11c081-e09d-4e81-b13f-107736cca84f`, `f97c8a59-f1f0-47a2-8f9a-40e55d13606e`,
  `a6827ea7-6fcd-40ad-aed7-ff5261ae37a8`, `164950b5-a9c0-497b-94e8-38cd1d789a0a`: 3006 passed, 18 skipped by the specs'
  own design, 2 failed. Then the same suite with every stack's terminals in terminal hosts, since host terminals render
  through the changed terminal emulator (`FARHELM_E2E_SUITE_TERMINALS=host`; this run covers only the specs listed for
  host terminals in `e2e/host-terminal-specs.ts`, so it holds fewer tests), runs `dd333943-4496-4b5d-86c5-f53574292f10`,
  `bf84db92-1302-4a47-a4a9-1e79f8322fe4`, `1a9080ad-2ab6-47c2-a1c6-0d0b74aa88ad`,
  `72f532ec-617f-43e9-97b5-f175663add10`, `ffd70aab-38f9-4fa4-a7e8-af8fce9bced0`,
  `cdc217a1-c48a-46af-846f-3ab6c4dbd8cf`, `155ec254-5415-4214-896d-ba2c3174d224`,
  `f5dc179b-e410-46b0-b8fb-cac944c6c12c`, `16ab6c47-8c70-4a37-ab2c-4b0cde0caeb9`,
  `1908e893-140c-4e6e-a665-0d18520bd29e`: 2082 passed, 75 skipped by the specs' own design, 3 failed. Of the five
  failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `1575baf5`,
    `f97c8a59`, `72f532ec`, `f5dc179b`), each time naming only the Add host form's "SSH" and "Tensorlake sandbox"
    choices, which have no hover text. Those came with the Tensorlake sandbox hosts (#2973), on main before both plans'
    bases, and the two previous rounds saw the same failures. It is a break on main and a follow-up there; nothing in
    TODO.md or the review queue records it yet.
  - "Trash header and global confirmation remain reachable at width 360" (Chromium, run `72f532ec`), a recorded flake.
- Not run: the doctests (no doc examples changed), the grid component's browser suite (no grid code changed), the
  desktop smoke tests, the installer tests, the development screenshot review (no shot changed), the docs website
  preview (the build above passed; no page was viewed) and the native Mac app (not runnable on Linux).

The executor's report calls the PRs drafts; what landed is its commits rebased onto current main, with their content
unchanged; only the surrounding lines of the review queue's index differ, because main removed neighboring entries.
Nothing else in it became untrue at landing.
