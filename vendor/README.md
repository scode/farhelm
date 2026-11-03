# Vendored terminal crates

NOTE: Do not edit the code under `alacritty_terminal/` or `vte/` directly. Each tree is an upstream crate plus the patch
next to it in `patches/`, and the procedure below depends on that staying true. A change goes into the tree and the
patch file together.

The supervisor's terminal emulator (`crates/farhelm-terminal-emulator/`) runs on `alacritty_terminal` 0.26.0 and its
parser `vte` 0.15.0. Both are used as published on crates.io except for a few small patches: accessors that snapshot and
restore need, caps on what hostile program output can make them store, two history counters for the grid stream, one
more query for the handler to answer, nine bug fixes, and one mode change that follows tmux over xterm (the UTF-8 and
SGR mouse encodings as separate modes). The root `Cargo.toml` swaps these trees in for the crates.io versions with
`[patch.crates-io]`.

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

Here's what each one changes and why. None changes behavior except the caps, the bug fixes and the mouse-encoding
change, and the XTVERSION hook changes it only for a handler that implements the new method.

### Snapshot accessors

The supervisor snapshots a terminal's emulator so that a new supervisor can rebuild it, and the snapshot spike showed
that a restore needs state upstream keeps private. These are read-only, except `inactive_grid_mut()` and
`set_preceding_char()`, which a restore uses to write the few states no escape sequence can produce.

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
  which carry the whole selection base64-encoded: xterm.js, which renders Farhelm's terminals today, drops OSC payloads
  over 10,000,000 characters (`PAYLOAD_LIMIT` in `crates/farhelm-ui/assets/vendor/xterm.js`), so 10 MiB accepts every
  copy that works there, about 7.5 MB of copied text. The `no_std` parser has its own fixed buffer and is unchanged.
- Zero-width characters per cell, `alacritty_terminal::term::cell::MAX_ZEROWIDTH_CHARS` = 16. Upstream appends every
  combining character to the previous cell, four bytes each, without limit. Further ones are dropped. Real text needs
  far fewer: stacked diacritics and Hangul jamo use two or three per cell, a subdivision flag such as Scotland's six.
  tmux, whose screen Farhelm reads today, stores at most 32 bytes of UTF-8 per cell, which in practice is fewer marks
  than this.
- Hyperlink length, `alacritty_terminal::term::MAX_HYPERLINK_URI_BYTES` = 2083 and `MAX_HYPERLINK_ID_BYTES` = 250. Every
  cell printed while an OSC 8 hyperlink is active holds it, and a program can start a new one before every character, so
  the length multiplies into the grid's memory. An OSC 8 over either limit ends the active hyperlink, and the text after
  it is unlinked. These are the limits VTE uses, as the
  [OSC 8 specification](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda) documents (iTerm2 uses the
  same URI limit). Separately, an OSC 8 that repeats the active link's explicit `id` and URI keeps the active link
  instead of building an equal copy, so the cells after it share one allocation. A link without an `id` always gets a
  fresh generated one, so it is never equal to the active link and is built as before (xterm.js also keeps such links
  apart).
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
code touched, the tests that pin it, and its upstream status. Sizes are columns by lines, and cursor positions are
(line, column) from 0. The xterm source checked is the `ThomasDickey/xterm-snapshots` mirror as of 2026-11-01
(`charproc.c` 1.2134, `cursor.c` 1.101, `main.h` 1.93, `misc.c` 1.1137, `ptydata.c` 1.173, `ptyx.h` 1.1174, `screen.c`
1.682, `util.c` 1.1032); the tmux source is the 3.7c release Farhelm pins (`scripts/build-pinned-tmux-ci.sh`,
`.github/release/source-pins.env`). xterm.js, which renders Farhelm's terminals today, was checked in its vendored copy
(`crates/farhelm-ui/assets/vendor/xterm.js`). The "observed" results were produced by feeding the bytes to the
published, unpatched crates. The fixes that keep the emulator's screen where tmux keeps it were found, and are pinned,
by the supervisor's comparison with a live tmux 3.7c (`crates/farhelm-supervisor/src/emulator_parity/`).

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
- Tests: `tests::is_ground_only_between_sequences` (vte).
- Upstream status: not yet reported. Upstreamable as is: additive and read-only.

