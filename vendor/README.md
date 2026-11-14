# Vendored crates

NOTE: Do not edit the code under `alacritty_terminal/` or `vte/` directly. Each tree is an upstream crate plus the patch
next to it in `patches/`, and the procedure below depends on that staying true. A change goes into the tree and the
patch file together.

The supervisor's terminal emulator (`crates/farhelm-terminal-emulator/`) runs on `alacritty_terminal` 0.26.0 and its
parser `vte` 0.15.0. Both are used as published on crates.io except for a few small patches: accessors that snapshot and
restore need, caps on what hostile program output can make them store, two history counters for the grid stream, one
more query for the handler to answer, nine bug fixes, emoji variation widths following Ghostty, and one mode change (the
UTF-8 and SGR mouse encodings as separate modes, which upstream, like xterm, makes exclusive). The root `Cargo.toml`
swaps these trees in for the crates.io versions with `[patch.crates-io]`.

The GPUI crates and the gpui-agent automation bridge, which the desktop GUI builds on, are vendored here too and have
their own section at the end, [GPUI](#gpui). Everything else in this file is about the two terminal crates.

## What is here

| Path                                     | What                                                                      |
| ---------------------------------------- | ------------------------------------------------------------------------- |
| `alacritty_terminal/`                    | `alacritty_terminal` 0.26.0 with `patches/alacritty_terminal-0.26.0.diff` |
| `vte/`                                   | `vte` 0.15.0 with `patches/vte-0.15.0.diff`                               |
| `patches/alacritty_terminal-0.26.0.diff` | Farhelm's changes to `alacritty_terminal`, against the published crate    |
| `patches/vte-0.15.0.diff`                | Farhelm's changes to `vte`, against the published crate                   |
| `pair-tests/`                            | Farhelm's tests of the two patched crates together (not upstream code)    |

Both trees started as the published `.crate` files, unpacked unmodified. Their SHA-256 checksums match the crates.io
index (and are what a `Cargo.lock` records for the registry crates):

- `alacritty_terminal-0.26.0.crate`: `bda177466b9524d59f1b12f0dd30b68696788e9992a7e959021c4a0ed96fcf59`, packaged from
  `alacritty/alacritty` commit `94e7c8874e526b1e67b349d9ba30ddf81669119e` (`.cargo_vcs_info.json`).
- `vte-0.15.0.crate`: `a5924018406ce0063cd67f8e008104968b74b563ee1b85dde3ed1f7cb87d3dbd`, packaged from `alacritty/vte`
  commit `3b3da71c34cc1256c7e20981cf03f8eb95e08ffc`.

Two parts of the published crates are not checked in:

- `alacritty_terminal/tests/`: upstream's reference tests, 45 MB of recorded grids as JSON. They are not needed to
  build, and that much JSON in the tree would bury every repository-wide search. The `[[test]]` entry in its
  `Cargo.toml` still names them, which is why its tests run with `--lib` (below). To run the reference suite against the
  patched tree, copy the tree somewhere outside the checkout, unpack the crate's `tests/` into the copy, and run
  `cargo test --test ref` there.
- `vte/Cargo.lock`: vte's own `.gitignore` excludes it, so it never reaches version control. Building vte on its own
  just resolves a fresh one, which stays ignored.

Licenses (`pair-tests/` is Farhelm's own code, so this is about the other two): `alacritty_terminal` is Apache-2.0 only
(its `LICENSE-APACHE`; there is no MIT option for it). `vte` is `Apache-2.0 OR MIT`, except `vte/src/ansi.rs`, which
came from `alacritty_terminal` and is Apache-2.0 only per its `SPDX-License-Identifier`. Apache-2.0 requires modified
files to say so, so every patched file starts with a `Modified for Farhelm` comment, and every patched spot is marked
`Farhelm patch`. `THIRD_PARTY_NOTICES.md` at the repository root has the entry.

## The patches

Here's what each one changes and why. The caps, bug fixes, emoji widths and mouse-encoding change affect terminal
behavior; the XTVERSION and REP hooks change it only for a handler that implements the new methods.

### Snapshot accessors

The supervisor snapshots a terminal's emulator so that a new supervisor can rebuild it, and the snapshot spike showed
that a restore needs state upstream keeps private. These are read-only, except `inactive_grid_mut()` and
`set_preceding_char()`, which a restore uses to write what its escape sequences cannot produce (for
`inactive_grid_mut()`, cells, the cursor and the saved cursor of the screen that is not showing).

- `alacritty_terminal` `Term`: `inactive_grid()` and `inactive_grid_mut()` (the primary screen and its scrollback while
  the alternate screen shows, otherwise the alternate screen), `scroll_region()`, `tab_stops()`, `active_charset()`,
  `title()`, `title_stack()` and `keyboard_mode_stacks()`.
- `vte` `Parser::is_ground()` and `Processor::is_ground()`: the parser is between sequences, in the ground state with no
  partial UTF-8 code point. Only there does a snapshot plus the following bytes rebuild the same state. It does not
  cover a pending synchronized update; check `sync_timeout()` for that too.
- `vte` `Processor::preceding_char()` and `set_preceding_char()`: the character `CSI Ps b` (REP) repeats, which survives
  every other sequence and so belongs in a snapshot.

### Caps on hostile output

One program must not be able to grow the shared supervisor's memory without bound. Upstream has no limit in the places
below. Each cap is enforced inside the crate, where the storage happens, rather than by scanning the byte stream before
it reaches the parser: a pre-scan would need a second parser to know where sequences start and end.

- OSC payload, `vte::OSC_PAYLOAD_LIMIT` = 10 MiB. vte's `std` parser buffers an OSC in a `Vec` that grows with whatever
  the program sends and keeps its capacity afterwards (the spike measured 8.3 MB of extra RSS after one 8 MB OSC 52).
  Past the limit the parser stops buffering, and when the sequence ends it drops it whole instead of dispatching a
  truncated payload, which for OSC 52 would be corrupt clipboard data. In place of the dispatch it calls
  `Perform::osc_overflow`, a method the patch adds (it does nothing by default), with only the parameters that ended
  before the limit, so no truncated data ever leaves the parser. vte's `ansi` processor uses it to end the active
  hyperlink when the dropped sequence is an OSC 8; otherwise a link made long enough for the parser to drop would leave
  the previous link on the text after it, where a slightly shorter one is rejected by the hyperlink cap below. After
  every OSC the buffer is shrunk back to 1 KiB if it grew beyond that. The value is set by OSC 52 clipboard writes,
  which carry the whole selection base64-encoded: 10 MiB carries about 7.5 MB of copied text, which is at least as much
  as a program could copy in Farhelm's terminals when the limit was set (the per-patch record below has the numbers).
  The `no_std` parser has its own fixed buffer and is unchanged.
- Zero-width characters per cell, `alacritty_terminal::term::cell::MAX_ZEROWIDTH_CHARS` = 16. Upstream appends every
  combining character to the previous cell, four bytes each, without limit. Further ones are dropped. Real text needs
  far fewer: stacked diacritics and Hangul jamo use two or three per cell, a subdivision flag such as Scotland's six.
  For comparison, tmux stores at most 32 bytes of UTF-8 per cell, which in practice is fewer marks than this, so 16
  leaves real text room to spare.
- Hyperlink length, `alacritty_terminal::term::MAX_HYPERLINK_URI_BYTES` = 2083 and `MAX_HYPERLINK_ID_BYTES` = 250. Every
  cell printed while an OSC 8 hyperlink is active holds it, and a program can start a new one before every character, so
  the length multiplies into the grid's memory. An OSC 8 over either limit ends the active hyperlink, and the text after
  it is unlinked. These are the limits VTE uses, as the
  [OSC 8 specification](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda) documents (iTerm2 uses the
  same URI limit). Separately, an OSC 8 that repeats the active link's explicit `id` and URI keeps the active link
  instead of building an equal copy, so the cells after it share one allocation. A link without an `id` always gets a
  fresh generated one, so it is never equal to the active link and is built as before.
- Cell extra storage per terminal, `alacritty_terminal::term::MAX_EXTRA_STORAGE_BYTES` = 64 MiB. A cell keeps its
  combining characters, underline color and hyperlink in a separate allocation (a cell extra), shared by the cells
  printed with one cursor template but made anew whenever a program varies them from cell to cell: a combining mark on
  every cell, an underline color changed before every character, a new hyperlink before every character (an OSC 8
  without an `id` is a new link even when its URI repeats). The caps above bound one cell's or one link's size, not how
  many there are: such output takes about 390 MiB per terminal at 12,000 lines of 200 columns, and about 6 GB with
  maximal hyperlinks. Each `Term` now has an `ExtraStorage` that every cell extra and every link is charged to: an extra
  `CELL_EXTRA_COST` (160 bytes, its allocation and the most its combining characters take, as 64-bit glibc rounds them),
  a link its `id` and URI lengths plus `HYPERLINK_COST` (128 bytes for its own allocations). A charge is given back when
  what it was charged for is dropped, and a copy that alacritty makes when it changes a shared extra is charged too. The
  limit is hard: every change that would make a new extra goes through the storage's setters, which check for room
  first, and degrade the cell when there is none. An added combining character is dropped, an added underline color
  falls back to the default, an OSC 8 ends the active hyperlink (like one over the length caps), and a removal that
  would have to copy an extra other cells share (an `SGR 59` after `ESC 8` restored a template whose extra the printed
  cells hold, say) drops the cell's whole extra rather than copy it. A cell can lose attributes this way; the storage
  never grows past the limit, whatever a program saves and restores. A cell's setters also leave its extra alone when
  the value does not change, and clearing a wide character copies its extra only when there are combining characters to
  clear, so that repeating an attribute copies nothing. 64 MiB holds an extra for about 400,000 cells, a sixth of that
  scrollback, or about 25,000 maximal hyperlinks; text with combining marks on a third of its cells fills it only past
  6,000 such lines. Note what "gone" means: alacritty can keep up to a thousand rows that leave the grid (cleared
  history, a shrunk screen) to reuse them, cells and all, so extras on those rows stay alive and counted until new
  output reuses the rows. An embedder that writes cells with extras itself, as a snapshot restore does, gives them their
  extras through `Term::extra_storage()`'s setters (or shares an extra already made that way); upstream's constructors
  still make uncharged ones, which the terminal never uses.
- Window title length, `alacritty_terminal::term::MAX_TITLE_BYTES` = 4096. A longer title is cut at the last character
  boundary that fits, and gives back the rest of its allocation, which the parser sized for the whole payload and which
  would otherwise travel on in `Event::Title` to whoever keeps the latest title. This one exists because of the OSC cap:
  the title stack holds up to 4096 titles, so with 10 MiB titles one program could pin about 40 GiB. 4096 bytes leaves
  room for a shell's title holding a long path.

NOTE: These caps bound each piece of state, and the extra storage the cells' extras together. A terminal's total is the
supervisor's per-terminal memory budget, measured and stated in the emulator crate
(`crates/farhelm-terminal-emulator/src/lib.rs`, "Memory budget", and its `tests/memory.rs`), at 12,000 lines of 200
columns: 56 MiB measured for ordinary output (a cell is 24 bytes), 120 MiB measured for the worst hostile output that
gives every cell an extra, and a bound of about 190 MiB for a terminal doing everything hostile at once (the 56 MiB
grid, 64 MiB of extras, 38 MiB measured outside the grid and about 32 MiB while a snapshot is taken). Outside the grid:
the title stack (4096 titles of up to 4096 bytes, 16 MiB), an OSC the program never ends (up to the 10 MiB limit
buffered, in a `Vec` whose capacity can reach 16 MiB as it doubles), and the 2 MiB buffer vte's `Processor` reserves up
front for synchronized updates (resident only as far as an update fills it): 34 MiB of buffers, 38 MiB resident with the
allocator's overhead.

### Per-patch records for upstream reports

Each record below holds what an upstream report or pull request needs, so that nobody has to rediscover it: what
upstream does, a byte-level reproduction, what xterm and tmux 3.7c do (with the source checked), the fix, the upstream
code touched, the patch hunks that make it up, the tests that pin it, and its upstream status. Sizes are columns by
lines, and cursor positions are (line, column) from 0. The xterm source checked is the `ThomasDickey/xterm-snapshots`
mirror at xterm-412 (commit `5c75ead9a75d4b2e666ca06fa94503b5766ed93f`: `button.c` 1.686, `charproc.c` 1.2134,
`cursor.c` 1.101, `main.h` 1.93, `misc.c` 1.1137, `ptydata.c` 1.173, `ptyx.h` 1.1174, `screen.c` 1.682, `util.c` 1.1032,
and its control sequence documentation `ctlseqs.ms` 1.748); the tmux source is the 3.7c release Farhelm pinned for its
tests when these records were written (`tmux-3.7c.tar.gz`, SHA-256
`7c60cae9a0e25288e2e24750aafc9e8800fc7fd4555e447e1b29ee4201cfb3bf`). xterm.js, which rendered Farhelm's terminals when
these records were written, was checked in the copy Farhelm then vendored (6.0.0) and, where a record cites a source
file, in the `xtermjs/xterm.js` source at tag 6.0.0 (commit `f447274f430fd22513f6adbf9862d19524471c04`). The alacritty
application (outside the crate) was read at the master commit below. "Observed" means the bytes were run through the
crates: the published, unpatched ones unless the record names another configuration (the patched trees, or Farhelm's
emulator). A result read from the code, an estimate, or a figure measured on another build says so. The fixes that keep
the emulator's screen where tmux keeps it were found by the supervisor's comparison with a live tmux 3.7c (its
`emulator_parity` tests, replaced in 2026-11 by checks that need no tmux); the tests each record names now are the
vendored crates' and the emulator's own.

Each status says whether the behavior was checked against upstream's current source on 2026-11-03: `alacritty/alacritty`
master at `aecf0f600fa840a415b55da5cf34fbeaec39ac32` (`alacritty_terminal` 0.26.1-dev; crates.io's newest is still
0.26.0) and `alacritty/vte` master at `89c9c22f5b8fcc3feba0a9d9f50a020801e74acc` (0.15.0; since the release,
`src/lib.rs` and `src/ansi.rs` changed only in comments, `docsrs` attributes and one `debug!` argument, `&buf` to
`buf`). Of the five patched `alacritty_terminal` Rust files, only `term/cell.rs` (the combining character cap, below),
`term/mod.rs` and `grid/row.rs` (`Row::new`) differ at that commit from 0.26.0. The `term/mod.rs` change is an upstream
fix to `push_keyboard_mode`, which in 0.26.0 removes the oldest entry of the title stack, not of the keyboard mode
stack, when the keyboard mode stack is full: that panics when the title stack is empty, and otherwise drops a title and
lets the keyboard mode stack grow past its limit. The vendored tree still has the bug; Farhelm cannot reach it, because
the emulator turns the kitty keyboard protocol off (`kitty_keyboard: false`, `farhelm-terminal-emulator`,
`src/emulator.rs`), and `push_keyboard_mode` returns at once then. No issue tracker was searched for existing reports.

The patch hunks are named by file and hunk header in `patches/alacritty_terminal-0.26.0.diff` and
`patches/vte-0.15.0.diff` as they stand; a change to a patch file moves headers after it, so it updates these too. Every
patched file's first hunk is its `Modified for Farhelm` header, shared by every record that touches the file and left
out below. A hunk that serves several records says how its lines divide between them (see also "Hunks shared between
records" after the records).

#### vte: whether the parser is between sequences

- Upstream today: nothing outside the crate can tell whether `Parser` (or the `ansi::Processor` around it) is in the
  middle of an escape sequence or a UTF-8 character. The state is private.
- Reproduction: none; this is a missing read accessor, not a misbehavior. The need: after feeding `ESC [ 3` the parser
  waits for the rest of a CSI, after `\xc3` for the rest of a character, and an embedder cannot know either.
- xterm and tmux have no such API. tmux tracks the same fact for its own use: it keeps the bytes received since the
  parser last left its ground state (`input.c`, `since_ground`, which `input_pending` hands to `capture-pane -P`), and
  starts a timer that gives up on a sequence left unfinished (`input_start_ground_timer`).
- Fix: `Parser::is_ground()` and `Processor::is_ground()`: the parser is in its ground state with no partial UTF-8 code
  point. Read-only.
- Why Farhelm needs it: a snapshot of the emulator is only continuable where this holds; elsewhere the snapshot plus the
  bytes after it would not rebuild the same state.
- Upstream code: `vte/src/lib.rs`, `Parser`; `vte/src/ansi.rs`, `Processor`.
- Patch hunks (vte): `lib.rs` `@@ -90,6 +119,18 @@` (`Parser::is_ground`); `ansi.rs` `@@ -363,6 +369,33 @@`, its
  `is_ground` (shared with the REP record, which owns the rest of the hunk); tests in `lib.rs` `@@ -1539,4 +1634,172 @@`
  (`is_ground_only_between_sequences`).
- Tests: `tests::is_ground_only_between_sequences` (vte) covers `Parser::is_ground()`. `Processor::is_ground()` only
  delegates to it and has no vte test; the emulator reaches it through `Emulator::is_clean`, which
  `clean_means_ground_with_no_open_update` (`farhelm-terminal-emulator`, `tests/boundary.rs`) and
  `snapshots_are_refused_away_from_clean_boundaries` (`tests/snapshot.rs`) pin at the same kinds of points.
- Upstream status: not yet reported. Upstreamable as is: additive and read-only. Checked against vte `89c9c22` on
  2026-11-03: still missing.

#### vte: the character REP repeats

- Upstream today: `ansi::Processor` remembers the last printed character for `CSI Ps b` (REP) in a private field, so an
  emulator rebuilt from a snapshot cannot be given it back.
- Reproduction (10x1): `x ESC [ 3 b` prints `xxxx`. An emulator rebuilt from a snapshot of the state after `x` has no
  way to learn the `x`, so the same `ESC [ 3 b` fed to it prints nothing.
- xterm and tmux have no such API; both keep the last character privately (xterm: `charproc.c`, `CASE_REP`, repeating
  `sp->lastchar`; tmux: `input.c`, `INPUT_CSI_REP`, repeating `ictx->last`), and both forget it at the next control or
  completed sequence. xterm sets `sp->lastchar` to -1 (no character) whenever a byte that printed nothing leaves the
  parser in its ground state (the end of `doparsing`), and REP repeats only a character of positive width; tmux repeats
  only while `INPUT_LAST` is set, which printing sets and every C0 control and dispatched sequence clears. So in both,
  `x CR ESC [ 3 b` or `x ESC [ 1 m ESC [ 3 b` repeats nothing. alacritty's `Processor` keeps the character across every
  other sequence until the next print, which is why its REP character is state a snapshot has to carry; that
  longer-lived character is itself a divergence from both references.
- Fix: `Processor::preceding_char()` and `Processor::set_preceding_char()`.
- Why Farhelm needs it: in alacritty the REP character outlives every other sequence, so a snapshot that leaves it out
  restores an emulator whose next REP repeats the wrong character, or none, and the restored terminal differs from the
  original from then on. The snapshot reads it with `preceding_char()` and sets it back with `set_preceding_char()` when
  no printable character can leave it (DEL or a C1 control, `farhelm-terminal-emulator`, `src/snapshot/v1/mod.rs`). How
  often real programs send REP was not measured. A program in a terminal host runs with `TERM=xterm-256color`
  (`HOST_TERM` in `crates/farhelm-supervisor/src/service/core/host_launch.rs`), whose terminfo entry has the `rep`
  capability (ncurses 6.6's: `rep=%p1%c\E[%p2%{1}%-%db`), so a curses program may send it. The one Claude Code and one
  Codex session the terminal host prototype recorded (not in the repository) contain no REP (searched for `ESC [ Ps b`
  on 2026-11-03).
- Upstream code: `vte/src/ansi.rs`, `Processor`.
- Patch hunks (vte): `ansi.rs` `@@ -363,6 +369,33 @@`, its `preceding_char` and `set_preceding_char` (shared with the
  `is_ground` record).
- Tests: none in the vendored trees. The emulator's snapshot tests exercise both accessors:
  `control_characters_as_the_rep_character_round_trip` and `a_hand_built_v1_blob_restores_as_its_layout_says`
  (`farhelm-terminal-emulator`, `tests/snapshot.rs`), the `del-rep-character` and `c1-rep-character` scripts of
  `golden_blobs_restore_to_their_recorded_state` (`tests/snapshot_golden.rs`), and every round trip in
  `tests/snapshot_roundtrip.rs`, which compares `preceding_char()` of the original and the restored emulator
  (`tests/common/compare.rs`).
- Upstream status: not yet reported. Upstreamable as is: additive. Checked against vte `89c9c22` on 2026-11-03: still
  missing.

#### Emoji variation widths and the character REP repeats

- Upstream today: `Term::input` appends FE0E and FE0F as zero-width marks without changing a cell's width. vte replaces
  its REP character after every input character, including those marks. Both remain true at the upstream commits named
  above.
- Reproduction (5x3, columns from 1): `❤ FE0F x` leaves `x` in column 2 upstream, rather than column 3. `⌚ FE0E x`
  leaves `x` in column 3 upstream, rather than column 2. At column 5, `❤ FE0F` leaves a narrow heart upstream; with the
  rule enabled it leaves a leading spacer and wraps a wide heart onto the next row. On a 10-column screen,
  `❤ FE0F CSI 2 b` repeats selectors upstream, rather than printing two bare hearts after the wide one. Vendored and
  paired tests exercise these byte sequences; there is no Ghostty runtime observation.
- Reference: Ghostty's default configuration selects Unicode grapheme widths (`Config.zig:490–515`, mapped in
  `Termio.zig:255–280`). At commit `c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed`,
  [`grapheme.zig:56–80`](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/unicode/grapheme.zig#L56-L80)
  accepts a selector only after a valid variation base. The immediate stored predecessor matters: a combining mark or an
  already attached selector makes a following selector ineligible, and an ineligible selector is dropped.
  [`Terminal.zig:1929–2126`](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L1929-L2126)
  chooses the target, changes width before appending the mark, overwrites rather than inserts the new tail, and
  relocates a wrapping base with the current pen. Its early return keeps REP's printable base (`:2117–2187`). These are
  source-derived results. xterm-412 and xterm.js selector behavior was not checked for this record. tmux 3.7c's
  `screen_write_combine` in `screen-write.c` widens any narrow cell followed by FE0F, never narrows for FE0E, and
  retains the selector as REP's character (`input.c`, `INPUT_CSI_REP`); until 2026-11 the supervisor's known differences
  pinned those choices against a live tmux (since removed with the rest of that comparison).
- Fix: `Term::input_emoji_selector` chooses from the current grid and its stored suffix, with no new remembered input.
  `unicode-width`'s sequence widths identify eligible bases, plus eight literal narrowing exceptions from Unicode 18.0's
  variation-sequence data. Formatting inputs Ghostty discards never become selector predecessors, and incoming V/T
  continuations attach only after the immediate Hangul or Prepend predecessor that permits them. The literal properties
  come from the same pinned Unicode data; `THIRD_PARTY_NOTICES.md` and `UNICODE-LICENSE.txt` carry its notice. This is
  not a general grapheme engine, and ordinary L and Prepend scalar widths stay alacritty's. No Ghostty implementation is
  copied. Mid-row widening preserves the base's pen; wrapping uses the current pen, and marks go through bounded extra
  storage. Layout changes even when that storage drops the selector. `set_emoji_width(false)` restores legacy attachment
  and REP bookkeeping during v1 snapshot painting; callers enable it before replaying journal output. Existing cell
  fixups represent changed widths, so the encoder, golden blobs and ordinary width fingerprint stay unchanged.
- Accepted differences from Ghostty: unattached V/T use ordinary scalar printing, preserving printable Kirat signs and
  zero-width Hangul suffixes. Retained signs block the next selector; an unattached nonzero-width sign also becomes
  REP's character. At the right edge with autowrap off, a plain space counts as empty when choosing the target.
  Farhelm's cells and v1 blobs cannot distinguish a printed space from an erased cell; Ghostty can
  ([`:1939–1967`](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L1939-L1967)).
  On a one-column screen, existing ordinary wide-input handling stays: drop before consuming a pending wrap, while
  Ghostty consumes it before clearing the too-wide input
  ([`:2189–2262`](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L2189-L2262)).
  Both are explicit maintainer decisions, pinned in tests. Ordinary autowrap-off writes retain their existing rule;
  selector widening into the final pair leaves a pending wrap even with autowrap off, as Ghostty does, so turning
  wrapping on before the next print wraps instead of overwriting that pair.
- Why Farhelm needs it: programs count eligible emoji sequences as two columns. Giving them one shifts the rest of a
  row, and restoring an old snapshot with the new live rule would silently change its saved cells and REP state.
- Upstream code: alacritty `term/mod.rs`, `Term::input` and the selector helper; its `Cargo.toml` opt-in
  `farhelm-rep-policy` feature; vte `ansi.rs`, `Handler::input_updates_rep` and `Performer::print`. The hook defaults to
  upstream behavior. The feature gates only Term's override, since standalone alacritty uses upstream vte, which lacks
  the hook. Paired tests and Farhelm enable it; standalone default tests still compile unchanged, while standalone
  `--all-features` requires patched vte. The inherent Term policy and width switch exist in either configuration.
- Patch hunks: alacritty `Cargo.toml` `@@ -33,6 +35,9 @@` (feature); `term/mod.rs` `@@ -327,6 +396,14 @@` (field),
  `@@ -441,15 +518,18 @@` (default), `@@ -953,6 +1033,235 @@` (switch, policy and helper, shared with restore accessors
  and extra storage), `@@ -1056,10 +1397,132 @@` (eligibility, gated hook, input dispatch), and tests
  `@@ -3299,4 +3832,1060 @@`; vte `ansi.rs` `@@ -505,6 +538,16 @@` (default hook), `@@ -1288,8 +1337,12 @@` (conditional
  REP update) and tests `@@ -2437,6 +2504,105 @@`.
- Tests: the `term::tests::emoji_selector*` / `emoji_selectors*` / `emoji_text_selector*` tests cover targets, widths,
  cursor, wrap, eligibility, styles, hyperlinks, insert/overwrite cleanup, both exceptions, storage exhaustion and the
  legacy switch. Paired `grapheme_continuations_keep_the_printable_rep_base` and
  `legacy_width_switch_also_restores_the_legacy_rep_policy` cover the actual parser interaction. vte's
  `rep_policy_preserves_the_existing_register_and_default` pins the additive default. Farhelm adds snapshot boundary and
  old-style stream tests, genuine bare-Term delegation, generated round trips and browser checks of replayed width and a
  narrowed glyph's one-cell box.
- Upstream status: not yet reported. Checked against alacritty `aecf0f6` and vte `89c9c22`: still absent. This is a
  choice of width policy, rather than a claim that upstream's current behavior violates its own contract. An upstream
  change would need its own policy/default decision and a compatible parser integration.

#### alacritty_terminal: the state a restore needs

- Upstream today: `Term` keeps private the inactive screen (the primary screen and its history while the alternate
  screen shows), the scroll region, the tab stops, the active character set, the title, the title stack and the kitty
  keyboard mode stacks. The inactive screen can be reached only by swapping it in with `swap_alt()`.
- Reproduction: none; this is missing API. The need: after `ESC [ 2 ; 4 r` (a scroll region) an embedder can neither
  read the region nor reproduce it in another `Term` without knowing it.
- xterm and tmux have no such API.
- Fix: `Term::inactive_grid()`, `inactive_grid_mut()`, `scroll_region()`, `tab_stops()`, `active_charset()`, `title()`,
  `title_stack()` and `keyboard_mode_stacks()`. All read-only except `inactive_grid_mut()`, through which a restore
  writes what its escape-sequence stream cannot paint on the primary screen while the alternate screen shows: cells, the
  cursor, and the saved cursor, which entering the alternate screen overwrites; and, while the primary screen shows, the
  alternate screen's saved cursor if the stream cannot place it (the same `paint.rs` function, before the primary
  screen). The restore applies its cell and cursor fixups to whichever grid they name (`farhelm-terminal-emulator`,
  `src/snapshot/v1/mod.rs`, `select`), and the encoder emits all three for the hidden primary screen: cells it cannot
  paint as cell fixups, a cursor it cannot place, and the saved cursor (`src/snapshot/v1/paint.rs`, the alternate-screen
  branch).
- Why Farhelm needs it: the supervisor snapshots each terminal's emulator so a restarted supervisor can rebuild it. The
  emulator also uses `inactive_grid_mut()` outside snapshots, to implement modes 47 and 1047 on top of alacritty's
  screen switch (the Farhelm-side record below).
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `impl<T> Term<T>`.
- Patch hunks (`alacritty_terminal`): `term/mod.rs` `@@ -953,6 +1033,235 @@`, the accessors from `inactive_grid()`
  through `keyboard_mode_stacks()`. `extra_storage()` belongs to its record, and the following selector methods to the
  emoji-width record.
- Tests: no vendored test is about the accessors, though two read them on the way: `term::tests::history_counters` reads
  `inactive_grid()` and `term::tests::title_length_is_capped` reads `title()` and `title_stack()`. The emulator's
  snapshot tests rest on all of them: the round trips in `farhelm-terminal-emulator`'s `tests/snapshot_roundtrip.rs` and
  several in `tests/snapshot.rs` compare the inactive grid, scroll region, tab stops, active charset, title stack and
  keyboard mode stacks of the original and the restored terminal (`tests/common/compare.rs`), and
  `the_review_reproducers_round_trip` (`tests/snapshot.rs`) restores a primary cursor the stream could not place and a
  primary saved cursor under the alternate screen, both written through `inactive_grid_mut()`.
- Upstream status: not yet reported. Upstreamable as is: additive. `inactive_grid_mut()` is the one an upstream reviewer
  may question. A narrower API would need setters for the inactive grid's cells, its cursor and its saved cursor, not
  only the saved cursor (the 47 and 1047 handling below needs only the last). Checked against alacritty `aecf0f6` on
  2026-11-03: still missing.

#### vte: a limit on the buffered OSC payload

- Upstream today: the `std` parser appends every OSC payload byte to a `Vec` without limit, dispatches the whole
  payload, and keeps the `Vec`'s capacity afterwards.
- Reproduction: `ESC ] 2 ;` + 11 MiB of `A` + BEL, fed to a `vte::Parser` in one call. Observed with the published
  crate: `osc_dispatch` with parameters of 1 and 11,534,336 bytes, and the parser still holding 16,777,216 heap bytes
  after the sequence ended (the buffer's doubled capacity; `osc_end` only calls `osc_raw.clear()`). The same with 2 MiB
  of `A`: dispatched whole, and 4,194,304 bytes held afterwards. Expected: the 11 MiB sequence not dispatched, and the
  buffer given back after either. With the patch, observed: the 11 MiB sequence not dispatched, the 2 MiB one dispatched
  whole, and 1,024 bytes held after each. Both measured on 2026-11-03 by a throwaway program with a counting global
  allocator, run against the published crates (the vendored trees with their patch files reversed) and against the
  patched trees, in one run on a Linux build sandbox; the program is not kept in the repository.
- xterm limits the strings it buffers, OSC among them, to the `maxStringParse` resource, 20,000 bytes by default
  (600,000 with ReGIS or sixel graphics), and drops a longer one whole when it ends ("Ignoring too-long string";
  `charproc.c`, the `strings_max` check in `doparsing` and `CASE_ST`; `main.h`, `DEF_STRINGS_MAX`). Not every string is
  buffered: APC, PM and SOS are ignored as they arrive, and sixel data goes to `parse_sixel_char` a byte at a time,
  instead of that check. tmux discards a sequence longer than `input-buffer-size`, 1 MiB by default and at least, whole
  (`input.c`, `input_input` setting `INPUT_DISCARD`, `input_exit_osc`), and shrinks its buffer back to 32 bytes on
  returning to the ground state (`input_ground`). xterm.js drops an OSC whose payload passes 10,000,000 characters
  (`PAYLOAD_LIMIT`).
- Fix: `vte::OSC_PAYLOAD_LIMIT` = 10 MiB. Past it the parser stops buffering, and when the sequence ends it drops it
  whole and calls `Perform::osc_overflow` (next record) instead of dispatching; after every OSC the buffer is shrunk
  back to 1 KiB (`MAX_OSC_RAW`, the size of upstream's `no_std` buffer). The `no_std` parser, which has a fixed buffer,
  is unchanged.
- Why Farhelm needs it: one program must not grow the shared supervisor's memory without bound. The limit is set so that
  every OSC 52 copy that worked in Farhelm's terminals when it was set, about 7.5 MB of text, still works: the browser
  renderer Farhelm used then dropped payloads over 10,000,000 characters (xterm.js's `PAYLOAD_LIMIT`, above), and 10 MiB
  is just over that.
- Upstream code: `vte/src/lib.rs`, `Parser::action_osc_put` and `Parser::osc_end`.
- Patch hunks (vte, `lib.rs`): `@@ -45,6 +50,23 @@` (`OSC_PAYLOAD_LIMIT`); `@@ -64,6 +86,13 @@` (the `osc_overflow`
  field, shared with the next record: this one needs only whether the limit was hit, the next one the count of complete
  parameters it holds); `@@ -548,14 +589,32 @@` (the limit check in `action_osc_put`, and in `osc_end` the drop and the
  `shrink_to`; the `osc_overflow_dispatch` call is the next record's); tests in `@@ -850,6 +937,8 @@`,
  `@@ -865,6 +954,12 @@` (the test performer records `OscOverflow`, shared) and `@@ -1539,4 +1634,172 @@`. A version of
  this record alone would drop the sequence without the `Perform` method, so its test would assert that nothing is
  dispatched rather than one `OscOverflow`.
- Tests: `tests::osc_at_payload_limit_is_dispatched` (a payload of exactly the limit is dispatched whole) and
  `tests::osc_over_payload_limit_is_dropped` (a payload four bytes over is dropped, at most the limit is buffered, the
  buffer's capacity is back to at most 1 KiB after each of BEL, ST and CAN, and the next OSC parses), both vte. The
  first case of `tests::osc_overflow_reports_only_complete_params` pins the boundary from the other side: a payload one
  byte over the limit is dropped. No test pins the shrink after a large OSC that is dispatched rather than dropped:
  `osc_at_payload_limit_is_dispatched` does not look at the capacity. The reproduction above observed it once.
  `clipboard_writes_are_bounded_by_the_payload_limit` (`farhelm-terminal-emulator`, `tests/hostile.rs`) covers the limit
  through the emulator: an 8 MiB OSC 52 delivered, one over the limit dropped.
- Upstream status: not yet reported. The limit's value is Farhelm policy. An upstream-friendly version makes it a
  parameter (a const generic like `Parser`'s existing `OSC_RAW_BUF_SIZE`, or a constructor argument) with no limit by
  default; the shrink after a large OSC is upstreamable as is. Checked against vte `89c9c22` on 2026-11-03: still
  present (no limit, no shrink).

#### vte: reporting a dropped OSC, and ending the hyperlink it would have replaced

- Upstream today: there is no limit to report. With the limit above alone, a dropped OSC would vanish without a trace,
  and a dropped OSC 8 would leave the previous hyperlink active.
- Reproduction (20x1, both crates): `ESC ] 8 ; ; http://a ESC \ A ESC ] 8 ; ; http://` + 11 MiB of `u` + `ESC \ B`.
  Observed upstream: `B` is linked to the 11,534,343-byte URI. From the code, with the payload limit and without this
  patch: `B` stays linked to `http://a`, a link the program moved away from. Expected: `B` unlinked.
- xterm does not implement OSC 8 (`misc.c`, `do_osc` has no case for it). tmux and xterm.js both keep the previous link,
  which is what upstream with only the payload limit does, and this change departs from both on purpose. tmux discards a
  sequence longer than `input-buffer-size` (1 MiB by default), and `input_exit_osc` returns at its `INPUT_DISCARD` check
  before any handler runs, so the cell's link is unchanged. xterm.js registers its OSC 8 handler through an `OscHandler`
  (`src/common/InputHandler.ts`), whose `end` never calls the handler once the payload passed `PAYLOAD_LIMIT`,
  10,000,000 characters (`src/common/parser/OscParser.ts`), so `setHyperlink` does not run.
- Fix: a `Perform::osc_overflow(params, bell_terminated)` method with an empty default, called with only the parameters
  that ended before the limit, so no truncated data leaves the parser. `ansi::Processor` implements it to end the active
  hyperlink when the dropped sequence is an OSC 8.
- Why Farhelm needs it, although tmux and xterm.js keep the link: Farhelm already ends the active link for any OSC 8
  over the hyperlink length limits below (2,083 bytes of URI, 250 of `id`), where tmux and xterm.js apply links up to
  their buffer limits. Without this change, a link from 2,084 bytes to 10 MiB would end the active link and a longer one
  would keep it, so making a link longer would bring the previous link back. With it, every too-long link does the same
  thing, and text after one is never attributed to a URI the program had moved away from. The change itself only affects
  an OSC 8 of more than 10 MiB; for shorter too-long links, the length limits already set Farhelm apart from tmux and
  xterm.js.
- Upstream code: `vte/src/lib.rs`, `trait Perform` and `Parser::osc_end`; `vte/src/ansi.rs`, `Performer::osc_overflow`.
- Patch hunks (vte): `lib.rs` `@@ -64,6 +86,13 @@` (the count of complete parameters in the `osc_overflow` field, shared
  with the previous record), `@@ -548,14 +589,32 @@` (the `osc_overflow_dispatch` call in `osc_end`, shared),
  `@@ -588,6 +647,17 @@` (`osc_overflow_dispatch`), `@@ -791,6 +865,19 @@` (`Perform::osc_overflow`), tests in
  `@@ -850,6 +937,8 @@`, `@@ -865,6 +954,12 @@` (shared) and `@@ -1539,4 +1634,172 @@`; `ansi.rs`
  `@@ -1325,6 +1378,18 @@` (`Performer::osc_overflow`) and the test in `@@ -2437,6 +2504,105 @@`.
- Tests: `tests::osc_overflow_reports_only_complete_params` (vte); `tests::dropped_osc8_ends_hyperlink` (vte, `ansi`);
  `too_long_hyperlink_ends_active_link_at_either_layer` (`pair-tests`).
- Upstream status: not yet reported. Upstreamable together with a configurable OSC limit: additive trait method with a
  default. Upstream may prefer tmux's and xterm.js's behavior for its own terminal, where no length cap ends links; the
  `Processor` half is the arguable one. Checked against vte `89c9c22` on 2026-11-03: still missing (vte has no limit to
  report).

#### alacritty_terminal: a limit on combining characters per cell

- Upstream today: `Cell::push_zerowidth` appends every zero-width (combining) character to the cell before it, four
  bytes each, without limit.
- Reproduction (10x2): `e` + 1,000 × U+0301 + `x`. Observed: the cell at (0, 0) holds 1,000 combining characters; `x` is
  at (0, 1). Expected: 16 kept, `x` at (0, 1).
- xterm keeps a fixed number of combining characters per cell, the `combiningChars` resource: 2 by default, at most 5
  (`charproc.c`, `XtNcombiningChars` and the clamp in `VTInitialize`), and drops the rest (`util.c`,
  `addXtermCombining`, which finds no free slot). tmux keeps at most 32 bytes of UTF-8 per cell (`tmux.h`, `UTF8_SIZE`;
  `screen-write.c`, `screen_write_combine`: "Check if this combined character would be too long").
- Fix: `cell::MAX_ZEROWIDTH_CHARS` = 16; further ones are dropped. Real text needs far fewer: stacked diacritics and
  Hangul jamo use two or three, a subdivision flag such as Scotland's six.
- Why Farhelm needs it: a program could otherwise grow one cell, and the supervisor's memory, by four bytes per byte
  pair it prints.
- Upstream code: `alacritty_terminal/src/term/cell.rs`, `Cell::push_zerowidth`.
- Patch hunks (`alacritty_terminal`, `term/cell.rs`): `@@ -36,6 +43,18 @@` (`MAX_ZEROWIDTH_CHARS`, which the extra
  storage's `CELL_EXTRA_COST` is sized by) and `@@ -162,6 +360,10 @@` (`push_zerowidth`); the test in `term/mod.rs`
  `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::zerowidth_characters_per_cell_are_capped` (vendored);
  `a_combining_character_flood_is_capped_per_cell` (`farhelm-terminal-emulator`, `tests/hostile.rs`).
- Upstream status: fixed upstream, differently. Checked against alacritty `aecf0f6` on 2026-11-03: commit
  `ede2ac144da4dec4c075bfa803aacf3b3739bce6` ("Fix unbounded zerowidth character limit", 2026-08-26) stores combining
  characters in an `ArrayVec<char, 9>` and drops any past nine (`try_push`). It is not in a crates.io release yet (the
  newest is 0.26.0). Nothing to report; on upgrading to a release that has it, this patch's hunks go, and Farhelm either
  takes upstream's 9 or keeps 16 on purpose. `CELL_EXTRA_COST` (the extra storage record) assumes up to 16 characters in
  a `Vec` and needs a second look either way.

#### alacritty_terminal: limits on hyperlink URI and id length

- Upstream today: an OSC 8 hyperlink of any length is stored, and every cell printed while it is active holds it.
- Reproduction (20x1): `ESC ] 8 ; ; http://ok ESC \ A`, then `ESC ] 8 ; ; http://e/` + 3,000 × `u` + `ESC \ B`, then
  `ESC ] 8 ; id=` + 300 × `i` + `; http://id ESC \ C`. Observed: `B` linked to the 3,009-byte URI, `C` to a link with a
  300-byte id. Expected: `B` and `C` unlinked.
- xterm does not implement OSC 8. tmux bounds a link only by `input-buffer-size` (above), but stores it once per pane
  and gives each cell a number, so length does not multiply by cells (`hyperlinks.c`, `hyperlinks_put`; it also keeps at
  most 5,000 links across the whole server, `MAX_HYPERLINKS`).
- Fix: `term::MAX_HYPERLINK_URI_BYTES` = 2083 and `MAX_HYPERLINK_ID_BYTES` = 250, the limits VTE uses (as the OSC 8
  specification documents; iTerm2 uses the same URI limit). An OSC 8 over either ends the active link, so the text after
  it is unlinked.
- Why Farhelm needs it: a program can start a new link before every character, so a link's length multiplies into the
  grid's memory.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::set_hyperlink`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -41,6 +53,63 @@`, its `MAX_HYPERLINK_URI_BYTES` and
  `MAX_HYPERLINK_ID_BYTES` (the hunk's other constants belong to the extra storage and title records);
  `@@ -1873,7 +2366,26 @@`, its first statement, the `filter` (the reuse and extra storage records own the rest); the
  test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::hyperlink_uri_and_id_lengths_are_capped` (vendored);
  `too_long_hyperlink_ends_active_link_at_either_layer` (`pair-tests`); `long_hyperlinks_are_not_stored`
  (`farhelm-terminal-emulator`, `tests/hostile.rs`).
- Upstream status: not yet reported. Upstreamable as is: the values follow VTE and the specification. Checked against
  alacritty `aecf0f6` on 2026-11-03: still present.

#### alacritty_terminal: reusing a repeated hyperlink

- Upstream today: every OSC 8 builds a new link, and setting it on the cursor's template copies the template's cell
  extra (`Arc::make_mut` on an extra that already printed cells share), so cells printed after two equal OSC 8s hold two
  equal but separate links and extras.
- Reproduction (10x2): `ESC ] 8 ; id=a ; http://e/ BEL x ESC ] 8 ; id=a ; http://e/ BEL y`. Observed with the published
  crate: the two links compare equal, but `x` and `y` hold separate cell extras (`Arc::ptr_eq` false) and separate URI
  allocations. Expected: one of each, shared. With the same OSC 8 before each of 1,000 characters on a 100x20 terminal:
  1,000 distinct cell extras and 1,000 distinct URI allocations. Observed with the patched trees: one cell extra and one
  URI allocation, for the two cells and for all 1,000. Run on 2026-11-03 by the throwaway program of the OSC limit
  record, in the same run. The patched result is the whole patch's: with this record's early return removed, the extra
  storage's own unchanged-value check (below) still keeps the cells sharing.
- xterm does not implement OSC 8. tmux reuses a link whose id and URI it already holds (`hyperlinks.c`,
  `hyperlinks_put`, the `RB_FIND` when the program gave an id), and like this patch never merges links without one.
- Fix: an OSC 8 that repeats the active link's explicit id and URI keeps the active link. A link without an id gets a
  fresh generated id, is never equal to the active one, and is built as before (xterm.js also keeps such links apart).
- Why Farhelm needs it: in upstream code, a program that repeats one link before every character costs a link and a cell
  extra per cell. In Farhelm's tree the extra storage's `set_hyperlink` already returns when the link is unchanged, so
  the cells share without this hunk too; what the hunk still saves there (read from the code) is building and charging a
  link only to drop it, once per repeat, and, once the storage is full, ending the active link: without the early
  return, `ExtraStorage::link` finds no room for the repeated link and `set_hyperlink` gets `None`, so a program that
  merely repeated its link would lose it.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::set_hyperlink`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -1873,7 +2366,26 @@`, its `if let` that returns early (the
  length caps and extra storage records own the rest); the test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::repeated_hyperlink_is_shared` (vendored) pins the sharing, not this hunk: with the early return
  removed in a scratch copy, it and all 151 of the crate's library tests still pass (recorded run `1f62884b`, local,
  2026-11-03), because the extra storage's check gives the same sharing. Missing is a check that tells the two apart:
  repeating the active link with the storage full and asserting the link stays, or counting the links built. Ported to
  upstream code without the extra storage, the existing test would pin this hunk as is.
- Upstream status: not yet reported. Upstreamable as is: no behavior changes, only sharing. Checked against alacritty
  `aecf0f6` on 2026-11-03: still present.

#### alacritty_terminal: a limit on title length

- Upstream today: an OSC 0 or 2 title of any length is stored, travels in `Event::Title`, and is copied onto the title
  stack, which holds up to 4,096 titles (`TITLE_STACK_MAX_DEPTH`).
- Reproduction (10x1): `ESC ] 2 ;` + 8 MiB of `A` + BEL. Observed: an `Event::Title` of 8,388,608 bytes, and the same
  title stored. Expected: 4,096 bytes.
- xterm bounds a title by `maxStringParse` (above, 20,000 bytes) and keeps at most 10 saved titles (`ptyx.h`,
  `MAX_SAVED_TITLES`; `misc.c`, `xtermPushTitle`). tmux bounds it by `input-buffer-size` (1 MiB) and keeps at most 10
  pushed titles (`screen.c`, `screen_push_title`).
- Fix: `term::MAX_TITLE_BYTES` = 4096. A longer title is cut at the last character boundary that fits, and gives back
  the rest of its allocation, which the parser sized for the whole payload.
- Why Farhelm needs it: with the OSC limit at 10 MiB and 4,096 stacked titles, one program could pin about 40 GiB; and
  status detection reads the title, which must not be split inside a character.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::set_title`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -41,6 +53,63 @@`, its `MAX_TITLE_BYTES`;
  `@@ -2221,6 +2740,20 @@` (`set_title`); the test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::title_length_is_capped` (vendored); `titles_are_capped` (`farhelm-terminal-emulator`,
  `tests/hostile.rs`).
- Upstream status: not yet reported. The value is Farhelm policy. An upstream-friendly version could instead lower
  `TITLE_STACK_MAX_DEPTH` toward xterm's and tmux's 10, which bounds the stack without cutting titles, with a length
  limit as a `Config` option. Checked against alacritty `aecf0f6` on 2026-11-03: still present.

#### alacritty_terminal: history counters

- Upstream today: nothing counts lines entering a grid's history; `history_size()` stops growing once history is full,
  so an embedder cannot tell how many lines scrolled in since it last looked.
- Reproduction (10x2, 2 lines of history): `1 CR LF 2 CR LF 3 CR LF 4 CR LF 5`. `history_size()` is 1 after the third
  line, 2 after the fourth and still 2 after the fifth, though three lines scrolled in. Expected: a count that reads 3.
- xterm has no such API. tmux keeps a similar count for its own use, `hscrolled`, but caps it at the history's size and
  resets it when history is cleared (`grid.c`, `grid_scroll_history`, `grid_clear_history`).
- Fix: `Grid::lines_scrolled_into_history()` counts every line that scrolls off the top of a grid that has history and
  keeps counting once history is full; `Grid::history_clears()` counts history being erased (`CSI 3 J`, a full reset).
  Neither decreases. Both fields are skipped by serde and left out of `PartialEq`.
- Why Farhelm needs it: the grid stream sends scrollback to viewers as appended ranges.
- Upstream code: `alacritty_terminal/src/grid/mod.rs`, `Grid::scroll_up` and `Grid::clear_history`.
- Patch hunks (`alacritty_terminal`): every hunk of `grid/mod.rs` (`@@ -135,6 +139,20 @@`, `@@ -147,9 +165,44 @@`,
  `@@ -270,6 +323,11 @@`, `@@ -381,6 +439,9 @@`); the tests in `term/mod.rs` `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::history_counters`, `term::tests::history_counters_clear_and_reset` (vendored).
- Upstream status: not yet reported. Upstreamable as is: additive. Checked against alacritty `aecf0f6` on 2026-11-03:
  still missing.

#### alacritty_terminal: a wide character on a grid one column wide

- Upstream today: `Term::input` writes a wide character's spacer into the cell after it even when the grid is one column
  wide and that cell does not exist, and panics.
- Reproduction (1x3, wrapping on): `a 中 b`. Observed: panic, "index out of bounds: the len is 1 but the index is 1"
  (`grid/mod.rs`). Expected: the narrow characters printed and the wide one dropped: rows `a`, `b`, blank. With wrapping
  off (`ESC [ ? 7 l a 中 b`) upstream does not panic and already gives `b`, blank, blank; the patch keeps that.
- xterm gives up on a character wider than the line (`charproc.c`, `dotext`: "give up - it is too big"). tmux ignores a
  wide character that does not fit while wrapping is off (`screen-write.c`, `screen_write_cell`, "If this character
  doesn't fit, ignore it"); with wrapping on, its `sx - width` arithmetic wraps around in a one-column pane, and the
  character is written anyway (probed: a one-column tmux 3.7c pane shows `中`).
- Fix: a wide character on a grid narrower than two columns is dropped, and the cursor does not move.
- Why Farhelm needs it: one column is a valid terminal size (the terminal host accepts it), and the emulator stays the
  size of the host's pty rather than widening itself, so any program's output could panic the supervisor's task.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::input` (the `Handler` implementation).
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -1080,7 +1543,17 @@`, its one-column check (its
  `push_zerowidth` change is the extra storage record's); the test in `@@ -3299,4 +3832,1060 @@`, shared with the reflow
  record.
- Tests: `term::tests::a_one_column_grid_drops_wide_characters` (vendored);
  `a_one_column_terminal_drops_wide_characters`, `a_zero_size_becomes_one_cell` and
  `narrow_terminals_survive_any_output` (`farhelm-terminal-emulator`, `tests/narrow.rs`).
- Upstream status: not yet reported. Upstreamable as is (a panic fix); upstream may prefer printing a placeholder.
  Checked against alacritty `aecf0f6` on 2026-11-03: still present.

#### alacritty_terminal: reflowing a wide character into one column

- Upstream today: `Grid::shrink_columns` moves a wide character that does not fit at the end of a row to the next row.
  At one column it fits on no row, so the reflow moves it on forever, allocating a row each time.
- Reproduction (6x3, primary screen): `中文x`, then resize to 1x3. Observed: the resize never returns; in a release
  build it reached a 3 GB address-space limit within 15 seconds and aborted on a failed allocation. Expected: the resize
  returns, with the wide characters blanked.
- xterm does not reflow on a resize (`screen.c`, `ScreenResize`). tmux's reflow puts a cell wider than the line on a
  line of its own and moves on (`grid.c`, `grid_reflow_split`).
- Fix: when shrinking to fewer than two columns, a wide character at the start of a row is replaced by a blank cell, and
  its spacer loses its spacer flag, so the reflow advances.
- Why Farhelm needs it: a viewer can resize a terminal to one column while it shows wide text.
- Upstream code: `alacritty_terminal/src/grid/resize.rs`, `Grid::shrink_columns`.
- Patch hunks (`alacritty_terminal`): `grid/resize.rs` `@@ -286,6 +289,17 @@`; the test in `term/mod.rs`
  `@@ -3299,4 +3832,1060 @@`, shared with the previous record.
- Tests: `term::tests::a_one_column_grid_drops_wide_characters` (vendored, its resize half, on a timed thread);
  `resizing_to_one_column_with_wide_characters` and `narrow_terminals_survive_any_output` (`tests/narrow.rs`, each
  bounded to 10 seconds).
- Upstream status: not yet reported. Upstreamable as is (a hang fix). Checked against alacritty `aecf0f6` on 2026-11-03:
  still present.

#### vte: a UTF-8 character split across calls swallows the characters after it

- Upstream today: when a call ends inside a multi-byte character, `advance_partial_utf8` copies up to three bytes from
  the next call to complete it. If an invalid byte follows among them, it prints the completed character but reports
  every valid byte it copied as consumed, so valid characters between it and the invalid byte are never printed.
- Reproduction: `\xc3` in one call, then `\xa9 a \x80 b`. Observed: print `é`, execute `0x80`, print `b`; the `a` is
  lost. Fed in one call: print `é`, print `a`, execute `0x80`, print `b`. Expected: the same as one call.
- xterm decodes UTF-8 a byte at a time as it arrives (`ptydata.c`, `decodeUtf8`), and so does tmux (`input.c`,
  `input_top_bit_set`), so where reads end cannot change what either shows.
- Fix: consume only the completed character's bytes (`c.len_utf8() - old_bytes`), as the function's other path already
  does; the remaining bytes are parsed normally.
- Why Farhelm needs it: characters a program wrote silently disappeared from the live screen, but only when a read
  happened to end inside the character before them, so a terminal rebuilt by feeding the same output in other reads
  would show them, and the two would differ for good.
- Upstream code: `vte/src/lib.rs`, `Parser::advance_partial_utf8`.
- Patch hunks (vte, `lib.rs`): `@@ -698,7 +768,11 @@`; the test in `@@ -1539,4 +1634,172 @@`.
- Tests: `tests::split_utf8_does_not_drop_the_characters_after_it` (vte); `the_cut_into_reads_does_not_matter`
  (`farhelm-terminal-emulator`, `tests/boundary.rs`).
- Upstream status: not yet reported. Upstreamable as is. Checked against vte `89c9c22` on 2026-11-03: still present
  (`return valid_bytes - old_bytes`).

#### vte: a split UTF-8 C1 control and REP (kept as upstream has it)

- Upstream today: a C1 control encoded in UTF-8 (U+0080 to U+009F, `\xc2 \x80` to `\xc2 \x9f`) is executed when it
  arrives in one call (`ground_dispatch`), but printed when it is split across two (`advance_partial_utf8`). alacritty
  draws nothing for it either way, but `ansi::Processor` then remembers it as the character a later REP repeats.
- Reproduction (10x1): `x \xc2 \x85 ESC [ 3 b`. Observed in one call: `xxxx`. Split after `\xc2`: `x`, since REP repeats
  U+0085, which draws nothing. Expected: the same result both ways, the unsplit path's `xxxx`. That expectation is
  consistency within alacritty, not reference behavior: xterm and tmux forget the REP character at any control (the REP
  record above), so where either executes the C1 control, REP after it repeats nothing.
- xterm and tmux decode UTF-8 a byte at a time (previous record), so the cut cannot matter there; what each does with an
  encoded C1 control was not checked.
- Fix: none; kept as upstream behavior. A fix would dispatch the completed character the way the unsplit path does
  (`ground_dispatch` on its UTF-8 encoding) in both arms of `advance_partial_utf8`.
- Why Farhelm accepts it: it takes an encoded C1 control split across reads and then a REP, and the vendored patch is
  kept to the bug that loses characters. The emulator documents it as the one known case where the cut into reads still
  changes the terminal after a final flush (`Emulator::feed`).
- Upstream code: `vte/src/lib.rs`, `Parser::advance_partial_utf8` and `Parser::ground_dispatch`.
- Patch hunks: none.
- Tests: `a_split_encoded_c1_control_changes_what_rep_repeats` (`farhelm-terminal-emulator`, `tests/boundary.rs`) pins
  today's behavior.
- Upstream status: not yet reported. Upstreamable as the one-line change above, if Farhelm or upstream wants it. Checked
  against vte `89c9c22` on 2026-11-03: still present.

#### vte: XTVERSION reaches the handler

- Upstream today: `CSI > q` and `CSI > 0 q` (XTVERSION, a program asking for the terminal's name and version) fall
  through `csi_dispatch` as unhandled, so no `Handler` can answer them.
- Reproduction: feed `ESC [ > 0 q` to `ansi::Processor` with any handler. Observed: no handler method is called.
  Expected: a call the handler can answer with `DCS > | name ST`.
- xterm answers `DCS > | XTerm(version) ST` for a parameter of 0 or none (`charproc.c`, `CASE_REPORT_VERSION`). tmux
  3.7c answers `DCS > | tmux 3.7c ST` for a parameter of 0 or none (`input.c`, `INPUT_CSI_XDA`, where a missing
  parameter defaults to 0). xterm.js 6.0.0 does not answer it: it registers no handler for `CSI > q`
  (`src/common/InputHandler.ts` registers `CSI > c`, DA2, as its only CSI handler with the `>` prefix; its CSI handlers
  ending in `q` are DECSCUSR and DECSCA), so the query goes unanswered.
- Fix: a `Handler::report_version` method with an empty default, called for `CSI > q` and `CSI > 0 q` only. A handler
  that does not implement it behaves as before.
- Why Farhelm needs it: Claude Code asks XTVERSION first, and with no answer asks nothing further, including whether the
  terminal supports synchronized output (DECRQM 2026), and then draws without it (read from Claude Code 2.1.316's
  startup code). The emulator answers `Farhelm <version>` (`farhelm-terminal-emulator`, `src/queries.rs`).
- Upstream code: `vte/src/ansi.rs`, `trait Handler` and `Performer::csi_dispatch`.
- Patch hunks (vte, `ansi.rs`): `@@ -526,6 +569,12 @@` (`Handler::report_version`), `@@ -1708,6 +1773,8 @@` (the
  `csi_dispatch` arm); the test in `@@ -2437,6 +2504,105 @@`.
- Tests: `ansi::tests::xtversion_reaches_the_handler` (vte); `identification_status_and_window_queries`
  (`farhelm-terminal-emulator`, `tests/queries.rs`); `recorded_agent_startup_probes_get_their_answers` (the same file).
- Upstream status: not yet reported. Upstreamable as an additive API; the method name is Farhelm's choice. Checked
  against vte `89c9c22` on 2026-11-03: still missing.

#### alacritty_terminal: `CSI 1 J` from the second line keeps the first

- Upstream today: `Term::clear_screen(ClearMode::Above)` clears the lines above the cursor only `if cursor.line > 1`, so
  with the cursor on the second line the first line stays.
- Reproduction (4x3): `abcd CR LF efgh CR LF ijkl ESC [ 2 ; 2 H ESC [ 1 J`. Observed rows: `abcd`, `  gh`, `ijkl`.
  Expected: `    `, `  gh`, `ijkl`.
- xterm clears every row above the cursor's (`util.c`, `ClearAbove`); tmux clears rows `0..cy` whenever `cy > 0`
  (`screen-write.c`, `screen_write_clearstartofscreen`); xterm.js does the same.
- Fix: the condition is `cursor.line > 0`. An off-by-one; Farhelm needs it because it reads agents' screens from this
  grid and stale text would stay there.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::clear_screen`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -1757,8 +2240,10 @@`; the test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::erase_above_from_the_second_line_clears_the_first` (vendored). Until 2026-11 also the
  supervisor's comparison with a live tmux, scenario "erase operations" (since removed).
- Upstream status: not yet reported. Upstreamable as is. Checked against alacritty `aecf0f6` on 2026-11-03: still
  present (`if cursor.line > 1`).

#### alacritty_terminal: the wrap flag only while autowrap is on

- Upstream today: `Term::input` sets `Cursor::input_needs_wrap` after writing the last column whether autowrap is on or
  off, and also when it drops a wide character that does not fit with autowrap off. The crate documents the flag as
  "Tracks if the next call to input will need to first handle wrapping", and with autowrap off nothing wraps
  (`Term::wrapline` returns at once). Every reader of the flag then takes a cursor on the last column for one past it:
  `CSI K` keeps the last character, the next character after autowrap is turned back on wraps, so the corner-cell idiom
  (autowrap off, write the bottom-right cell, autowrap on) scrolls the screen, a resize moves the cursor one column too
  far, and a combining mark joins the last column, not the cell before the cursor.
- Reproduction (4x2), observed with the published crate:
  - `ESC [ ? 7 l abcd ESC [ K ESC [ ? 7 h`, and `ESC [ ? 7 l abcd ESC [ ? 7 h ESC [ K`. Observed first row in both:
    `abcd`. Expected (tmux): `abc `.
  - `ESC [ ? 7 l abcd ESC [ ? 7 h x`. Observed rows `abcd`, `x   `, cursor (1, 1). Expected: `abcx`, `    `, with a wrap
    pending on line 0.
  - `ESC [ ? 7 l abc 一 ESC [ ? 7 h x` (the wide character does not fit at the last column and is dropped). Observed
    rows `abc `, `x   `, cursor (1, 1). Expected (tmux): `abcx`, `    `.
  - The corner-cell idiom: `ESC [ 2 ; 1 H wxyz ESC [ 1 ; 1 H ESC [ ? 7 l ESC [ 2 ; 4 H q ESC [ ? 7 h r`. Observed rows
    `wxyq`, `r   ` (scrolled). Expected: `    `, `wxyr`.
  - `ESC [ ? 7 l abcd ESC [ ? 7 h`, then a resize to 6x2. Observed cursor (0, 4). Expected (0, 3).
  - `ESC [ ? 7 l abcd ESC [ ? 7 h` U+0301. Observed: the mark on `d`. Expected (tmux): on `c`.
- tmux keeps the cursor on the last column when it is written with autowrap off and one past it (`cx == sx`) when it is
  written with autowrap on (`screen-write.c`, `screen_write_cell`, `sx - not_wrap`); turning autowrap on or off later
  only flips the mode and does not move the cursor (`input.c`, the DECAWM cases of `input_csi_dispatch_sm_private` and
  `input_csi_dispatch_rm_private`). From there `CSI K` skips only for `cx > sx - 1` (`screen_write_clearendofline`), the
  next character wraps only for `cx > sx - width` (`screen_write_cell`), a combining mark joins the cell at `cx - 1`
  (`screen_write_combine`), and a wide character dropped with autowrap off leaves the cursor where it is.

  xterm and xterm.js side with upstream more often than with tmux (read from their source, not run). xterm raises its
  pending-wrap flag, `do_wrap`, whenever text fills the line up to the right margin, whether autowrap is on or not
  (`charproc.c`, `dotext`: `need_wrap` and `screen->do_wrap = need_wrap`), and only acts on it if autowrap is on when
  the next character comes; DECAWM itself only flips the mode (`dpmodes`, `srm_DECAWM`). So in xterm, as upstream, the
  next character after autowrap is turned back on wraps, the corner-cell idiom scrolls, and a combining mark joins `d`,
  the last cell written (`last_written_col`, in `doparsing`'s combining branch). xterm erases `d` on `CSI K`, as tmux
  does, because its erases start at the cursor's column whatever the flag (`util.c`, `ClearRight`); and a wider screen
  leaves its cursor on column 3 (`screen.c`, `ScreenResize` moves the cursor only when it is past the new last column).
  What xterm does with a wide character that does not fit with autowrap off depends on how the text is batched (`dotext`
  moves a batch's last character into the last cell that fits) and was not worked out. xterm.js moves its cursor to
  `x == cols` after writing the last column whatever autowrap is (`src/common/InputHandler.ts`, `print`), and only a
  character that arrives with autowrap on wraps from there; its `CSI K` and `CSI J` first clamp the cursor to `cols`,
  not `cols - 1` (`_restrictCursor(this._bufferService.cols)` in `eraseInLine` and `eraseInDisplay`), so they erase
  nothing of the line and keep `d`; and a combining mark joins the cell before `x`, which is `d`. Like tmux it drops a
  wide character that does not fit with autowrap off with the cursor on the last column, so the following `x` lands
  there. In the cases above, then, xterm agrees with the patch on `CSI K` and the resize, and with upstream on the wrap,
  the corner-cell idiom and the combining mark; xterm.js agrees with the patch on the dropped wide character only (its
  resize was not checked).
- Fix: `Term::input` raises the flag at the end of a last-column write only while autowrap is on, and returns without
  raising it when it drops a wide character with autowrap off (a flag already raised stays raised, since nothing moves).
  Every other reader of the flag (except `CSI J`, under its own record below) keeps upstream's code; with the flag
  meaning what its documentation says, they behave as tmux does in the cases above. Farhelm's cursor report
  (`farhelm-terminal-emulator`, `src/queries.rs`) reads the flag as a cursor one past the last column.
- Why Farhelm needs it: a kept last-column character or an extra scroll changes the screen status detection reads and
  viewers see, and the status readers were written against the screens Farhelm's terminals showed before terminal hosts,
  which neither keep that character nor scroll. The recorded Claude Code and Codex sessions never turn autowrap off, so
  this matters for other programs in a session's terminal.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::input`.
- Tests: `term::tests::writing_the_last_column_with_autowrap_off_raises_no_wrap` (vendored); the cursor-report rows of
  `identification_status_and_window_queries` (`farhelm-terminal-emulator`, `tests/queries.rs`). Until 2026-11 the
  supervisor's comparison with a live tmux 3.7c also covered scenarios "autowrap off at the last column: reports and
  erases", "autowrap off at the last column, then a resize" and "autowrap off at the last column of the alternate
  screen, then a wider screen", and pinned what still differs from tmux at a pending wrap (a tab, a wider alternate
  screen, a wide character after the corner-cell idiom, writing after autowrap is turned off); it has since been
  removed.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -1112,8 +1585,12 @@` (no flag for a wide character dropped with
  autowrap off) and `@@ -1130,9 +1607,13 @@` (no flag for a last-column write with autowrap off); the test in
  `@@ -3299,4 +3832,1060 @@`.
- Upstream status: not yet reported. Arguable upstream: it makes the flag mean what the crate documents, but xterm and
  xterm.js both keep a wrap pending through a last-column write with autowrap off (above), so upstream can point at
  either for its current behavior; the case for the change is tmux and the crate's own documentation. The hunk for the
  dropped wide character is the easiest part to argue, since xterm.js and tmux agree with it. Checked against alacritty
  `aecf0f6` on 2026-11-03: still present.

#### alacritty_terminal: `CSI J` with a wrap pending keeps the last column

- Upstream today: after a program writes the last column with autowrap on (a wrap is pending), `CSI J`
  (`ClearMode::Below`) erases from the cursor's column, which is the character just written. Upstream's own `CSI K`
  (`clear_line`, `Right`) already leaves it when a wrap is pending.
- Reproduction (4x2): `ESC [ 2 ; 1 H zzzz ESC [ 1 ; 1 H abcd ESC [ J`. Observed rows: `abc `, `    `. Expected (tmux):
  `abcd`, `    `. The same holds when autowrap is turned off after the write: `abcd ESC [ ? 7 l ESC [ J ESC [ ? 7 h`,
  observed `abc `, expected `abcd`.
- tmux keeps the cursor one past the last column (`cx == sx`) when the last column is written with autowrap on
  (`screen-write.c`, `screen_write_cell`), and later DECAWM changes do not move it (`input.c`). `CSI J` clears the
  cursor's line only `if (s->cx <= sx - 1)` (`screen_write_clearendofscreen`). xterm.js erases from its pending position
  `x == cols` and so keeps the character too. xterm erases it: it keeps the cursor on the last column and `ClearRight`
  (`util.c`) erases from `cur_col`, for `CSI J` and `CSI K` alike.
- Fix: erasing below starts after the cursor's column when `input_needs_wrap` is set. Ordinary input raises the flag
  only with autowrap on, so an ordinary final-column write with autowrap off is erased, as tmux erases it. A reflowing
  resize can also raise the flag, as upstream does. Selector widening into the final pair raises it even with autowrap
  off, per the emoji-width record, and an erase then keeps that pair.
- Why Farhelm needs it: the erased character changes the screen status detection reads, and the status readers were
  written against the screens Farhelm's terminals showed before terminal hosts, which keep it.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::clear_screen`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -1774,7 +2259,15 @@`; the test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::erase_below_with_a_wrap_pending_keeps_the_last_column` (vendored: written with autowrap on, off,
  off then on, on then off, and on then a wide character dropped with autowrap off). Until 2026-11 also the supervisor's
  comparison with a live tmux 3.7c, scenarios "erase operations" and "autowrap off at the last column: reports and
  erases" (since removed).
- Upstream status: not yet reported. Arguable upstream: it makes `CSI J` agree with alacritty's own `CSI K`, but xterm
  does the opposite for both, so upstream may prefer xterm's behavior. On its own, without the wrap-flag fix, it would
  also keep a character written with autowrap off, which tmux erases (and xterm.js keeps). Checked against alacritty
  `aecf0f6` on 2026-11-03: still present.

#### alacritty_terminal: relative vertical moves stop at the scroll margins

- Upstream today: `move_up`, `move_down`, `move_up_and_cr` and `move_down_and_cr` (CUU, CUD, CPL, CNL) compute the
  target line and call `goto`, which clamps to the screen only, so a move from inside the scroll region passes its
  margin; in origin mode `goto` adds the region's offset to the already absolute line.
- Reproduction (4x6, region lines 2 to 4): `ESC [ 2 ; 4 r ESC [ 3 ; 2 H ESC [ 9 A`. Observed cursor (0, 1). Expected (1,
  1), the top margin. In origin mode: `ESC [ 2 ; 4 r ESC [ ? 6 h ESC [ 1 B`. Observed (3, 0). Expected (2, 0).
- xterm stops at the margin in the direction of travel unless the cursor starts beyond it (above the top margin for CUU,
  below the bottom margin for CUD), and then at the screen's edge (`cursor.c`, `CursorUp`, `CursorDown`): a cursor below
  the region moving up stops at the top margin. In origin mode it moves from the absolute row. tmux does the same
  (`screen-write.c`, `screen_write_cursorup`, `screen_write_cursordown`). xterm.js stops at the margins too, but in
  origin mode adds the offset a second time, as upstream does.
- Fix: a `vertical_move` helper that applies those limits and sets the cursor directly. The count is clamped to the
  screen's height first, so the addition cannot overflow.
- Why Farhelm needs it: programs that move relative to the cursor inside a scroll region would draw in the wrong place
  on the screen agents' status is read from and viewers see.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::move_up`, `move_down`, `move_up_and_cr`,
  `move_down_and_cr`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -1017,6 +1328,36 @@` (`vertical_move`, which the CHA record
  uses too), `@@ -1215,18 +1698,18 @@` (`move_up`, `move_down`), `@@ -1349,16 +1832,16 @@` (`move_down_and_cr`,
  `move_up_and_cr`); the test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::relative_vertical_moves_stop_at_the_scroll_margins` (vendored). Until 2026-11 also the
  supervisor's comparison with a live tmux, scenario "scroll regions" and generated streams (since removed).
- Upstream status: not yet reported. Upstreamable as is. Checked against alacritty `aecf0f6` on 2026-11-03: still
  present.

#### alacritty_terminal: CHA in origin mode keeps the cursor's line

- Upstream today: `goto_col` (CHA, `CSI Ps G`) calls `goto` with the cursor's absolute line, which in origin mode adds
  the region's offset again and moves the cursor down.
- Reproduction (4x6): `ESC [ 2 ; 4 r ESC [ ? 6 h ESC [ 2 ; 1 H ESC [ 3 G`. Observed cursor (3, 2). Expected (2, 2).
- xterm keeps the line (`charproc.c`, `CASE_HPA`, which passes `CursorRow`, the line relative to the region, to
  `CursorSet`); tmux leaves the line alone (`input.c`, `INPUT_CSI_HPA`, `screen_write_cursormove` with `py = -1`).
  xterm.js adds the offset a second time, as upstream does.
- Fix: `goto_col` moves through `vertical_move` with no lines to move.
- Why Farhelm needs it: a program that sets a scroll region and origin mode and then positions with CHA would draw each
  such line the region's offset too far down, on the screen status detection reads and viewers see. No recorded agent
  session was checked for origin mode; the comparison with a live tmux, since removed, covered this case.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::goto_col`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -1180,7 +1661,9 @@`, which needs `vertical_move` from
  `@@ -1017,6 +1328,36 @@` (the relative moves record); the test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::cha_in_origin_mode_keeps_the_line` (vendored). Until 2026-11 also the supervisor's comparison
  with a live tmux, scenario "scroll regions" (since removed).
- Upstream status: not yet reported. Upstreamable as is together with the relative moves fix; filed alone, it would
  instead pass `goto` the line relative to the region, as xterm's `CASE_HPA` does. Checked against alacritty `aecf0f6`
  on 2026-11-03: still present.

#### alacritty_terminal: resetting origin mode homes the cursor

- Upstream today: setting origin mode (`CSI ? 6 h`) homes the cursor, resetting it (`CSI ? 6 l`) leaves the cursor where
  it is.
- Reproduction (4x6): `ESC [ 2 ; 4 r ESC [ ? 6 h ESC [ 2 ; 2 H ESC [ ? 6 l`. Observed cursor (2, 1). Expected (0, 0).
- xterm homes it on both (`charproc.c`, `srm_DECOM` in `dpmodes`, `CursorSet(screen, 0, 0, ...)`); so does tmux
  (`input.c`, `input_csi_dispatch_rm_private`, mode 6) and xterm.js.
- Fix: the reset calls `goto(0, 0)` as the set does.
- Why Farhelm needs it: a program that leaves origin mode and draws without moving the cursor first would draw where its
  cursor was rather than at the top left, on the screen status detection reads and viewers see. As for CHA, no recorded
  agent session was checked for origin mode; the comparison with a live tmux, since removed, covered it.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::unset_private_mode`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -2031,7 +2545,12 @@`; the test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::resetting_origin_mode_homes_the_cursor` (vendored). Until 2026-11 also the supervisor's
  comparison with a live tmux on generated streams (since removed).
- Upstream status: not yet reported. Upstreamable as is. Checked against alacritty `aecf0f6` on 2026-11-03: still
  present.

#### alacritty_terminal: the UTF-8 and SGR mouse encodings are separate modes

- Upstream today: `Term::set_private_mode` treats the UTF-8 (1005) and SGR (1006) mouse encodings as mutually exclusive
  ("Mouse encodings are mutually exclusive"): setting either clears the other's `TermMode` bit, so the one set last is
  the only one left, and DECRQM reports the other as reset.
- Reproduction (10x2), observed with the published crate: `ESC [ ? 1006 h ESC [ ? 1005 h`, then
  `ESC [ ? 1005 $ p ESC [ ? 1006 $ p`. Observed: `UTF8_MOUSE` set, `SGR_MOUSE` clear, answers `ESC [ ? 1005 ; 1 $ y` and
  `ESC [ ? 1006 ; 2 $ y`. Expected (tmux): both set, both answered `; 1 $ y`, and reports in SGR. The other order
  (`ESC [ ? 1005 h ESC [ ? 1006 h`, or `ESC [ ? 1005 ; 1006 h`) leaves only `SGR_MOUSE`, where tmux has both.
- tmux 3.7c keeps them as separate bits (`tmux.h`, `MODE_MOUSE_UTF8` and `MODE_MOUSE_SGR`), sets and resets each on its
  own (`input.c`, `input_csi_dispatch_sm_private` and `input_csi_dispatch_rm_private`, cases 1005 and 1006), answers
  DECRQM for each from its own bit (`input.c`, `input_csi_dispatch`, `INPUT_CSI_QUERY_PRIVATE`), and encodes a mouse
  event in SGR when 1006 is set and the event reached tmux in SGR, otherwise in UTF-8 when 1005 is set (`input-keys.c`,
  `input_key_get_mouse`: `m->sgr_type != ' ' && (s->mode & MODE_MOUSE_SGR)`; a legacy report cannot say which button was
  released, so it cannot become SGR). In practice the event is SGR whenever a pane asks for any mouse mode, because tmux
  then turns 1006 on in its own outer terminal (`tty.c`, `tty_update_mode`), so a pane with both set gets SGR. Farhelm's
  grid events carry the button directly, so the encoder needs no such fallback and uses SGR whenever 1006 is set.
  xterm.js 6.0.0 does not implement 1005 at all ("DECSET 1005 not supported"; DECRQM 1005 answers 4, permanently reset),
  so 1006 stays in force there whatever follows it. xterm, which upstream follows, makes all its coordinate modes (1005,
  1006, 1015, 1016) exclusive on purpose, the last one set winning, and resets one only if it is the active one
  (`charproc.c`, `dpmodes`, the `srm_EXT_MODE_MOUSE` group: "Rather than choose an arbitrary precedence among the
  coordinate modes, they are mutually exclusive").
- Fix: setting 1005 or 1006 sets only its own bit; resetting was already independent. An embedder that sees both uses
  SGR; the emulator's `Modes::from_term_mode` already reads SGR first (`farhelm-terminal-emulator`, `src/modes.rs`).
- Why Farhelm needs it: the supervisor encodes viewers' mouse events from these modes, and a program that asks for SGR
  and then also for UTF-8 keeps getting SGR reports, as it always did in Farhelm's terminals; without the patch the
  emulator would switch it to UTF-8 coordinates. DECRQM reports each mode from its own bit too.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::set_private_mode`.
- Patch hunks (`alacritty_terminal`, `term/mod.rs`): `@@ -1968,15 +2484,13 @@`; the test in `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::utf8_and_sgr_mouse_encodings_are_independent` (vendored: both orders, one sequence with both, and
  resetting either); `mouse_modes_replace_one_another_but_encodings_combine` (`farhelm-terminal-emulator`,
  `tests/modes.rs`); `both_mouse_encodings_set_round_trip` (`tests/snapshot.rs`); and the encoder's
  `sgr_wins_over_utf8_whichever_was_set_first` (`farhelm-supervisor`, `grid_input`). Until 2026-11 also two rows of the
  supervisor's comparison with a live tmux 3.7c (both orders; since removed).
- Upstream status: not yet reported. Not a bug fix: the exclusion follows xterm on purpose, so this is a question of
  which reference to follow, and is better raised upstream as a question. The alacritty application already reads the
  two bits the way this patch needs: its `mouse_report` checks `SGR_MOUSE` first and sends SGR whenever it is set, and
  only otherwise falls back to the normal encoding, with UTF-8 coordinates if `UTF8_MOUSE` is set
  (`alacritty/src/input/mod.rs`, `mouse_report` and `normal_mouse_report`, at `aecf0f6`). With both bits kept, then,
  alacritty itself would report in SGR after `CSI ? 1006 h CSI ? 1005 h`, as tmux does, where today it reports in UTF-8.
  Checked against alacritty `aecf0f6` on 2026-11-03: still present.

#### alacritty_terminal: a per-terminal limit on cell extra storage

- Upstream today: a cell keeps combining characters, an underline color and an OSC 8 hyperlink in a separate
  `Arc<CellExtra>`, shared by the cells printed from one cursor template and copied on write (`Arc::make_mut`) whenever
  one cell's set changes. Nothing bounds how many there are, so output that varies them from cell to cell gives every
  cell its own allocation, and a hyperlink sent without an `id` is always a new link. One terminal's memory then grows
  with its scrollback times the size of an extra and its link.
- Reproduction (200x60, `scrolling_history` 12,000): 12,060 lines that each repeat, 200 times,
  `ESC [ 58 ; 2 ; <r> ; <c> ; <g> m` (a different underline color per cell), then `e` and sixteen U+0301. Measured
  through Farhelm's emulator, not the bare crates, by the ignored measurements in `farhelm-terminal-emulator`'s
  `tests/memory.rs` (`combining_marks_and_underline_colors_on_every_cell`): 387 MiB resident before the limit existed,
  against 56 MiB for the same grid of plain text, and 120 MiB with it. With
  `ESC ] 8 ; id=<unique 250 bytes> ; <2083-byte URI> ESC \ x` per cell: an estimated 6 GB, the links' size times the
  cells, never generated (`a_new_maximal_hyperlink_on_every_cell` generates maximal links only until the limit is
  reached). Without any combining marks, saving a template with an underline color and a link
  (`ESC ] 8 ; id=keep ; http://k/ ESC \ ESC [ 58 ; 5 ; 3 m ESC 7 ESC [ 59 m ESC ] 8 ; ; ESC \`), then repeating `ESC 8`,
  a cursor move and `ESC [ 59 m x` once per cell also gives every cell a copy: a review measured 122 MiB after 4,000
  rows on an earlier Farhelm build, whose extra storage did not yet check such copies
  (`a_restored_saved_cursor_before_every_character`). Expected with the patch: at most `MAX_EXTRA_STORAGE_BYTES` (64
  MiB) charged.
- tmux 3.7c keeps at most 32 bytes of UTF-8 per cell (`tmux.h`, `UTF8_SIZE`) and at most 5,000 hyperlinks in the whole
  server, evicting the oldest (`hyperlinks.c`, `MAX_HYPERLINKS`); its extended cell attributes are a small fixed-size
  entry per cell. xterm has none of the three: no underline color (its SGR handling in `charproc.c`, `CASE_SGR`, has no
  58 or 59) and no OSC 8 (above), and its combining characters live in a fixed number of per-line arrays, one entry per
  cell each, allocated with the line (`ptyx.h`, `LineData`: `combSize` arrays in `combData`, sized by the
  `combiningChars` resource, at most 5; the combining character record above). So xterm's memory per line is fixed by
  its width, and nothing a program prints makes a cell bigger.
- Fix: each `Term` has an `ExtraStorage`, a byte count shared through an `Arc<AtomicUsize>` by everything charged to it.
  A `CellExtra` is charged `CELL_EXTRA_COST` (160 bytes) when `ExtraStorage::adopt` takes on a fresh one and again when
  a charged one is copied (the charge's `Clone`); a hyperlink made by `ExtraStorage::link` is charged its `id` and URI
  lengths plus `HYPERLINK_COST` (128); every charge is refunded in `Drop`. Every path that could make a new extra goes
  through `ExtraStorage`'s setters (`push_zerowidth`, `set_underline_color`, `set_hyperlink`, `clear_wide`), which check
  for room first; without room they degrade the cell (a combining mark dropped, an underline color falling back to the
  default, a link ended, or, when removing an attribute would copy a shared extra, the cell's whole extra dropped), so
  the count never exceeds `MAX_EXTRA_STORAGE_BYTES`.
- Why Farhelm needs it: every terminal's emulator runs in one long-lived supervisor, where one program must not be able
  to grow the shared process without bound. The per-terminal memory budget rests on this limit
  (`farhelm-terminal-emulator`, `src/lib.rs`, "Memory budget").
- Upstream code: `alacritty_terminal/src/term/cell.rs`, `ExtraStorage`, `StorageCharge`, the `charge` field of
  `CellExtra` and `HyperlinkInner`; `alacritty_terminal/src/term/mod.rs`, `MAX_EXTRA_STORAGE_BYTES`, `CELL_EXTRA_COST`,
  `HYPERLINK_COST`, the `extra_storage` field and accessor, and the calls in `Term::input` (combining characters),
  `Term::write_at_cursor` (`clear_wide`), `Term::set_hyperlink` and `Term::terminal_attribute` (`Attr::UnderlineColor`,
  `Attr::Reset`).
- Patch hunks (`alacritty_terminal`): in `term/cell.rs`, `@@ -1,5 +1,12 @@` beyond the header (the `Hash`, `Hasher` and
  `AtomicUsize` imports), `@@ -72,6 +91,177 @@` (`ExtraStorage`, `StorageCharge`), `@@ -80,6 +270,10 @@` and
  `@@ -93,7 +287,7 @@` (`HyperlinkInner`'s charge), `@@ -126,6 +320,10 @@` (`CellExtra`'s charge); in `term/mod.rs`,
  `@@ -1,23 +1,35 @@` (the import), `@@ -41,6 +53,63 @@`, its `MAX_EXTRA_STORAGE_BYTES`, `CELL_EXTRA_COST` and
  `HYPERLINK_COST`, `@@ -327,6 +396,14 @@` and `@@ -441,15 +518,18 @@` (the field), `@@ -953,6 +1033,235 @@`, its last
  accessor, `extra_storage()`, `@@ -998,7 +1307,9 @@` (`write_at_cursor`), `@@ -1080,7 +1543,17 @@`, its
  `push_zerowidth` change, `@@ -1873,7 +2366,26 @@`, its last two statements, `@@ -1881,15 +2393,19 @@`
  (`terminal_attribute`); the tests in `@@ -3299,4 +3832,1060 @@`. `ExtraStorage`'s setters also return early when
  nothing changes, the same check as the next record's, at this layer.
- Tests: `term::tests::hyperlinks_are_bounded_by_the_extra_storage`,
  `term::tests::combining_characters_and_underline_colors_are_bounded` and
  `term::tests::restoring_a_saved_cursor_cannot_grow_the_storage_past_the_limit` (vendored);
  `floods_of_cell_extras_are_bounded` (`farhelm-terminal-emulator`, `tests/hostile.rs`),
  `fixups_share_extras_and_respect_the_storage` (`tests/snapshot.rs`), and the ignored measurements in
  `tests/memory.rs`.
- Upstream status: not yet reported. The limit itself is Farhelm policy (one process hosting many terminals); alacritty
  runs one terminal per window, where unbounded growth is the user's own program's doing. An upstream-friendly version
  would make the limit a `Config` field (`max_extra_storage: Option<usize>`, `None` meaning unlimited, as today), keep
  the accounting, and leave the degrade behavior as documented here. Checked against alacritty `aecf0f6` on 2026-11-03:
  still unbounded in the number of extras (upstream's new nine-character cap, in the combining character record, bounds
  only one extra's size).

#### alacritty_terminal: a cell's setters leave its extra alone when nothing changes

- Upstream today: `Cell::set_underline_color` and `Cell::set_hyperlink` call `Arc::make_mut` on the cell's extra even
  when the value is the one it already has, and `Cell::clear_wide` does so even when there are no combining characters
  to clear. On the cursor template, whose extra the printed cells share, each such call copies the extra.
- Reproduction (100x20, no history): `ESC ] 8 ; id=a ; http://e/ ESC \`, then `ESC [ 59 m ESC [ 0 m x` repeated 1,000
  times. Observed: a new `CellExtra` for every `x` (1,000 extras holding the same link). Expected: one extra shared by
  all 1,000 cells.
- xterm and tmux: not applicable, as neither shares per-cell storage this way.
- Fix: the setters return early when the value would not change, after their existing "drop an empty extra" branch;
  `clear_wide` touches the extra only when it holds combining characters. No visible state changes.
- Why Farhelm needs it: only the `clear_wide` hunk is reached in Farhelm's tree. `ExtraStorage::clear_wide` calls
  `Cell::clear_wide` unconditionally, and `Term::write_at_cursor` reaches it when a wide character is overwritten, where
  the condition keeps it from copying an extra other cells share when there are no combining characters to clear. The
  two setter hunks never see an unchanged value here: the extra storage's `set_underline_color` and `set_hyperlink`
  return first, and neither the terminal nor Farhelm's own code calls the `Cell` setters directly outside a unit test.
  They matter for upstream code without the extra storage, where a program repeating `SGR 0` before every character
  while a link is active makes a copy per cell, and are kept so that this change can go upstream on its own.
- Upstream code: `alacritty_terminal/src/term/cell.rs`, `Cell::set_underline_color`, `Cell::set_hyperlink`,
  `Cell::clear_wide`.
- Patch hunks (`alacritty_terminal`, `term/cell.rs`): `@@ -170,7 +372,9 @@` (`clear_wide`), `@@ -186,7 +390,10 @@`
  (`set_underline_color`), `@@ -208,7 +415,9 @@` (`set_hyperlink`).
- Tests: none isolates these hunks. `term::tests::repeating_an_unchanged_attribute_copies_nothing` (vendored) pins the
  behavior through the terminal, but the extra storage's setters return before calling `Cell::set_underline_color` or
  `Cell::set_hyperlink` when nothing changes, and nothing tests `clear_wide` on a shared extra without combining
  characters. Observed on 2026-11-03: with the three hunks reverted in a scratch copy of the vendored tree, that test
  and all 151 of the crate's library tests still pass (recorded runs `e25d8901` and `1e2415ba`, local). An upstream
  version without the extra storage would test the `Cell` setters directly, by checking with `Arc::ptr_eq` that setting
  an unchanged value on a cell whose extra another cell shares keeps them sharing.
- Upstream status: not yet reported. Upstreamable as is: a pure saving of memory and allocation with no behavior change.
  Checked against alacritty `aecf0f6` on 2026-11-03: still present.

#### alacritty_terminal: clearing a row after it grew

- Upstream today: `Row::reset` skips the cells from `occ` on when the row's last cell already has the template's
  background, trusting that those cells hold a blank of that background. `Row::grow` appends default cells without
  counting the old ones in `occ`. After a row was reset with a colored background (which leaves `occ` at 0) and then
  narrowed and widened again, a reset with the default background skips the old cells and leaves their color.
- Reproduction (10x3, no history): `ESC [ ? 1049 h ESC [ 44 m ESC [ 2 J` (the alternate screen resizes without
  reflowing), resize to 4x3 and back to 10x3, then `ESC [ 0 m ESC [ 2 J`. Observed: cells 0 to 3 of every row keep a
  blue background (`Color::Named(NamedColor::Blue)`). Expected: every cell the default background. In alacritty itself
  this shows as stale colored blocks after a full-screen program with a colored background clears its screen once the
  window was made narrower and wider again.
- tmux clears every cell of the screen on `CSI 2 J`, whatever resizes came before (`screen-write.c`,
  `screen_write_clearscreen`, through `grid_view_clear`). So does xterm: `CSI 2 J` ends in `ClearBufRows` over every row
  (`util.c`, `ClearScreen`), which clears every cell of the current width (`screen.c`, `ClearBufRows`); in builds with
  `OPT_RESIZE_ADJUST` (off by default, `ptyx.h`), where a row keeps cells past the current width, it clears up to the
  widest the row has been (`maxLineSize`).
- Fix: `Row::grow` sets `occ` to at least the old length before appending, so the old cells count as occupied.
- Why Farhelm needs it: correct screens, and because a snapshot cannot reproduce a row's `occ`, the stale colors made a
  restored terminal differ from the original after the next clear (the snapshot round-trip test found it).
- Upstream code: `alacritty_terminal/src/grid/row.rs`, `Row::grow`.
- Patch hunks (`alacritty_terminal`): `grid/row.rs` `@@ -62,6 +65,14 @@`; the test in `term/mod.rs`
  `@@ -3299,4 +3832,1060 @@`.
- Tests: `term::tests::clearing_after_a_resize_clears_every_cell` (vendored; fails without the fix).
- Upstream status: not yet reported. Upstreamable as is. Checked against alacritty `aecf0f6` on 2026-11-03: still
  present (`Row::grow` unchanged; only `Row::new` changed in that file).

#### Not a patch: modes 47 and 1047 built on alacritty's screen switch

NOTE: This record is about Farhelm's own code, not the vendored trees. It is here because it works around upstream
behavior, which upstream gets as an issue, not a patch.

- Upstream today: `Term` ignores DECSET and DECRST 47 and 1047 (vte names neither, so they arrive as
  `PrivateMode::Unknown`, and `Term::set_private_mode` and `unset_private_mode` log "Ignoring unknown mode" and return),
  so a program that uses them to show a full-screen view draws over the primary screen. The crate's only screen switch,
  `Term::swap_alt`, is shaped for 1049: on entering it copies the primary screen's cursor over that screen's saved
  cursor ("Drop information about the primary screens saved cursor"), and on leaving it shows the primary screen with
  the cursor it had on entering, since each grid keeps its own cursor. Both are right for 1049, which saves the cursor
  on entering and restores it on leaving, and wrong for 47 and 1047, which do neither, so an embedder cannot build them
  on `swap_alt` alone.
- Reproduction (10x3), observed on 2026-11-03 with the published crates and with the patched trees alike (the throwaway
  program of the OSC limit record, in the same run): `a b ESC 7 CR LF ESC [ ? 47 h z ESC [ ? 47 l ESC 8 X`. Observed:
  the alternate screen never shows (`ALT_SCREEN` stays clear after `ESC [ ? 47 h` and after `ESC [ ? 1047 h`), and the
  rows are `abX`, `z`, blank, the `z` left on the primary screen (the rows were run with 47 only). Expected: `abX`,
  blank, blank, the `z` drawn on the alternate screen and gone with it. Through `swap_alt` instead (`a b ESC 7 CR LF`,
  `swap_alt()`, `z`, `swap_alt()`, `ESC 8 X`): back on the primary screen the cursor is at (1, 0), where it was on
  entering, not (1, 1), where the `z` left it, and the saved cursor is (1, 0) instead of the (0, 2) that `ESC 7` saved,
  so the rows end up `ab`, `X`, blank.
- xterm: 47 switches buffers and nothing else, and 1047 also clears the alternate screen when leaving it (`ctlseqs.ms`:
  "Use Alternate Screen Buffer", "Use Normal Screen Buffer", and for 1047 "Clear the screen first if in the Alternate
  Screen Buffer"; 1049 "combines the effects of the 1047 and 1048 modes"). In the source, `dpmodes` (`charproc.c`) calls
  `ToAlternate` and `FromAlternate` for `srm_ALTBUF` (47) and `srm_OPT_ALTBUF` (1047) with no `CursorSave` or
  `CursorRestore`, which only `srm_OPT_ALTBUF_CURSOR` (1049) adds. xterm keeps one cursor position for both buffers and
  a saved cursor per buffer (`cursor.c`, `CursorSave` into `screen->sc[screen->whichBuf]`), so neither mode moves the
  cursor or touches a saved cursor. tmux 3.7c: 47 and 1047 call `screen_write_alternateon` and
  `screen_write_alternateoff` with `cursor` 0 (`input.c`), so `screen_alternate_on` (`screen.c`) clears the alternate
  grid and saves no cursor, and `screen_alternate_off` restores none; tmux keeps one cursor too. xterm.js 6.0.0: 47 and
  1047 call `activateAltBuffer` and `activateNormalBuffer` (`src/common/InputHandler.ts`), which fill the alternate
  buffer blank, carry the cursor position across in both directions, and clear the alternate buffer on leaving
  (`src/common/buffer/BufferSet.ts`); only 1049 adds `saveCursor` and `restoreCursor`, and each buffer has its own saved
  cursor. All three leave the cursor where the program left it and the saved cursor alone. They differ on clearing:
  xterm clears for 1047 on leaving and never for 47; tmux clears on entering and xterm.js on leaving (and fills it blank
  on entering), so both enter a blank alternate screen for either mode.
- What Farhelm does: the emulator's handler (`crates/farhelm-terminal-emulator/src/queries.rs`,
  `QueryHandler::enter_alternate_screen` and `leave_alternate_screen`, module docs "Beyond queries") takes 47 and 1047
  before alacritty sees them and calls `swap_alt`, then undoes what `swap_alt` does for 1049: on entering it puts the
  primary screen's saved cursor back, through `Term::inactive_grid_mut()` (the accessor record above), and on leaving it
  carries the alternate screen's cursor, position and pen, over to the primary screen. It follows tmux in clearing the
  alternate screen on entering for both modes. DECRQM reports both as the alternate screen's state. Mixing them with
  1049 (entering with one and leaving with the other) still leaves the cursor elsewhere than tmux would, because
  alacritty keeps a cursor per screen where tmux keeps one; that difference is pinned rather than fixed.
- Why Farhelm needs it: a program that uses either mode for a full-screen view would otherwise draw over the shell's
  screen, which status detection reads and viewers see. Farhelm's terminals had both modes before terminal hosts, so
  leaving them out would have taken them away from programs that use them.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::swap_alt`, `Term::set_private_mode`,
  `Term::unset_private_mode`; `vte/src/ansi.rs`, `PrivateMode::new` (no names for 47 and 1047).
- Patch hunks: none. It depends on the `inactive_grid_mut()` accessor (`term/mod.rs` `@@ -953,6 +1033,235 @@`).
- Tests (`farhelm-terminal-emulator`): `modes_47_and_1047_show_the_alternate_screen` (`tests/modes.rs`);
  `modes_47_and_1047_read_the_alternate_screen` (`tests/screen.rs`: the alternate screen captured, the cursor kept where
  the program left it, the saved cursor case above, a second entry clearing the screen, and the two mixes with 1049
  pinned as what the emulator does); and the 47 and 1047 rows of `mode_reports_on_a_fresh_terminal` and
  `mode_reports_follow_mode_changes` (`tests/queries.rs`). `tests/delegation.rs`, which compares the handler with a bare
  `Term` on generated output, leaves these modes out on purpose, since a bare `Term` ignores them.
- Upstream status: not yet reported. An issue for alacritty, since the code is Farhelm's and uses only `Term`'s public
  API (plus the accessor patch). An upstream fix would handle 47 and 1047 in `set_private_mode` and `unset_private_mode`
  (as `PrivateMode::Unknown(47 | 1047)`, or with names added to vte) with a switch that leaves the saved cursor alone
  and keeps the cursor position across, as xterm, tmux and xterm.js do; which of them to follow on clearing is
  upstream's call. Checked against alacritty `aecf0f6` and vte `89c9c22` on 2026-11-03: still ignored, and `swap_alt`
  unchanged.

#### Hunks shared between records

Splitting the patch files into one upstream change per record means dividing these hunks; every other hunk belongs to
one record (and every file's header hunk to all of that file's records).

- vte `ansi.rs` `@@ -363,6 +369,33 @@`: `is_ground` (the parser state record) and `preceding_char`, `set_preceding_char`
  (the REP record).
- vte `lib.rs` `@@ -64,6 +86,13 @@` and `@@ -548,14 +589,32 @@`: the OSC payload limit and the overflow report. The
  limit alone needs a flag where the field holds a count, and in `osc_end` a plain drop where the patch calls
  `osc_overflow_dispatch`; the shrink after every OSC goes with the limit. The test performer's `OscOverflow`
  (`@@ -850,6 +937,8 @@`, `@@ -865,6 +954,12 @@`) and `osc_over_payload_limit_is_dropped`'s expectation of one
  `OscOverflow` belong to the report.
- `alacritty_terminal` `term/mod.rs` `@@ -41,6 +53,63 @@`: constants for the hyperlink length, extra storage and title
  records.
- `term/mod.rs` `@@ -953,6 +1033,235 @@`: the restore accessors, `extra_storage()` for the extra storage record, and the
  following switch, REP policy and selector helper for the emoji-width record.
- `term/mod.rs` `@@ -1017,6 +1328,36 @@`: `vertical_move`, for the relative moves and CHA records.
- `term/mod.rs` `@@ -1080,7 +1543,17 @@`: the extra storage's `push_zerowidth` call and the one-column wide character
  check.
- `term/mod.rs` `@@ -1873,7 +2366,26 @@` (`Term::set_hyperlink`): the length filter (hyperlink length), the early return
  (hyperlink reuse) and the last two statements (extra storage). Upstream's version of either of the first two keeps
  upstream's `self.grid.cursor.template.set_hyperlink(...)` as the last line.
- The test hunks, one per crate file: vte `ansi.rs` `@@ -2437,6 +2504,105 @@` (the overflow report's, XTVERSION's and
  emoji-width REP-policy tests), vte `lib.rs` `@@ -1539,4 +1634,172 @@` (the parser state, split UTF-8, OSC limit and
  overflow report tests) and `alacritty_terminal` `term/mod.rs` `@@ -3299,4 +3832,1060 @@` (every other test, and
  `fed_term`, the helper they all use). Each test belongs to one record, except
  `a_one_column_grid_drops_wide_characters`, shared by the two one-column records. Some lean on another record's code:
  `erase_below_with_a_wrap_pending_keeps_the_last_column` assumes the wrap-flag fix for its autowrap-off cases,
  `repeating_an_unchanged_attribute_copies_nothing` reads the extra storage's count, and `title_length_is_capped` and
  `history_counters` read state through the restore accessors (`title()`, `title_stack()`, `inactive_grid()`).
- `term/cell.rs`: the extra storage's setters make the same unchanged-value check as `Cell`'s setters (the record on
  leaving a cell's extra alone), at their own layer; each upstream change carries its own.

### History counters

The grid stream sends scrollback to viewers as appended ranges and needs to know how many lines entered history since it
last looked. `Grid::lines_scrolled_into_history()` counts every line that scrolls off the top of a grid that has history
(so never on the alternate screen), and keeps counting once history is full. `Grid::history_clears()` counts history
being erased, which a program does on its own with `CSI 3 J` or a full reset. Neither ever decreases. They do not
describe a resize or reflow, which the embedder starts itself and has to treat as a change of shape. Both fields are
skipped by serde and left out of `Grid`'s `PartialEq`, so serialized grids and grid comparisons are unchanged.

### Bug fixes

Three of the nine upstream bugs the patches fix: the two that ordinary program output reaches on a one-column terminal
or across reads, and clearing a row after it grew; each fix is a few lines in the place the bug is. The other six are
described only in their per-patch records above (the two erase fixes, the wrap-flag fix, and the cursor-movement, CHA
and origin-mode fixes). The independent mouse encodings are not counted among them: upstream's behavior there follows
xterm on purpose, and the patch chooses tmux's instead (its record).

- A wide character on a grid one column wide. A wide character needs two cells, and upstream's `Term::input` writes its
  spacer cell into the second column even when there is none, which panics with an index out of bounds; and
  `Grid::shrink_columns`, reflowing a screen that holds wide characters down to one column, moves each wide character to
  the next row, where it does not fit either, and repeats that forever, allocating rows until memory runs out. One
  column is a valid size (the terminal host accepts it), and the emulator stays the size of the host's pty, so it cannot
  widen itself to avoid this. `Term::input` now drops a wide character on a one-column grid without moving the cursor,
  and the reflow blanks it and its spacer. tmux, for comparison, ignores a wide character that does not fit only while
  wrapping is off, and with wrapping on, a one-column pane shows the character anyway.
- A UTF-8 character split across two parser calls. When a call ends inside a multi-byte character, `vte`'s
  `advance_partial_utf8` completes it from the next call's first bytes. If an invalid byte follows within those few
  bytes, upstream printed the completed character but consumed the valid characters between it and the invalid byte
  without printing them: characters the program wrote silently disappeared from the screen, and only when a read
  happened to end inside the character before them, so a terminal rebuilt by feeding the same output in other reads
  would keep showing them. It now consumes only the completed character's bytes, as its other path already did.

  A related upstream behavior is left alone: a C1 control encoded in UTF-8 (U+0080 to U+009F) and split across two calls
  is printed, where unsplit it is executed. alacritty draws nothing for it either way, but the parser then remembers it
  as the character a later REP (`CSI Ps b`) repeats, so REP repeats nothing instead of the character before it. It takes
  an encoded C1 control split across reads and then a REP to show, and the patch is kept to the bug that loses
  characters.

- Clearing a row after it grew. `Row::reset` skips the cells from the row's `occ` (how many cells may differ from a
  cleared one) on when the row's last cell already has the background it clears to, trusting that those cells hold a
  blank of that background. Upstream's `Row::grow` appended default cells without counting the old ones in `occ`, so
  after a row was cleared with a colored background (which leaves `occ` at 0), narrowed and widened again (the alternate
  screen resizes without reflowing), a clear with the default background skipped the old cells and left them colored.
  `grow` now counts the old cells as occupied. Besides the stale color, the bug made a terminal's state depend on how
  its rows came to be, which a snapshot cannot reproduce: the snapshot round-trip test found it.

## How the trees stay out of the workspace's checks

They are upstream's code, so they are not held to Farhelm's formatting and lints.

- The root `Cargo.toml` lists them under `workspace.exclude`. They are not members, so `cargo clippy --workspace`,
  `cargo test --workspace` and nextest never select them, and clippy's `-D warnings` applies only to members. Without
  the exclusion a `cargo` command inside `vendor/` would find the workspace and refuse to build a package under it that
  is not a member.
- `cargo fmt --all` formats workspace members and their `path =` dependencies, but does not follow `[patch.crates-io]`,
  which is the only way the workspace reaches these trees. Keep it that way: a crate that depends on them must name them
  by version, never by `path`, or `cargo fmt --all` reformats upstream's code.
- `dprint.json` excludes `vendor/alacritty_terminal/**` and `vendor/vte/**`, so their Markdown and TOML are left as
  published. This README and the patch files are still checked.
- `scripts/check-test-sleeps.py` reads only `crates/` and `e2e/tests/`.
- `pair-tests/` is not a member either: the empty `[workspace]` table in its `Cargo.toml` makes it a workspace of its
  own, so cargo does not go looking for Farhelm's (which would refuse it, as above), and no workspace command reaches
  it. Unlike the trees it is Farhelm's code, so keep it formatted with `rustfmt --edition 2024` and warning-free by
  hand; dprint checks its `Cargo.toml`.

NOTE: Cargo does not cap lints for path dependencies the way it does for crates from a registry, so a rustc warning in
these trees prints on every build of a crate that uses them. Keep the patches warning-free. If a toolchain upgrade makes
upstream code warn, that is a reason to look at upgrading the crates, not to edit their code outside the patch.

## Running their tests

The patches' own tests (and upstream's unit tests) run on each crate by itself, outside the workspace, and the tests
that need both patches run in `pair-tests/`:

```sh
cargo test --manifest-path vendor/vte/Cargo.toml --features ansi
cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml
cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml
```

`alacritty_terminal` builds on its own against crates.io's `vte`, not the patched one, because `[patch.crates-io]`
belongs to the workspace it is excluded from, so none of its patch tests may depend on the vte patch. A test that needs
one patch acting through the other (such as an OSC 8 the patched parser drops ending the terminal's hyperlink) goes in
`pair-tests/`, which swaps the vendored `vte` in the way the workspace will. `--locked` keeps cargo to a known
`Cargo.lock`: the one `alacritty_terminal` was published with, and for `pair-tests/` a copy of it that cargo adjusted
for the swap. vte has no lockfile here (above), so its run resolves its few dependencies fresh.

## Checking a tree against upstream, and upgrading

Both procedures start from the published `.crate` file. Cargo keeps a copy under `~/.cargo/registry/cache/` once any
build has fetched the version; otherwise download it from
`https://static.crates.io/crates/<name>/<name>-<version>.crate`. Check its SHA-256 against the `cksum` field for that
version in the crates.io index (`https://index.crates.io/al/ac/alacritty_terminal`, `https://index.crates.io/3/v/vte`).
Work in a scratch directory outside the checkout.

To check that a tree is upstream plus its patch:

- `tar -xzf alacritty_terminal-0.26.0.crate` and rename the result to `alacritty_terminal` (for vte, `vte`).
- For `alacritty_terminal`, remove `tests/` from the unpacked copy.
- From the scratch directory, `patch -p1 < <checkout>/vendor/patches/alacritty_terminal-0.26.0.diff`.
- `diff -r alacritty_terminal <checkout>/vendor/alacritty_terminal` prints nothing (for vte, at most that `Cargo.lock`
  exists only in the unpacked copy, and that a `Cargo.lock` or `target/` from running its tests exists only in the
  checkout).

To upgrade to a new version:

- Read upstream's changelog and the diff between the two versions for anything touching the patched code: the OSC buffer
  and its dispatch and `advance_partial_utf8` in `vte/src/lib.rs`, `Processor` and the CSI dispatch in
  `vte/src/ansi.rs`, `Cell::push_zerowidth`, `Cell`'s setters and `CellExtra`, `Hyperlink` and `HyperlinkInner`,
  `Term::input` (and every reader of `Cursor::input_needs_wrap`, whose upstream code the wrap-flag fix relies on),
  `Term::clear_screen`, `Term::set_private_mode`, `Term::unset_private_mode`, the `Term` cursor moves,
  `Term::set_hyperlink`, `Term::terminal_attribute`, `Term::set_title`, `Grid::scroll_up`, `Grid::shrink_columns`,
  `Grid::clear_history` and `Row::grow`. A cap that upstream now enforces itself may make a hunk unnecessary; a moved
  field may need the hunk rewritten. The combining character cap is one already: alacritty's master has its own since
  2026-08 (that record's upstream status).
- Unpack the new crate twice in the scratch directory, as `a/<name>` (left as published, minus `tests/` for
  `alacritty_terminal`) and `b/<name>`, and apply the old patch to `b` with `patch -p1 -d b`. Fix rejected hunks by
  hand, keeping each hunk small and marked `Farhelm patch`.
- Regenerate the patch from the scratch directory with `git diff --no-index --no-prefix a/<name> b/<name>`, saved as
  `patches/<name>-<version>.diff` in place of the old file, and update the hunk headers the per-patch records name
  ("Patch hunks" in each, and "Hunks shared between records").
- Replace `vendor/<name>/` with `b/<name>`, update the version pins in the crates that depend on it, the checksums and
  commits above, and `THIRD_PARTY_NOTICES.md`, and run the tests above plus the emulator crate's
  (`cargo nextest run -p farhelm-terminal-emulator`, recorded as `.agents/validation.md` gives it). For an
  `alacritty_terminal` upgrade, also start `pair-tests/Cargo.lock` again from a copy of the new crate's `Cargo.lock` and
  let one unlocked `cargo test` there adjust it.
- A `vte` upgrade usually comes with an `alacritty_terminal` upgrade that requires it; upgrade them together.

## GPUI

NOTE: As with the terminal crates, do not edit the code under `gpui-pre/`, `gpui-pre-linux/`, `gpui-pre-wgpu/`,
`gpui-pre-macos/` or `gpui-agent/` directly. Each tree is a published source plus the patch files in `patches/gpui/`,
applied in the order below, and a change goes into the tree and its patch file together.

The desktop GUI is built on GPUI, Zed's GPU-rendered UI framework, through the `gpui-pre` crates (a published snapshot
of Zed's GPUI) and GPUI Kit (`gpui-component` and `gpui-base`), all pinned exactly in the root `Cargo.toml`. Kit and
everything else come from crates.io as published. Four of the GPUI crates are vendored because the app needs changes to
them, and the root `Cargo.toml` swaps the vendored trees in with `[patch.crates-io]`. `gpui-agent/` is the
test-automation bridge that lets an agent drive the running app.

These trees came from a private GPUI prototype of Farhelm, which built and measured them; the changes were ported, not
re-derived. They are the prototype's trees minus its two measurement-only layers (per-draw CPU attribution and a
frame-wake counter for one test), plus the change notices below and two layers of Farhelm's own, `frame-source` and
`no-kde-blur`.

### What is here

| Path                    | What                                                                                    |
| ----------------------- | --------------------------------------------------------------------------------------- |
| `gpui-pre/`             | `gpui-pre` 0.3.12, GPUI's core                                                          |
| `gpui-pre-linux/`       | `gpui-pre-linux` 0.3.12, the Linux platform (Wayland and X11)                           |
| `gpui-pre-wgpu/`        | `gpui-pre-wgpu` 0.3.12, the wgpu renderer Linux uses                                    |
| `gpui-pre-macos/`       | `gpui-pre-macos` 0.3.12, the macOS platform                                             |
| `gpui-agent/`           | `wsp-labs/gpui-agent` at `c9b8b6328877755685e1233b3770f78a68f2a5d1`, the bridge and CLI |
| `patches/gpui/<layer>/` | one directory per patch layer, one patch per tree it touches                            |

The four GPUI trees started as the published `.crate` files, unpacked unmodified. Their SHA-256 checksums match both a
fresh download from `https://static.crates.io/crates/<name>/<name>-0.3.12.crate` and the `cksum` field in the crates.io
index (checked 2026-11-13):

- `gpui-pre-0.3.12.crate`: `29db89613527079737f53752eab27fb65c9a7dd94979256c65a0e590c9fd34d8`
- `gpui-pre-linux-0.3.12.crate`: `54bab418d0b7c027af9495f278619b46887362b3bd6446e246c87c5b0fa167df`
- `gpui-pre-wgpu-0.3.12.crate`: `dc76608d385f6d0dcbad44504b1bf669dac3b7d7ffa2885acdb7c1c358caf852`
- `gpui-pre-macos-0.3.12.crate`: `2de7d3f4ad51ac387e822495db363fcc1188d349793365731d054444c4cb06ee`

One file of the published `gpui-pre` is not checked in: `examples/image/black-cat-typing.gif`, a 4.5 MB animation that
two upstream examples load at run time. It is half the tree's size and nothing builds from it. Each manifest records Zed
revision `d2a4fb6ded7c470be25d1541281da3882495b65b`. The other GPUI crates, including `gpui-pre-platform` (which picks
the platform crate) and `gpui_macros`, stay ordinary registry dependencies.

`gpui-agent/` is the upstream repository at that commit without its `.git`, `docs/` (a demo GIF and a screenshot),
`scripts/` (upstream's end-to-end and recipe scripts) and `patches/` (upstream's own copies of the bridge patches for
gpui-pre 0.3.9 and for Zed; the 0.3.12 ports here supersede them, and only upstream's README still links to them). Its
in-app crate (`gpui/gpui-agent/`), protocol crate (`crates/protocol/`), CLI (`crates/cli/`), README and license are
retained. So is its small test app (`gpui/testapp/`): the in-app crate inherits its manifest settings from
`gpui/Cargo.toml`, whose workspace lists the test app as a member, so dropping it would take one more local patch.

### Licenses

The GPUI crates and gpui-agent are Apache-2.0 only; each tree keeps its license file (`LICENSE-APACHE`, and
`gpui-agent/LICENSE`). Apache-2.0 requires modified files to say so, so the last layer, `modified-notices`, puts a
one-line `Modified for Farhelm` comment at the top of every file an earlier layer changed or added (29 files; the
bridge's `gpui/Cargo.lock` is left without one, because cargo rewrites lockfiles), the same marker the terminal crates
use, so one search finds every modified vendored file. Unlike the terminal crates' patches, these layers do not mark
each patched spot with a `Farhelm patch` comment: they were carried over from the prototype as they were, and the patch
files themselves are the record of what changed. `THIRD_PARTY_NOTICES.md` has the entries, including one for the Lucide
icons GPUI Kit bundles.

Zed's crates other than `gpui`, `gpui_macros`, `gpui_platform`, `gpui_util`, `gpui_tokio` and `scheduler` are treated as
GPL unless checked; none of their code is here, and none may be copied in. The `gpui-pre-*` utility crates in this graph
(`gpui-pre-sum-tree`, `-collections`, `-refineable`, `-http-client`, `-util` and the rest) are published copies of Zed
crates; every one of them declares Apache-2.0, which was checked on 2026-11-13.

### The patch layers

Here's what each layer changes and why, in application order. Sizes are `git apply --numstat` added / removed lines.

| Layer                  | Trees (added / removed)                                                        | Why                                                        |
| ---------------------- | ------------------------------------------------------------------------------ | ---------------------------------------------------------- |
| `bridge`               | gpui-pre 411 / 4, gpui-pre-linux 30 / 1, gpui-pre-wgpu 147 / 101               | Hooks the automation bridge reads; window screenshots      |
| `capture-alpha`        | gpui-pre-wgpu 6 / 1                                                            | Screenshot colors on premultiplied-alpha surfaces          |
| `gpui-agent`           | gpui-agent 11 / 11, 29 / 11 and a lockfile refresh                             | Build the bridge against these gpui-pre 0.3.12 trees       |
| `native-key`           | gpui-pre 354 / 11, gpui-pre-linux 89 / 4, gpui-pre-macos 27 / 17               | Physical key identity and the original Shift on key events |
| `attachment-clipboard` | gpui-pre-linux 159 / 1, gpui-pre-macos 92 / 0                                  | Image-before-text async clipboard reads for paste          |
| `activation-token`     | gpui-pre 13 / 0, gpui-pre-linux 9 / 0                                          | Activate a window with another client's Wayland token      |
| `frame-source`         | gpui-pre-macos 10 / 1                                                          | Keep the lower-latency frame source on macOS 14 and later  |
| `no-kde-blur`          | gpui-pre-linux 4 / 36, gpui-agent 0 / 14                                       | Drop the KDE-blur fallback and its LGPL-bundling crate     |
| `modified-notices`     | gpui-pre 11, gpui-pre-linux 8, gpui-pre-wgpu 1, gpui-pre-macos 4, gpui-agent 5 | The license's change notices (above)                       |

Without the bridge's own patches and the notices, the GPUI changes come to 1,351 added and 177 removed lines, tests
included.

- **bridge.** The bridge records what each frame drew (elements, text, accessibility roles and names, focus and input
  targets) while it is enabled, lets its driver answer file panels and edit text inputs, and renders a window's scene to
  an image for screenshots on Wayland and X11 (`render_to_image`, compiled only in test or bench builds, through
  `test-support`, which the bridge's in-app crate turns on). The bridge's upstream publishes these patches only for
  gpui-pre 0.3.9 and for Zed; these are three-way ports of the 0.3.9 copies. The hooks are compiled into every build,
  release builds included, but stay inert unless the bridge starts, which needs the GUI's automation feature (never in
  shipped or devbuild builds) and an explicit launch through its CLI or `GPUI_AGENT=1`.
- **capture-alpha.** Screenshot capture reuses the window's blend pipelines, so its shader must use the surface's
  premultiplied-alpha mode. With straight alpha hard-coded, translucent selection highlights came out opaque. This
  changes capture only, not what is presented on screen.
- **gpui-agent.** One patch removes upstream's optional paths to a sibling Zed checkout and points its standalone GPUI
  workspace at patched crates; the second adds a `gpui-pre-0-3-12` feature, its compile guards and 0.3.12 versions, and
  points that workspace's `[patch.crates-io]` at the four trees here; the third (`gpui-agent-lockfile.patch`) refreshes
  `gpui/Cargo.lock` so that workspace resolves to them
  (`cargo metadata --locked --all-features --manifest-path vendor/gpui-agent/gpui/Cargo.toml` succeeds and lists
  gpui-pre 0.3.12 from the vendored paths). gpui-pre 0.3.12 already has Zed's keystroke and scroll API shapes, so the
  bridge builds it through the same compatibility branch as its `zed` feature.
- **native-key.** Key down and up events, and the keystroke interceptor's payload, carry the key's physical identity (an
  XKB position name on Linux, a virtual keycode on macOS) and the original Shift, which logical keystrokes lose for
  symbols. The terminal needs both to send the right bytes on non-US layouts. Logical key matching is unchanged. Parsed
  or injected keys, ASCII input-method commits and replayed keystrokes carry no native identity. On macOS the forwarded
  original key-down keeps its repeat flag where upstream forced it off, so held-key filters can behave differently. The
  layer adds fields to `KeyDownEvent` and `KeyUpEvent`, so the unpatched `gpui-pre-windows` and `gpui-pre-web` crates
  would not compile against it; neither is a Farhelm target.
- **attachment-clipboard.** The async clipboard read prefers images over text (and files over both on macOS), so pasting
  a screenshot into a terminal uploads the image rather than its text label. Wayland reads the pipe on a background
  thread. The synchronous clipboard API that text fields use keeps its text-first behavior.
- **activation-token.** `Window::activate_window_with_token` activates a window with an xdg-activation token another
  client obtained, which a second launch needs to bring the running app's window forward on Wayland. Upstream can only
  request its own token, which compositors usually refuse. Other platforms fall back to plain activation.
- **frame-source.** On macOS 14 and later GPUI paces a window with `CAMetalDisplayLink`, whose default
  `preferredFrameLatency` of 2 presents each frame about three refreshes after the update that drew it. On the test Mac
  that made typing in the prototype's terminal about 20 ms slower than the old app (55-58 ms median against 35-37 ms);
  GPUI's other frame source, `CVDisplayLink` with `nextDrawable`, which it already uses before macOS 14, measured 29-36
  ms with the same flood frame rate (the flood ended about 0.1 s later). Setting `preferredFrameLatency` to 1 had no
  effect there. The cost: `CVDisplayLink` gives GPUI no frame waker and does not pause between needed frames, so an idle
  window used 0.4% CPU instead of 0.1%, and frame rates no longer follow the display's power policy (ProMotion, not
  testable on the test Mac). The layer keeps the `CVDisplayLink` source on every macOS version (a constant in
  `src/window.rs`). The numbers are from the test Mac VM (60 Hz virtual display) on 2026-11-13, three runs per setting
  (two for idle CPU). The runs used a scratch tree that forced `MetalFrameSource::is_available()` to false; this layer
  instead skips the Metal source where the window chooses it, which takes the same path, because `gpui-pre-apple` checks
  availability only inside `MetalFrameSource::new` and this crate constructs that source only for a window that chose
  it. Drop the layer when an upstream GPUI presents `CAMetalDisplayLink` frames within one refresh, measured the same
  way. The maintainer chose to keep it with those costs (2026-11-13); a TODO.md entry revisits it, in particular whether
  Apple or GPUI may drop the older path.
- **no-kde-blur.** `wayland-protocols-plasma` declares the MIT license but bundles KDE's protocol XML files, several of
  them LGPL-2.1-or-later, which `wayland-scanner` turns into bindings compiled into the Linux build, and SPEC_impl lets
  no GPL-family code into the tree as a dependency. GPUI's Linux backend used the crate only for the KDE-blur protocol,
  its fallback for blurring behind a window whose background appearance is `Blurred` when the compositor lacks the
  DE-agnostic `ext-background-effect` protocol (KDE itself dropped KDE blur in Plasma 6.7). The layer removes the crate
  from the backend's `wayland` feature and its dependencies, and deletes the fallback; `ext-background-effect` blur is
  unchanged. Nothing Farhelm draws is lost: no Farhelm window asks for `Blurred`. A compositor without
  `ext-background-effect` would no longer blur such a window. The second patch drops the crate from the bridge
  workspace's `gpui/Cargo.lock`, so `cargo metadata --locked` on that workspace still succeeds. Removed by the
  maintainer's decision (2026-11-13); a TODO.md entry revisits it. Drop the layer if an upstream GPUI stops depending on
  the crate.

Two Linux hunks are carried but not compiled: the bridge's capture hook in `src/linux/x11/window.rs` and native-key's
event conversion in `src/linux/x11/client.rs`. The workspace builds `gpui-pre-platform` without its `x11` feature, so
nothing type-checks them. Linux code also cannot check the macOS tree: `gpui-pre-macos` compiles only on a Mac, so a
change to it has to be built there.

These layers make no claim about native behavior they were not tested against: native IME, real-desktop clipboard
transfer and whether a compositor grants a forwarded activation token were not established by the prototype.

NOTE: The bridge's socket is unauthenticated. Anyone who can connect to it controls the app, including its terminals and
approval cards. Bridge runs keep it in a private 0700 directory.

### Reproducing the trees

Work in a scratch directory outside the checkout, holding a copy of `patches/gpui/` and an empty `vendor/`:

- Download the four `.crate` files above, check their SHA-256 against the list, and unpack each into `vendor/`
  (`tar -xzf <name>-0.3.12.crate -C vendor`), then rename `vendor/<name>-0.3.12` to `vendor/<name>`.
- Clone `https://github.com/wsp-labs/gpui-agent`, check out `c9b8b6328877755685e1233b3770f78a68f2a5d1`, and copy its
  contents without `.git` into `vendor/gpui-agent/`.
- From the scratch directory, apply the layers in this order (`--directory` names the tree, and running from the scratch
  directory keeps `git apply` from resolving paths against an enclosing repository):

```sh
git apply --directory=vendor/gpui-pre patches/gpui/bridge/gpui-pre-0.3.12.patch
git apply --directory=vendor/gpui-pre-linux patches/gpui/bridge/gpui-pre-linux-0.3.12.patch
git apply --directory=vendor/gpui-pre-wgpu patches/gpui/bridge/gpui-pre-wgpu-0.3.12.patch
git apply --directory=vendor/gpui-pre-wgpu patches/gpui/capture-alpha/gpui-pre-wgpu-0.3.12.patch
git apply --directory=vendor/gpui-agent patches/gpui/gpui-agent/gpui-agent-crates-io.patch
git apply --directory=vendor/gpui-agent patches/gpui/gpui-agent/gpui-agent-gpui-pre-0.3.12.patch
git apply --directory=vendor/gpui-pre patches/gpui/native-key/gpui-pre-0.3.12.patch
git apply --directory=vendor/gpui-pre-linux patches/gpui/native-key/gpui-pre-linux-0.3.12.patch
git apply --directory=vendor/gpui-pre-macos patches/gpui/native-key/gpui-pre-macos-0.3.12.patch
git apply --directory=vendor/gpui-pre-linux patches/gpui/attachment-clipboard/gpui-pre-linux-0.3.12.patch
git apply --directory=vendor/gpui-pre-macos patches/gpui/attachment-clipboard/gpui-pre-macos-0.3.12.patch
git apply --directory=vendor/gpui-pre patches/gpui/activation-token/gpui-pre-0.3.12.patch
git apply --directory=vendor/gpui-pre-linux patches/gpui/activation-token/gpui-pre-linux-0.3.12.patch
git apply --directory=vendor/gpui-pre-macos patches/gpui/frame-source/gpui-pre-macos-0.3.12.patch
git apply --directory=vendor/gpui-agent patches/gpui/gpui-agent/gpui-agent-lockfile.patch
git apply --directory=vendor/gpui-pre-linux patches/gpui/no-kde-blur/gpui-pre-linux-0.3.12.patch
git apply --directory=vendor/gpui-agent patches/gpui/no-kde-blur/gpui-agent-lockfile.patch
git apply --directory=vendor/gpui-pre patches/gpui/modified-notices/gpui-pre-0.3.12.patch
git apply --directory=vendor/gpui-pre-linux patches/gpui/modified-notices/gpui-pre-linux-0.3.12.patch
git apply --directory=vendor/gpui-pre-wgpu patches/gpui/modified-notices/gpui-pre-wgpu-0.3.12.patch
git apply --directory=vendor/gpui-pre-macos patches/gpui/modified-notices/gpui-pre-macos-0.3.12.patch
git apply --directory=vendor/gpui-agent patches/gpui/modified-notices/gpui-agent.patch
```

- Remove `vendor/gpui-agent/docs/`, `vendor/gpui-agent/scripts/`, `vendor/gpui-agent/patches/` and
  `vendor/gpui-pre/examples/image/black-cat-typing.gif`.
- `diff -r vendor/<tree> <checkout>/vendor/<tree>` prints nothing for each of the five trees (for `gpui-agent`, apart
  from a `target/` left by building its CLI in the checkout).

Every layer applies without offsets. The sequence was checked end to end on 2026-11-13, and against the prototype: its
trees equal these minus the notices, `frame-source` and `no-kde-blur`, with its two measurement layers added back for
`gpui-pre`.

Layers touch overlapping files, so to check one layer by itself with `git apply --reverse --check`, first reverse every
later layer in a disposable copy.

### Upgrading

Upgrade only as a dedicated change, at a milestone, with GPUI Kit: Kit pins its gpui-pre exactly, so the two move
together. Unpack the new crates twice, as `a/<name>` (as published) and `b/<name>`, apply each layer to `b` in order
with `git apply`, and fix rejected hunks by hand; a three-way merge (`git apply -3` inside a scratch git repository
holding the old pristine, old patched and new pristine trees) does most of it. Regenerate each patch from the scratch
directory with `git diff --no-index --no-prefix a/<name> b/<name>` after each layer, and the notices layer last. The
prototype's bump from 0.3.9 to 0.3.12 took about half an hour and found one silently non-compiling line from a three-way
merge, so build every tree, on Linux and on a Mac, before trusting the result. Then update the checksums above, the
version pins in the root `Cargo.toml`, and `THIRD_PARTY_NOTICES.md`.

The bridge upgrades separately: copy the new upstream commit, re-apply the `gpui-agent` patches, refresh and re-record
its lockfile patch, and update the commit here and in `THIRD_PARTY_NOTICES.md`.

Bringing GPUI and Kit into the workspace moved a few existing dependencies in `Cargo.lock`, because their new dependents
require newer versions: the `icu_*` crates from 2.3 to 2.4, with `tinystr`, `yoke`, `zerovec` and `zerovec-derive`
(gpui-pre's `icu_segmenter` 2.4), `toml` from 0.8.2 to 0.8.23 and `toml_datetime` from 0.6.3 to 0.6.11 (Kit's
`rust-i18n-support` needs `toml` 0.8.8 or newer), and `proc-macro-crate` from 2.0.2 to 2.0.0 (2.0.2 pins the older
`toml_datetime`; `glib-macros` uses it). `cargo update --precise` refuses to keep any of the old versions.

### Running their tests

The native-key layer's test of the core, outside the workspace like the terminal crates' tests (minutes, since it builds
GPUI on its own; `.agents/validation.md` has the recorded form):

```sh
cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support --test native_key_metadata
```

### Staying out of the workspace's checks

The same rules as the terminal crates (above) apply, for the same reasons:

- The root `Cargo.toml` lists all five trees under `workspace.exclude`, so workspace clippy, fmt, test and nextest runs
  never select them, and `-D warnings` applies only to members.
- Dependents name GPUI by version (`gpui = { package = "gpui-pre", version = "=0.3.12" }`) and reach the trees only
  through `[patch.crates-io]`, which `cargo fmt --all` does not follow. The bridge is the exception: the GUI will depend
  on `gpui-agent/gpui/gpui-agent` by path, behind its automation feature, so `cargo fmt --all` would reach it once that
  dependency exists; that crate's own workspace (`gpui-agent/gpui/Cargo.toml`) keeps it out of Farhelm's.
- `dprint.json` excludes the five trees. This README and the patch files are still checked.
- `scripts/check-test-sleeps.py` reads only `crates/` and `e2e/tests/`.
- cargo-deny checks the vendored crates' declared licenses like any other dependency; its source ban does not apply to
  path dependencies (`deny.toml`).

Build the bridge's CLI from the checkout root, so it uses the repository's Rust toolchain instead of the one pinned in
`gpui-agent/rust-toolchain.toml`:

```sh
cargo build --locked --manifest-path vendor/gpui-agent/Cargo.toml -p gpui-agent-cli
```
