# Printed file paths (soft-wrapped and hard-cut) can consume and underline adjacent OSC 8 cells (including concealed ones) when hovered outside the OSC 8 span

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When a program hyperlink (including one whose cells are concealed) sits immediately beside plain terminal text with no
space between them — on the same row, across a soft wrap, or on the second row of a hard-cut path — hovering the plain
text can turn the combined text into a clickable file path that incorporates the hyperlink's hidden text and underlines
across the hyperlink's cells, even though the program hyperlink is supposed to own those cells.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-grid-model/src/paths.rs:321`.

- In `crates/gui/farhelm-grid-model/src/paths.rs:321-328`, `GridModel::path_link_at` only checks
  `self.has_osc_link_at(position)` at the single hovered cell `position`, and both `soft_path_links`
  (`paths.rs:353-380`) and `cut_path_link` (`paths.rs:392-440`) read all cell text on each line via `read_cells` ->
  `walk_cells(line, false, ...)` (`paths.rs:338-349`) without checking `GridRow::has_osc_link(from, to)` across the
  candidate path's span.
- By contrast, printed URL detection in `crates/gui/farhelm-grid-model/src/links.rs:395-398` rejects any candidate URL
  whose range overlaps an OSC 8 run (`if row.has_osc_link(from, to) { ranges.clear(); break; }`).
- Concrete examples:
  1. Same-row partial overlap: plain visible `notes` (cols 0..5, no OSC 8) followed immediately by concealed `.txt`
     (cols 5..9, OSC 8 with `style_flags::HIDDEN`), or plain `src/` (cols 0..4) followed by concealed `secret.txt` (cols
     4..14, OSC 8 with `style_flags::HIDDEN`). Hovering col 1 passes `has_osc_link_at(col 1) == false` and
     `link_at(col 1) == None`, so `path_link_at` returns a `LinkKind::Path` covering cols `0..9` (`notes.txt`) or
     `0..14` (`src/secret.txt`), and `TerminalView::underlined_rows` (`crates/gui/farhelm-gui/src/terminal.rs:713-729`)
     underlines across the concealed OSC 8 cells too. Visible OSC 8 runs adjacent to plain text, and soft-wrapped rows,
     exhibit the same overlap.
  2. Hard-cut sibling (`cut_path_link`, `paths.rs:392-440`): Row 0 ends with plain `src/long_dir/` (no OSC 8) and Row 1
     starts with concealed OSC 8 `secret.txt` (too long to fit in Row 0's remaining columns minus `TUI_MARGIN`).
     Hovering Row 0 passes `has_osc_link_at` on Row 0 and `cut_path_link(0)` joins both rows into
     `src/long_dir/secret.txt`, underlining Row 1's concealed OSC 8 cells.
- This violates SPEC.md ("Terminal experience", lines 3593-3596): "They are also excluded from a program hyperlink's
  visible label, its hover and click area, and its underline. The program hyperlink still owns those cells, preventing a
  printed URL or file path from taking its place there."
- Note on test coverage: the new test `a_concealed_program_link_keeps_ownership_over_a_printed_path`
  (`crates/gui/farhelm-grid-model/tests/paths.rs:121-149`) only tests hovering inside a fully-tagged concealed path and
  explicitly asserts `paths(&tagged, 0) == [label]`, missing the partial-overlap and hard-cut cases.
- How to verify: construct a row with `[0, 5, "notes", [0,-1,-1]]` and `[5, 4, ".txt", [style_flags::HIDDEN,-1,-1], 0]`
  with an OSC 8 link table entry, and call `model.path_link_at(Position { line: 0, col: 1 })` — it returns
  `Some(TerminalLink { kind: Path, uri: "notes.txt", ranges: [0..9], .. })` instead of `None`.
- Fix: reject any `LinkKind::Path` candidate whose covered cell ranges overlap an OSC 8 run
  (`row.has_osc_link(range.start.col, range.end.col)` for each row range in `link.ranges`, covering both
  `soft_path_links` and `cut_path_link`), matching `row_links`'s printed-URL ownership rule in `links.rs:395-398`.

Reviewer metadata (g58_osc8_link_disclosure/f1): suggested bucket: other (File downloads still require a visible hover
disclosure showing the host-resolved path before a click can transfer anything, so this does not bypass disclosure or
cause data loss; it is a correctness and spec-compliance violation where a printed path steals cells owned by an OSC 8
hyperlink and underlines concealed OSC 8 cells.); confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g58_osc8_link_disclosure/f1`.