#### vte: the character REP repeats

- Upstream today: `ansi::Processor` remembers the last printed character for `CSI Ps b` (REP) in a private field, so an
  emulator rebuilt from a snapshot cannot be given it back.
- Reproduction (10x1): `x ESC [ 3 b` prints `xxxx`. An emulator rebuilt from a snapshot of the state after `x` has no
  way to learn the `x`, so the same `ESC [ 3 b` fed to it prints nothing.
- xterm and tmux have no such API; both keep the last character privately (xterm: `charproc.c`, `CASE_REP`, repeating
  `sp->lastchar`; tmux: `input.c`, `INPUT_CSI_REP`, repeating `ictx->last`).
- Fix: `Processor::preceding_char()` and `Processor::set_preceding_char()`.
- Why Farhelm needs it: the snapshot spike found real Codex and Claude Code output that a restore without the REP
  character rebuilds wrongly.
- Upstream code: `vte/src/ansi.rs`, `Processor`.
- Tests: none in the vendored trees (the accessors only read and write the field).
- Upstream status: not yet reported. Upstreamable as is: additive.

#### alacritty_terminal: the state a restore needs

- Upstream today: `Term` keeps private the inactive screen (the primary screen and its history while the alternate
  screen shows), the scroll region, the tab stops, the active character set, the title, the title stack and the kitty
  keyboard mode stacks. The inactive screen can be reached only by swapping it in with `swap_alt()`.
- Reproduction: none; this is missing API. The need: after `ESC [ 2 ; 4 r` (a scroll region) an embedder can neither
  read the region nor reproduce it in another `Term` without knowing it.
- xterm and tmux have no such API.
- Fix: `Term::inactive_grid()`, `inactive_grid_mut()`, `scroll_region()`, `tab_stops()`, `active_charset()`, `title()`,
  `title_stack()` and `keyboard_mode_stacks()`. All read-only except `inactive_grid_mut()`, which a restore uses for the
  primary screen's saved cursor while the alternate screen shows, the one state no escape sequence can set.
- Why Farhelm needs it: the supervisor snapshots each terminal's emulator so a restarted supervisor can rebuild it.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `impl<T> Term<T>`.
- Tests: none in the vendored trees (read-only accessors).
- Upstream status: not yet reported. Upstreamable as is: additive. `inactive_grid_mut()` is the one an upstream reviewer
  may question; it could be limited to the saved cursor.

#### vte: a limit on the buffered OSC payload

- Upstream today: the `std` parser appends every OSC payload byte to a `Vec` without limit, dispatches the whole
  payload, and keeps the `Vec`'s capacity afterwards.
- Reproduction: `ESC ] 2 ;` + 11 MiB of `A` + BEL. Observed: `osc_dispatch` with parameters of 1 and 11,534,336 bytes,
  and the buffer keeps its capacity (from the code: `osc_raw.clear()` only). Expected: the sequence is not dispatched,
  and the buffer is given back.
- xterm limits every string sequence to the `maxStringParse` resource, 20,000 bytes by default (600,000 with ReGIS or
  sixel graphics), and drops a longer one whole when it ends ("Ignoring too-long string"; `charproc.c`, the
  `strings_max` check in `doparsing` and `CASE_ST`; `main.h`, `DEF_STRINGS_MAX`). tmux discards a sequence longer than
  `input-buffer-size`, 1 MiB by default and at least, whole (`input.c`, `input_input` setting `INPUT_DISCARD`,
  `input_exit_osc`), and shrinks its buffer back to 32 bytes on returning to the ground state (`input_ground`). xterm.js
  drops an OSC whose payload passes 10,000,000 characters (`PAYLOAD_LIMIT`).
