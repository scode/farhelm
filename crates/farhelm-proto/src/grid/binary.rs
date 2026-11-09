//! Fixed-width grid payloads. Every read stays inside its message or row
//! slice, so a forged count cannot allocate from an unchecked peer value.
//! Semantic acceptance still belongs to the parent module's shared checks.

use super::*;

const ROWS: u8 = 1;
const FRAME: u8 = 2;
const HISTORY: u8 = 3;
const FG: u16 = 1 << 12;
const BG: u16 = 1 << 13;
const UNDERLINE: u16 = 1 << 14;
const LINK: u16 = 1 << 15;

/// Bytes a run contributes to its row body, for an already validated run.
/// The link index has a fixed width, so its numeric value cannot change
/// packing decisions when a row joins another message.
pub(super) fn run_len(text: &str, style: Style, linked: bool) -> usize {
    8 + text.len()
        + 4 * usize::from(style.fg != Color::Default)
        + 4 * usize::from(style.bg != Color::Default)
        + 4 * usize::from(style.underline_color.is_some())
        + 2 * usize::from(linked)
}

/// Whether this message uses the binary half of the versioned grid format.
pub(super) fn carries_rows(message: &GridOutput) -> bool {
    matches!(
        message,
        GridOutput::Rows { .. } | GridOutput::Frame { .. } | GridOutput::History { .. }
    )
}

/// Encode a validated message, or return `None` for a JSON control message.
/// The exact size is checked before allocating the output buffer; callers
/// must validate first so all narrowing integer conversions are bounded.
pub(super) fn encode(message: &GridOutput) -> Result<Option<Vec<u8>>, GridError> {
    let len = match message {
        GridOutput::Rows { rows, links } => {
            8 + links.iter().map(Link::binary_len).sum::<usize>()
                + rows
                    .iter()
                    .map(|row| 6 + row.row.binary_len())
                    .sum::<usize>()
        }
        GridOutput::History { rows, links, .. } => {
            18 + links.iter().map(Link::binary_len).sum::<usize>()
                + rows.iter().map(|row| 4 + row.binary_len()).sum::<usize>()
        }
        GridOutput::Frame { .. } => 21,
        _ => return Ok(None),
    };
    check_len(len, GRID_MAX_MESSAGE)?;
    let mut out = Vec::with_capacity(len);
    match message {
        GridOutput::Rows { rows, links } => {
            out.extend_from_slice(&[0, ROWS]);
            write_links(&mut out, links);
            out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
            for row in rows {
                out.extend_from_slice(&row.y.to_le_bytes());
                write_row(&mut out, &row.row);
            }
        }
        GridOutput::History {
            first,
            cols,
            rows,
            links,
        } => {
            out.extend_from_slice(&[0, HISTORY]);
            out.extend_from_slice(&first.to_le_bytes());
            out.extend_from_slice(&cols.to_le_bytes());
            write_links(&mut out, links);
            out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
            for row in rows {
                write_row(&mut out, row);
            }
        }
        GridOutput::Frame {
            rev,
            cols,
            rows,
            shift,
            cursor,
        } => {
            out.extend_from_slice(&[0, FRAME]);
            out.extend_from_slice(&rev.to_le_bytes());
            for value in [*cols, *rows, *shift, cursor.x, cursor.y] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            let shape = match cursor.shape {
                CursorShape::Block => 0,
                CursorShape::Underline => 1,
                CursorShape::Bar => 2,
            };
            out.push(u8::from(cursor.visible) | (u8::from(cursor.blink) << 1) | (shape << 2));
        }
        _ => unreachable!("only binary message kinds have an encoded length"),
    }
    debug_assert_eq!(out.len(), len);
    Ok(Some(out))
}

/// Write one message-local link table. Lengths were checked by the shared
/// validator, including byte rather than character limits for UTF-8.
fn write_links(out: &mut Vec<u8>, links: &[Link]) {
    out.extend_from_slice(&(links.len() as u16).to_le_bytes());
    for link in links {
        for text in [&link.id, &link.uri] {
            out.extend_from_slice(&(text.len() as u16).to_le_bytes());
            out.extend_from_slice(text.as_bytes());
        }
    }
}

