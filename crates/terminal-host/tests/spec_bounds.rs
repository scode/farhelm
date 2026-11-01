//! The resource bounds in the crate's SPEC.md against the constants the
//! code uses.
//!
//! The bounds are part of the permanent contract: a client has to know
//! what every host it may meet accepts, and running hosts keep the bounds
//! they were built with. So the table in SPEC.md ("Resource bounds") is the
//! specification and the constants in `protocol` must say the same thing.
//! Reading the table out of SPEC.md itself (rather than restating the values
//! here) means neither can change without the other.

use std::time::Duration;
use terminal_host::protocol;

/// What a row of the table states: a byte count, a plain count, or a time.
#[derive(Debug, PartialEq)]
enum Bound {
    Bytes(u64),
    Count(u64),
    Time(Duration),
}

/// Parse a table value: a sum of terms like `16 MiB`, `64 KiB`, `1 GiB`,
/// `64 bytes`, or a bare count, `5 seconds` or `200 ms`.
fn parse(value: &str) -> Bound {
    if let Some(seconds) = value.strip_suffix(" seconds") {
        return Bound::Time(Duration::from_secs(seconds.parse().expect(value)));
    }
    if let Some(millis) = value.strip_suffix(" ms") {
        return Bound::Time(Duration::from_millis(millis.parse().expect(value)));
    }
    if let Ok(count) = value.parse() {
        return Bound::Count(count);
    }
    let bytes = value
        .split(" + ")
        .map(|term| {
            let (number, unit) = term.split_once(' ').expect(value);
            let number: u64 = number.parse().expect(value);
            number
                * match unit {
                    "bytes" => 1,
                    "KiB" => 1024,
                    "MiB" => 1024 * 1024,
                    "GiB" => 1024 * 1024 * 1024,
                    _ => panic!("unknown unit in {value:?}"),
                }
        })
        .sum();
    Bound::Bytes(bytes)
}

/// The "Resource bounds" table of SPEC.md, as (constant name, value) rows.
fn spec_table() -> Vec<(String, String)> {
    let spec = include_str!("../SPEC.md");
    let section = spec
        .split("\n## Resource bounds\n")
        .nth(1)
        .expect("SPEC.md has a Resource bounds section");
    let section = section.split("\n## ").next().unwrap();
    section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            (cells[1].trim_matches('`').to_owned(), cells[2].to_owned())
        })
        .collect()
}

/// Every bound SPEC.md names has exactly the value the code uses, and every
/// bound the code exports to clients is in the table.
///
/// Why: these limits are what a client may rely on from every host of
/// version 1, and a host built with a different value stays running for
/// weeks; a change to either side alone is the kind of silent drift the
/// crate's change policy exists to stop. Spec: SPEC.md, "Resource bounds".
#[test]
fn the_resource_bounds_in_spec_md_are_the_ones_the_code_uses() {
    let code = [
        ("MAX_FRAME", Bound::Bytes(protocol::MAX_FRAME as u64)),
        ("MAX_SPEC", Bound::Bytes(protocol::MAX_SPEC as u64)),
        ("MAX_SNAPSHOT", Bound::Bytes(protocol::MAX_SNAPSHOT as u64)),
        ("MAX_TAG", Bound::Bytes(protocol::MAX_TAG as u64)),
        ("MIN_JOURNAL_CAP", Bound::Bytes(protocol::MIN_JOURNAL_CAP)),
        ("MAX_JOURNAL_CAP", Bound::Bytes(protocol::MAX_JOURNAL_CAP)),
        ("RECORD_COST", Bound::Bytes(protocol::RECORD_COST)),
        ("MAX_INPUT", Bound::Bytes(protocol::MAX_INPUT as u64)),
        ("ATOMIC_INPUT", Bound::Bytes(protocol::ATOMIC_INPUT as u64)),
        (
            "MAX_PENDING_INPUT",
            Bound::Bytes(protocol::MAX_PENDING_INPUT as u64),
        ),
        ("INPUT_COST", Bound::Bytes(protocol::INPUT_COST as u64)),
        (
            "OUTGOING_HIGH_WATER",
            Bound::Bytes(protocol::OUTGOING_HIGH_WATER as u64),
        ),
        (
            "MAX_OUTPUT_CHUNK",
            Bound::Bytes(protocol::MAX_OUTPUT_CHUNK as u64),
        ),
        (
            "MAX_PENDING_CONNECTIONS",
            Bound::Count(protocol::MAX_PENDING_CONNECTIONS as u64),
        ),
        (
            "MAX_BEFORE_HELLO",
            Bound::Bytes(protocol::MAX_BEFORE_HELLO as u64),
        ),
        ("HELLO_TIMEOUT", Bound::Time(protocol::HELLO_TIMEOUT)),
        ("EXIT_DRAIN_TIME", Bound::Time(protocol::EXIT_DRAIN_TIME)),
        (
            "EXIT_DRAIN_BYTES",
            Bound::Bytes(protocol::EXIT_DRAIN_BYTES as u64),
        ),
        ("GOODBYE_FLUSH", Bound::Time(protocol::GOODBYE_FLUSH)),
    ];
    let table = spec_table();
    assert_eq!(
        table
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        code.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        "SPEC.md's table and this test list the same bounds, in the same order"
    );
    for ((name, value), (_, expected)) in table.iter().zip(&code) {
        assert_eq!(&parse(value), expected, "{name}: SPEC.md says {value}");
    }
}

/// The two bounds stated with the message definitions rather than in the
/// table: an `Error` message's length and a terminal dimension.
///
/// Why: as for the table, a client relies on them from every host.
/// Spec: SPEC.md's `Error` row ("at most 1024 bytes"), and its `Resize` row
/// and child spec size ("each 1 to 10000").
#[test]
fn the_bounds_stated_with_the_messages_are_the_ones_the_code_uses() {
    let spec = include_str!("../SPEC.md");
    assert!(spec.contains(&format!(
        "at most {} bytes of UTF-8 text",
        protocol::MAX_ERROR_MESSAGE
    )));
    assert!(spec.contains(&format!(
        "`cols` u16, `rows` u16 (each 1 to {})",
        protocol::MAX_DIMENSION
    )));
    assert!(spec.contains(&format!(
        "Initial terminal size, each 1 to {}.",
        protocol::MAX_DIMENSION
    )));
}
