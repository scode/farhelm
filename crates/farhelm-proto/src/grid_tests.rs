//! Tests of the grid stream format: the golden fixtures the browser's grid
//! client also reads, every bound at and just past its limit, the size
//! limits, colors, and the cross-message rules of [`GridStreamCheck`].

use super::*;
use std::path::{Path, PathBuf};

// ---- Golden fixtures --------------------------------------------------------

/// Every fixture under `grid-fixtures/`, by path relative to it.
///
/// The fixtures are the contract the browser's grid client is tested
/// against, so a file the tests do not know about would be one the Rust side
/// never checked, and a listed file that is missing would be a case that
/// silently stopped being tested. The directory must hold exactly these.
const FIXTURES: &[&str] = &[
    "input/invalid/code-too-long.json",
    "input/invalid/focus-missing-in.json",
    "input/invalid/key-mac-not-bool.json",
    "input/invalid/key-too-long.json",
    "input/invalid/key-undefined-modifier.json",
    "input/invalid/mouse-down-no-button.json",
    "input/invalid/mouse-unknown-kind.json",
    "input/invalid/mouse-wheel-button-2.json",
    "input/invalid/mouse-x-past-largest.json",
    "input/invalid/paste-missing-last.json",
    "input/invalid/paste-too-long.json",
    "input/invalid/text-too-long.json",
    "input/invalid/unknown-type.json",
    "input/tolerated/key-mac-false.json",
    "input/tolerated/key-unknown-field.json",
    "input/valid/focus-in.json",
    "input/valid/focus-out.json",
    "input/valid/key-all-modifiers.json",
    "input/valid/key-longest.json",
    "input/valid/key-mac.json",
    "input/valid/key.json",
    "input/valid/mouse-down.json",
    "input/valid/mouse-move-no-button.json",
    "input/valid/mouse-up-right.json",
    "input/valid/mouse-wheel-down.json",
    "input/valid/paste-last-empty.json",
    "input/valid/paste.json",
    "input/valid/text.json",
    "output/invalid/clipboard-missing-last.json",
    "output/invalid/clipboard-unknown-target.json",
    "output/invalid/hello-history-cap-past-largest.json",
    "output/invalid/hello-missing-history-cap.json",
    "output/invalid/hello-rows-past-largest.json",
    "output/invalid/hello-zero-cols.json",
    "output/invalid/missing-type.json",
    "output/invalid/not-an-object.json",
    "output/invalid/state-default-color-as-default-bg.json",
    "output/invalid/state-missing-default-fg.json",
    "output/invalid/state-missing-exit.json",
    "output/invalid/state-palette-default-fg.json",
    "output/invalid/state-unknown-exit.json",
    "output/invalid/state-unknown-mouse-mode.json",
    "output/invalid/unknown-type.json",
    "output/tolerated/hello-unknown-field.json",
    "output/tolerated/state-unknown-field.json",
    "output/valid/clipboard-first-part.json",
    "output/valid/clipboard.json",
    "output/valid/hello-limits.json",
    "output/valid/hello.json",
    "output/valid/history-reset.json",
    "output/valid/state-click.json",
    "output/valid/state-drag-alt-screen.json",
    "output/valid/state-exited.json",
    "output/valid/state-running.json",
    "output/valid/state-signaled.json",
];

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("grid-fixtures")
}

/// Every file under `dir`, as `/`-separated paths relative to `base`.
fn files_under(base: &Path, dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files_under(base, &path, out);
        } else {
            let relative = path.strip_prefix(base).unwrap();
            let parts: Vec<_> = relative
                .components()
                .map(|c| c.as_os_str().to_str().unwrap().to_string())
                .collect();
            out.push(parts.join("/"));
        }
    }
}

/// The listed fixtures in one category (`"output/valid/"`, say), with their
/// bytes.
fn fixtures_in(prefix: &str) -> Vec<(&'static str, Vec<u8>)> {
    let found: Vec<_> = FIXTURES
        .iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| {
            let bytes = std::fs::read(fixture_dir().join(name))
                .unwrap_or_else(|e| panic!("read fixture {name}: {e}"));
            (*name, bytes)
        })
        .collect();
    assert!(!found.is_empty(), "no fixtures under {prefix}");
    found
}

/// The fixture directory holds exactly the listed fixtures.
///
/// Why: the round-trip and rejection tests check only the fixtures they are
/// given, and the browser's tests read the directory as it is. A file added
/// without a listing entry would be tested on one side only, and a listed
/// file deleted would quietly drop a case. Spec: the files under
/// `grid-fixtures/` are exactly [`FIXTURES`] plus the paired binary
/// descriptions/bytes returned by [`binary_fixture_cases`].
#[farhelm_testtrace::test]
fn the_fixture_directory_holds_exactly_the_listed_fixtures() {
    let base = fixture_dir();
    let mut found = Vec::new();
    files_under(&base, &base, &mut found);
    found.sort();
    let mut listed: Vec<String> = FIXTURES.iter().map(|s| s.to_string()).collect();
    listed.sort();
    for case in binary_fixture_cases() {
        let prefix = if case.valid { "valid" } else { "invalid" };
        for extension in ["bin", "json"] {
            listed.push(format!(
                "output/binary/{prefix}/{}.{}",
                case.name, extension
            ));
        }
    }
    listed.sort();
    assert_eq!(found, listed);
}

/// Every valid JSON control/input fixture decodes and encodes back to the
/// same JSON. Binary descriptions and exact bytes are checked separately.
///
/// Why: the fixtures are the format as the browser sees it. Decoding proves
/// the Rust types accept what the format document describes; encoding back
/// to the same value proves the supervisor's sender produces that same shape
/// (no extra fields, no renamed keys, optional parts left out exactly when
/// the fixture leaves them out). Compared as JSON values, since the files
/// are formatted for reading.
#[farhelm_testtrace::test]
fn valid_fixtures_decode_and_encode_back_to_the_same_json() {
    let same_json = |name: &str, fixture: &[u8], encoded: &[u8]| {
        let expected: serde_json::Value = serde_json::from_slice(fixture).unwrap();
        let actual: serde_json::Value = serde_json::from_slice(encoded).unwrap();
        assert_eq!(actual, expected, "{name} does not encode back to itself");
    };
    for (name, bytes) in fixtures_in("output/valid/") {
        let encoded = GridOutput::decode(&bytes)
            .and_then(|message| message.encode())
            .unwrap_or_else(|e| panic!("valid fixture {name} was refused: {e}"));
        same_json(name, &bytes, &encoded);
    }
    for (name, bytes) in fixtures_in("input/valid/") {
        let encoded = GridInput::decode(&bytes)
            .and_then(|message| message.encode())
            .unwrap_or_else(|e| panic!("valid fixture {name} was refused: {e}"));
        same_json(name, &bytes, &encoded);
    }
}

/// Every tolerated fixture decodes.
///
/// Why: unknown fields are how a field gets added within one protocol
/// version, and explicitly empty optional fields are a legitimate way for a
/// sender to say "none". Refusing either would turn an additive change into
/// a disconnect. Spec: the fixtures under `*/tolerated/` decode without
/// error; what they encode back to is not compared.
#[farhelm_testtrace::test]
fn tolerated_fixtures_decode() {
    for (name, bytes) in fixtures_in("output/tolerated/") {
        GridOutput::decode(&bytes)
            .unwrap_or_else(|e| panic!("tolerated fixture {name} was refused: {e}"));
    }
    for (name, bytes) in fixtures_in("input/tolerated/") {
        GridInput::decode(&bytes)
            .unwrap_or_else(|e| panic!("tolerated fixture {name} was refused: {e}"));
    }
}