/// Prefix a complete row body for the browser's later lazy indexing.
/// The optional underline preserves absent versus explicitly default,
/// while foreground and background omit their default representation.
fn write_row(out: &mut Vec<u8>, row: &Row) {
    out.extend_from_slice(&(row.binary_len() as u32).to_le_bytes());
    out.push(u8::from(row.wraps));
    out.extend_from_slice(&(row.runs.len() as u16).to_le_bytes());
    for run in &row.runs {
        let mut metadata = run.style.flags;
        for (present, flag) in [
            (run.style.fg != Color::Default, FG),
            (run.style.bg != Color::Default, BG),
            (run.style.underline_color.is_some(), UNDERLINE),
            (run.link.is_some(), LINK),
        ] {
            if present {
                metadata |= flag;
            }
        }
        for value in [run.col, run.width, metadata, run.text.len() as u16] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for color in [
            (run.style.fg != Color::Default).then_some(run.style.fg),
            (run.style.bg != Color::Default).then_some(run.style.bg),
            run.style.underline_color,
        ]
        .into_iter()
        .flatten()
        {
            out.extend_from_slice(&(color.to_wire() as i32).to_le_bytes());
        }
        if let Some(link) = run.link {
            out.extend_from_slice(&link.to_le_bytes());
        }
        out.extend_from_slice(run.text.as_bytes());
    }
}

/// A reader can only consume bytes from its own bounded slice. Row bodies
/// get independent readers so a forged inner length cannot borrow fields
/// from the next row and still appear well formed.
struct Reader<'a> {
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    /// Take a checked field without arithmetic that can wrap at the end of
    /// an attacker-controlled length. No allocation precedes this check.
    fn take(&mut self, len: usize) -> Result<&'a [u8], GridError> {
        if len > self.rest.len() {
            return Err(bound!(
                "truncated binary field: need {len} bytes, have {}",
                self.rest.len()
            ));
        }
        let (bytes, rest) = self.rest.split_at(len);
        self.rest = rest;
        Ok(bytes)
    }

    /// Read a fixed integer only after its complete byte array exists.
    fn array<const N: usize>(&mut self) -> Result<[u8; N], GridError> {
        Ok(self
            .take(N)?
            .try_into()
            .expect("take returns the requested length"))
    }

    /// Flags are byte-sized; their allowed bit sets are kind-specific and
    /// checked by the caller after this structural read.
    fn u8(&mut self) -> Result<u8, GridError> {
        Ok(self.take(1)?[0])
    }
    /// Read one small dimension, index or string length without borrowing
    /// bytes from outside the current message/row boundary.
    fn u16(&mut self) -> Result<u16, GridError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    /// Counts and row lengths need more than sixteen bits. This reads the
    /// integer only; users must check it before allocating or slicing.
    fn u32(&mut self) -> Result<u32, GridError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    /// All counters must survive JavaScript's integer representation
    /// exactly, including an empty history append's first line.
    fn counter(&mut self) -> Result<u64, GridError> {
        let value = u64::from_le_bytes(self.array()?);
        if value > GRID_MAX_LINE_NUMBER {
            return Err(bound!(
                "binary counter {value} is over {GRID_MAX_LINE_NUMBER}"
            ));
        }
        Ok(value)
    }

    /// Decode UTF-8 fatally after enforcing the field's own byte cap.
    /// Lossy decoding would silently change both text and link identity.
    fn text(&mut self, max: usize) -> Result<String, GridError> {
        let len = usize::from(self.u16()?);
        if len > max {
            return Err(bound!("binary string of {len} bytes, over {max}"));
        }
        let text = std::str::from_utf8(self.take(len)?)
            .map_err(|_| bound!("invalid UTF-8 in binary string"))?;
        Ok(text.to_owned())
    }

    /// Default foreground/background must be absent rather than encoded
    /// explicitly; underline intentionally permits the explicit default.
    fn color(&mut self, default_allowed: bool) -> Result<Color, GridError> {
        let value = i32::from_le_bytes(self.array()?);
        let color = Color::from_wire(i64::from(value))
            .ok_or_else(|| bound!("{value} is not a grid color"))?;
        if !default_allowed && color == Color::Default {
            return Err(bound!(
                "noncanonical explicit default foreground/background"
            ));
        }
        Ok(color)
    }

    /// Check the cheapest possible representation before reserving any
    /// peer-supplied count. Semantic limits are checked separately.
    fn count(&mut self, minimum: usize) -> Result<usize, GridError> {
        let count = self.u32()? as usize;
        if count > self.rest.len() / minimum {
            return Err(bound!("binary count {count} exceeds remaining bytes"));
        }
        Ok(count)
    }

    /// Both outer messages and row bodies must end exactly. Otherwise an
    /// unrecognized extension could be accepted under the same protocol.
    fn finish(self) -> Result<(), GridError> {
        if !self.rest.is_empty() {
            return Err(bound!("{} trailing binary bytes", self.rest.len()));
        }
        Ok(())
    }

    /// Validate the table count against its format cap and remaining
    /// prefix bytes before reserving entries; strings check their caps.
    fn links(&mut self) -> Result<Vec<Link>, GridError> {
        let count = usize::from(self.u16()?);
        if count > GRID_MAX_LINKS || count > self.rest.len() / 4 {
            return Err(bound!("binary link count {count} exceeds bounds"));
        }
        let mut links = Vec::with_capacity(count);
        for _ in 0..count {
            links.push(Link {
                id: self.text(GRID_MAX_LINK_ID_BYTES)?,
                uri: self.text(GRID_MAX_LINK_URI_BYTES)?,
            });
        }
        Ok(links)
    }

    /// Read and validate one whole row before handing it to a message.
    /// Run count is bounded by width and by its minimum byte footprint;
    /// string allocation is additionally bounded by each run's cells.
    fn row(&mut self, cols: u16, links: usize) -> Result<Row, GridError> {
        let len = self.u32()? as usize;
        let mut body = Reader {
            rest: self.take(len)?,
        };
        let wraps = match body.u8()? {
            0 => false,
            1 => true,
            _ => return Err(bound!("binary wraps is not 0 or 1")),
        };
        let count = usize::from(body.u16()?);
        if count > usize::from(cols) || count > body.rest.len() / 8 {
            return Err(bound!("binary run count {count} exceeds bounds"));
        }
        let mut runs = Vec::with_capacity(count);
        for _ in 0..count {
            let col = body.u16()?;
            let width = body.u16()?;
            let metadata = body.u16()?;
            let text_len = usize::from(body.u16()?);
            if width == 0 || u32::from(col) + u32::from(width) > u32::from(cols) {
                return Err(bound!("binary run outside row width {cols}"));
            }
            if text_len > GRID_MAX_CELL_TEXT_BYTES * usize::from(width) {
                return Err(bound!("binary run text exceeds cell byte budget"));
            }
            let style = Style {
                flags: metadata & 0x0fff,
                fg: if metadata & FG != 0 {
                    body.color(false)?
                } else {
                    Color::Default
                },
                bg: if metadata & BG != 0 {
                    body.color(false)?
                } else {
                    Color::Default
                },
                underline_color: if metadata & UNDERLINE != 0 {
                    Some(body.color(true)?)
                } else {
                    None
                },
            };
            let link = if metadata & LINK != 0 {
                Some(body.u16()?)
            } else {
                None
            };
            let text = std::str::from_utf8(body.take(text_len)?)
                .map_err(|_| bound!("invalid UTF-8 in binary run text"))?
                .to_owned();
            runs.push(Run {
                col,
                width,
                text,
                style,
                link,
            });
        }
        body.finish()?;
        let row = Row { wraps, runs };
        row.validate(cols, links)?;
        Ok(row)
    }
}

