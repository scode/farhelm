# Wide-glyph padding cells in terminal rows

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers two TODO.md entries (bucket "Near term", subsection "Plan follow-up") and one bullet of a third entry
(bucket "Near term"). Verbatim:

> **Keep the terminal's colored margin when a wide glyph wraps.** A wide glyph printed in the last column wraps to the
> next row. The emulator leaves a styled blank in the old column, but the grid encoder skips the wrap placeholder and
> the viewer paints the default background there. Carry that cell's visible style through screen and history. Give the
> viewer a way to distinguish the placeholder from a space the program actually printed. Sending an ordinary styled
> space makes selection, printed-path detection and hyperlink joining treat the padding as text. The current client
> infers padding from an uncovered last column before a wide glyph. The row format has only a wrap bit and styled runs,
> so settle how to carry that distinction across versions before changing the encoder. Cover both a colored placeholder
> and a real styled space before a wrapped wide glyph. Plan: `plans/queue/wide-glyph-padding-cells.md`.

> **Show the new background after erasing half of a wide terminal glyph.** If a program erases just the right column of
> a wide character using a different background, the grid encoder sends the old two-column run and hides the erased
> cell's style. Emit a one-column glyph and a separate erased cell when the continuation spacer is gone, on screen and
> in scrollback. Also preserve the previous row's wide-wrap padding when the half-erased glyph starts a wrapped row. The
> viewer currently recognizes that padding by the next run occupying two columns. With separate cells it needs the same
> explicit wrap distinction as the colored-margin item above. Cover erasure both mid-row and just after a wrap,
> including selection and links across the seam. Plan: `plans/queue/wide-glyph-padding-cells.md`.

From **Consolidate the helpers duplicated across Farhelm.**, this bullet only (the entry's other bullets belong to other
plans):

> - `terminal-grid-row-traversal-duplicated.md`. In
>   `crates/gui/farhelm-grid-model/src/{paths.rs:335–383,selection.rs:241–281}`, both row walks fill gaps, split plain
>   runs, keep other runs' widths and drop wide-wrap padding. Share a walker yielding column, width and text. Keep byte
>   offsets for paths and UTF-16 offsets for selection with the callers. Plan:
>   `plans/queue/wide-glyph-padding-cells.md`.

### Where it stands (from planning research at main b34ceee9f)

Background: when a program prints a wide character (CJK, most emoji) with one column left on the row, the emulator
leaves that last column empty, wraps, and prints the character at the start of the next row. The vendored emulator
writes that empty cell with the current pen (`vendor/alacritty_terminal/src/term/mod.rs`, about lines 1575-1590, flag
`LEADING_WIDE_CHAR_SPACER`; it carries foreground, background, style flags and any hyperlink), so a real terminal shows
a colored background running to the right edge. Call that cell the wide-wrap padding.

- **The encoder drops the padding.** `row_from_cells` (`crates/farhelm-supervisor/src/grid_output.rs`, about 143-213)
  skips both `WIDE_CHAR_SPACER` and `LEADING_WIDE_CHAR_SPACER` cells before reading their style (about 153-159), so the
  viewer paints the default background there. The same function encodes screen rows (about line 336) and history rows
  (about line 688), and screen diffs compare `CellRow` values (about line 413), so a new `CellRow` field takes part in
  diffing automatically. `CellRow` is at about 119-132; `to_wire` (about 797-815) turns it into the wire `Row`.