/// Why each invalid fixture is refused: a piece of the refusal's text, which
/// names the one rule the fixture breaks.
///
/// Without it, a fixture broken by accident (a typo that makes it non-JSON,
/// a field its rule does not need) would still pass as "refused", and the
/// rule it was written for would go untested on both sides. The text is the
/// Rust side's own wording; the browser's tests decide their own.
///
/// The input size fixtures' reasons name the limit as well as the size
/// ("over 16384"): the browser copies those limits by hand and is checked
/// against these fixtures, which are one byte past them. Pinning the limit
/// makes a changed constant fail here until the fixture is made again at
/// the new limit, rather than leave a fixture that still passes on both
/// sides while the client keeps sending messages the supervisor refuses.
const INVALID_REASONS: &[(&str, &str)] = &[
    (
        "input/invalid/code-too-long.json",
        "code of 65 bytes, over 64",
    ),
    ("input/invalid/focus-missing-in.json", "missing field `in`"),
    ("input/invalid/key-mac-not-bool.json", "expected a boolean"),
    (
        "input/invalid/key-too-long.json",
        "key of 65 bytes, over 64",
    ),
    (
        "input/invalid/key-undefined-modifier.json",
        "modifier bits 0x10",
    ),
    (
        "input/invalid/mouse-down-no-button.json",
        "mouse Down with button -1",
    ),
    (
        "input/invalid/mouse-unknown-kind.json",
        "unknown variant `drag`",
    ),
    (
        "input/invalid/mouse-wheel-button-2.json",
        "mouse Wheel with button 2",
    ),
    (
        "input/invalid/mouse-x-past-largest.json",
        "mouse at (1000, 0)",
    ),
    (
        "input/invalid/paste-missing-last.json",
        "missing field `last`",
    ),
    (
        "input/invalid/paste-too-long.json",
        "paste part of 32769 bytes, over 32768",
    ),
    (
        "input/invalid/text-too-long.json",
        "text input of 16385 bytes, over 16384",
    ),
    (
        "input/invalid/unknown-type.json",
        "unknown variant `resize`",
    ),
    (
        "output/invalid/clipboard-missing-last.json",
        "missing field `last`",
    ),
    (
        "output/invalid/clipboard-unknown-target.json",
        "unknown variant `secondary`",
    ),
    (
        "output/invalid/hello-history-cap-past-largest.json",
        "history_cap 100001",
    ),
    (
        "output/invalid/hello-missing-history-cap.json",
        "missing field `history_cap`",
    ),
    (
        "output/invalid/hello-rows-past-largest.json",
        "hello rows is 1001",
    ),
    ("output/invalid/hello-zero-cols.json", "hello cols is 0"),
    ("output/invalid/missing-type.json", "missing field `t`"),
    ("output/invalid/not-an-object.json", "not a JSON object"),
    (
        "output/invalid/state-default-color-as-default-bg.json",
        "default_bg is -1, not a direct color",
    ),
    (
        "output/invalid/state-missing-default-fg.json",
        "missing field `default_fg`",
    ),
    (
        "output/invalid/state-missing-exit.json",
        "missing field `exit`",
    ),
    (
        "output/invalid/state-palette-default-fg.json",
        "default_fg is 4, not a direct color",
    ),
    (
        "output/invalid/state-unknown-exit.json",
        "unknown variant `status`",
    ),
    (
        "output/invalid/state-unknown-mouse-mode.json",
        "unknown variant `any`",
    ),
    ("output/invalid/unknown-type.json", "unknown variant `bell`"),
];

