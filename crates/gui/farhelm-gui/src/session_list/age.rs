//! How long ago a session was last active, as its row shows it: a short relative age (`2m`, `3h`) in the row's last
//! column, with the full timestamp as the age's hover help.
//!
//! SPEC.md, "Status": the age says when the agent was last seen working or started waiting on the user, falling back to
//! the session's creation time for a helm that sends no activity stamp, and a session with neither stamp shows no age
//! at all, "never one counted from 1970". The age is a difference between two machines' clocks and only as good as
//! they are, so it is never the only place the time is recorded: the timestamp beside it is the stamp itself, in the
//! viewer's local time ([`crate::local_time`]; the old app wrote it in UTC and said so, and the maintainer decided on
//! 2026-11-15 that the app's times are local).
//!
//! Ported from the old app's `activity.rs` at the reference commit (`ActivityStamp`, its buckets and `utc_stamp`), whose
//! unit tests pinned the bucket edges this module's tests pin again. Here's the TLDR of the rules:
//!
//! - **One stamp.** [`SessionInfo::effective_activity`](farhelm_proto::SessionInfo::effective_activity) picks it (the
//!   last activity, else the creation time), the same stamp the seen/unseen comparison uses, so the age and the unseen
//!   dot never disagree about what "last active" means.
//! - **Buckets round down.** Under ten seconds reads `now`, under a minute `<1m`, then whole minutes, hours and days,
//!   unpadded and with no upper bound (`1000d` is a real age, so nothing may size itself to four characters).
//! - **The future reads `now`.** A stamp ahead of the viewer's clock (the two machines disagree) is no evidence of a
//!   negative age; its timestamp still shows the instant the host reported.
//! - **No clock, no age.** A viewer clock at or before the epoch is broken, not a time to count from, so it draws no
//!   age rather than painting every row `now` or a 56-year age.
//!
//! The time comes from the ticker ([`crate::ticker::wall_now`]), never from the system clock, and the list holds the
//! ticker's 30-second clock while any row shows an age, so ages move on with no row running a timer of its own
//! (SPEC_impl.md, "Animation time: the ticker").

use std::time::{SystemTime, UNIX_EPOCH};

use crate::local_time::{self, Zone};

/// A session's age as its row draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Age {
    /// The relative age shown in the row: `now`, `<1m`, `5m`, `3h`, `12d`.
    pub(crate) short: String,
    /// The hover help, the stamp itself in the viewer's zone: `Last activity 2025-07-01 23:57:58`.
    pub(crate) full: String,
}

/// The age of a session last active at `stamp` (seconds since the epoch; 0 or less is no stamp), seen from `now`, the
/// viewer's clock in seconds (`None` when it is unusable), with the timestamp written in `zone`, the viewer's. `None`
/// when either is missing: no age is drawn at all, the relative age and the timestamp together or neither (the old
/// `ActivityStamp::new`).
pub(crate) fn of(stamp: i64, now: Option<i64>, zone: Zone) -> Option<Age> {
    let now = now.filter(|now| *now > 0)?;
    if stamp <= 0 {
        return None;
    }
    Some(Age {
        short: relative(now.saturating_sub(stamp)),
        full: format!("Last activity {}", local_time::stamp(stamp, zone)),
    })
}

/// `elapsed` seconds as the row's relative age, rounded down into its bucket. A negative `elapsed` (a stamp from the
/// future) reads `now`.
fn relative(elapsed: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    match elapsed.max(0) {
        elapsed if elapsed < 10 => "now".to_owned(),
        elapsed if elapsed < MINUTE => "<1m".to_owned(),
        elapsed if elapsed < HOUR => format!("{}m", elapsed / MINUTE),
        elapsed if elapsed < DAY => format!("{}h", elapsed / HOUR),
        elapsed => format!("{}d", elapsed / DAY),
    }
}