- **The encoder always sends a wide character two columns wide** (about 195-208) and skips the next cell, without
  checking that it is still the character's spacer. The vendored ECH (``erase_chars``, about 2002-2018) replaces just
  the erased cells with blanks in the current background (``bg.into()``, a space with no flags) and leaves the head's
  ``WIDE_CHAR`` flag in place. DCH (``delete_chars``, about 2021-2047), ICH (``insert_blank``, about 1670-1695) and
  insert-mode printing move whole cells with ``row.swap``, flags included, with no wide-character fixups. So DCH can
  shift an ordinary printed character into the cell after a wide head, and a DCH followed by an ICH can leave a wide
  head at ``cols-2`` directly before the padding cell (with its ``WRAPLINE``) at ``cols-1``. Today's width rule (`wide
  && col
  - 1 < cols`) would send that head two columns wide, over the padding.
- **The row format.** `crates/farhelm-proto/src/grid.rs` module docs, "Rows and runs" (about 110-160) and "Binary
  layout" (about 71-97): a binary row body is `u8 wraps (0/1)`, `u16` run count, then runs; run metadata has no spare
  bit (low twelve bits style, bits 12-15 optional-field markers). "Binary messages have no tolerated extensions". `Row`
  is at about 857-877 (JSON form `{"w":bool,"r":[...]}`), its `validate` at about 916-929. The codec is
  `crates/farhelm-proto/src/grid/binary.rs`: encode at about line 123, decode refuses wraps other than 0/1 at about
  286-289. `crates/farhelm-proto/src/grid_tests.rs` pins `2` as invalid (about line 632) and regenerates binary fixtures
  with the ignored `write_binary_grid_fixtures` (about 942-958) into `crates/farhelm-proto/grid-fixtures/`. A screen
  row's width is not known where rows are decoded: `GridOutput::validate` checks screen rows only against the largest
  dimension, and the stream check compares each pending row's right edge with the committing frame's `cols` (`grid.rs`,
  about 1286-1289 and 1411).
- **The viewer guesses.** `pads_wide_wrap` (`crates/gui/farhelm-grid-model/src/selection.rs`, about 357-371) treats the
  last column as padding when the row wraps, the last column is uncovered, and the next row starts with a two-column
  non-ASCII run at column 0. Callers: `word_cells` (about line 280), `selection_text` (about line 344), `read_cells` in
  `paths.rs` (about line 380), and `previous_piece` in `links.rs` (about 184-208). The row-link identity walk in
  `links.rs` (about 228-252) continues a row's first run from `above.last()`, the key of the previous row's last run.
  `GridRow` (`crates/gui/farhelm-grid-model/src/lib.rs`, about 85-150) holds `wraps` and `runs`; `GridRow::text` is the
  copy primitive.
- **The guess breaks both ways.** A styled padding run covers the last column, so the guess fails and the padding
  becomes a copied space, a printed path or URL breaks at the wrap, and a link split by the wrap stops joining. A
  half-erased glyph at the start of a wrapped row becomes a one-column run, so the guess fails on the previous row. And
  a real bug exists today: a program that fills a row ending in a real default-colored space, then prints a wide
  character that wraps normally, loses that space on copy, because it is indistinguishable from padding.
- **The duplicated walkers.** `read_cells` (`paths.rs`, about 335-383) and `word_cells` (`selection.rs`, about 241-281)
  both fill gaps with blanks, split plain-flow runs per cell, keep other runs whole and drop the padding. `word_cells`
  also blanks concealed runs when asked; `read_cells` does not.
- **The painter needs no change.** It fills each run's box with the run's background and clips non-ASCII text to its own
  box (`crates/gui/farhelm-gui/src/terminal.rs`, about 1215-1245), so a one-column run holding a wide glyph shows its
  left half. The encoder already relies on that for a wide character in the last column.
- **Pinned link oracle.** `crates/gui/farhelm-grid-model/tests/fixtures/links-oracle.json` holds expectations computed
  by the old browser client's JavaScript, replayed by `pinned_cell_links_and_label_join_oracle`
  (`crates/gui/farhelm-grid-model/tests/links.rs`, about 200-236). Two of its grids rely on the inferred padding (a
  wrapping row with an uncovered last column, then an emoji at column 0). Its generator
  (`crates/gui/farhelm-grid-model/tools/generate-links.ts`) reads JavaScript sources that are no longer in the
  repository, so it cannot be rerun.
- **What stays untouched.** The terminal host journals raw bytes and stores an opaque snapshot (`crates/terminal-host/`,
  its SPEC.md "Change policy": its protocol only grows and every client must work with every host ever shipped). The
  snapshot format v1 (`crates/farhelm-terminal-emulator/src/snapshot/v1/`; SPEC_impl.md "Terminal snapshots": every
  snapshot format stays readable forever) already stores all three wide flags (`wire.rs`, about 30-45) and restores a
  padding cell with its pen and a half-erased pair (`paint.rs`, about 560-630). A restored emulator holds exactly the
  cells the new encoder reads, so open sessions show the fix after the supervisor upgrades, with no restart.
- **Protocol rules.** `PROTOCOL_VERSION` is 83 (`crates/farhelm-proto/src/lib.rs`, about 215-270) and must match exactly
  between helm and supervisor; a change to what the encoder emits that a same-version viewer would misread "is not
  additive" and earns a bump. The pinning test is `protocol_version_is_pinned_at_83` (about line 6039), renamed at every
  bump. SPEC_impl.md "Grid streams" (about 2749-2820) says the grid format changes with ordinary bumps, and describes
  the binary row ("a byte length, a wrap bit and runs"). The desktop app embeds its helm, so the GUI edge has no hello.
  Another queued plan (`recipe-credentials-in-launch-env.md`) also bumps; a concurrent bump is a rebase on the constant
  and the pinning test's name.
- SPEC.md "Terminal experience" (about line 3236) excludes "wide-character edge cases" from full fidelity; the
  maintainer chose to fix these two anyway, within the fences below. No SPEC.md change is needed.

### Acceptance criteria

1. The row's flags byte (today `u8 wraps`) carries bit 0 "wraps" and bit 1 "the last column is wide-wrap padding". The
   decoder refuses bit 1 without bit 0, and any of bits 2-7. It checks nothing that needs the row's width (the shape of
   a run covering the padding column, or a minimum row width): a screen row's width is known only at the stream check,
   and growing that check is outside the fences. The encoder's own tests pin the padding run's shape instead. The JSON
   fixture form carries the bit as an optional `"p"` (absent means false; written only when true, so existing fixtures
   do not change). `farhelm-proto`'s tests and fixtures cover the accepted and refused shapes, and the invalid-wraps
   case is updated to the new rule.
2. The supervisor's encoder sets bit 1 exactly when the row wraps and its last cell is the emulator's
   `LEADING_WIDE_CHAR_SPACER`, for screen and history rows alike. A wide character's run never covers a
   `LEADING_WIDE_CHAR_SPACER` cell: a wide head directly before one is sent one column wide.
3. When that padding cell's style is not the default, the encoder sends it as its own one-column `" "` run at `cols-1`
   with the cell's style and no link, never merged into a neighboring run and never trimmed. The desktop app then paints
   the program's background through to the right edge.
4. The desktop app's text consumers read the bit instead of guessing: copy, word and URL selection, printed-path
   detection and hyperlink joining across a soft wrap all drop column `cols-1` when the bit is set, whatever covers it,
   and keep it otherwise. A real space before a wrapped wide glyph, default or styled, is kept on copy. The inference in
   `pads_wide_wrap` is gone.
5. A wide character is sent two columns wide when the next cell is still its spacer. When the spacer is gone, it is sent
   two columns wide only when the next cell is a plain blank with the glyph's background, where a plain blank is defined
   on the emulator's cell, not the wire style: the character `' '`, no combining characters, no hyperlink, and no cell
   flags other than `WRAPLINE`. That is exactly what an erase leaves, so the maintainer's "keep width 2 when the
   backgrounds are equal" holds for every erase. In every other case (a different background, a printed character that
   DCH shifted there, the padding cell itself) the encoder sends a one-column glyph run (the desktop app shows its left
   half) and the next cell as its own run, or as nothing when that cell is padding or a default blank. This is the
   planner's reading of the maintainer's decision, kept because a printed character would otherwise stay hidden and
   because it doubles as acceptance criterion 2's guard. Screen and history alike. Backgrounds compare as the wire style
   gives them (`Style::bg`); a glyph with the inverse flag is an edge case to accept, not to special-case.
6. The two grid-model row walkers (`read_cells`, `word_cells`) share one walker yielding column, width and text, with
   byte offsets kept in the paths caller and UTF-16 offsets in the selection caller, in a behavior-preserving first PR.
7. `PROTOCOL_VERSION` is bumped once, in the PR that changes the row format, with its pinning test renamed, the
   `grid.rs` docs ("Rows and runs", "Binary layout"), the encoder's docs in `grid_output.rs`, and SPEC_impl.md "Grid
   streams" updated to describe the flags byte, the padding run, and why the change is a bump.
8. Tests (beyond keeping the existing ones passing, updated to state the padding bit where their rows relied on the
   guess): copy, word and URL selection, printed paths and link joining across a wide wrap; history at a width other
   than the screen's; a colored placeholder versus a real styled space before a wrapped wide glyph; a real default space
   before a wrapped wide glyph; a half-erase mid-row and just after a wrap (the previous row's padding still dropped
   from copy, links and selection still joining across the seam); a same-background half-erase still sent width 2; and
   one encoder test in which DCH and ICH leave a wide head directly before the padding cell, checking that the row
   encodes with the head one column wide and the padding bit set.
9. Golden scenes show a colored wrapped wide-character row and a half-erased wide glyph, and pass the visual review.
10. Nothing changes under `crates/terminal-host/`, `crates/farhelm-terminal-emulator/src/snapshot/` or
    `vendor/alacritty_terminal/`.
11. The two TODO entries are removed, and the `terminal-grid-row-traversal-duplicated.md` bullet is removed from
    "Consolidate the helpers duplicated across Farhelm." (the whole entry goes if no bullets remain).

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Fix both items, within fences.** One new idea only: a "this last cell is wide-wrap padding" bit in the row's
  existing wraps byte (bit 0 wraps, bit 1 padding), set by the supervisor's grid encoder from the emulator's
  `LEADING_WIDE_CHAR_SPACER`; the padding cell's style, when not default, travels as an ordinary one-column space run at
  `cols-1` that is never merged; the desktop app's copy/selection, printed-path and link-joining code reads the bit
  instead of guessing (`pads_wide_wrap`). No change under `crates/terminal-host/`, no change to the snapshot format
  (`crates/farhelm-terminal-emulator/src/snapshot/`), no new mechanism beyond the flag; if the work needs more, block
  rather than grow. The maintainer's words: "I don't want this to turn into some hideous complexity, but it's fine if
  it's more than a few tens of lines. Like it can be a reasonable amount of complexity, but I don't want this to
  explode."
- **Protocol bump accepted**, as an ordinary upgrade: remote hosts show Needs update until they are updated. The report
  must call out the bump so the release can test it. No remote-hosts entry goes in public notes (or in a fragment).
- **Half-erased wide glyph: "draw the left half plus the erased cell, only when the color differs".** The encoder sends
  width 2 when the next cell is still the glyph's spacer; when the spacer is gone and the erased cell's background
  differs from the glyph's, it sends a one-column glyph run plus the erased cell as its own run; when the backgrounds
  are equal it keeps width 2 as today. The maintainer was shown Ghostty's behavior (it widens an erase that starts or
  ends on half a wide character to the whole character, blanking both cells, inside the emulator) and declined it, in
  their words because "it was intended to avoid more vendor patches". Do not change the vendored emulator
  (`vendor/alacritty_terminal/`).
- **Fold in the row-walker de-duplication** as a first `refactor:` PR, since this plan rewrites the padding logic both
  walkers share. This plan removes only that bullet from the helpers entry; four sibling plans cover the rest.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

Separately from this planning page, the maintainer's standing rule applies: a plan whose PRs change what the desktop app
draws gets a mandatory Opus visual review against the agreed design (see "Visual review"). Here that covers one golden
scene.

## Requirement sources

**The maintainer's request and decisions:** the TODO entries and bullet quoted above, the decisions above, and the
standing visual-review rule.

**Binding repository constraints:**

- `crates/farhelm-proto/src/lib.rs`, the `PROTOCOL_VERSION` doc comment (bump rules, the pinning test, where a bump's
  reason is recorded), and `crates/farhelm-proto/src/grid.rs` module docs (strict binary decoding, no tolerated
  extensions).
- SPEC_impl.md "Grid streams" (format changes ride ordinary bumps) and "Terminal snapshots" (permanent format),
  `crates/terminal-host/SPEC.md` "Change policy", and root `AGENTS.md` "Instructions by path" for
  `crates/terminal-host/` and `vendor/`, which back the fences.
- `releasing/EDITORIAL_GUIDANCE.md`, last bullet, and `releasing/.agents/changelog.md`, "Format" (a protocol bump alone
  is reported for release testing, never a public entry).
- `.agents/test-authoring.md` for every test added or changed; the GUI goldens check (`.agents/validation.md`).
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "Code rules".

**Planner proposals:**

- **Define the whole format and the viewer's reading of it in one PR (PR 2).** The flags byte, the viewer's "drop
  `cols-1` whatever covers it", and the link-identity change land with the bump, before the encoder ever sends a padding
  run or splits a glyph. Then PRs 3 and 4 change only what the encoder sends, in shapes a PR 2 viewer already reads
  correctly, so one bump covers the plan and no same-version build misreads another.
- **The padding run carries style, never a link.** The emulator's padding cell can hold the pen's hyperlink, but today
  it is not sent at all; sending it without a link keeps link hover and joining as they are. It does carry the cell's
  whole style (an underline or inverse pen shows, as in a real terminal).
- **Link joining must skip the padding run in two places.** `previous_piece` compares the previous row's last run; with
  a covered padding run that run is the padding. And the identity walk continues from `above.last()`, the key of the
  previous row's last run, which would then be the padding's empty key. Both must look at the last run before `cols-1`
  when the bit is set. A test with a styled padding run between two halves of one hyperlink proves it.
- **One predicate for the guard and the half-erase rule.** PR 2 adds acceptance criterion 2's guard as the single
  condition it needs; PR 4's plain-blank predicate (acceptance criterion 5) already excludes the padding cell, which
  carries `LEADING_WIDE_CHAR_SPACER`, so PR 4 may fold the guard into it rather than keep two conditions.
- **Pitfalls found in planning:** (a) the encoder's ASCII run-merging would fold a styled padding space into a preceding
  run of the same style; push it as a separate run. (b) `trim_default_blanks` drops default-style spaces; a default
  padding cell is still dropped (the bit alone marks it), a styled one must survive. (c) The grid-model tests' row
  helpers and the two links-oracle grids built rows that relied on the guess; give the helpers a way to set the bit, and
  add `"p": true` by hand to those two oracle input rows (their recorded expectations were computed with the padding
  dropped, which is what the bit now says; the generator cannot be rerun). Log the oracle edit as a DECISION. (d) The
  invalid-wraps binary case writes `2`, which stays invalid (padding without wrap), but for a new reason; add a case for
  bits 2-7 too. Do not add width-dependent padding checks to the decoder or the stream check (acceptance criterion 1).
  (e) `GridRow` gains the bit as a field, so its constructors (`lib.rs`, `tests/fixtures.rs`) and the supervisor's
  `CellRow` users (`grid_output/test_client.rs`, `grid_output/tests.rs`, `examples/grid_stream_bench.rs`) need it.
- **Goldens use hand-built rows.** The GUI golden scenes feed wire rows to the grid model, not the supervisor's encoder
  (`crates/gui/farhelm-gui/src/goldens/terminal.rs`, `terminal_styles_setup` about line 86). Build the new scene's rows
  in exactly the shapes the supervisor tests pin, and register the scene in `goldens/mod.rs` and
  `docs/gpui/screenshot-scenes.json`. One scene, added in PR 3 and extended in PR 4, is enough.

## Outline

1. `plan/wide-glyph-padding-cells/01-share-row-walker`: `refactor: share the terminal row walker`. One walker in
   `crates/gui/farhelm-grid-model/src/` yielding column, width and text for a shown line (gaps as blanks, plain-flow
   runs per cell, other runs whole, padding dropped as today), used by `read_cells` and `word_cells`; byte offsets stay
   in the paths caller, UTF-16 offsets and concealed-run blanking in the selection caller (or as a walker argument, your
   call). Behavior-preserving: the existing grid-model tests pass unchanged. No fragment.
2. `plan/wide-glyph-padding-cells/02-mark-wide-wrap-padding`:
   `fix: keep a printed space before a wrapped wide character on copy`. The protocol bump; `Row` gains the padding bit
   (codec, JSON `"p"`, the bit-level refusals of acceptance criterion 1, binary fixtures regenerated and new valid and
   invalid cases); `CellRow` gains it and the encoder sets it (still sending no padding style), with acceptance
   criterion 2's guard and its DCH and ICH test; `GridRow` gains it, the walker, `selection_text` and both link-joining
   places read it, and `pads_wide_wrap`'s inference goes; docs per acceptance criterion 7. Tests: existing ones restated
   with the bit, a real default space before a wrapped wide glyph kept on copy, the encoder setting the bit on screen
   and in history (and not for a full row ending in a real space), hand-built rows with a covered padding run (copy,
   word and URL selection, printed paths, links joining across the wrap). This alone fixes the dropped real space.
   Fragment, `kind: fixed`.
3. `plan/wide-glyph-padding-cells/03-wide-wrap-background`:
   `fix: keep the background color where a wide character wraps`. The encoder sends a styled padding cell as its own
   one-column space run, screen and history; the `row_from_cells` doc comment and module docs say so. Tests through the
   real emulator: a colored placeholder versus a real styled space before a wrapped wide glyph (the bit, the runs, and
   in the grid model copy, word and URL selection, printed paths and link joining across the wrap), and history at
   another width. The golden scene with a colored wrapped wide-character row. Edits PR 2's fragment.
4. `plan/wide-glyph-padding-cells/04-half-erased-wide-glyph`:
   `fix: show the new background after erasing half a wide character`. The encoder rule in acceptance criterion 5 and
   "Planner proposals", screen and history. Tests through the real emulator: ECH of the right half with another
   background mid-row (one-column glyph run plus the erased cell's run), the same just after a wrap (the previous row
   keeps its padding bit; copy, selection and links across the seam behave as before), a same-background erase still
   width 2, and one encoder test of a printed character that DCH shifted after a wide head (drawn, not hidden). The
   golden scene gains a half-erased glyph next to a same-background one. Edits PR 2's fragment. Removes the two TODO
   entries and the helpers bullet (and the helpers entry if it has no bullets left).

Validation: `farhelm-proto` (the grid tests and the pinned-version test), the supervisor's `grid_output` tests, the
grid-model tests, and the GUI goldens check for PRs 3 and 4; compile everything that constructs the changed structs.
Consider the e2e host-terminal tests in `.agents/validation.md` once, at PR 2 or the end, since they decode real grid
output. No terminal-host tests: that crate does not change.

## Fallbacks

- The fences are hard. If a PR turns out to need a change under `crates/terminal-host/`, the snapshot format,
  `vendor/alacritty_terminal/`, or any wire idea beyond the padding bit and the padding run (a per-run marker, a new
  message, a second row flag), stop and block with the concrete reason rather than grow the plan.
- Erasing the left half of a wide glyph leaves an orphan spacer that the encoder skips (default background in that
  cell). It is outside both TODO entries; leave it, and name it in the report's follow-ups.
- If a concurrent plan bumps `PROTOCOL_VERSION` first, rebase onto it and take the next number; that is mechanical.
- If the walker refactor in PR 1 cannot stay behavior-preserving (a test changes), the difference is a bug in the
  refactor, not a fix to fold in; keep behavior and log it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-wide-glyph-padding-cells-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-wide-glyph-padding-cells-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/wide-glyph-padding-cells/01-share-row-walker`, `plan/wide-glyph-padding-cells/02-mark-wide-wrap-padding`,
  `plan/wide-glyph-padding-cells/03-wide-wrap-background`, `plan/wide-glyph-padding-cells/04-half-erased-wide-glyph`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entries this plan covers ("Keep the terminal's colored margin when a wide
  glyph wraps.", "Show the new background after erasing half of a wide terminal glyph.") and the
  `terminal-grid-row-traversal-duplicated.md` bullet of "Consolidate the helpers duplicated across Farhelm.", deleting
  that entry only if no bullets remain, as root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Other plans remove that
  entry's other bullets; on a rebase conflict there, keep both sides' removals.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section. The fragment never mentions the
  protocol bump or remote hosts.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The report calls out the protocol bump (from which number to which, in which PR) under "things you should know", so
  the release can test remote hosts against it.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Visual review

The maintainer's standing rule demands a visual review for every PR that changes what the desktop app draws (here PRs 3
and 4, over the single golden scene they add and extend; add no further scenes): a fresh-context `claude-opus-5-5` agent
at `high` effort, launched natively where your harness can (otherwise through `$scode-harness-shellout`), separate from
the reviewer above. Give it headless renders of every changed scene in both the light and the dark appearance (the GUI
golden scenes, regenerated with `python3 -B scripts/check-gui-goldens.py --update`, and the golden changes page that run
writes), the design this file describes, and the PR's diff. The agreed design: the colored background runs to the right
edge at a wide wrap; a half-erased wide glyph shows its left half and the erased cell in its new color when the colors
differ. It must be satisfied that the rendered result matches the intent of the agreed design; loop until it reports no
mismatch. A mismatch with the agreed design is binding and is fixed; any other observation it makes is an input, handled
like the reviewer's findings above. Do not write a launch command here or in the log.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, the links-oracle edit, and every review
finding you decided not to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate and,
where it changes what the app draws, its visual review, and the last removing the TODO.md entries and the helpers bullet
this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
