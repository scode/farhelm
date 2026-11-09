//! Generate the browser benchmark's inputs through the production grid packer.
//!
//! Synthetic cells isolate encoding and client storage from emulator throughput.
//! The private file envelope is a big-endian u32 length followed by one encoded
//! grid message; it never becomes part of the product protocol. Run in release
//! with a new output directory, then use `e2e/grid-client/benchmark.mts`.

use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, ensure};
use farhelm_proto::grid::{
    Color, Cursor, CursorShape, GridOutput, GridStreamCheck, MouseMode, Style, TerminalState,
    style_flags,
};
use farhelm_supervisor::grid_output::{CellRow, CellRun, history_messages, rows_messages};
use serde::Serialize;
use sha2::{Digest, Sha256};

const COLS: u16 = 200;
const SCREEN_ROWS: u16 = 60;
const HISTORY: usize = 12_000;
const BATCH: usize = 128;

/// The workload, not a serializer choice: every case uses the same wire path.
#[derive(Clone, Copy)]
enum Workload {
    Typical,
    Indexed,
    Truecolor,
}

impl Workload {
    fn name(self) -> &'static str {
        match self {
            Self::Typical => "typical",
            Self::Indexed => "indexed",
            Self::Truecolor => "truecolor",
        }
    }

    /// A full-width row with few runs, or one independently styled run per cell.
    /// Truecolor values are unique across screen and history, so interning a
    /// small repeated palette cannot hide the representation's worst case.
    fn row(self, line: usize) -> CellRow {
        let runs = match self {
            Self::Typical => {
                let text = "the quick brown fox jumps over the lazy dog ".repeat(5);
                vec![
                    run(
                        0,
                        format!("{line:06} "),
                        Color::Palette((line % 8) as u8),
                        0,
                    ),
                    run(7, text[..149].to_owned(), Color::Default, 0),
                    run(
                        156,
                        text[..44].to_owned(),
                        Color::Default,
                        style_flags::BOLD,
                    ),
                ]
            }
            Self::Indexed | Self::Truecolor => (0..COLS)
                .map(|col| {
                    let index = line * usize::from(COLS) + usize::from(col);
                    let color = match self {
                        Self::Indexed => Color::Palette((index % 256) as u8),
                        Self::Truecolor => {
                            let rgb = index + 1;
                            Color::Rgb {
                                r: (rgb >> 16) as u8,
                                g: (rgb >> 8) as u8,
                                b: rgb as u8,
                            }
                        }
                        Self::Typical => unreachable!(),
                    };
                    run(
                        col,
                        char::from(b'a' + (index % 26) as u8).to_string(),
                        color,
                        0,
                    )
                })
                .collect(),
        };
        CellRow { wraps: false, runs }
    }
}

/// ASCII cells have one byte per column; every generated run uses that contract.
fn run(col: u16, text: String, fg: Color, flags: u16) -> CellRun {
    CellRun {
        col,
        width: text.len() as u16,
        text,
        style: Style {
            fg,
            flags,
            ..Style::PLAIN
        },
        link: None,
    }
}

/// Reject an accidentally cheap or incomplete fixture before it reaches the page.
fn check_row(workload: Workload, row: &CellRow) -> anyhow::Result<()> {
    let mut end = 0;
    for run in &row.runs {
        ensure!(
            run.col == end && run.text.is_ascii(),
            "noncontiguous benchmark row"
        );
        ensure!(
            run.width as usize == run.text.len(),
            "text does not cover its cells"
        );
        end += run.width;
    }
    ensure!(end == COLS, "benchmark row does not cover 200 columns");
    if !matches!(workload, Workload::Typical) {
        ensure!(
            row.runs.len() == usize::from(COLS),
            "colored row must style every cell"
        );
        ensure!(
            row.runs
                .windows(2)
                .all(|pair| pair[0].style.fg != pair[1].style.fg),
            "adjacent colors repeat"
        );
    }
    Ok(())
}

/// An input's portable identity and size; payload bytes exclude benchmark framing.
#[derive(Serialize)]
struct Input {
    scenario: &'static str,
    file: String,
    sha256: String,
    frames: usize,
    payload_bytes: usize,
}

