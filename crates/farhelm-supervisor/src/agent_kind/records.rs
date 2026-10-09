//! Bounded readers and parsers for the exact vendor files reports identify.
//!
//! No directory is searched for an identity. These helpers refuse non-regular
//! files and bound reads of growing transcripts; integrations decide what a
//! supported header means and whether it verifies their reported locator.

use super::{AgentIntegration, RECORD_PREFIX_BYTES};
use std::path::Path;

/// Validated fields from a vendor record's own header.
/// The cwd and timestamp remain part of header validation even where the caller
/// only needs its conversation id; they are never used to select nearby records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCorrelators {
    /// The vendor's conversation id, validated before it is retained.
    pub conversation: String,
    /// The directory recorded in the header, not inferred from its file path.
    pub cwd: String,
    /// The header's timestamp, represented as Unix seconds.
    pub created_at: i64,
}

/// Read the bounded header of one reported record.
/// `Ok(None)` means the path disappeared or the integration positively does not
/// recognize it. I/O and parse failures remain errors so the caller can refuse
/// Resume without inventing a replacement identity.
pub async fn read_record(
    path: &Path,
    integration: &dyn AgentIntegration,
) -> anyhow::Result<Option<RecordCorrelators>> {
    let Some(text) =
        read_header(path, |first| integration.record_header_has_prelude(first)).await?
    else {
        return Ok(None);
    };
    integration.parse_record(&text)
}

/// Open a vendor record without following a symlink or blocking on a FIFO.
///
/// Opened `O_NOFOLLOW | O_NONBLOCK` and then re-validated through the
/// resulting descriptor: `O_NOFOLLOW` refuses a symlink placed where a
/// record should be, `O_NONBLOCK` keeps a FIFO from parking this task on
/// an open that would never return, and the `fstat`-based regular-file
/// check validates the object actually opened, even if the path was replaced
/// between report admission and verification.
async fn open_regular_file(path: &Path) -> anyhow::Result<Option<tokio::fs::File>> {
    let opened = tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .await;
    let file = match opened {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(anyhow::Error::new(e).context(format!("opening {}", path.display())));
        }
    };
    let metadata = file
        .metadata()
        .await
        .map_err(|e| anyhow::Error::new(e).context(format!("stat of open {}", path.display())))?;
    if !metadata.is_file() {
        anyhow::bail!(
            "{} is not a regular file; refusing to read it as a conversation record",
            path.display()
        );
    }
    Ok(Some(file))
}

/// Read at most [`RECORD_PREFIX_BYTES`] of a growing record as lossy text.
///
/// Record identity lives at the beginning of an otherwise unbounded stream,
/// so truncation and a final partial UTF-8 code point are both expected here.
/// `Ok(None)` means the path disappeared before it could be opened.
pub(crate) async fn read_prefix(path: &Path) -> anyhow::Result<Option<String>> {
    use tokio::io::AsyncReadExt;

    let Some(file) = open_regular_file(path).await? else {
        return Ok(None);
    };
    let mut buffer = Vec::new();
    file.take(RECORD_PREFIX_BYTES as u64)
        .read_to_end(&mut buffer)
        .await
        .map_err(|e| anyhow::Error::new(e).context(format!("reading {}", path.display())))?;
    Ok(Some(String::from_utf8_lossy(&buffer).into_owned()))
}

/// Read only the first record, retaining its newline when it fits under the cap.
///
/// Header verification needs none of the transcript's later records. The cap
/// remains a byte ceiling, not a read target; incomplete final lines are left
/// for the vendor parser to judge. Valid UTF-8 transfers its buffer into the
/// String without another allocation. Malformed bytes keep the existing lossy
/// decoding behavior, including a code point cut by the byte ceiling.
pub(crate) async fn read_first_line(path: &Path) -> anyhow::Result<Option<String>> {
    read_header(path, |_| false).await
}

