# Vendored terminal crates

NOTE: Do not edit the code under `alacritty_terminal/` or `vte/` directly. Each tree is an upstream crate plus the patch
next to it in `patches/`, and the procedure below depends on that staying true. A change goes into the tree and the
patch file together.

The supervisor's terminal emulator (`crates/farhelm-terminal-emulator/`) runs on `alacritty_terminal` 0.26.0 and its
parser `vte` 0.15.0. Both are used as published on crates.io except for a few small patches: accessors that snapshot and
restore need, caps on what hostile program output can make them store, two history counters for the grid stream, and two
bug fixes. The root `Cargo.toml` swaps these trees in for the crates.io versions with `[patch.crates-io]`.

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

Here's what each one changes and why. None changes behavior except the caps and the bug fixes.

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
- Window title length, `alacritty_terminal::term::MAX_TITLE_BYTES` = 4096. A longer title is cut at the last character
  boundary that fits, and gives back the rest of its allocation, which the parser sized for the whole payload and which
  would otherwise travel on in `Event::Title` to whoever keeps the latest title. This one exists because of the OSC cap:
  the title stack holds up to 4096 titles, so with 10 MiB titles one program could pin about 40 GiB. 4096 bytes leaves
  room for a shell's title holding a long path.

NOTE: These caps bound each piece of state, not a terminal's total, and they cannot hold a terminal anywhere near the
plain grid's size (a cell is 24 bytes, so 12,000 lines of history at 200 columns is about 58 MB). A bound on the total
is for the supervisor's emulator and its memory budget to set; it does not exist yet. Here is what is left, estimated
from the types' sizes and glibc's allocation rounding, not measured:

- Hyperlinks are the worst case. An OSC 8 without an `id` is a new link even when its URI repeats, so a program that
  sends one with a 2083-byte URI before every character gives each cell its own cell extra, link and copy of the URI:
  about 2.2 KB per cell, about 2.5 KB with a 250-byte `id`. On the grid above that is about 5.5 to 6 GB, from about 5 GB
  of output. Reusing a repeated link does not change this, since it only applies to links with an explicit `id`.
- Combining characters and underline colors have the same per-cell shape on a smaller scale: about 100 bytes per cell
  for one combining mark, about 150 for sixteen, and about 64 for an underline color changed before every cell. On the
  same grid that is a few hundred MB.
- Outside the grid, each terminal can hold up to about 30 MiB more: the title stack (4096 titles of up to 4096 bytes, 16
  MiB), an OSC the program never ends (up to the 10 MiB limit buffered, in a `Vec` whose capacity can reach 16 MiB as it
  doubles), and the 2 MiB buffer vte's `Processor` reserves up front for synchronized updates (resident only as far as
  an update fills it).

### Per-patch records for upstream reports

Each record below holds what an upstream report or pull request needs, so that nobody has to rediscover it: what
upstream does, a byte-level reproduction, what xterm and tmux 3.7c do (with the source checked), the fix, the upstream
code touched, the tests that pin it, and its upstream status. Sizes are columns by lines, and cursor positions are
(line, column) from 0. The xterm source checked is the `ThomasDickey/xterm-snapshots` mirror as of 2026-11-01
(`charproc.c` 1.2134, `cursor.c` 1.101, `main.h` 1.93, `misc.c` 1.1137, `ptydata.c` 1.173, `ptyx.h` 1.1174, `screen.c`
1.682, `util.c` 1.1032); the tmux source is the 3.7c release Farhelm pins (`scripts/build-pinned-tmux-ci.sh`,
`.github/release/source-pins.env`). xterm.js, which renders Farhelm's terminals today, was checked in its vendored copy
(`crates/farhelm-ui/assets/vendor/xterm.js`). The "observed" results were produced by feeding the bytes to the
published, unpatched crates.

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

### History counters

The grid stream sends scrollback to viewers as appended ranges and needs to know how many lines entered history since it
last looked. `Grid::lines_scrolled_into_history()` counts every line that scrolls off the top of a grid that has history
(so never on the alternate screen), and keeps counting once history is full. `Grid::history_clears()` counts history
being erased, which a program does on its own with `CSI 3 J` or a full reset. Neither ever decreases. They do not
describe a resize or reflow, which the embedder starts itself and has to treat as a change of shape. Both fields are
skipped by serde and left out of `Grid`'s `PartialEq`, so serialized grids and grid comparisons are unchanged.

### Bug fixes

Two upstream bugs that ordinary program output reaches. Each fix is a few lines in the place the bug is.

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
  and its dispatch and `advance_partial_utf8` in `vte/src/lib.rs`, `Processor` in `vte/src/ansi.rs`,
  `Cell::push_zerowidth`, `Term::input`, `Term::set_hyperlink`, `Term::set_title`, `Grid::scroll_up`,
  `Grid::shrink_columns` and `Grid::clear_history`. A cap that upstream now enforces itself may make a hunk unnecessary;
  a moved field may need the hunk rewritten.
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