/// Validate the complete attach sequence while recording exactly what it encoded.
struct Stream {
    output: BufWriter<std::fs::File>,
    check: GridStreamCheck,
    hash: Sha256,
    frames: usize,
    payload_bytes: usize,
}

impl Stream {
    /// Emit one production payload without assuming that its representation is JSON.
    fn send(&mut self, message: GridOutput) -> anyhow::Result<()> {
        self.check.accept(&message)?;
        let bytes = message.encode()?;
        let len = u32::try_from(bytes.len())?.to_be_bytes();
        self.output.write_all(&len)?;
        self.output.write_all(&bytes)?;
        self.hash.update(len);
        self.hash.update(&bytes);
        self.frames += 1;
        self.payload_bytes += bytes.len();
        Ok(())
    }
}

/// Keep production packing, its limits and the browser's attach order in the fixture.
fn generate(directory: &Path, workload: Workload) -> anyhow::Result<Input> {
    let file = format!("{}.frames", workload.name());
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(&file))?;
    let mut stream = Stream {
        output: BufWriter::new(output),
        check: GridStreamCheck::new(),
        hash: Sha256::new(),
        frames: 0,
        payload_bytes: 0,
    };
    stream.send(GridOutput::Hello {
        cols: COLS,
        rows: SCREEN_ROWS,
        history_cap: HISTORY as u32,
    })?;
    stream.send(GridOutput::State(TerminalState {
        mouse: MouseMode::Off,
        focus: false,
        alt_screen: false,
        default_fg: None,
        default_bg: None,
        history_trimmed: false,
        exit: None,
    }))?;
    let screen: Vec<_> = (0..SCREEN_ROWS)
        .map(|y| workload.row(HISTORY + usize::from(y)))
        .collect();
    for row in &screen {
        check_row(workload, row)?;
    }
    let screen_refs: Vec<_> = screen
        .iter()
        .enumerate()
        .map(|(y, row)| (y as u16, row))
        .collect();
    for message in rows_messages(&screen_refs) {
        stream.send(message)?;
    }
    stream.send(GridOutput::Frame {
        rev: 1,
        cols: COLS,
        rows: SCREEN_ROWS,
        shift: 0,
        cursor: Cursor {
            x: 0,
            y: 0,
            visible: false,
            shape: CursorShape::Block,
            blink: false,
        },
    })?;
    let mut emitted = 0;
    for first in (0..HISTORY).step_by(BATCH) {
        let end = (first + BATCH).min(HISTORY);
        let rows: Vec<_> = (first..end).map(|line| workload.row(line)).collect();
        for row in &rows {
            check_row(workload, row)?;
        }
        for message in history_messages(first as u64, COLS, &rows) {
            stream.send(message)?;
        }
        emitted += rows.len();
    }
    ensure!(emitted == HISTORY, "history does not contain 12,000 lines");
    stream.output.flush()?;
    Ok(Input {
        scenario: workload.name(),
        file,
        sha256: stream
            .hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        frames: stream.frames,
        payload_bytes: stream.payload_bytes,
    })
}

/// Refuse overwrites so a failed run cannot silently replace comparison inputs.
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let directory = args
        .next()
        .context("usage: grid_stream_bench <new-output-directory>")?;
    ensure!(
        args.next().is_none(),
        "usage: grid_stream_bench <new-output-directory>"
    );
    let directory = Path::new(&directory);
    fs::create_dir(directory).context("output directory must be new")?;
    let inputs = [Workload::Typical, Workload::Indexed, Workload::Truecolor]
        .into_iter()
        .map(|workload| generate(directory, workload))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let manifest = serde_json::json!({
        "format": 1, "protocol": farhelm_proto::PROTOCOL_VERSION,
        "cols": COLS, "screen_rows": SCREEN_ROWS, "history_lines": HISTORY, "inputs": inputs,
    });
    fs::write(
        directory.join("inputs.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("{}", serde_json::to_string(&manifest)?);
    Ok(())
}
