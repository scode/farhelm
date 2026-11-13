//! Alt-click cursor movement from the pinned grid-input.js / xterm.js 6.0.0
//! MoveToCell.ts (MIT; notices in LICENSE-PORTED). Preserve the
//! original branch behavior, including its column-as-row argument quirk.

use crate::Platform;
use farhelm_grid_model::GridModel;
use farhelm_proto::grid::GridInput;

/// At most three runs represent a whole logical move without allocating one
/// object per arrow. The transport peeks before a send and advances only after
/// it succeeds, so an incoming frame winning a select cannot lose an arrow.
#[derive(Clone, Debug, Default)]
pub struct ArrowMoves {
    runs: [(u8, usize); 3],
    mac: bool,
}

impl ArrowMoves {
    /// Remaining wire frame count; local queue admission needs only one slot.
    pub fn len(&self) -> usize {
        self.runs.iter().map(|(_, count)| count).sum()
    }

    /// An unchanged cursor or unavailable grid has no input side effect.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One ordinary Key lets the supervisor apply its current cursor-key mode.
    /// This method is repeatable until advance; it never consumes input itself.
    pub fn peek(&self) -> Option<GridInput> {
        let (direction, _) = self.runs.iter().find(|(_, count)| *count != 0)?;
        let (key, kc) = match direction {
            b'A' => ("ArrowUp", 38),
            b'B' => ("ArrowDown", 40),
            b'C' => ("ArrowRight", 39),
            b'D' => ("ArrowLeft", 37),
            _ => unreachable!("only the pinned algorithm constructs runs"),
        };
        Some(GridInput::Key {
            key: key.into(),
            code: key.into(),
            kc,
            mods: 0,
            mac: self.mac,
        })
    }

    /// Acknowledge exactly one successfully sent frame, retaining run order.
    pub fn advance(&mut self) {
        if let Some((_, count)) = self.runs.iter_mut().find(|(_, count)| *count != 0) {
            *count -= 1;
        }
    }
}

/// Borrow the existing committed screen; there is no second buffer or history
/// copy. The old algorithm's line flags describe continuation *from* the prior
/// row, whereas the wire's wraps flag means continuation *to* the following row.
struct Screen<'a>(&'a GridModel);

impl Screen<'_> {
    /// Screen row zero has no preceding screen row, even with retained history.
    fn wrapped(&self, row: i32) -> bool {
        row > 0 && row < i32::from(self.0.size().1) && self.0.screen()[row as usize - 1].wraps
    }

    /// Number of preceding rows belonging to the same soft-wrapped line.
    fn wrapped_rows(&self, mut row: i32) -> i32 {
        let mut count = 0;
        while self.wrapped(row) {
            count += 1;
            row -= 1;
        }
        count
    }

    /// Vertical arrows count logical lines, omitting their continuation rows.
    fn requested_row(&self, start: i32, target: i32) -> usize {
        let first = start - self.wrapped_rows(start);
        let last = target - self.wrapped_rows(target);
        let direction = if start > target { -1 } else { 1 };
        let distance = (first - last).unsigned_abs() as usize;
        distance
            - (0..distance)
                .filter(|i| self.wrapped(first + direction * *i as i32))
                .count()
    }

    /// Match linkLines.text's JavaScript UTF-16 length without building strings.
    /// Starting on a wide spacer contributes a space here; GridRow::text omits
    /// that spacer for copy, so it cannot be substituted for this calculation.
    fn text_len(&self, row: i32, from: i32, to: i32) -> usize {
        let Some(line) = usize::try_from(row)
            .ok()
            .and_then(|row| self.0.screen().get(row))
        else {
            return 0;
        };
        let mut col = from.max(0) as u16;
        let end = to.clamp(0, i32::from(self.0.size().0)) as u16;
        let mut run_index = line.runs.partition_point(|run| run.col + run.width <= col);
        let mut length = 0;
        while col < end {
            while line
                .runs
                .get(run_index)
                .is_some_and(|run| run.col + run.width <= col)
            {
                run_index += 1;
            }
            if let Some(run) = line.runs.get(run_index).filter(|run| run.col <= col)
                && col == run.col
                && !run.is_plain_flow()
            {
                length += run.text.encode_utf16().count().max(1);
                col += run.width;
                continue;
            }
            length += 1;
            col += 1;
        }
        length
    }

    /// The port's asymmetric backward slices include the starting cell at each
    /// crossed row boundary. Do not replace this with rectangular cell distance.
    fn between(
        &self,
        mut col: i32,
        mut row: i32,
        end_col: i32,
        end_row: i32,
        forward: bool,
    ) -> usize {
        let mut start = col;
        let mut length = 0;
        let cols = i32::from(self.0.size().0);
        while (col != end_col || row != end_row) && row >= 0 && row < i32::from(self.0.size().1) {
            col += if forward { 1 } else { -1 };
            if forward && col > cols - 1 {
                length += self.text_len(row, start, col);
                col = 0;
                start = 0;
                row += 1;
            } else if !forward && col < 0 {
                length += self.text_len(row, 0, start + 1);
                col = cols - 1;
                start = col;
                row -= 1;
            }
        }
        length + self.text_len(row, start, col)
    }
}