- Fix: `vte::OSC_PAYLOAD_LIMIT` = 10 MiB. Past it the parser stops buffering, and when the sequence ends it drops it
  whole and calls `Perform::osc_overflow` (next record) instead of dispatching; after every OSC the buffer is shrunk
  back to 1 KiB (`MAX_OSC_RAW`, the size of upstream's `no_std` buffer). The `no_std` parser, which has a fixed buffer,
  is unchanged.
- Why Farhelm needs it: one program must not grow the shared supervisor's memory without bound. The limit is set so that
  every OSC 52 copy that works in the browser's xterm.js today, about 7.5 MB of text, still works.
- Upstream code: `vte/src/lib.rs`, `Parser::action_osc_put` and `Parser::osc_end`.
- Tests: `tests::osc_at_payload_limit_is_dispatched`, `tests::osc_over_payload_limit_is_dropped` (vte).
- Upstream status: not yet reported. The limit's value is Farhelm policy. An upstream-friendly version makes it a
  parameter (a const generic like `Parser`'s existing `OSC_RAW_BUF_SIZE`, or a constructor argument) with no limit by
  default; the shrink after a large OSC is upstreamable as is.

#### vte: reporting a dropped OSC, and ending the hyperlink it would have replaced

- Upstream today: there is no limit to report. With the limit above alone, a dropped OSC would vanish without a trace,
  and a dropped OSC 8 would leave the previous hyperlink active.
- Reproduction (20x1, both crates): `ESC ] 8 ; ; http://a ESC \ A ESC ] 8 ; ; http://` + 11 MiB of `u` + `ESC \ B`.
  Observed upstream: `B` is linked to the 11,534,343-byte URI. Observed with the payload limit and without this patch:
  `B` stays linked to `http://a`, a link the program moved away from. Expected: `B` unlinked.
- xterm does not implement OSC 8 (`misc.c`, `do_osc` has no case for it). tmux keeps the previous link: a discarded
  sequence returns from `input_exit_osc` before any handler runs, so the cell's link is unchanged.
- Fix: a `Perform::osc_overflow(params, bell_terminated)` method with an empty default, called with only the parameters
  that ended before the limit, so no truncated data leaves the parser. `ansi::Processor` implements it to end the active
  hyperlink when the dropped sequence is an OSC 8.
- Why Farhelm needs it: text after a dropped link must not be attributed to a link it never had; and a link made long
  enough for the parser to drop would otherwise behave differently from a slightly shorter one, which the hyperlink
  limit below ends.
- Upstream code: `vte/src/lib.rs`, `trait Perform` and `Parser::osc_end`; `vte/src/ansi.rs`, `Performer::osc_overflow`.
- Tests: `tests::osc_overflow_reports_only_complete_params` (vte); `tests::dropped_osc8_ends_hyperlink` (vte, `ansi`);
  `too_long_hyperlink_ends_active_link_at_either_layer` (`pair-tests`).
- Upstream status: not yet reported. Upstreamable together with a configurable OSC limit: additive trait method with a
  default.

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
- Tests: `term::tests::zerowidth_characters_per_cell_are_capped` (vendored).
- Upstream status: not yet reported. The value is Farhelm policy; an upstream-friendly version is a `Config` option, or
  a fixed limit chosen with upstream (xterm's and tmux's precedents above suggest one would be accepted).

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
- Tests: `term::tests::hyperlink_uri_and_id_lengths_are_capped` (vendored);
  `too_long_hyperlink_ends_active_link_at_either_layer` (`pair-tests`).
- Upstream status: not yet reported. Upstreamable as is: the values follow VTE and the specification.

#### alacritty_terminal: reusing a repeated hyperlink

- Upstream today: every OSC 8 builds a new link, and setting it on the cursor's template copies the template's cell
  extra (`Arc::make_mut` on an extra that already printed cells share), so cells printed after two equal OSC 8s hold two
  equal but separate links and extras.
- Reproduction (10x2): `ESC ] 8 ; id=a ; http://e/ BEL x ESC ] 8 ; id=a ; http://e/ BEL y`. Observed (from the code):
  `x` and `y` hold separate allocations of the same link. Expected: one shared allocation.
- xterm does not implement OSC 8. tmux reuses a link whose id and URI it already holds (`hyperlinks.c`,
  `hyperlinks_put`, the `RB_FIND` when the program gave an id), and like this patch never merges links without one.
- Fix: an OSC 8 that repeats the active link's explicit id and URI keeps the active link. A link without an id gets a
  fresh generated id, is never equal to the active one, and is built as before (xterm.js also keeps such links apart).
- Why Farhelm needs it: a program that repeats one link before every character would otherwise cost a link allocation
  per cell.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::set_hyperlink`.
- Tests: `term::tests::repeated_hyperlink_is_shared` (vendored).
- Upstream status: not yet reported. Upstreamable as is: no behavior changes, only sharing.

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
- Tests: `term::tests::title_length_is_capped` (vendored).
- Upstream status: not yet reported. The value is Farhelm policy. An upstream-friendly version could instead lower
  `TITLE_STACK_MAX_DEPTH` toward xterm's and tmux's 10, which bounds the stack without cutting titles, with a length
  limit as a `Config` option.

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
- Tests: `term::tests::history_counters`, `term::tests::history_counters_clear_and_reset` (vendored).
- Upstream status: not yet reported. Upstreamable as is: additive.

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
- Tests: `term::tests::a_one_column_grid_drops_wide_characters` (vendored);
  `a_one_column_terminal_drops_wide_characters`, `a_zero_size_becomes_one_cell` and
  `narrow_terminals_survive_any_output` (`farhelm-terminal-emulator`, `tests/narrow.rs`).
- Upstream status: not yet reported. Upstreamable as is (a panic fix); upstream may prefer printing a placeholder.

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
- Tests: `term::tests::a_one_column_grid_drops_wide_characters` (vendored, its resize half, on a timed thread);
  `resizing_to_one_column_with_wide_characters` and `narrow_terminals_survive_any_output` (`tests/narrow.rs`, each
  bounded to 10 seconds).
- Upstream status: not yet reported. Upstreamable as is (a hang fix).

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
- Tests: `tests::split_utf8_does_not_drop_the_characters_after_it` (vte); `the_cut_into_reads_does_not_matter`
  (`farhelm-terminal-emulator`, `tests/boundary.rs`).
- Upstream status: not yet reported. Upstreamable as is.

#### vte: a split UTF-8 C1 control and REP (kept as upstream has it)

- Upstream today: a C1 control encoded in UTF-8 (U+0080 to U+009F, `\xc2 \x80` to `\xc2 \x9f`) is executed when it
  arrives in one call (`ground_dispatch`), but printed when it is split across two (`advance_partial_utf8`). alacritty
  draws nothing for it either way, but `ansi::Processor` then remembers it as the character a later REP repeats.
- Reproduction (10x1): `x \xc2 \x85 ESC [ 3 b`. Observed in one call: `xxxx`. Split after `\xc2`: `x`, since REP repeats
  U+0085, which draws nothing. Expected: `xxxx` both ways.
- xterm and tmux decode UTF-8 a byte at a time (previous record), so the cut cannot matter there; what each does with an
  encoded C1 control was not checked.
- Fix: none; kept as upstream behavior. A fix would dispatch the completed character the way the unsplit path does
  (`ground_dispatch` on its UTF-8 encoding) in both arms of `advance_partial_utf8`.
- Why Farhelm accepts it: it takes an encoded C1 control split across reads and then a REP, and the vendored patch is
  kept to the bug that loses characters. The emulator documents it as the one known case where the cut into reads still
  changes the terminal after a final flush (`Emulator::feed`).
- Upstream code: `vte/src/lib.rs`, `Parser::advance_partial_utf8` and `Parser::ground_dispatch`.
- Tests: `a_split_encoded_c1_control_changes_what_rep_repeats` (`farhelm-terminal-emulator`, `tests/boundary.rs`) pins
  today's behavior.
- Upstream status: not yet reported. Upstreamable as the one-line change above, if Farhelm or upstream wants it.

#### vte: XTVERSION reaches the handler

- Upstream today: `CSI > q` and `CSI > 0 q` (XTVERSION, a program asking for the terminal's name and version) fall
  through `csi_dispatch` as unhandled, so no `Handler` can answer them.
- Reproduction: feed `ESC [ > 0 q` to `ansi::Processor` with any handler. Observed: no handler method is called.
  Expected: a call the handler can answer with `DCS > | name ST`.
- xterm answers `DCS > | XTerm(version) ST` for a parameter of 0 or none (`charproc.c`, `CASE_REPORT_VERSION`). tmux
  3.7c answers `DCS > | tmux 3.7c ST` for 0 (`input.c`, `INPUT_CSI_XDA`). xterm.js was not checked for this one.
- Fix: a `Handler::report_version` method with an empty default, called for `CSI > q` and `CSI > 0 q` only. A handler
  that does not implement it behaves as before.
- Why Farhelm needs it: Claude Code asks XTVERSION first, and with no answer asks nothing further, including whether the
  terminal supports synchronized output (DECRQM 2026), and then draws without it (read from Claude Code 2.1.316's
  startup code). The emulator answers `Farhelm <version>` (`farhelm-terminal-emulator`, `src/queries.rs`).
- Upstream code: `vte/src/ansi.rs`, `trait Handler` and `Performer::csi_dispatch`.
- Tests: `ansi::tests::xtversion_reaches_the_handler` (vte); `identification_status_and_window_queries`
  (`farhelm-terminal-emulator`, `tests/queries.rs`); `emulator_parity::queries::recorded_agent_probes_get_tmux_answers`.
- Upstream status: not yet reported. Upstreamable as an additive API; the method name is Farhelm's choice.

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
- Tests: `term::tests::erase_above_from_the_second_line_clears_the_first` (vendored);
  `emulator_parity::generated::hand_picked_screens_match_tmux`, scenario "erase operations".
- Upstream status: not yet reported. Upstreamable as is.

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
  (`screen_write_combine`), and a wide character dropped with autowrap off leaves the cursor where it is. xterm and
  xterm.js were not checked for these.
- Fix: `Term::input` raises the flag at the end of a last-column write only while autowrap is on, and returns without
  raising it when it drops a wide character with autowrap off (a flag already raised stays raised, since nothing moves).
  Every other reader of the flag (except `CSI J`, under its own record below) keeps upstream's code; with the flag
  meaning what its documentation says, they behave as tmux does in the cases above. Farhelm's cursor report
  (`farhelm-terminal-emulator`, `src/queries.rs`) and its parity harness read the flag as tmux's cursor one past the
  last column.
- Why Farhelm needs it: the status readers were written against tmux's captures, and a kept last-column character or an
  extra scroll changes the screen they read and viewers see. The recorded Claude Code and Codex sessions never turn
  autowrap off, so this matters for other programs in a session's terminal.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::input`.
- Tests: `term::tests::writing_the_last_column_with_autowrap_off_raises_no_wrap` (vendored);
  `emulator_parity::generated::hand_picked_screens_match_tmux`, scenarios "autowrap off at the last column: reports and
  erases", "autowrap off at the last column, then a resize" and "autowrap off at the last column of the alternate
  screen, then a wider screen", against a live tmux 3.7c; the cursor-report rows of
  `identification_status_and_window_queries` (`farhelm-terminal-emulator`, `tests/queries.rs`) and of
  `emulator_parity::queries::the_answer_table_matches_tmux`. What still differs from tmux at a pending wrap (a tab, a
  wider alternate screen, a wide character after the corner-cell idiom, writing after autowrap is turned off) is pinned
  in `emulator_parity::generated::known_differences_stay_as_documented`.
- Upstream status: not yet reported. Upstreamable as is: it makes the flag mean what the crate documents.

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
- Fix: erasing below starts after the cursor's column when `input_needs_wrap` is set. With the wrap-flag fix above, the
  flag is set only for a column written with autowrap on (or by a reflowing resize, which upstream code leaves as it
  is), so a column written with autowrap off is erased, as tmux erases it.
- Why Farhelm needs it: the status readers were written against tmux's captures, and the erased character changes them.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::clear_screen`.
- Tests: `term::tests::erase_below_with_a_wrap_pending_keeps_the_last_column` (vendored: written with autowrap on, off,
  off then on, on then off, and on then a wide character dropped with autowrap off);
  `emulator_parity::generated::hand_picked_screens_match_tmux`, scenarios "erase operations" and "autowrap off at the
  last column: reports and erases", against a live tmux 3.7c.
- Upstream status: not yet reported. Arguable upstream: it makes `CSI J` agree with alacritty's own `CSI K`, but xterm
  does the opposite for both, so upstream may prefer xterm's behavior. On its own, without the wrap-flag fix, it would
  also keep a character written with autowrap off, which tmux erases.

#### alacritty_terminal: relative vertical moves stop at the scroll margins

- Upstream today: `move_up`, `move_down`, `move_up_and_cr` and `move_down_and_cr` (CUU, CUD, CPL, CNL) compute the
  target line and call `goto`, which clamps to the screen only, so a move from inside the scroll region passes its
  margin; in origin mode `goto` adds the region's offset to the already absolute line.
- Reproduction (4x6, region lines 2 to 4): `ESC [ 2 ; 4 r ESC [ 3 ; 2 H ESC [ 9 A`. Observed cursor (0, 1). Expected (1,
  1), the top margin. In origin mode: `ESC [ 2 ; 4 r ESC [ ? 6 h ESC [ 1 B`. Observed (3, 0). Expected (2, 0).
- xterm stops at the margin when the cursor starts inside it, else at the screen's edge (`cursor.c`, `CursorUp`,
  `CursorDown`), and moves from the absolute row in origin mode; tmux does the same (`screen-write.c`,
  `screen_write_cursorup`, `screen_write_cursordown`). xterm.js stops at the margins too, but in origin mode adds the
  offset a second time, as upstream does.
- Fix: a `vertical_move` helper that applies those limits and sets the cursor directly. The count is clamped to the
  screen's height first, so the addition cannot overflow.
- Why Farhelm needs it: programs that move relative to the cursor inside a scroll region would draw in the wrong place
  on the screen agents' status is read from and viewers see.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::move_up`, `move_down`, `move_up_and_cr`,
  `move_down_and_cr`.
- Tests: `term::tests::relative_vertical_moves_stop_at_the_scroll_margins` (vendored);
  `emulator_parity::generated::hand_picked_screens_match_tmux`, scenario "scroll regions";
  `emulator_parity::generated::generated_streams_match_tmux`.
- Upstream status: not yet reported. Upstreamable as is.

#### alacritty_terminal: CHA in origin mode keeps the cursor's line

- Upstream today: `goto_col` (CHA, `CSI Ps G`) calls `goto` with the cursor's absolute line, which in origin mode adds
  the region's offset again and moves the cursor down.
- Reproduction (4x6): `ESC [ 2 ; 4 r ESC [ ? 6 h ESC [ 2 ; 1 H ESC [ 3 G`. Observed cursor (3, 2). Expected (2, 2).
- xterm keeps the line (`charproc.c`, `CASE_HPA`, which passes `CursorRow`, the line relative to the region, to
  `CursorSet`); tmux leaves the line alone (`input.c`, `INPUT_CSI_HPA`, `screen_write_cursormove` with `py = -1`).
  xterm.js adds the offset a second time, as upstream does.
- Fix: `goto_col` moves through `vertical_move` with no lines to move.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::goto_col`.
- Tests: `term::tests::cha_in_origin_mode_keeps_the_line` (vendored);
  `emulator_parity::generated::hand_picked_screens_match_tmux`, scenario "scroll regions".
- Upstream status: not yet reported. Upstreamable as is.

#### alacritty_terminal: resetting origin mode homes the cursor

- Upstream today: setting origin mode (`CSI ? 6 h`) homes the cursor, resetting it (`CSI ? 6 l`) leaves the cursor where
  it is.
- Reproduction (4x6): `ESC [ 2 ; 4 r ESC [ ? 6 h ESC [ 2 ; 2 H ESC [ ? 6 l`. Observed cursor (2, 1). Expected (0, 0).
- xterm homes it on both (`charproc.c`, `srm_DECOM` in `dpmodes`, `CursorSet(screen, 0, 0, ...)`); so does tmux
  (`input.c`, `input_csi_dispatch_rm_private`, mode 6) and xterm.js.
- Fix: the reset calls `goto(0, 0)` as the set does.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::unset_private_mode`.
- Tests: `term::tests::resetting_origin_mode_homes_the_cursor` (vendored);
  `emulator_parity::generated::generated_streams_match_tmux`.
- Upstream status: not yet reported. Upstreamable as is.

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
  xterm.js, which renders Farhelm's terminals today, does not implement 1005 at all ("DECSET 1005 not supported"; DECRQM
  1005 answers 4, permanently reset), so 1006 stays in force there whatever follows it. xterm, which upstream follows,
  makes all its coordinate modes (1005, 1006, 1015, 1016) exclusive on purpose, the last one set winning, and resets one
  only if it is the active one (`charproc.c`, `dpmodes`, the `srm_EXT_MODE_MOUSE` group: "Rather than choose an
  arbitrary precedence among the coordinate modes, they are mutually exclusive").
- Fix: setting 1005 or 1006 sets only its own bit; resetting was already independent. An embedder that sees both uses
  SGR; the emulator's `Modes::from_term_mode` already reads SGR first (`farhelm-terminal-emulator`, `src/modes.rs`).
- Why Farhelm needs it: the supervisor encodes viewers' mouse events from these modes for terminals in terminal hosts,
  and must report to a program the way today's terminal does: a program that asks for SGR and then also for UTF-8 gets
  SGR from xterm.js and from tmux, and would have got UTF-8 coordinates from the emulator. DECRQM answers now match
  tmux's for both modes too.
- Upstream code: `alacritty_terminal/src/term/mod.rs`, `Term::set_private_mode`.
- Tests: `term::tests::utf8_and_sgr_mouse_encodings_are_independent` (vendored: both orders, one sequence with both, and
  resetting either); `mouse_modes_replace_one_another_but_encodings_combine` (`farhelm-terminal-emulator`,
  `tests/modes.rs`); `both_mouse_encodings_set_round_trip` (`tests/snapshot.rs`); and the encoder's
  `sgr_wins_over_utf8_whichever_was_set_first` (`farhelm-supervisor`, `grid_input`); and two rows of
  `emulator_parity::queries::the_answer_table_matches_tmux` (both orders, against a live tmux 3.7c).
- Upstream status: not yet reported. Not a bug fix: the exclusion follows xterm on purpose, so this is a question of
  which reference to follow, and is better raised upstream as a question. Before raising it, check how the alacritty
  application's own mouse reporting reads the two bits when both are set (not checked for this record; it is outside the
  crate).

#### alacritty_terminal: a per-terminal limit on cell extra storage

- Upstream today: a cell keeps combining characters, an underline color and an OSC 8 hyperlink in a separate
  `Arc<CellExtra>`, shared by the cells printed from one cursor template and copied on write (`Arc::make_mut`) whenever
  one cell's set changes. Nothing bounds how many there are, so output that varies them from cell to cell gives every
  cell its own allocation, and a hyperlink sent without an `id` is always a new link. One terminal's memory then grows
  with its scrollback times the size of an extra and its link.
- Reproduction (200x60, `scrolling_history` 12,000): 12,060 lines that each repeat, 200 times,
  `ESC [ 58 ; 2 ; <r> ; <c> ; <g> m` (a different underline color per cell), then `e` and sixteen U+0301. Observed: 387
  MiB resident, against 56 MiB for the same grid of plain text. With
  `ESC ] 8 ; id=<unique 250 bytes> ; <2083-byte URI> ESC \ x` per cell, about 6 GB. Without any combining marks, saving
  a template with an underline color and a link
  (`ESC ] 8 ; id=keep ; http://k/ ESC \ ESC [ 58 ; 5 ; 3 m ESC 7 ESC [ 59 m ESC ] 8 ; ; ESC \`), then repeating `ESC 8`,
  a cursor move and `ESC [ 59 m x` once per cell also gives every cell a copy (122 MiB after 4,000 rows). Expected with
  the patch: at most `MAX_EXTRA_STORAGE_BYTES` (64 MiB) charged, 120 MiB resident in the first case.
- tmux 3.7c keeps at most 32 bytes of UTF-8 per cell (`tmux.h`, `UTF8_SIZE`) and at most 5,000 hyperlinks in the whole
  server, evicting the oldest (`hyperlinks.c`, `MAX_HYPERLINKS`); its extended cell attributes are a small fixed-size
  entry per cell. xterm keeps a fixed number of combining characters per cell (its `combiningChars` resource; its source
  was not checked for this record).
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
- Tests: `term::tests::hyperlinks_are_bounded_by_the_extra_storage`,
  `term::tests::combining_characters_and_underline_colors_are_bounded` and
  `term::tests::restoring_a_saved_cursor_cannot_grow_the_storage_past_the_limit` (vendored);
  `floods_of_cell_extras_are_bounded` (`farhelm-terminal-emulator`, `tests/hostile.rs`),
  `fixups_share_extras_and_respect_the_storage` (`tests/snapshot.rs`), and the ignored measurements in
  `tests/memory.rs`.
- Upstream status: not yet reported. The limit itself is Farhelm policy (one process hosting many terminals); alacritty
  runs one terminal per window, where unbounded growth is the user's own program's doing. An upstream-friendly version
  would make the limit a `Config` field (`max_extra_storage: Option<usize>`, `None` meaning unlimited, as today), keep
  the accounting, and leave the degrade behavior as documented here.

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
- Why Farhelm needs it: without it, a program repeating `SGR 0` before every character while a link is active made a
  copy per cell, which the extra storage limit above could only catch by refusing an attribute the program never
  changed.
- Upstream code: `alacritty_terminal/src/term/cell.rs`, `Cell::set_underline_color`, `Cell::set_hyperlink`,
  `Cell::clear_wide`.
- Tests: `term::tests::repeating_an_unchanged_attribute_copies_nothing` (vendored). It goes through the terminal, whose
  extra storage setters make the same check before calling these, so it pins the behavior rather than this hunk alone;
  an upstream version without the storage would carry the test as is.
- Upstream status: not yet reported. Upstreamable as is: a pure saving of memory and allocation with no behavior change.

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
  `screen_write_clearscreen`, through `grid_view_clear`). xterm's source was not checked for this record.
- Fix: `Row::grow` sets `occ` to at least the old length before appending, so the old cells count as occupied.
- Why Farhelm needs it: correct screens, and because a snapshot cannot reproduce a row's `occ`, the stale colors made a
  restored terminal differ from the original after the next clear (the snapshot round-trip test found it).
- Upstream code: `alacritty_terminal/src/grid/row.rs`, `Row::grow`.
- Tests: `term::tests::clearing_after_a_resize_clears_every_cell` (vendored; fails without the fix).
- Upstream status: not yet reported. Upstreamable as is.

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
  and the reflow blanks it and its spacer. tmux has no consistent behavior to copy here: it ignores a wide character
  that does not fit only while wrapping is off, and with wrapping on, a one-column pane shows the character anyway.
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
  field may need the hunk rewritten.
- Unpack the new crate twice in the scratch directory, as `a/<name>` (left as published, minus `tests/` for
  `alacritty_terminal`) and `b/<name>`, and apply the old patch to `b` with `patch -p1 -d b`. Fix rejected hunks by
  hand, keeping each hunk small and marked `Farhelm patch`.
- Regenerate the patch from the scratch directory with `git diff --no-index --no-prefix a/<name> b/<name>`, saved as
  `patches/<name>-<version>.diff` in place of the old file.
- Replace `vendor/<name>/` with `b/<name>`, update the version pins in the crates that depend on it, the checksums and
  commits above, and `THIRD_PARTY_NOTICES.md`, and run the tests above plus the emulator crate's
  (`cargo nextest run -p farhelm-terminal-emulator`, recorded as `.agents/validation.md` gives it). For an
  `alacritty_terminal` upgrade, also start `pair-tests/Cargo.lock` again from a copy of the new crate's `Cargo.lock` and
  let one unlocked `cargo test` there adjust it.
- A `vte` upgrade usually comes with an `alacritty_terminal` upgrade that requires it; upgrade them together.
