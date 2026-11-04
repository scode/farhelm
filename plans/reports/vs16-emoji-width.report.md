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

### Landing

Landed with one test fix at landing, first of the three plans landed together in this round (with launch-progress and
session-summary-beside-list, stacked above it). In new terminals, an emoji such as ❤ followed by the emoji variation
selector (❤️) now takes two columns, and the text variation selector narrows where Ghostty narrows, following Ghostty's
default rules except for the two exceptions the maintainer approved on 2026-11-04 (pinned in tests and described in the
report).

- #2971: the width rules in the vendored terminal parser and emulator (with their patch files), the snapshot handling,
  the parity tests against tmux, the Unicode data notice, the specification, a docs page and the removed TODO entry.

#### What the landing fixed, and why

Since this plan was based, faster-snapshot-encoder landed with new frozen snapshot fixtures whose test scripts feed
generated output through a test step this plan had removed (it replaced that step with two that rebuild old repeat
states). The rebase applied to that test file without a textual conflict, but the combined code did not compile. The
landing kept the plan's two new steps and restored the removed one beside them, as a live feed; those scripts print only
titles and plain text, which the new width rules do not change, and the fixtures' bytes held (the checks below). No
product code changed. The executor's report says the rebase was clean; that is no longer true of what landed.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained faster-snapshot-encoder (a faster snapshot encoder with frozen byte fixtures),
colored-harness-marks, discord-link-in-app, grid-drop-and-drag-notice, and PRs that only added TODO entries and plans. A
fresh-context review read this plan against the new encoder: no frozen fixture, golden script or benchmark contains a
variation selector, the encoder still records any cell a selector widened or narrowed as a correction to the replayed
output, its reuse paths depend on titles, trimming and size but never on widths, and saved snapshots still replay under
the old rules before live output resumes under the new ones. It also confirmed that applying the patch files to the
published crates reproduces both vendored trees exactly. The rebase conflicted only in one implementation-notes sentence
listing what grid terminals lack: main had already shortened it when grid-drop-and-drag-notice landed, and this plan
removed the emoji-width sentence beside it; the landing kept main's shortened sentence without the emoji one.

#### Things you should know

- New terminals now drop zero-width spaces, bidirectional marks, the byte-order mark and two Hangul fillers entirely, so
  copied text loses them. The vendor notes understate this, and neither the implementation notes, the changelog nor the
  tmux parity tests describe it.
- The vendored parser's patch includes two unmarked formatting edits to upstream code (a moved import and a reflowed
  comment), against the vendor notes' request for small, marked patches; reverting them is a follow-up.
- The vendor notes' file table does not list the new Unicode license file, and their upgrade checklist does not name the
  parser's new repeat hook.

#### Checks

This round landed vs16-emoji-width, launch-progress and session-summary-beside-list, stacked in that order on main
`5c5af756`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 403 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older reports), the docs website build (`bun run build` in `website/`), the workspace debug build and
  the UI's release web build: all passed. These ran before launch-progress's landing fix, a stylesheet value that
  computes to the same style; the builds were redone with it for the JavaScript run `c5047ec6`, the trash dialog runs
  `d30273b9` and `03eda759`, and launch-progress's own specs below.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `b14ee80d-08f8-4500-8471-e106fd12b13d`: all 842 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `b4981abf-b680-4336-8ed1-c1b993d4f947`: all 1031 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `83ad6a8c-ae24-43a4-a2a5-be982d905cb0`: 515 passed, 1 failed (the stylesheet test
  launch-progress's landing fix addresses); with the fix, run `c5047ec6-c334-493a-b9d8-4ad22ecd5b84`: all 516 passed.