/// Every invalid fixture is refused, for the reason it exists.
///
/// Why: these are the malformed and out-of-bounds messages both receivers
/// must refuse, one rule each, and a refusal for some other reason would
/// leave that rule untested. Spec: decoding each fixture under `*/invalid/`
/// in its direction fails with an error whose text contains the fixture's
/// entry in [`INVALID_REASONS`], and that table names exactly the invalid
/// fixtures.
#[farhelm_testtrace::test]
fn invalid_fixtures_are_rejected_for_their_own_reason() {
    let mut listed: Vec<&str> = FIXTURES
        .iter()
        .copied()
        .filter(|name| name.contains("/invalid/"))
        .collect();
    listed.sort();
    let mut explained: Vec<&str> = INVALID_REASONS.iter().map(|(name, _)| *name).collect();
    explained.sort();
    assert_eq!(
        explained, listed,
        "every invalid fixture needs exactly one reason"
    );

    let mut wrong = Vec::new();
    for (name, reason) in INVALID_REASONS {
        let bytes = std::fs::read(fixture_dir().join(name)).unwrap();
        let refusal = if name.starts_with("output/") {
            GridOutput::decode(&bytes).map(|m| format!("{m:?}"))
        } else {
            GridInput::decode(&bytes).map(|m| format!("{m:?}"))
        };
        match refusal {
            Ok(message) => wrong.push(format!("{name}: accepted as {message}")),
            Err(e) if !e.to_string().contains(reason) => {
                wrong.push(format!("{name}: refused as {e:#}, not for {reason:?}"));
            }
            Err(_) => {}
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A paste part cut inside a surrogate pair is refused.
///
/// Why: a client that slices a JavaScript string by length can split a pair,
/// and `JSON.stringify` then writes the lone half as an escape that is no
/// Unicode text; the format makes the client split at character boundaries
/// because the supervisor cannot take such a part. No fixture holds this
/// case, since the repository's JSON formatter refuses the file. Spec: a
/// `paste` whose `data` ends in a lone `\ud83d` escape fails to decode.
#[farhelm_testtrace::test]
fn a_paste_part_with_a_lone_surrogate_is_refused() {
    let lone = br#"{"t":"paste","data":"split \ud83d","last":false}"#;
    let whole = br#"{"t":"paste","data":"split \ud83d\ude00","last":false}"#;
    assert!(GridInput::decode(lone).is_err());
    assert_eq!(
        GridInput::decode(whole).unwrap(),
        GridInput::Paste {
            data: "split \u{1f600}".into(),
            last: false,
        }
    );
}

// ---- Binary fixtures and deliberate corruptions -----------------------------

/// Both fixture directions share one inventory, but binary descriptions are
/// semantic JSON only. `valid` chooses their checked-in category; invalid
/// descriptions explain a mutation rather than pretending to decode it.
struct BinaryFixture {
    name: String,
    bytes: Vec<u8>,
    description: serde_json::Value,
    valid: bool,
}

/// Generate the valid corpus with the production encoder, then corrupt
/// known fields only after proving the original message and byte premise.
/// A changed layout must break this construction, not silently turn a
/// width test into an unrelated truncation test.
fn binary_fixture_cases() -> Vec<BinaryFixture> {
    let mut cases = Vec::new();
    let mut add_valid = |name: String, message: GridOutput| {
        let bytes = message.encode().unwrap();
        assert_eq!(
            GridOutput::decode(&bytes).unwrap(),
            message,
            "{name}: encoder premise"
        );
        cases.push(BinaryFixture {
            name,
            bytes,
            description: serde_json::to_value(message).unwrap(),
            valid: true,
        });
    };
    for name in [
        "frame-shift",
        "frame-underline-cursor",
        "frame",
        "history-empty-at-last-line",
        "history-full-width",
        "history-last-line",
        "history-wrapped-with-trailing-blanks",
        "history",
        "rows-blank",
        "rows-empty",
        "rows",
    ] {
        let path = fixture_dir().join(format!("output/binary/valid/{name}.json"));
        let message = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        add_valid(name.into(), message);
    }
    // This LCG is a fixture recipe, not a source of runtime randomness.
    // A fixed seed keeps cross-language byte comparisons reproducible.
    let mut seed = 0x53b1_9e27u32;
    let mut next = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    for case in 0..16 {
        let cols = 64;
        let links = vec![
            Link {
                id: "example".into(),
                uri: "https://example.invalid/é".into(),
            },
            Link {
                id: String::new(),
                uri: String::new(),
            },
        ];
        let mut rows = Vec::new();
        for _ in 0..8 {
            let mut runs = Vec::new();
            let mut col = 0u16;
            while col + 5 < cols {
                let value = next();
                let (width, text) = match value % 5 {
                    0 => (2, "漢".to_string()),
                    1 => (1, "e\u{301}".to_string()),
                    2 => (1, "\0".to_string()),
                    _ => (3, "a\\\"".to_string()),
                };
                let fg = match (value >> 4) % 3 {
                    0 => Color::Default,
                    1 => Color::Palette((value >> 8) as u8),
                    _ => Color::Rgb {
                        r: value as u8,
                        g: (value >> 8) as u8,
                        b: (value >> 16) as u8,
                    },
                };
                let style = Style {
                    flags: ((value >> 12) as u16) & 0x0fff,
                    fg,
                    bg: if value & 8 != 0 {
                        Color::Palette(255)
                    } else {
                        Color::Default
                    },
                    underline_color: match value % 3 {
                        0 => None,
                        1 => Some(Color::Default),
                        _ => Some(fg),
                    },
                };
                runs.push(Run {
                    col,
                    width,
                    text,
                    style,
                    link: (value & 16 != 0).then_some((value & 1) as u16),
                });
                col += width + ((value >> 20) % 3) as u16;
            }
            rows.push(Row {
                wraps: next() & 1 != 0,
                runs,
            });
        }
        let message = if case % 2 == 0 {
            GridOutput::History {
                first: 123 + case,
                cols,
                rows,
                links,
            }
        } else {
            GridOutput::Rows {
                rows: rows
                    .into_iter()
                    .enumerate()
                    .map(|(y, row)| ScreenRow { y: y as u16, row })
                    .collect(),
                links,
            }
        };
        add_valid(format!("corpus-{case:02}"), message);
    }
    let probe = GridOutput::Rows {
        rows: vec![ScreenRow {
            y: 0,
            row: Row {
                wraps: false,
                runs: vec![
                    Run {
                        style: Style {
                            flags: 0,
                            fg: Color::Palette(3),
                            bg: Color::Rgb { r: 0, g: 0, b: 0 },
                            underline_color: Some(Color::Default),
                        },
                        link: Some(0),
                        ..run(0, 1, "a")
                    },
                    run(2, 1, "b"),
                ],
            },
        }],
        links: vec![Link {
            id: "i".into(),
            uri: "u".into(),
        }],
    };
    add_valid("optional-fields".into(), probe.clone());
    add_valid(
        "frame-bar".into(),
        GridOutput::Frame {
            rev: GRID_MAX_LINE_NUMBER,
            cols: 1,
            rows: 1,
            shift: 1,
            cursor: Cursor {
                x: 0,
                y: 0,
                visible: false,
                shape: CursorShape::Bar,
                blink: true,
            },
        },
    );
    // A field-leading BOM is actual terminal/link data. Browser decoders
    // must preserve it rather than apply TextDecoder's default BOM removal.
    add_valid(
        "field-leading-bom".into(),
        GridOutput::History {
            first: 0,
            cols: 1,
            rows: vec![Row {
                wraps: false,
                runs: vec![Run {
                    link: Some(0),
                    ..run(0, 1, "\u{feff}a")
                }],
            }],
            links: vec![Link {
                id: "\u{feff}id".into(),
                uri: "\u{feff}https://example.invalid".into(),
            }],
        },
    );
    let probe = probe.encode().unwrap();
    let mut add_invalid = |name: &str, bytes: Vec<u8>, reason: &str| {
        let error = GridOutput::decode(&bytes).expect_err(name);
        assert!(
            error.to_string().contains(reason),
            "{name}: {error}, expected {reason}"
        );
        cases.push(BinaryFixture {
            name: name.into(),
            bytes,
            description: serde_json::json!({"invalid": reason}),
            valid: false,
        });
    };
    // Each replacement asserts its complete original field. All base
    // buffers were decoded successfully above or immediately below.
    for (name, offset, before, after, reason) in [
        ("unknown-kind", 1, vec![1], vec![255], "unknown binary"),
        (
            "too-many-links",
            2,
            1u16.to_le_bytes().to_vec(),
            257u16.to_le_bytes().to_vec(),
            "link count",
        ),
        (
            "link-id-too-long",
            4,
            1u16.to_le_bytes().to_vec(),
            251u16.to_le_bytes().to_vec(),
            "string",
        ),
        (
            "link-uri-too-long",
            7,
            1u16.to_le_bytes().to_vec(),
            2084u16.to_le_bytes().to_vec(),
            "string",
        ),
        ("invalid-link-utf8", 6, vec![b'i'], vec![255], "UTF-8"),
        (
            "huge-row-count",
            10,
            1u32.to_le_bytes().to_vec(),
            u32::MAX.to_le_bytes().to_vec(),
            "count",
        ),
        (
            "screen-y-past-largest",
            14,
            0u16.to_le_bytes().to_vec(),
            1000u16.to_le_bytes().to_vec(),
            "screen row",
        ),
        (
            "huge-row-length",
            16,
            35u32.to_le_bytes().to_vec(),
            u32::MAX.to_le_bytes().to_vec(),
            "truncated",
        ),
        (
            "row-length-before-next-field",
            16,
            35u32.to_le_bytes().to_vec(),
            34u32.to_le_bytes().to_vec(),
            "truncated",
        ),
        ("invalid-wraps", 20, vec![0], vec![2], "wraps"),
        (
            "huge-run-count",
            21,
            2u16.to_le_bytes().to_vec(),
            1001u16.to_le_bytes().to_vec(),
            "run count",
        ),
        (
            "zero-width",
            25,
            1u16.to_le_bytes().to_vec(),
            0u16.to_le_bytes().to_vec(),
            "outside row",
        ),
        (
            "run-end-past-largest",
            23,
            0u16.to_le_bytes().to_vec(),
            1000u16.to_le_bytes().to_vec(),
            "outside row",
        ),
        (
            "text-past-cell-budget",
            29,
            1u16.to_le_bytes().to_vec(),
            69u16.to_le_bytes().to_vec(),
            "cell byte",
        ),
        (
            "color-below-default",
            31,
            3i32.to_le_bytes().to_vec(),
            (-2i32).to_le_bytes().to_vec(),
            "grid color",
        ),
        (
            "color-between-palette-and-rgb",
            31,
            3i32.to_le_bytes().to_vec(),
            256i32.to_le_bytes().to_vec(),
            "grid color",
        ),
        (
            "color-past-rgb",
            31,
            3i32.to_le_bytes().to_vec(),
            0x200_0000i32.to_le_bytes().to_vec(),
            "grid color",
        ),
        (
            "noncanonical-foreground-default",
            31,
            3i32.to_le_bytes().to_vec(),
            (-1i32).to_le_bytes().to_vec(),
            "noncanonical",
        ),
        (
            "noncanonical-background-default",
            35,
            0x100_0000i32.to_le_bytes().to_vec(),
            (-1i32).to_le_bytes().to_vec(),
            "noncanonical",
        ),
        (
            "link-out-of-range",
            43,
            0u16.to_le_bytes().to_vec(),
            1u16.to_le_bytes().to_vec(),
            "names link",
        ),
        ("invalid-text-utf8", 45, vec![b'a'], vec![255], "UTF-8"),
        (
            "overlapping-runs",
            46,
            2u16.to_le_bytes().to_vec(),
            0u16.to_le_bytes().to_vec(),
            "overlaps",
        ),
        (
            "ascii-width-mismatch",
            48,
            1u16.to_le_bytes().to_vec(),
            2u16.to_le_bytes().to_vec(),
            "ASCII run",
        ),
    ] {
        let bytes = replace_field(&probe, offset, &before, &after);
        add_invalid(name, bytes, reason);
    }
    for (name, end) in [
        ("truncated-header", 1),
        ("truncated-integer", 3),
        ("truncated-row-text", probe.len() - 1),
    ] {
        assert!(end < probe.len());
        add_invalid(name, probe[..end].to_vec(), "truncated");
    }
    let mut trailing = probe.clone();
    trailing.push(0);
    add_invalid("trailing-message-byte", trailing, "trailing");
    add_invalid(
        "invalid-discriminator",
        replace_field(&probe, 0, &[0], &[1]),
        "not a JSON object",
    );
    let mut row_trailing = replace_field(&probe, 16, &35u32.to_le_bytes(), &36u32.to_le_bytes());
    row_trailing.push(0);
    add_invalid("trailing-row-byte", row_trailing, "trailing");
    // Remove the last run's sole text byte while fixing its row length,
    // so this tests empty text rather than an inconsistent outer boundary.
    let mut empty = replace_field(&probe, 52, &1u16.to_le_bytes(), &0u16.to_le_bytes());
    assert_eq!(empty.pop(), Some(b'b'));
    empty = replace_field(&empty, 16, &35u32.to_le_bytes(), &34u32.to_le_bytes());
    add_invalid("empty-text", empty, "no text");
    let mut non_ascii = replace_field(&probe, 48, &1u16.to_le_bytes(), &3u16.to_le_bytes());
    non_ascii = replace_field(&non_ascii, 54, b"b", &[1]);
    add_invalid("non-ascii-width", non_ascii, "non-ASCII");
    let Either::Out(frame) = frame(7, 80, 24, 0, 0, 0) else {
        unreachable!()
    };
    let frame_bytes = frame.encode().unwrap();
    assert_eq!(GridOutput::decode(&frame_bytes).unwrap(), frame);
    for (name, offset, before, after, reason) in [
        (
            "unsafe-revision",
            2,
            7u64.to_le_bytes().to_vec(),
            (GRID_MAX_LINE_NUMBER + 1).to_le_bytes().to_vec(),
            "counter",
        ),
        (
            "zero-frame-cols",
            10,
            80u16.to_le_bytes().to_vec(),
            0u16.to_le_bytes().to_vec(),
            "frame cols",
        ),
        (
            "zero-frame-rows",
            12,
            24u16.to_le_bytes().to_vec(),
            0u16.to_le_bytes().to_vec(),
            "frame rows",
        ),
        (
            "frame-shift-past-rows",
            14,
            0u16.to_le_bytes().to_vec(),
            25u16.to_le_bytes().to_vec(),
            "frame shift",
        ),
        (
            "frame-cols-past-largest",
            10,
            80u16.to_le_bytes().to_vec(),
            1001u16.to_le_bytes().to_vec(),
            "frame cols",
        ),
        (
            "frame-rows-past-largest",
            12,
            24u16.to_le_bytes().to_vec(),
            1001u16.to_le_bytes().to_vec(),
            "frame rows",
        ),
        (
            "cursor-past-cols",
            16,
            0u16.to_le_bytes().to_vec(),
            80u16.to_le_bytes().to_vec(),
            "cursor at",
        ),
        (
            "cursor-past-rows",
            18,
            0u16.to_le_bytes().to_vec(),
            24u16.to_le_bytes().to_vec(),
            "cursor at",
        ),
        (
            "reserved-cursor-flags",
            20,
            vec![1],
            vec![17],
            "cursor flags",
        ),
        (
            "invalid-cursor-shape",
            20,
            vec![1],
            vec![13],
            "cursor shape",
        ),
    ] {
        add_invalid(
            name,
            replace_field(&frame_bytes, offset, &before, &after),
            reason,
        );
    }
    let Either::Out(history) = history_at(GRID_MAX_LINE_NUMBER - 1, 80, 2) else {
        unreachable!()
    };
    let history_bytes = history.encode().unwrap();
    assert_eq!(GridOutput::decode(&history_bytes).unwrap(), history);
    for (name, offset, before, after, reason) in [
        (
            "unsafe-first-line",
            2,
            (GRID_MAX_LINE_NUMBER - 1).to_le_bytes().to_vec(),
            (GRID_MAX_LINE_NUMBER + 1).to_le_bytes().to_vec(),
            "counter",
        ),
        (
            "unsafe-last-line",
            2,
            (GRID_MAX_LINE_NUMBER - 1).to_le_bytes().to_vec(),
            GRID_MAX_LINE_NUMBER.to_le_bytes().to_vec(),
            "last safe line",
        ),
        (
            "zero-history-cols",
            10,
            80u16.to_le_bytes().to_vec(),
            0u16.to_le_bytes().to_vec(),
            "history cols",
        ),
        (
            "history-cols-past-largest",
            10,
            80u16.to_le_bytes().to_vec(),
            1001u16.to_le_bytes().to_vec(),
            "history cols",
        ),
        (
            "huge-history-count",
            14,
            2u32.to_le_bytes().to_vec(),
            u32::MAX.to_le_bytes().to_vec(),
            "count",
        ),
    ] {
        add_invalid(
            name,
            replace_field(&history_bytes, offset, &before, &after),
            reason,
        );
    }
    let GridOutput::Rows { rows, links } = GridOutput::decode(&probe).unwrap() else {
        unreachable!()
    };
    let narrow = GridOutput::History {
        first: 0,
        cols: 3,
        rows: rows.into_iter().map(|row| row.row).collect(),
        links,
    };
    let narrow_bytes = narrow.encode().unwrap();
    assert_eq!(GridOutput::decode(&narrow_bytes).unwrap(), narrow);
    add_invalid(
        "history-run-past-cols",
        replace_field(&narrow_bytes, 10, &3u16.to_le_bytes(), &2u16.to_le_bytes()),
        "outside row width",
    );
    cases
}

/// Replace a whole known field, refusing stale offsets. This is shared by
/// fixture construction and direct boundary tests so malformed cases keep
/// the premise their names claim even if the encoder layout changes.
fn replace_field(bytes: &[u8], offset: usize, before: &[u8], after: &[u8]) -> Vec<u8> {
    assert_eq!(before.len(), after.len());
    assert_eq!(
        &bytes[offset..offset + before.len()],
        before,
        "mutation premise at {offset}"
    );
    let mut changed = bytes.to_vec();
    changed[offset..offset + after.len()].copy_from_slice(after);
    changed
}

/// Checked-in bytes must retain both their meaning and their exact encoder
/// representation. The malformed descriptors also pin the claimed refusal,
/// preventing a corruption recipe from drifting to a different failure.
#[farhelm_testtrace::test]
fn binary_fixtures_match_the_rust_encoder_and_refusal_premises() {
    for case in binary_fixture_cases() {
        let category = if case.valid { "valid" } else { "invalid" };
        let stem = fixture_dir().join(format!("output/binary/{category}/{}", case.name));
        let bytes = std::fs::read(stem.with_extension("bin")).unwrap();
        assert_eq!(bytes, case.bytes, "{}: binary drift", case.name);
        let description: serde_json::Value =
            serde_json::from_slice(&std::fs::read(stem.with_extension("json")).unwrap()).unwrap();
        assert_eq!(
            description, case.description,
            "{}: description drift",
            case.name
        );
        if case.valid {
            let message = GridOutput::decode(&bytes).unwrap();
            assert_eq!(serde_json::to_value(&message).unwrap(), description);
            assert_eq!(message.encode().unwrap(), bytes);
        } else {
            assert!(GridOutput::decode(&bytes).is_err(), "{}", case.name);
        }
    }
}

/// Explicit regeneration uses the real encoder and the same premise-checked
/// mutations as the reader tests. It never runs during normal validation;
/// a reviewer must inspect any intentional changes to the checked-in bytes.
#[farhelm_testtrace::test]
#[ignore = "explicit binary grid fixture regeneration"]
fn write_binary_grid_fixtures() {
    for case in binary_fixture_cases() {
        let category = if case.valid { "valid" } else { "invalid" };
        let dir = fixture_dir().join(format!("output/binary/{category}"));
        std::fs::create_dir_all(&dir).unwrap();
        let stem = dir.join(case.name);
        std::fs::write(stem.with_extension("bin"), case.bytes).unwrap();
        let mut json = serde_json::to_vec_pretty(&case.description).unwrap();
        json.push(b'\n');
        std::fs::write(stem.with_extension("json"), json).unwrap();
    }
}

/// The protocol has one encoding for each kind. Serde remains useful for
/// semantic descriptions, but accepting their JSON on the wire would hide
/// a mismatched sender after a protocol bump.
#[farhelm_testtrace::test]
fn json_rows_frame_and_history_are_refused_on_the_wire() {
    for message in [
        screen_rows(&[]),
        match frame(0, 80, 24, 0, 0, 0) {
            Either::Out(m) => m,
            _ => unreachable!(),
        },
        match history(0, 0) {
            Either::Out(m) => m,
            _ => unreachable!(),
        },
    ] {
        let json = serde_json::to_vec(&message).unwrap();
        assert_eq!(
            serde_json::from_slice::<GridOutput>(&json).unwrap(),
            message
        );
        assert!(
            GridOutput::decode(&json)
                .unwrap_err()
                .to_string()
                .contains("require binary")
        );
    }
}

/// Message bytes, not an arbitrary row-count cap, bound blank history.
/// At seven bytes per blank row this reaches the exact 1 MiB payload edge,
/// and also exercises a count larger than the maximum retained scrollback.
#[farhelm_testtrace::test]
fn binary_history_accepts_the_exact_message_limit_without_a_row_count_cap() {
    let count = (GRID_MAX_MESSAGE - 18) / 7;
    assert_eq!(18 + 7 * count, GRID_MAX_MESSAGE, "premise: exact boundary");
    let Either::Out(message) = history(0, count) else {
        unreachable!()
    };
    let mut bytes = message.encode().unwrap();
    assert_eq!(bytes.len(), GRID_MAX_MESSAGE);
    assert_eq!(GridOutput::decode(&bytes).unwrap(), message);
    bytes.push(0);
    assert!(matches!(
        GridOutput::decode(&bytes),
        Err(GridError::TooLarge { .. })
    ));
}

/// Optional-field flags must not absorb unsupported style bits. This tests
/// the typed sender boundary because every binary metadata bit has a real
/// meaning and cannot represent a reserved style bit independently.
#[farhelm_testtrace::test]
fn reserved_style_bits_are_refused_by_shared_validation_and_encoding() {
    let Either::Out(mut message) = one_row(0, vec![run(0, 1, "a")], vec![]) else {
        unreachable!()
    };
    let GridOutput::Rows { rows, .. } = &mut message else {
        unreachable!()
    };
    rows[0].row.runs[0].style.flags = 0x1000;
    assert!(message.validate().is_err());
    assert!(message.encode().is_err());
}

/// Truncating any field of the three binary kinds must fail without a panic
/// or partial message. These small independently shaped messages exercise
/// all integer widths and the row/link string boundaries.
#[farhelm_testtrace::test]
fn every_binary_prefix_is_refused_until_the_complete_message_arrives() {
    for case in binary_fixture_cases().into_iter().filter(|case| {
        case.valid && ["optional-fields", "frame", "history"].contains(&case.name.as_str())
    }) {
        for end in 0..case.bytes.len() {
            assert!(
                GridOutput::decode(&case.bytes[..end]).is_err(),
                "{} prefix {end}",
                case.name
            );
        }
    }
}

// ---- Bounds at and just past their limits -----------------------------------

/// A message of either direction, so one table can hold both.
#[derive(Debug)]
enum Either {
    Out(GridOutput),
    In(GridInput),
}

impl Either {
    /// Validate, then encode and decode, which is everything a sender and a
    /// receiver do with it.
    fn check(&self) -> Result<(), GridError> {
        match self {
            Either::Out(m) => {
                m.validate()?;
                let decoded = GridOutput::decode(&m.encode()?)?;
                assert_eq!(&decoded, m, "decode must give back the encoded message");
                m.encode().map(drop)
            }
            Either::In(m) => {
                m.validate()?;
                let decoded = GridInput::decode(&serde_json::to_vec(m).unwrap())?;
                assert_eq!(&decoded, m, "decode must give back the encoded message");
                m.encode().map(drop)
            }
        }
    }

    /// Exercise the receiver without the sender's semantic gate. These
    /// bounded invalid values still fit the binary fields, allowing the
    /// table below to distinguish sender and receiver enforcement.
    fn receiver_refuses(&self) -> bool {
        match self {
            Either::Out(m) => {
                let bytes = binary::encode(m)
                    .unwrap()
                    .unwrap_or_else(|| serde_json::to_vec(m).unwrap());
                GridOutput::decode(&bytes).is_err()
            }
            Either::In(m) => GridInput::decode(&serde_json::to_vec(m).unwrap()).is_err(),
        }
    }
}

fn hello(cols: u16, rows: u16, history_cap: u32) -> Either {
    Either::Out(GridOutput::Hello {
        cols,
        rows,
        history_cap,
    })
}

fn run(col: u16, width: u16, text: &str) -> Run {
    Run {
        col,
        width,
        text: text.to_string(),
        style: Style::PLAIN,
        link: None,
    }
}

fn one_row(y: u16, runs: Vec<Run>, links: Vec<Link>) -> Either {
    Either::Out(GridOutput::Rows {
        rows: vec![ScreenRow {
            y,
            row: Row { wraps: false, runs },
        }],
        links,
    })
}

fn links(count: usize, id_len: usize, uri_len: usize) -> Vec<Link> {
    (0..count)
        .map(|_| Link {
            id: "i".repeat(id_len),
            uri: "u".repeat(uri_len),
        })
        .collect()
}

fn frame(rev: u64, cols: u16, rows: u16, shift: u16, x: u16, y: u16) -> Either {
    Either::Out(GridOutput::Frame {
        rev,
        cols,
        rows,
        shift,
        cursor: Cursor {
            x,
            y,
            visible: true,
            shape: CursorShape::Block,
            blink: false,
        },
    })
}

fn history(first: u64, count: usize) -> Either {
    history_at(first, 80, count)
}

fn history_at(first: u64, cols: u16, count: usize) -> Either {
    Either::Out(GridOutput::History {
        first,
        cols,
        rows: vec![
            Row {
                wraps: false,
                runs: Vec::new(),
            };
            count
        ],
        links: Vec::new(),
    })
}

fn history_row(cols: u16, runs: Vec<Run>) -> Either {
    Either::Out(GridOutput::History {
        first: 0,
        cols,
        rows: vec![Row { wraps: false, runs }],
        links: Vec::new(),
    })
}

fn clipboard(text: String, last: bool) -> Either {
    Either::Out(GridOutput::Clipboard {
        target: ClipboardTarget::Clipboard,
        text,
        last,
    })
}

fn key(key: &str, code: &str, mods: u8) -> Either {
    Either::In(GridInput::Key {
        key: key.to_string(),
        code: code.to_string(),
        kc: 65,
        mods,
        mac: false,
    })
}

fn mouse(kind: MouseEventKind, button: i8, x: u16, y: u16) -> Either {
    Either::In(GridInput::Mouse {
        kind,
        button,
        x,
        y,
        mods: 0,
    })
}

/// An `e` loaded with combining marks to exactly `bytes` bytes of UTF-8:
/// U+0301 (2 bytes) as often as fits, plus one U+20D7 (3 bytes) when the
/// count is otherwise one short. Not plain ASCII, so only the byte budget
/// applies to it, not the ASCII width rule.
fn combining_text(bytes: usize) -> String {
    let mut text = String::from("e");
    if (bytes - 1) % 2 == 1 {
        text.push('\u{20d7}');
    }
    while text.len() < bytes {
        text.push('\u{301}');
    }
    assert_eq!(text.len(), bytes);
    text
}

/// Every bound of the format accepts its limit and refuses one step past it.
///
/// Why: an off-by-one in either direction is the classic bound bug, and the
/// two sides of this format are written in different languages; a bound the
/// supervisor allows and the client refuses ends attachments for no reason,
/// and the reverse lets hostile output through. Spec: each pair below is
/// (at the limit, accepted by sender and receiver; one step past, refused by
/// the receiver), one pair per bound in the module docs' table and per rule
/// with a numeric edge.
#[farhelm_testtrace::test]
fn every_bound_accepts_its_limit_and_refuses_one_past_it() {
    let max = GRID_MAX_DIMENSION;
    let cases: Vec<(&str, Either, Either)> = vec![
        ("hello cols", hello(max, 24, 0), hello(max + 1, 24, 0)),
        ("hello rows", hello(80, max, 0), hello(80, max + 1, 0)),
        ("hello cols minimum", hello(1, 24, 0), hello(0, 24, 0)),
        ("hello rows minimum", hello(80, 1, 0), hello(80, 0, 0)),
        (
            "history cap",
            hello(80, 24, GRID_MAX_HISTORY_CAP),
            hello(80, 24, GRID_MAX_HISTORY_CAP + 1),
        ),
        (
            "links per message",
            one_row(0, vec![], links(GRID_MAX_LINKS, 1, 1)),
            one_row(0, vec![], links(GRID_MAX_LINKS + 1, 1, 1)),
        ),
        (
            "link id",
            one_row(0, vec![], links(1, GRID_MAX_LINK_ID_BYTES, 1)),
            one_row(0, vec![], links(1, GRID_MAX_LINK_ID_BYTES + 1, 1)),
        ),
        (
            "link uri",
            one_row(0, vec![], links(1, 1, GRID_MAX_LINK_URI_BYTES)),
            one_row(0, vec![], links(1, 1, GRID_MAX_LINK_URI_BYTES + 1)),
        ),
        (
            "link index",
            one_row(
                0,
                vec![Run {
                    link: Some(1),
                    ..run(0, 1, "a")
                }],
                links(2, 1, 1),
            ),
            one_row(
                0,
                vec![Run {
                    link: Some(2),
                    ..run(0, 1, "a")
                }],
                links(2, 1, 1),
            ),
        ),
        (
            "run text per cell",
            one_row(
                0,
                vec![run(0, 1, &combining_text(GRID_MAX_CELL_TEXT_BYTES))],
                vec![],
            ),
            one_row(
                0,
                vec![run(0, 1, &combining_text(GRID_MAX_CELL_TEXT_BYTES + 1))],
                vec![],
            ),
        ),
        (
            "run text of a wide cell",
            one_row(
                0,
                vec![run(0, 2, &combining_text(2 * GRID_MAX_CELL_TEXT_BYTES))],
                vec![],
            ),
            one_row(
                0,
                vec![run(0, 2, &combining_text(2 * GRID_MAX_CELL_TEXT_BYTES + 1))],
                vec![],
            ),
        ),
        (
            "non-ASCII run width",
            one_row(0, vec![run(0, 2, "漢")], vec![]),
            one_row(0, vec![run(0, 3, "漢")], vec![]),
        ),
        (
            "run end",
            one_row(0, vec![run(max - 2, 2, "ab")], vec![]),
            one_row(0, vec![run(max - 1, 2, "ab")], vec![]),
        ),
        (
            "run width",
            one_row(0, vec![run(0, 1, "\u{301}")], vec![]),
            one_row(0, vec![run(0, 0, "\u{301}")], vec![]),
        ),
        (
            "runs touching versus overlapping",
            one_row(0, vec![run(0, 2, "ab"), run(2, 1, "c")], vec![]),
            one_row(0, vec![run(0, 2, "ab"), run(1, 1, "c")], vec![]),
        ),
        (
            "ASCII width",
            one_row(0, vec![run(0, 3, "abc")], vec![]),
            one_row(0, vec![run(0, 4, "abc")], vec![]),
        ),
        (
            "screen row",
            one_row(max - 1, vec![], vec![]),
            one_row(max, vec![], vec![]),
        ),
        (
            "frame cols",
            frame(1, max, 24, 0, 0, 0),
            frame(1, max + 1, 24, 0, 0, 0),
        ),
        (
            "frame rows",
            frame(1, 80, max, 0, 0, 0),
            frame(1, 80, max + 1, 0, 0, 0),
        ),
        (
            "frame shift",
            frame(1, 80, 24, 24, 0, 0),
            frame(1, 80, 24, 25, 0, 0),
        ),
        (
            "cursor x",
            frame(1, 80, 24, 0, 79, 0),
            frame(1, 80, 24, 0, 80, 0),
        ),
        (
            "cursor y",
            frame(1, 80, 24, 0, 0, 23),
            frame(1, 80, 24, 0, 0, 24),
        ),
        (
            "frame rev",
            frame(GRID_MAX_LINE_NUMBER, 80, 24, 0, 0, 0),
            frame(GRID_MAX_LINE_NUMBER + 1, 80, 24, 0, 0, 0),
        ),
        (
            "history line numbers",
            history(GRID_MAX_LINE_NUMBER - 2, 3),
            history(GRID_MAX_LINE_NUMBER - 2, 4),
        ),
        (
            "history first line of an empty append",
            history(GRID_MAX_LINE_NUMBER, 0),
            history(GRID_MAX_LINE_NUMBER + 1, 0),
        ),
        (
            "history cols",
            history_at(0, max, 1),
            history_at(0, max + 1, 1),
        ),
        (
            "history cols minimum",
            history_at(0, 1, 1),
            history_at(0, 0, 1),
        ),
        (
            "history run end",
            history_row(80, vec![run(78, 2, "ab")]),
            history_row(80, vec![run(79, 2, "ab")]),
        ),
        (
            "clipboard part",
            clipboard("a".repeat(GRID_MAX_CLIPBOARD_PART_BYTES), true),
            clipboard("a".repeat(GRID_MAX_CLIPBOARD_PART_BYTES + 1), true),
        ),
        (
            "key",
            key(&"k".repeat(GRID_MAX_KEY_BYTES), "KeyK", 0),
            key(&"k".repeat(GRID_MAX_KEY_BYTES + 1), "KeyK", 0),
        ),
        (
            "code",
            key("k", &"c".repeat(GRID_MAX_KEY_BYTES), 0),
            key("k", &"c".repeat(GRID_MAX_KEY_BYTES + 1), 0),
        ),
        (
            "key modifiers",
            key("k", "KeyK", modifiers::ALL),
            key("k", "KeyK", modifiers::ALL + 1),
        ),
        (
            "text input",
            Either::In(GridInput::Text {
                data: "a".repeat(GRID_MAX_TEXT_INPUT_BYTES),
            }),
            Either::In(GridInput::Text {
                data: "a".repeat(GRID_MAX_TEXT_INPUT_BYTES + 1),
            }),
        ),
        (
            "paste part",
            Either::In(GridInput::Paste {
                data: "a".repeat(GRID_MAX_PASTE_PART_BYTES),
                last: true,
            }),
            Either::In(GridInput::Paste {
                data: "a".repeat(GRID_MAX_PASTE_PART_BYTES + 1),
                last: true,
            }),
        ),
        (
            "mouse x",
            mouse(MouseEventKind::Down, 0, max - 1, 0),
            mouse(MouseEventKind::Down, 0, max, 0),
        ),
        (
            "mouse y",
            mouse(MouseEventKind::Up, 2, 0, max - 1),
            mouse(MouseEventKind::Up, 2, 0, max),
        ),
        (
            "button pressed",
            mouse(MouseEventKind::Down, 2, 0, 0),
            mouse(MouseEventKind::Down, 3, 0, 0),
        ),
        (
            "button for motion",
            mouse(MouseEventKind::Move, -1, 0, 0),
            mouse(MouseEventKind::Move, -2, 0, 0),
        ),
        (
            "button for a press",
            mouse(MouseEventKind::Down, 0, 0, 0),
            mouse(MouseEventKind::Down, -1, 0, 0),
        ),
        (
            "wheel direction",
            mouse(MouseEventKind::Wheel, 1, 0, 0),
            mouse(MouseEventKind::Wheel, 2, 0, 0),
        ),
    ];
    for (name, at_limit, past_limit) in cases {
        if let Err(e) = at_limit.check() {
            panic!("{name}: the message at the limit was refused: {e}\n{at_limit:?}");
        }
        assert!(
            past_limit.receiver_refuses(),
            "{name}: the receiver accepted a message past the limit"
        );
        assert!(
            past_limit.check().is_err(),
            "{name}: the sender would send a message past the limit"
        );
    }
}

/// The direct-color range ends exactly where the format says.
///
/// Why: colors are the one numeric field whose valid values have gaps, and
/// the client turns them into CSS; a value in a gap must never decode.
/// Spec: -1, 0..=255 and 0x1000000..=0x1FFFFFF decode, each to the color it
/// means; -2, 256, 0xFFFFFF and 0x2000000 do not.
#[farhelm_testtrace::test]
fn color_wire_values_and_their_gaps() {
    for (wire, color) in [
        (-1, Color::Default),
        (0, Color::Palette(0)),
        (255, Color::Palette(255)),
        (0x100_0000, Color::Rgb { r: 0, g: 0, b: 0 }),
        (
            0x112_2333,
            Color::Rgb {
                r: 0x12,
                g: 0x23,
                b: 0x33,
            },
        ),
        (
            0x1ff_ffff,
            Color::Rgb {
                r: 255,
                g: 255,
                b: 255,
            },
        ),
    ] {
        assert_eq!(Color::from_wire(wire), Some(color), "{wire:#x}");
        assert_eq!(color.to_wire(), wire);
        assert_eq!(
            serde_json::from_str::<Color>(&wire.to_string()).unwrap(),
            color
        );
    }
    for wire in [-2, 256, 0xff_ffff, 0x200_0000] {
        assert_eq!(Color::from_wire(wire), None, "{wire:#x}");
        assert!(serde_json::from_str::<Color>(&wire.to_string()).is_err());
    }
}

/// Both message size limits are inclusive, and the sender refuses what the
/// receiver would.
///
/// Why: the limits bound what a receiver parses at all, and a sender that
/// produced an oversized message would have it refused one hop later as a
/// protocol error with the whole attachment lost. Spec: a message of exactly
/// the limit decodes (padded with JSON whitespace, which changes nothing
/// else), one byte more is refused as too large, and `encode` refuses a
/// message whose encoded form is over the limit.
#[farhelm_testtrace::test]
fn message_size_limits_are_inclusive_and_enforced_on_both_sides() {
    let mut output = br#"{"t":"history_reset"}"#.to_vec();
    output.resize(GRID_MAX_MESSAGE, b' ');
    GridOutput::decode(&output).expect("a message of exactly the limit decodes");
    output.push(b' ');
    assert!(matches!(
        GridOutput::decode(&output),
        Err(GridError::TooLarge { .. })
    ));

    let mut input = br#"{"t":"focus","in":true}"#.to_vec();
    input.resize(GRID_MAX_INPUT_MESSAGE, b' ');
    GridInput::decode(&input).expect("an input message of exactly the limit decodes");
    input.push(b' ');
    assert!(matches!(
        GridInput::decode(&input),
        Err(GridError::TooLarge { .. })
    ));

    // 120 rows of 1000 one-cell runs exceed a megabyte of binary data,
    // although every row is valid on its own.
    let wide = Row {
        wraps: false,
        runs: (0..GRID_MAX_DIMENSION)
            .map(|col| run(col, 1, "x"))
            .collect(),
    };
    let oversized = GridOutput::History {
        first: 0,
        cols: GRID_MAX_DIMENSION,
        rows: vec![wide; 120],
        links: Vec::new(),
    };
    oversized
        .validate()
        .expect("premise: only the size is wrong");
    assert!(matches!(
        oversized.encode(),
        Err(GridError::TooLarge { .. })
    ));

    // A paste part within its 32 KiB of text can still be too large as JSON
    // (every newline escapes to two bytes), which is why the client splits by
    // both measures.
    let newlines = GridInput::Paste {
        data: "\n".repeat(GRID_MAX_PASTE_PART_BYTES),
        last: true,
    };
    newlines
        .validate()
        .expect("premise: the text is within its bound");
    assert!(matches!(newlines.encode(), Err(GridError::TooLarge { .. })));
}

/// An unknown `t` is refused and an unknown field is ignored, in both
/// directions.
///
/// Why: those are the format's two compatibility rules, and serde makes each
/// easy to get backwards (`deny_unknown_fields`, or an `other` catch-all
/// variant). The fixtures cover them too; this pins the rule itself next to
/// the types. Spec: `{"t":"nope"}` fails to decode either way; a known
/// message with an extra field decodes to the same value as without it; and
/// a known message written as an array, which serde alone would accept for a
/// tagged enum, is refused in both directions.
#[farhelm_testtrace::test]
fn unknown_types_are_refused_and_unknown_fields_ignored() {
    assert!(GridOutput::decode(br#"{"t":"nope"}"#).is_err());
    assert!(GridInput::decode(br#"{"t":"nope"}"#).is_err());
    assert!(GridOutput::decode(br#" ["hello",80,24,0]"#).is_err());
    assert!(GridInput::decode(br#"["focus",true]"#).is_err());
    assert_eq!(
        GridOutput::decode(br#"{"t":"history_reset","extra":{"a":1}}"#).unwrap(),
        GridOutput::HistoryReset
    );
    assert_eq!(
        GridInput::decode(br#"{"t":"focus","in":false,"extra":1}"#).unwrap(),
        GridInput::Focus { focused: false }
    );
}

/// The state message refuses a missing field, `null` ones included.
///
/// Why: serde reads a missing `Option` as `None`, which here would read a
/// sender that forgot `exit` as "the program is running"; [`TerminalState`]
/// opts out of that on purpose and this keeps it so. Spec: every field of
/// `state` is required; `null` is accepted where the type allows it.
#[farhelm_testtrace::test]
fn every_state_field_is_required() {
    let full = serde_json::json!({
        "t": "state", "mouse": "off", "focus": false, "alt_screen": false,
        "default_fg": null, "default_bg": null, "history_trimmed": false, "exit": null,
    });
    GridOutput::decode(full.to_string().as_bytes()).expect("the full state decodes");
    for field in [
        "mouse",
        "focus",
        "alt_screen",
        "default_fg",
        "default_bg",
        "history_trimmed",
        "exit",
    ] {
        let mut partial = full.clone();
        partial.as_object_mut().unwrap().remove(field);
        assert!(
            GridOutput::decode(partial.to_string().as_bytes()).is_err(),
            "a state without {field} was accepted"
        );
    }
}

// ---- The stream as a whole --------------------------------------------------

fn screen_rows(entries: &[(u16, u16)]) -> GridOutput {
    // Each entry is a row index and how many columns of text it holds.
    GridOutput::Rows {
        rows: entries
            .iter()
            .map(|&(y, width)| ScreenRow {
                y,
                row: Row {
                    wraps: false,
                    runs: if width == 0 {
                        Vec::new()
                    } else {
                        vec![run(0, width, &"x".repeat(usize::from(width)))]
                    },
                },
            })
            .collect(),
        links: Vec::new(),
    }
}

fn out(either: Either) -> GridOutput {
    match either {
        Either::Out(message) => message,
        Either::In(_) => unreachable!(),
    }
}

fn state() -> GridOutput {
    GridOutput::State(TerminalState {
        mouse: MouseMode::Off,
        focus: false,
        alt_screen: false,
        default_fg: None,
        default_bg: None,
        history_trimmed: false,
        exit: None,
    })
}

/// Run `messages` through a fresh check and return the first refusal's
/// position, or `None` when the whole stream is accepted.
fn first_refusal(messages: &[GridOutput]) -> Option<usize> {
    let mut check = GridStreamCheck::new();
    messages.iter().position(|m| check.accept(m).is_err())
}

/// The documented attach sequence, then live updates, passes the stream
/// check.
///
/// Why: this is the stream the supervisor's sender is built to produce, so
/// the check must accept it as written in the module docs, including a
/// resize that grows the screen for rows sent before its frame and a
/// history reset followed by a rebuild from a new start. Spec: hello, state,
/// rows, frame, history appends, the append a shift pushes off followed by
/// rows and that shifting frame, a clipboard write in two parts, a resize
/// frame, a reset and a rebuild, a history line wider than the screen
/// (written at its own `cols`), and a row sent twice before a shrinking
/// frame, are all accepted.
#[farhelm_testtrace::test]
fn the_documented_attach_sequence_and_live_updates_pass_the_stream_check() {
    let stream = [
        out(hello(80, 24, 1000)),
        state(),
        screen_rows(&[(0, 80), (23, 5)]),
        out(frame(1, 80, 24, 0, 5, 23)),
        out(history(500, 3)),
        out(history(503, 2)),
        // A line scrolls off: its history append comes before the frame
        // whose shift pushes it off the screen.
        out(history(505, 1)),
        screen_rows(&[(23, 10)]),
        out(frame(4, 80, 24, 1, 10, 23)),
        out(clipboard("first ".into(), false)),
        out(clipboard("second".into(), true)),
        // A row as wide as the NEW size, sent before the frame that grows
        // the screen: it is checked against the committing frame.
        screen_rows(&[(29, 100)]),
        out(frame(5, 100, 30, 0, 0, 0)),
        GridOutput::HistoryReset,
        out(history(0, 2)),
        out(history(2, 1)),
        // History keeps the width it was written at: a 1000-column line is
        // fine on a 100-column screen.
        GridOutput::History {
            first: 3,
            cols: GRID_MAX_DIMENSION,
            rows: vec![Row {
                wraps: false,
                runs: vec![run(0, GRID_MAX_DIMENSION, &"h".repeat(1000))],
            }],
            links: Vec::new(),
        },
        state(),
        // A later entry for a row replaces the earlier one before the frame,
        // so only the 50-column version is held against the shrinking frame.
        screen_rows(&[(0, 100)]),
        screen_rows(&[(0, 50)]),
        out(frame(6, 50, 30, 0, 0, 0)),
    ];
    assert_eq!(first_refusal(&stream), None);
}

/// `hello` and the `state` that must follow it: how every stream starts.
fn opened(cols: u16) -> Vec<GridOutput> {
    vec![out(hello(cols, 24, 0)), state()]
}

/// `opened(cols)` followed by `rest`.
fn after_open(cols: u16, rest: Vec<GridOutput>) -> Vec<GridOutput> {
    let mut stream = opened(cols);
    stream.extend(rest);
    stream
}

/// Each cross-message rule refuses the stream that breaks it.
///
/// Why: these are the bounds no single message can show, and a client that
/// skipped them would draw rows outside its screen, act on a screen before it
/// knows the terminal's modes, or show history with a silent hole in it.
/// Spec: refused at the named message are: anything before `hello`; a second
/// `hello`; anything but `state` right after `hello`; a buffered row at or
/// past the committing frame's height; a buffered row ending past the
/// committing frame's width; a `rev` that does not grow; a history append
/// that skips or repeats lines; `state`, `history`, `history_reset` or
/// `clipboard` inside an open screen update; anything but the next part
/// inside an open clipboard write; a part naming another target; and a write
/// past its total.
#[farhelm_testtrace::test]
fn each_cross_message_rule_refuses_the_stream_that_breaks_it() {
    let mut cases: Vec<(&str, Vec<GridOutput>, usize)> = vec![
        ("message before hello", vec![state()], 0),
        (
            "second hello",
            after_open(80, vec![out(hello(80, 24, 0))]),
            2,
        ),
        (
            "row below the frame",
            after_open(
                80,
                vec![screen_rows(&[(24, 1)]), out(frame(1, 80, 24, 0, 0, 0))],
            ),
            3,
        ),
        (
            "row wider than the frame",
            after_open(
                80,
                vec![screen_rows(&[(0, 81)]), out(frame(1, 80, 24, 0, 0, 0))],
            ),
            3,
        ),
        (
            "row wider than a shrinking frame",
            after_open(
                100,
                vec![screen_rows(&[(0, 100)]), out(frame(1, 80, 24, 0, 0, 0))],
            ),
            3,
        ),
        (
            "repeated rev",
            after_open(
                80,
                vec![
                    out(frame(3, 80, 24, 0, 0, 0)),
                    out(frame(3, 80, 24, 0, 0, 0)),
                ],
            ),
            3,
        ),
        (
            "history gap",
            after_open(80, vec![out(history(10, 2)), out(history(13, 1))]),
            3,
        ),
        (
            "history overlap",
            after_open(80, vec![out(history(10, 2)), out(history(11, 1))]),
            3,
        ),
    ];
    for (name, intruder) in [
        ("rows right after hello", screen_rows(&[(0, 1)])),
        ("frame right after hello", out(frame(1, 80, 24, 0, 0, 0))),
        ("history right after hello", out(history(0, 1))),
        ("history reset right after hello", GridOutput::HistoryReset),
        (
            "clipboard right after hello",
            out(clipboard("x".into(), true)),
        ),
    ] {
        cases.push((name, vec![out(hello(80, 24, 0)), intruder], 1));
    }
    for (name, intruder) in [
        ("state inside a screen update", state()),
        ("history inside a screen update", out(history(0, 1))),
        (
            "history reset inside a screen update",
            GridOutput::HistoryReset,
        ),
        (
            "clipboard inside a screen update",
            out(clipboard("x".into(), true)),
        ),
    ] {
        cases.push((
            name,
            after_open(80, vec![screen_rows(&[(0, 1)]), intruder]),
            3,
        ));
    }
    for (name, intruder) in [
        ("rows inside a clipboard write", screen_rows(&[(0, 1)])),
        ("state inside a clipboard write", state()),
        ("history inside a clipboard write", out(history(0, 1))),
        (
            "a part for another target",
            GridOutput::Clipboard {
                target: ClipboardTarget::Primary,
                text: "x".into(),
                last: true,
            },
        ),
    ] {
        cases.push((
            name,
            after_open(80, vec![out(clipboard("part".into(), false)), intruder]),
            3,
        ));
    }
    // Sixteen parts of 512 KiB make exactly the 8 MiB a write may hold; one
    // more byte is one too many.
    let part = || out(clipboard("a".repeat(GRID_MAX_CLIPBOARD_PART_BYTES), false));
    let mut whole = opened(80);
    whole.extend((0..15).map(|_| part()));
    whole.push(out(clipboard(
        "a".repeat(GRID_MAX_CLIPBOARD_PART_BYTES),
        true,
    )));
    assert_eq!(
        first_refusal(&whole),
        None,
        "a write of exactly the limit passes"
    );
    let mut over = opened(80);
    over.extend((0..16).map(|_| part()));
    over.push(out(clipboard("a".into(), true)));
    cases.push(("a clipboard write past its total", over, 18));

    for (name, stream, at) in cases {
        assert_eq!(first_refusal(&stream), Some(at), "{name}");
    }
}

/// A refused message leaves the stream check as it was.
///
/// Why: [`GridStreamCheck::accept`] promises it, and a sender test that
/// probes with a deliberately bad message relies on the next good one being
/// judged against the state before the probe. Spec: after a refused message
/// right after `hello`, the `state` is still awaited and accepted; after a
/// refused frame (its `rev` repeats), a good frame still checks the rows the
/// refused frame did not commit, and still sees the earlier `rev`; after a
/// refused history append (a gap), the right append is accepted.
#[farhelm_testtrace::test]
fn a_refused_message_leaves_the_stream_check_unchanged() {
    let mut check = GridStreamCheck::new();
    check.accept(&out(hello(80, 24, 0))).unwrap();
    assert!(check.accept(&out(history(10, 2))).is_err());
    check
        .accept(&state())
        .expect("the state is still the one message awaited");
    for message in [out(frame(5, 80, 24, 0, 0, 0)), out(history(10, 2))] {
        check.accept(&message).unwrap();
    }
    assert!(check.accept(&out(history(13, 1))).is_err());
    check
        .accept(&out(history(12, 1)))
        .expect("the append after the refused one is judged as if it never came");

    check.accept(&screen_rows(&[(23, 80)])).unwrap();
    assert!(check.accept(&out(frame(5, 80, 24, 0, 0, 0))).is_err());
    // The rows are still pending: a frame too short for them is refused.
    assert!(check.accept(&out(frame(6, 80, 23, 0, 0, 0))).is_err());
    check
        .accept(&out(frame(6, 80, 24, 0, 0, 0)))
        .expect("a good frame commits the rows the refused ones left");
}
