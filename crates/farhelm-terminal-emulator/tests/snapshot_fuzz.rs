//! Restoring hostile blobs never panics: mutated golden blobs and blobs
//! built with random fixups either restore or are refused, and what restores
//! keeps working through more output, resizes and further snapshots.
//!
//! A blob comes from the terminal host, which keeps whatever a supervisor
//! gave it, so the decoder must hold up against blobs no encoder wrote (a bug
//! in an old build, memory corruption, a forged blob): a panic in a restore
//! takes the shared supervisor and every terminal in it down. The review that
//! found the decoder's unbounded allocation ran 230,000 such blobs; the
//! ignored tests here repeat that, and the workspace run a sample. Sizes are
//! kept small here (the size bound itself is tested in `snapshot.rs`), so
//! that a mutated size field cannot make a test allocate hundreds of MiB.

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::synthetic::Rng;
use common::t0;
use farhelm_terminal_emulator::{Collected, Emulator, FeedMode, Size};

const MAX: usize = 16 * 1024 * 1024;

/// Output that reaches the grid's awkward states.
const OPS: &[&str] = &[
    "\x1b[5@",
    "\x1b[3P",
    "\x1b[2X",
    "\x1b[K",
    "\x1b[1K",
    "\x1b[2K",
    "\x1b[J",
    "\x1b[1J",
    "\x1b[2J",
    "\x1b[L",
    "\x1b[M",
    "\x1b[S",
    "\x1b[T",
    "中",
    "😀",
    "ab",
    "e\u{301}",
    "\x1b[3b",
    "\t",
    "\x1b[Z",
    "\r\n",
    "\x1bM",
    "\x1bD",
    "\x1b7",
    "\x1b8",
    "\x1b[?1049h",
    "\x1b[?1049l",
    "\x1b[?6h",
    "\x1b[?6l",
    "\x1b[2;3r",
    "\x1b[r",
    "\x1b[4h",
    "\x1b[4l",
    "\x1b[?7l",
    "\x1b[?7h",
    "\x1b[1;80H",
    "\x1b[99;1H",
    "\x1b[A",
    "\x1b[B",
    "\x1b[C",
    "\x1b[D",
    "\x1b[3J",
    "\x1bc",
    "\x1b#8",
    "x",
    "\x1b]8;id=a;http://a/\x1b\\L\x1b]8;;\x1b\\",
    "\x1b[58;5;3mU\x1b[0m",
];

/// Drive a restored emulator: output, resizes, snapshots and restores, and
/// the reads a supervisor does.
fn exercise(emulator: &mut Emulator, r: &mut Rng) {
    for _ in 0..40 {
        match r.below(10) {
            0 => {
                let size = Size {
                    columns: r.range(1, 14) as u16,
                    lines: r.range(1, 7) as u16,
                };
                emulator.resize(size, FeedMode::Live, &mut Collected::default());
            }
            1 => {
                if let Ok(snapshot) = emulator.snapshot(MAX)
                    && let Ok(restored) = Emulator::restore(&snapshot.blob, 1000)
                {
                    *emulator = restored.emulator;
                }
            }
            2 => {
                let _ = emulator.snapshot(300);
                let _ = emulator.history_seed(300);
            }
            _ => {
                let op = *r.pick(OPS);
                emulator.feed(
                    op.as_bytes(),
                    t0(),
                    FeedMode::Live,
                    &mut Collected::default(),
                );
            }
        }
        let _ = emulator.screen_text();
    }
}

/// The golden blobs, as bytes.
fn goldens() -> Vec<Vec<u8>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/snapshots/v1");
    let mut blobs = Vec::new();
    for entry in std::fs::read_dir(dir).expect("the fixture directory") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|extension| extension == "hex") {
            let text = std::fs::read_to_string(&path).expect("a hex fixture");
            blobs.push(
                text.split_whitespace()
                    .map(|pair| u8::from_str_radix(pair, 16).expect("hex"))
                    .collect(),
            );
        }
    }
    assert!(!blobs.is_empty());
    blobs
}

/// Whether a blob's size fields stay small (bytes 7 to 10: columns, lines).
fn small(blob: &[u8]) -> bool {
    blob.len() < 11 || {
        let columns = usize::from(u16::from_be_bytes([blob[7], blob[8]]));
        let lines = usize::from(u16::from_be_bytes([blob[9], blob[10]]));
        columns * lines <= 4000
    }
}

/// Restore `blob` and exercise what restores, returning a panic's message.
fn survives(blob: &[u8], seed: u64) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(|| {
        if let Ok(restored) = Emulator::restore(blob, 1000) {
            let mut emulator = restored.emulator;
            exercise(&mut emulator, &mut Rng::new(seed));
        }
    }))
    .map_err(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default()
    })
}

/// `count` golden blobs with one to three bytes changed after the magic and
/// version, restored and exercised.
fn mutated(count: u64) {
    let sources = goldens();
    let mut r = Rng::new(424_242);
    for round in 0..count {
        let mut blob = sources[round as usize % sources.len()].clone();
        for _ in 0..r.range(1, 3) {
            let at = r.range(6, blob.len() as u64 - 1) as usize;
            match r.below(3) {
                0 => blob[at] ^= 1 << r.below(8),
                1 => blob[at] = r.below(256) as u8,
                _ => blob[at] = *r.pick(&[0u8, 1, 2, 0xff, 0x7f, 0x80]),
            }
        }
        if !small(&blob) {
            continue;
        }
        let seed = r.next();
        if let Err(message) = survives(&blob, seed) {
            panic!("round {round}: {message}\n  blob {}", blob.escape_ascii());
        }
    }
}