- The vendored crates (vs16-emoji-width patches them): `cargo test` of vendored vte with `--features ansi`, run
  `c1feddd1-8351-46cd-a4dd-fe2a3c2c34d6` (62 passed), of vendored alacritty_terminal, run
  `ac66eb6f-33df-414f-8023-8e6f82d808ac` (165 passed), and of the paired tests, run
  `11056c7f-5f7e-41fb-8052-54cb6fdb7f15` (3 passed). The terminal emulator's ignored round-trip and hostile-snapshot
  tests in release (`cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E '...'`, the
  full-size round trips, the 300,000 short scripts and the 230,000 hostile blobs), which their documentation asks for
  when the decoder or the vendored patches change, run `c957bc92-29df-4bec-9178-06713631da5c`: all 10 passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux first on `PATH` (so the emulator's parity tests against tmux ran), no retries, on a machine with rootless Podman,
  run `68783337-311c-45d3-b891-a1dfc3eb9f29`: 5428 passed, 70 left out by the test runner (tests marked ignored), 3
  failed: two restart-gap session-listing tests and an OMP report test, which have failed the same way on main since
  round 169 found them (stop-checkpoint-burst's landing notes name them), in code none of this round's plans changes.
  They are recorded nowhere yet; the maintainer was told in conversation, and after three rounds finding their cause, or
  at least recording them, is a follow-up.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack before launch-progress's landing fix; Node 24.16 there, against 24.20 locally; the sandboxes' ssh to
  themselves worked, with a fix to their setup that lands separately), runs `268ae178-8cf1-49e9-bc0e-0185f774a4f7`,
  `fc0161c2-634f-412d-91b3-edae353e7a45`, `e57f9fe2-36de-43f7-aade-46eadb89f6ee`,
  `a865423e-2157-473d-bd78-5bafcce7811e`, `85787359-a26b-4ba5-886b-5db26f29c782`,
  `d09a94d4-9ba3-4ffe-852d-a1f9e540e9bc`, `0a181417-63c4-4824-a8cb-6346a34bc6c2`,
  `905592e5-268b-477d-807f-d8ea2dd8225f`, `f88dbc3b-0bd0-42bb-b708-9d04a42e31a1`,
  `0a8e20e1-eaac-43c9-a9d9-b7a27ee49d3b`: 2891 passed, 18 skipped by design, 3 failed. Then the browser suite's
  host-terminal run (every stack's terminals in terminal hosts, `FARHELM_E2E_SUITE_TERMINALS=host`) the same way, runs
  `7f6083bf-d51d-442c-88e1-5b70d51e3309`, `917531e9-bc85-48de-87c7-d92f0a5b42b4`,
  `89a737f6-3967-42aa-a519-a90238061970`, `76be9ed9-983d-442a-a852-38bfdfad7051`,
  `cda6b941-c943-4e8e-b96d-294bf94ae240`, `d0d688aa-0374-4cd4-b816-88b95b5b3234`,
  `0a773593-de91-4798-898a-bfd970f183ed`, `60cb802f-838e-4d99-95fd-b2642908dae4`,
  `57d262a1-fe43-4398-8ff0-53f5c9acbd4c`, `a4c9e736-ed83-48cc-8198-6b43fa19b22f`: 2050 passed, 75 skipped by the specs
  themselves on terminal hosts, 5 failed.
- Each failure re-ran three times on the same build, also on sandboxes (runs `de7f9716-ee9c-48b5-8577-9b97467bd32f`,
  `e6e3b26c-8418-4e4a-9cfc-c3c976c4c8bf`, and in the host-terminal run `df17440f-34ee-4932-89b3-6519ae4ef4b5`,
  `e049551b-554f-4faa-a60a-076f30d9358e`, `32c0bae4-b4da-457c-b221-e9111ffd593f`,
  `5988d115-8b2b-45b5-a1f1-6570ec14cc8c`, `816213fe-60b3-494f-b8f3-2316945399da`). Passed 3 of 3, each a single failure
  not recorded as a flake: "create-intent-key-rebinds-on-host-change" (WebKit, multi-host: a radio button did not take
  its value), "an old session's band hides until the next visit" (Chromium, a test route's response was already
  disposed), "an idle session already seen uses its row's status foreground" (WebKit; its status word came out white),
  and "header controls stay interactive and text stays selectable" (Chromium; double-clicking the version selected
  nothing). "Post-clone commands with carriage returns refuse edits but can be cleared" (WebKit) is recorded as a flake
  by an open draft PR (#2549) and failed once more in the three; that evidence goes to its record when it lands.
- "Trash header and global confirmation remain reachable" (Chromium; after PageDown from the Delete all confirmation's
  Cancel, its scroll area stays at the top) failed at both widths in the full suite and at width 360 in the
  host-terminal run, then once in nine re-runs (six over both widths, run `e6e3b26c`, one failure at width 1000; three
  at width 360 in the host-terminal run, `32c0bae4`, all passed). Twenty runs each on sandboxes (ten per width): main
  `5c5af756` without this round's plans passed all (run `7ce0fa35-3abc-400d-ab8d-5fc457bb0731`); main with
  vs16-emoji-width and launch-progress failed 2, both at width 1000 (run `d30273b9-fc29-4e19-b4c0-036465123b34`); the
  whole stack with launch-progress's landing fix failed 3, all at width 1000 (run
  `03eda759-efdb-41ff-956b-564f134ad83d`). Those counts point at launch-progress without settling it: in the previous
  round, on a stack without launch-progress whose code is now main, the same check failed once in a valid pass (width
  360; that round's notes called it a cleanup failure, which came after this check failed) and three more times in a
  pass whose ssh-reached second host never connected, which that round discarded. So the test can fail without
  launch-progress, but whether launch-progress makes it fail more often is not shown either way, and its cause is
  unknown. The landing judged it not a reason to hold the plan; the test is being recorded as a latent flake with these
  counts after this round, and checking whether launch-progress's waiting status (its once-a-second clock or its pill)
  affects the confirmation's scrolling is a follow-up.
- launch-progress's own specs after its landing fix, on both engines and in both the ordinary and host-terminal runs
  (runs `a7cfb2f2-53ee-462f-8aae-14a6de7f04d1`, `b28405ed-9d16-47c2-a2ce-ee1c8d1c0187`,
  `215b38cf-56ba-4c89-ad57-cce8bd5d3390`, `4d14e3a3-6f3e-477c-9ab6-f3cc5080787e`): all 56 passed.
- Reused: nothing. Not run: the doctests (no doc examples changed), the desktop smoke tests (no desktop-specific code
  changed) and the native Mac app (not runnable on Linux).

The executor's report names its commit before the landing's rebase and test fix, and calls the PR a draft; what landed
is that commit rebased, with the restored test step. Nothing else in it became untrue at landing.