/// The viewer's clock in whole seconds since the epoch, as [`of`] takes it: `None` for a reading at or before the
/// epoch, which only a broken clock gives.
pub(crate) fn seconds(now: SystemTime) -> Option<i64> {
    let secs = now.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(secs).ok().filter(|secs| *secs > 0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// The buckets are what a user reads a fleet's recency from, and the old unit test pinned exactly where each starts
    /// and ends; an off-by-one here shows `1m` for 59 seconds or `24h` for a day. Specified: 0 and 9 s read `now`; 10
    /// and 59 s `<1m`; 60 and 119 s `1m`; 3599 s `59m`; 3600 s `1h`; 23 h 59 m `23h`; 86400 s `1d`; 400 and 1000 days
    /// `400d` and `1000d` (no ceiling); a negative elapsed time `now`.
    #[test]
    fn each_relative_bucket_starts_and_ends_where_it_says() {
        let cases = [
            (0, "now"),
            (9, "now"),
            (10, "<1m"),
            (59, "<1m"),
            (60, "1m"),
            (119, "1m"),
            (3_599, "59m"),
            (3_600, "1h"),
            (23 * 3_600 + 59 * 60, "23h"),
            (86_400, "1d"),
            (400 * 86_400, "400d"),
            (1_000 * 86_400, "1000d"),
            (-86_400, "now"),
        ];
        for (elapsed, shown) in cases {
            assert_eq!(relative(elapsed), shown, "{elapsed} s");
        }
    }

    /// The age and its timestamp are built from one stamp, and the timestamp is the stamp itself rather than the
    /// viewer's clock minus the age, so a skewed clock cannot falsify the record; the timestamp is the viewer's local
    /// time, so the zone changes the record's words but never the age. Specified: a stamp two hours old reads `2h` over
    /// `Last activity 2023-11-14 20:13:20` in UTC and `Last activity 2023-11-14 15:13:20` five hours west of it; a
    /// stamp a day in the future reads `now` and keeps its own future instant; the timestamp is zero-padded, names no
    /// zone, and lands on leap days and centuries correctly (the calendar itself is pinned in `local_time_tests.rs`).
    #[test]
    fn a_present_stamp_carries_both_the_age_and_the_record() {
        let now = 1_700_000_000;
        assert_eq!(
            of(now - 2 * 3_600, Some(now), Zone::UTC),
            Some(Age {
                short: "2h".into(),
                full: "Last activity 2023-11-14 20:13:20".into(),
            })
        );
        assert_eq!(
            of(now - 2 * 3_600, Some(now), Zone::fixed(-5 * 3_600)),
            Some(Age {
                short: "2h".into(),
                full: "Last activity 2023-11-14 15:13:20".into(),
            })
        );
        assert_eq!(
            of(now + 86_400, Some(now), Zone::UTC),
            Some(Age {
                short: "now".into(),
                full: "Last activity 2023-11-15 22:13:20".into(),
            })
        );
        for (stamp, full) in [
            (1_709_164_800, "Last activity 2024-02-29 00:00:00"),
            (951_782_400, "Last activity 2000-02-29 00:00:00"),
            (4_107_542_400, "Last activity 2100-03-01 00:00:00"),
        ] {
            assert_eq!(
                of(stamp, Some(now), Zone::UTC)
                    .map(|age| age.full)
                    .as_deref(),
                Some(full)
            );
        }
    }

    /// SPEC.md, "Status": a session with no stamp gets "no age at all, never one counted from 1970", and a broken viewer
    /// clock must not paint every row `now`. Specified: a stamp of 0 or below gives no age; a viewer clock that is
    /// unavailable, at the epoch or before it gives none either; a clock reading before the epoch converts to `None`
    /// and one after it to its whole seconds.
    #[test]
    fn a_missing_stamp_or_clock_produces_no_age_at_all() {
        assert_eq!(of(0, Some(1_700_000_000), Zone::UTC), None);
        assert_eq!(of(-1, Some(1_700_000_000), Zone::UTC), None);
        assert_eq!(of(1_700_000_000, None, Zone::UTC), None);
        assert_eq!(of(1_700_000_000, Some(0), Zone::UTC), None);
        assert_eq!(of(1_700_000_000, Some(-5), Zone::UTC), None);
        assert_eq!(seconds(UNIX_EPOCH), None);
        assert_eq!(seconds(UNIX_EPOCH - Duration::from_secs(5)), None);
        assert_eq!(
            seconds(UNIX_EPOCH + Duration::from_millis(1_700_000_000_900)),
            Some(1_700_000_000)
        );
    }
}