/// Stop after the header, including one integration-authorized prelude.
///
/// OMP's optional title precedes its actual session header. Reading that second
/// line is necessary to keep its existing format contract; other integrations
/// stop after the first newline. Both lines share one byte ceiling and one open
/// descriptor. Buffered I/O may fetch a small suffix beyond the last newline,
/// but the returned buffer contains only the relevant records.
async fn read_header(
    path: &Path,
    has_prelude: impl FnOnce(&str) -> bool,
) -> anyhow::Result<Option<String>> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

    let Some(file) = open_regular_file(path).await? else {
        return Ok(None);
    };
    let mut buffer = Vec::new();
    let mut reader = BufReader::new(file.take(RECORD_PREFIX_BYTES as u64));
    reader.read_until(b'\n', &mut buffer).await.map_err(|e| {
        anyhow::Error::new(e).context(format!("reading header of {}", path.display()))
    })?;
    if buffer.ends_with(b"\n") && has_prelude(&String::from_utf8_lossy(&buffer)) {
        reader.read_until(b'\n', &mut buffer).await.map_err(|e| {
            anyhow::Error::new(e).context(format!("reading header of {}", path.display()))
        })?;
    }
    let text = String::from_utf8(buffer)
        .unwrap_or_else(|error| String::from_utf8_lossy(&error.into_bytes()).into_owned());
    Ok(Some(text))
}

/// Read one complete small vendor file as strict UTF-8.
///
/// Unlike [`read_prefix`], callers use this when trailing bytes affect the
/// meaning of the document. Reading one byte beyond the shared bound makes an
/// oversized file a refusal instead of accepting a valid-looking prefix.
pub(crate) async fn read_complete(path: &Path) -> anyhow::Result<Option<String>> {
    use tokio::io::AsyncReadExt;

    let Some(file) = open_regular_file(path).await? else {
        return Ok(None);
    };
    let mut buffer = Vec::new();
    file.take((RECORD_PREFIX_BYTES + 1) as u64)
        .read_to_end(&mut buffer)
        .await
        .map_err(|e| anyhow::Error::new(e).context(format!("reading {}", path.display())))?;
    anyhow::ensure!(
        buffer.len() <= RECORD_PREFIX_BYTES,
        "{} exceeds the bounded complete-file limit",
        path.display()
    );
    let text = String::from_utf8(buffer)
        .map_err(|e| anyhow::Error::new(e).context(format!("decoding {}", path.display())))?;
    Ok(Some(text))
}

/// Unix seconds for diagnostics and fixtures; shares the store's clock spelling.
pub use crate::store::now_unix;

// Timestamp parsing stays strict because malformed vendor headers are not
// acceptable proof for an exact-file locator.
/// Days in `month` of `year`, proleptic Gregorian. Zero for a month
/// outside 1..=12, which makes every day comparison against it fail.
fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Parse a fixed-width run of ASCII digits at `offset`, or `None`.
///
/// Explicit rather than `str::parse`, which accepts forms this grammar
/// does not: `"+1".parse::<i64>()` succeeds, and a two-character field of
/// this format is never a signed number.
fn digits(text: &str, offset: usize, width: usize) -> Option<i64> {
    let slice = text.get(offset..offset + width)?;
    if !slice.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    slice.parse().ok()
}

/// Parse an RFC 3339 timestamp into seconds since the Unix epoch, or
/// `None` if it is not one.
///
/// Strict about the represented instant, and total: every rejection returns `None`, and a `None` makes the
/// record a parse failure rather than silently inventing a time.
///
/// - Layout is exact: `YYYY-MM-DDThh:mm:ss`, with `T` (either case) or a
///   space as the separator, and the input must be CONSUMED — no trailing
///   garbage after the zone.
/// - Ranges are real: month 1–12, day within that month in that year
///   (leap years included), hour 0–23, minute and second 0–59, offset hour
///   0–23 and offset minute 0–59. A leap second (`:60`) is rejected;
///   neither vendor writes one, and admitting it would need a policy for
///   what instant it names.
/// - A `.` must be followed by at least one digit.
/// - A zone is MANDATORY. A local-time timestamp with no offset cannot be
///   placed on the epoch line at all, and assuming UTC for one would shift
///   a record by hours.
///
/// Fractional seconds are accepted and truncated to this function's Unix-second
/// return value; callers needing sub-second ordering use their own parser.
pub fn parse_rfc3339(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !matches!(bytes[10], b'T' | b't' | b' ') {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let year = digits(text, 0, 4)?;
    let month = digits(text, 5, 2)? as u32;
    let day = digits(text, 8, 2)? as u32;
    let hour = digits(text, 11, 2)?;
    let minute = digits(text, 14, 2)?;
    let second = digits(text, 17, 2)?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        rest = &fraction[digits..];
    }
    // Offsets are applied by SUBTRACTION: a `+02:00` timestamp names a
    // moment two hours earlier in UTC than its digits read.
    let offset_seconds = match rest.as_bytes().first() {
        Some(b'Z' | b'z') if rest.len() == 1 => 0,
        Some(sign @ (b'+' | b'-')) => {
            if rest.len() != 6 || rest.as_bytes()[3] != b':' {
                return None;
            }
            let offset_hour = digits(rest, 1, 2)?;
            let offset_minute = digits(rest, 4, 2)?;
            if offset_hour > 23 || offset_minute > 59 {
                return None;
            }
            let sign = if *sign == b'-' { -1 } else { 1 };
            sign * (offset_hour * 3600 + offset_minute * 60)
        }
        // Everything else — a missing zone, `Zjunk`, a stray character —
        // is a refusal rather than a default.
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset_seconds)
}