/// `count` blobs with random cell and cursor fixups (every flag, position,
/// color and extra the format allows), restored and exercised.
fn crafted(count: u64) {
    let mut r = Rng::new(99);
    for round in 0..count {
        let columns = r.range(1, 8) as u16;
        let lines = r.range(1, 5) as u16;
        let stream: &[u8] = match r.below(3) {
            0 => b"",
            1 => b"hello \xe4\xb8\xad",
            _ => b"\x1b[?1049hfull",
        };
        let mut blob = b"FHTS\x00\x01\x00".to_vec();
        blob.extend_from_slice(&columns.to_be_bytes());
        blob.extend_from_slice(&lines.to_be_bytes());
        blob.extend_from_slice(&[0; 8]);
        blob.extend_from_slice(&[16, 0, 0]);
        blob.extend_from_slice(&0u64.to_be_bytes());
        blob.extend_from_slice(&(stream.len() as u32).to_be_bytes());
        blob.extend_from_slice(stream);
        let cell = |r: &mut Rng, blob: &mut Vec<u8>| {
            let c = *r.pick(&['a', '中', '\t', ' ', '\u{301}', '😀']);
            blob.extend_from_slice(&(c as u32).to_be_bytes());
            for _ in 0..2 {
                match r.below(3) {
                    0 => {
                        blob.push(0);
                        let slot = *r.pick(&[1u16, 256, 257, 258, 260, 267]);
                        blob.extend_from_slice(&slot.to_be_bytes());
                    }
                    1 => blob.extend_from_slice(&[1, r.below(256) as u8]),
                    _ => blob.extend_from_slice(&[2, 1, 2, 3]),
                }
            }
            let flags = (r.next() as u16) & 0x7fff;
            let flags = if r.chance(0.5) {
                flags & (1 << r.below(15))
            } else {
                flags
            };
            blob.extend_from_slice(&flags.to_be_bytes());
            let zerowidth = r.below(3) as u8;
            blob.push(zerowidth);
            for _ in 0..zerowidth {
                blob.extend_from_slice(&('\u{301}' as u32).to_be_bytes());
            }
            if r.chance(0.3) {
                blob.extend_from_slice(&[1, 1, 7]);
            } else {
                blob.push(0);
            }
            if r.chance(0.3) {
                blob.push(1);
                blob.extend_from_slice(&1u32.to_be_bytes());
                blob.push(b'i');
                blob.extend_from_slice(&4u32.to_be_bytes());
                blob.extend_from_slice(b"http");
            } else {
                blob.push(0);
            }
        };
        let cells = r.below(12) as u32;
        blob.extend_from_slice(&cells.to_be_bytes());
        for _ in 0..cells {
            blob.push(r.below(2) as u8);
            blob.extend_from_slice(&(r.below(u64::from(lines)) as i32).to_be_bytes());
            blob.extend_from_slice(&(r.below(u64::from(columns)) as u32).to_be_bytes());
            cell(&mut r, &mut blob);
        }
        let cursors = r.below(4) as u32;
        blob.extend_from_slice(&cursors.to_be_bytes());
        for _ in 0..cursors {
            blob.extend_from_slice(&[r.below(2) as u8, r.below(2) as u8]);
            blob.extend_from_slice(&(r.below(u64::from(lines)) as i32).to_be_bytes());
            blob.extend_from_slice(&(r.below(u64::from(columns)) as u32).to_be_bytes());
            blob.push(r.below(2) as u8);
            cell(&mut r, &mut blob);
            for _ in 0..4 {
                blob.push(r.below(2) as u8);
            }
        }
        match r.below(3) {
            0 => blob.push(0),
            1 => blob.push(1),
            _ => {
                blob.push(2);
                let c = *r.pick(&['x', '中', '\u{301}']);
                blob.extend_from_slice(&(c as u32).to_be_bytes());
            }
        }
        let seed = r.next();
        if let Err(message) = survives(&blob, seed) {
            panic!("round {round}: {message}\n  blob {}", blob.escape_ascii());
        }
    }
}

/// A sample of mutated golden blobs restores or is refused, without a
/// panic, and what restores survives more output, resizes and snapshots.
///
/// Why: see the module docs. Spec: `Emulator::restore`, `SnapshotError`.
#[test]
fn mutated_golden_blobs_never_panic() {
    mutated(3_000);
}

/// A sample of blobs with random fixups, likewise.
///
/// Why: fixups write cells and cursors directly, past the checks the
/// stream gets from the emulator: wide-character halves, flags no output
/// combines, cursors anywhere; the emulator must cope with all of them.
#[test]
fn crafted_fixup_blobs_never_panic() {
    crafted(2_000);
}

/// The review's full run: 200,000 mutated blobs and 30,000 crafted ones.
/// Minutes; run in release when changing the decoder or the vendored patches.
#[test]
#[ignore = "minutes; the review's full run, in release"]
fn hostile_blobs_never_panic_at_review_size() {
    mutated(200_000);
    crafted(30_000);
}