/// Move from the committed cursor to a clamped screen cell. Normal-screen
/// arrows count cells; alternate-screen arrows count untrimmed text through
/// soft wraps. Admission (quick Alt release, short selection, following tail)
/// belongs to the frontend's existing primary selection gesture.
pub fn move_to_cell(model: &GridModel, x: u16, y: u16, platform: Platform) -> ArrowMoves {
    let mut result = ArrowMoves {
        mac: platform == Platform::Mac,
        ..Default::default()
    };
    let (cols, rows) = model.size();
    let (Some(cursor), Some(state)) = (model.cursor(), model.state()) else {
        return result;
    };
    if cols == 0 || rows == 0 {
        return result;
    }
    let (x, y) = (i32::from(x.min(cols - 1)), i32::from(y.min(rows - 1)));
    let (start_x, start_y) = (i32::from(cursor.x), i32::from(cursor.y));
    if !state.alt_screen {
        let (direction, count) = if start_y == y {
            (
                if start_x > x { b'D' } else { b'C' },
                (start_x - x).unsigned_abs() as usize,
            )
        } else {
            (
                if start_y > y { b'D' } else { b'C' },
                ((start_y - y).abs() * i32::from(cols)
                    + if start_y > y {
                        start_x - x
                    } else {
                        x - start_x
                    }) as usize,
            )
        };
        result.runs[0] = (direction, count);
        return result;
    }
    let screen = Screen(model);
    let vertical = screen.requested_row(start_y, y);
    let reset = if vertical == 0 {
        0
    } else {
        screen.between(
            start_x,
            start_y,
            start_x,
            start_y - screen.wrapped_rows(start_y),
            false,
        )
    };
    let first = if vertical > 0 {
        y - screen.wrapped_rows(y)
    } else {
        start_y
    };
    // The pinned horizontalDirection passes targetX where a start row belongs.
    // It is an observed compatibility quirk, preserved by the full oracle table.
    let direction_row = if screen.requested_row(x, y) > 0 {
        y - screen.wrapped_rows(y)
    } else {
        start_y
    };
    let right = (start_x < x && direction_row <= y) || (start_x >= x && direction_row < y);
    let horizontal = screen.between(start_x, first, x, y, right);
    result.runs = [
        (b'D', reset),
        (if start_y > y { b'A' } else { b'B' }, vertical),
        (if right { b'C' } else { b'D' }, horizontal),
    ];
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::grid::GridOutput;
    use serde_json::json;

    /// JavaScript's seeded generator multiplies as f64 before its bitwise mask;
    /// integer multiplication produces a different table once precision is lost.
    fn random(seed: &mut u64, n: u64) -> u64 {
        *seed = ((*seed as f64 * 1103515245. + 12345.) as u64) & 0x7fff_ffff;
        *seed % n
    }

    /// Alt-click counts the link adapter's UTF-16 slices: supplementary and
    /// combining text can take two units in one head cell, while a wide spacer
    /// contributes one blank if the requested slice starts on that spacer.
    #[test]
    fn cursor_text_slices_preserve_utf16_and_wide_spacer_rules() {
        let mut model = GridModel::new();
        for value in [
            json!({"t":"hello","cols":4,"rows":1,"history_cap":0}),
            json!({"t":"state","mouse":"off","focus":false,"alt_screen":true,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
            json!({"t":"rows","rows":[[0,{"w":false,"r":[[0,2,"😀",[0,-1,-1]],[2,1,"a\u{301}",[0,-1,-1]]]}]],"links":[]}),
            json!({"t":"frame","rev":1,"cols":4,"rows":1,"cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
        ] {
            let message: GridOutput = serde_json::from_value(value).unwrap();
            let _ = model.apply(&message.encode().unwrap()).unwrap();
        }
        let screen = Screen(&model);
        assert_eq!(screen.text_len(0, 0, 1), 2);
        assert_eq!(screen.text_len(0, 1, 2), 1);
        assert_eq!(screen.text_len(0, 2, 3), 2);
        assert_eq!(screen.text_len(0, 0, 4), 5);
    }

    /// Replay the pinned generator's 1600 screens/moves against its unchanged
    /// independent oracle. Case order also consumes the application-mode cases;
    /// the frontend sends Key messages so cursor mode remains the supervisor's.
    #[test]
    fn pinned_alt_click_table_matches_every_move() {
        let mut seed = 11;
        let mut expected = include_str!("../fixtures/move-to-cell.txt")
            .lines()
            .filter(|line| !line.starts_with('#'));
        let mut cases = 0;
        for _ in 0..400 {
            let cols = 2 + random(&mut seed, 9);
            let rows = 1 + random(&mut seed, 6);
            let lines: Vec<String> = (0..rows)
                .map(|_| {
                    (0..cols)
                        .map(|_| if random(&mut seed, 4) != 0 { 'x' } else { ' ' })
                        .collect()
                })
                .collect();
            let wrapped: Vec<_> = (0..rows)
                .map(|row| row > 0 && random(&mut seed, 3) == 0)
                .collect();
            for scrollback in [false, true] {
                for _application in [false, true] {
                    let start = (random(&mut seed, cols), random(&mut seed, rows));
                    let target = (random(&mut seed, cols), random(&mut seed, rows));
                    let mut model = GridModel::new();
                    let screen: Vec<_> = lines.iter().enumerate().map(|(row, text)| json!([row, {"w": wrapped.get(row + 1).copied().unwrap_or(false), "r": [[0, cols, text, [0, -1, -1]]]}])).collect();
                    for value in [
                        json!({"t":"hello", "cols":cols, "rows":rows, "history_cap":0}),
                        json!({"t":"state", "mouse":"off", "focus":false, "alt_screen":!scrollback, "default_fg":null, "default_bg":null, "history_trimmed":false, "exit":null}),
                        json!({"t":"rows", "rows":screen, "links":[]}),
                        json!({"t":"frame", "rev":1, "cols":cols, "rows":rows, "cursor":{"x":start.0,"y":start.1,"visible":true,"shape":"block","blink":false}}),
                    ] {
                        let message: GridOutput = serde_json::from_value(value).unwrap();
                        let _ = model.apply(&message.encode().unwrap()).unwrap();
                    }
                    let moves =
                        move_to_cell(&model, target.0 as u16, target.1 as u16, Platform::Linux);
                    let mut actual = String::new();
                    for (direction, count) in moves.runs {
                        if count != 0 {
                            // Adjacent runs with the same arrow are one table entry.
                            let arrow = direction as char;
                            actual.extend(std::iter::repeat_n(arrow, count));
                        }
                    }
                    let mut oracle = String::new();
                    let mut count = String::new();
                    for ch in expected.next().unwrap().chars() {
                        if ch.is_ascii_digit() {
                            count.push(ch);
                        } else if ch != '-' {
                            oracle.extend(std::iter::repeat_n(
                                ch.to_ascii_uppercase(),
                                count.parse().unwrap(),
                            ));
                            count.clear();
                        }
                    }
                    assert_eq!(
                        actual, oracle,
                        "case={cases}, start={start:?}, target={target:?}, wrapped={wrapped:?}, scrollback={scrollback}"
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 1600);
        assert!(expected.next().is_none());
    }
}