/// Decode one binary payload. The caller already checked the outer size;
/// shared validation follows this structural and allocation-bound pass.
pub(super) fn decode(bytes: &[u8]) -> Result<GridOutput, GridError> {
    let mut input = Reader { rest: bytes };
    if input.u8()? != 0 {
        return Err(bound!("invalid binary discriminator"));
    }
    let message = match input.u8()? {
        ROWS => {
            let links = input.links()?;
            let count = input.count(9)?;
            let mut rows = Vec::with_capacity(count);
            for _ in 0..count {
                let y = input.u16()?;
                if y >= GRID_MAX_DIMENSION {
                    return Err(bound!("binary screen row {y} is outside largest grid"));
                }
                rows.push(ScreenRow {
                    y,
                    row: input.row(GRID_MAX_DIMENSION, links.len())?,
                });
            }
            GridOutput::Rows { rows, links }
        }
        HISTORY => {
            let first = input.counter()?;
            let cols = input.u16()?;
            if !(1..=GRID_MAX_DIMENSION).contains(&cols) {
                return Err(bound!("binary history cols {cols} is outside grid bounds"));
            }
            let links = input.links()?;
            let count = input.count(7)?;
            if first + count as u64 > GRID_MAX_LINE_NUMBER + 1 {
                return Err(bound!("binary history lines run past last safe line"));
            }
            let mut rows = Vec::with_capacity(count);
            for _ in 0..count {
                rows.push(input.row(cols, links.len())?);
            }
            GridOutput::History {
                first,
                cols,
                rows,
                links,
            }
        }
        FRAME => {
            let rev = input.counter()?;
            let cols = input.u16()?;
            let rows = input.u16()?;
            let shift = input.u16()?;
            let x = input.u16()?;
            let y = input.u16()?;
            let flags = input.u8()?;
            if flags & 0xf0 != 0 {
                return Err(bound!("reserved binary cursor flags"));
            }
            let shape = match (flags >> 2) & 3 {
                0 => CursorShape::Block,
                1 => CursorShape::Underline,
                2 => CursorShape::Bar,
                _ => return Err(bound!("invalid binary cursor shape")),
            };
            GridOutput::Frame {
                rev,
                cols,
                rows,
                shift,
                cursor: Cursor {
                    x,
                    y,
                    visible: flags & 1 != 0,
                    blink: flags & 2 != 0,
                    shape,
                },
            }
        }
        kind => return Err(bound!("unknown binary grid kind {kind}")),
    };
    input.finish()?;
    Ok(message)
}