/// Format a Unix second as an RFC 3339 UTC timestamp
/// (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// Public because the fake-agent fixture writes records the supervisor
/// then parses, and sharing one implementation is what keeps a fixture bug
/// from masquerading as a parser bug (or the reverse).
pub fn format_rfc3339(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = farhelm_proto::time::civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60,
        seconds_of_day % 60
    )
}

/// Days since 1970-01-01 for a proleptic-Gregorian civil date (Howard
/// Hinnant's `days_from_civil`). Exact for every year this could ever see.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = ((month + 9) % 12) as i64;
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Header verification must stop at the newline rather than treating the
    /// byte ceiling as a read target. Invalid tail bytes prove that only the
    /// first record is decoded, even when the transcript has already grown.
    #[farhelm_testtrace::test]
    async fn a_record_header_ignores_the_rest_of_the_transcript() {
        let dir = farhelm_teststate::tempdir().unwrap();
        let path = dir.path().join("record.jsonl");
        let mut bytes = b"header\n".to_vec();
        bytes.extend(vec![0xff; RECORD_PREFIX_BYTES]);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            read_first_line(&path).await.unwrap().as_deref(),
            Some("header\n")
        );
    }

    /// The original byte ceiling still separates complete and truncated
    /// headers: a newline exactly at the ceiling fits, one beyond it does not.
    #[farhelm_testtrace::test]
    async fn a_record_header_keeps_the_byte_ceiling() {
        let dir = farhelm_teststate::tempdir().unwrap();
        let path = dir.path().join("record.jsonl");
        for content_len in [
            RECORD_PREFIX_BYTES - 1,
            RECORD_PREFIX_BYTES,
            RECORD_PREFIX_BYTES + 1,
        ] {
            let mut bytes = vec![b'x'; content_len];
            bytes.push(b'\n');
            std::fs::write(&path, &bytes).unwrap();
            let actual = read_first_line(&path).await.unwrap().unwrap();
            assert_eq!(
                actual.as_bytes(),
                &bytes[..bytes.len().min(RECORD_PREFIX_BYTES)]
            );
            assert_eq!(actual.ends_with('\n'), content_len < RECORD_PREFIX_BYTES);
        }
    }

    /// Valid headers avoid a copy without changing the old lossy decoding
    /// contract for malformed bytes or the missing-file verdict.
    #[farhelm_testtrace::test]
    async fn a_record_header_preserves_lossy_decoding_and_missing_files() {
        let dir = farhelm_teststate::tempdir().unwrap();
        let path = dir.path().join("record.jsonl");
        assert!(read_first_line(&path).await.unwrap().is_none());
        std::fs::write(&path, b"\xff\n").unwrap();
        assert_eq!(
            read_first_line(&path).await.unwrap().as_deref(),
            Some("\u{fffd}\n")
        );
    }

    /// The file reader must preserve OMP's title-slot exception without letting
    /// that tolerance leak into Pi. An invalid tail also proves decoding stops
    /// at the header for both OMP shapes, rather than reading a fixed prefix.
    #[farhelm_testtrace::test]
    async fn record_reads_preserve_the_vendor_header_boundary() {
        let dir = farhelm_teststate::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let header = br#"{"type":"session","version":3,"id":"omp-id-1","cwd":"/workspace","timestamp":"2026-09-17T00:00:00Z"}
"#;
        let title = r#"{"type":"title","v":1,"title":"t","updatedAt":"u","pad":""#;
        let title = format!("{title}{}\"}}\n", " ".repeat(256 - title.len() - 3));
        assert_eq!(title.len(), 256, "the fixture uses OMP's title-slot width");
        let omp = super::super::integration_for(farhelm_proto::AgentKind::Omp).unwrap();
        let pi = super::super::integration_for(farhelm_proto::AgentKind::Pi).unwrap();
        let spaced_title = format!("\u{00a0}{title}");
        for prelude in ["", title.as_str(), spaced_title.as_str()] {
            let mut bytes = prelude.as_bytes().to_vec();
            bytes.extend_from_slice(header);
            bytes.extend(vec![0xff; RECORD_PREFIX_BYTES]);
            std::fs::write(&path, bytes).unwrap();
            let text = read_header(&path, |first| omp.record_header_has_prelude(first))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(text.as_bytes(), [prelude.as_bytes(), header].concat());
            assert_eq!(
                read_record(&path, omp).await.unwrap().unwrap().conversation,
                "omp-id-1"
            );
            if prelude.is_empty() {
                assert_eq!(
                    read_record(&path, pi).await.unwrap().unwrap().conversation,
                    "omp-id-1"
                );
            } else {
                assert!(
                    read_record(&path, pi).await.is_err(),
                    "Pi cannot skip OMP's title slot"
                );
            }
        }
    }

    /// A growing transcript must not make report verification read without a
    /// bound. ASCII distinguishes the input byte cap from lossy UTF-8 expansion.
    #[farhelm_testtrace::test]
    async fn a_record_prefix_stops_at_the_byte_bound() {
        let dir = farhelm_teststate::tempdir().unwrap();
        let path = dir.path().join("record.jsonl");
        std::fs::write(&path, vec![b'x'; RECORD_PREFIX_BYTES + 17]).unwrap();
        let prefix = read_prefix(&path).await.unwrap().unwrap();
        assert_eq!(prefix, "x".repeat(RECORD_PREFIX_BYTES));
    }

    /// Exact-file verification rejects malformed header timestamps instead of
    /// normalizing them into plausible instants. Each case is a distinct way
    /// a lenient parser could accept an unsupported vendor header.
    #[farhelm_testtrace::test]
    fn rfc3339_parsing_rejects_everything_that_is_not_an_instant() {
        for bad in [
            "",
            "nope",
            "2026-07-29T12:00",
            "2026-13-29T12:00:00Z",
            "2026-00-29T12:00:00Z",
            "2026-02-31T12:00:00Z",
            "2025-02-29T12:00:00Z",
            "2026-07-00T12:00:00Z",
            "2026-07-32T12:00:00Z",
            "2026-07-29T24:00:00Z",
            "2026-07-29T25:99:99Z",
            "2026-07-29T12:60:00Z",
            "2026-07-29T12:00:60Z",
            "2026-07-29T12:00:00+99:99",
            "2026-07-29T12:00:00+24:00",
            "2026-07-29T12:00:00+01",
            "2026-07-29T12:00:00+0100",
            "2026-07-29T12:00:00.Z",
            "2026-07-29T12:00:00.123",
            "2026-07-29T12:00:00",
            "2026-07-29T12:00:00Zjunk",
            "2026-07-29T12:00:00Z ",
            "2026-07-29X12:00:00Z",
            "2026-07-29T+2:00:00Z",
        ] {
            assert_eq!(parse_rfc3339(bad), None, "{bad:?} must not parse");
        }

        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339("2026-07-29T12:00:05Z"), Some(1_785_326_405));
        assert_eq!(
            parse_rfc3339("2026-07-29T12:00:05.987654Z"),
            Some(1_785_326_405),
            "fractional seconds truncate rather than round"
        );
        assert_eq!(
            parse_rfc3339("2026-07-29T14:00:05+02:00"),
            Some(1_785_326_405),
            "a positive offset names an earlier UTC moment"
        );
        assert_eq!(
            parse_rfc3339("2026-07-29T10:00:05-02:00"),
            Some(1_785_326_405)
        );
        assert_eq!(parse_rfc3339("2026-07-29 12:00:05Z"), Some(1_785_326_405));
        assert!(
            parse_rfc3339("2024-02-29T00:00:00Z").is_some(),
            "a real leap day must still parse"
        );

        for unix in [0, 1_785_326_405, 951_782_400, 4_102_444_800] {
            assert_eq!(
                parse_rfc3339(&format_rfc3339(unix)),
                Some(unix),
                "formatting and parsing must be exact inverses at {unix}"
            );
        }
    }
}
