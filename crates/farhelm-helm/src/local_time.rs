//! The local time of the machine this process runs on, read from the C library: the hour ntfy quiet hours compare
//! against (`crate::ntfy`), and the UTC offsets the desktop app writes its clock times with (its `time_zone` module
//! delegates here).
//!
//! On macOS and Linux that is POSIX local time: `tzset` reads the zone setting (the `TZ` variable, else
//! `/etc/localtime`, which macOS's System Settings and Linux's `timedatectl` both rewrite), and `localtime_r` turns an
//! instant into a local time with the zone data it loaded, daylight saving time included. Here's the TLDR of why it is
//! shaped this way:
//!
//! - **[`refresh`] is the only read of the setting.** glibc's `localtime_r` uses the rules loaded once and never looks
//!   at the setting again on its own, so a caller that must follow a zone change calls [`refresh`] (`tzset`) first;
//!   glibc rereads `/etc/localtime` there only when the file changed. macOS's `localtime_r` also notices a change by
//!   itself, through a system notification. [`utc_offset_at`] reads no file.
//! - **No new dependency.** `libc` is already a dependency, and these are two plain POSIX calls; a date crate would
//!   bring a second copy of the zone database logic for the same answer.
//! - **Elsewhere, UTC.** On any other platform there is no zone: [`utc_offset_at`] answers `None` and [`hour_now`]
//!   reads UTC. Farhelm ships for macOS and Linux only.
//!
//! NOTE: Both calls read the process environment. That is sound only while nothing sets environment variables as the
//! process runs, which holds: neither the helm, the desktop app nor the GUI sets any after startup, and tests may not
//! either (`.agents/test-authoring.md`).

/// Read the time zone setting again (`tzset`), so that later offsets follow a zone the user changed since. Cheap when
/// nothing changed; it locks the C library's own zone state.
pub fn refresh() {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    // SAFETY: `tzset` takes no arguments and only updates the C library's own zone state, which it locks; it reads the
    // environment, which nothing in this process writes while it runs (the module docs).
    unsafe {
        tzset()
    }
}

/// The local zone's offset from UTC, in seconds east, at `seconds` since the Unix epoch, with the zone data the last
/// [`refresh`] loaded. `None` for an instant the C library cannot place, or on a platform without a zone.
pub fn utc_offset_at(seconds: i64) -> Option<i32> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let time = libc::time_t::try_from(seconds).ok()?;
        // SAFETY: `tm` is plain integers and a pointer, for which all zeroes is a valid value.
        let mut local: libc::tm = unsafe { std::mem::zeroed() };
        // SAFETY: both pointers are valid for the call, and `localtime_r` writes only to `local`. It returns null when
        // the instant is out of the C library's range, and then `local` is not read.
        let written = unsafe { libc::localtime_r(&time, &mut local) };
        if written.is_null() {
            return None;
        }
        i32::try_from(local.tm_gmtoff).ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = seconds;
        None
    }
}

/// The local hour now (0 to 23), after a [`refresh`], so that a zone change since the last read counts. Neither failure
/// fails it: a clock before the epoch reads as the epoch, and an instant the zone cannot place reads as UTC.
pub fn hour_now() -> u8 {
    refresh();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(0));
    hour_at(now, utc_offset_at(now).unwrap_or(0))
}

/// The hour of the day (0 to 23) at `seconds` since the epoch in a zone `offset` seconds east of UTC.
fn hour_at(seconds: i64, offset: i32) -> u8 {
    let local = seconds.saturating_add(i64::from(offset));
    u8::try_from(local.rem_euclid(86_400) / 3_600).expect("an hour of the day is below 24")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
unsafe extern "C" {
    /// POSIX `tzset`, which `libc` does not declare on unix: reads the time zone setting into the C library's state.
    fn tzset();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desktop app writes every clock time through this read and quiet hours compare its hour, and an offset that
    /// failed, or answered something no real zone does, would put both on UTC with no warning. The test cannot know the
    /// machine's zone, so it checks only what holds in every zone. Specified: after a refresh, the offset at the epoch,
    /// at a recent instant and a second before the epoch is known and less than a day from UTC, and a refresh in
    /// between changes nothing while the setting stays as it is.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[farhelm_testtrace::test]
    fn the_platforms_offsets_are_known_and_less_than_a_day() {
        refresh();
        let instants = [0, 1_800_000_000, -1];
        let first: Vec<Option<i32>> = instants.iter().map(|at| utc_offset_at(*at)).collect();
        for (at, offset) in instants.iter().zip(&first) {
            let offset = offset.unwrap_or_else(|| panic!("no offset at {at}"));
            assert!(offset.unsigned_abs() < 86_400, "{offset} s at {at}");
        }
        refresh();
        let again: Vec<Option<i32>> = instants.iter().map(|at| utc_offset_at(*at)).collect();
        assert_eq!(first, again);
    }

    /// An instant far outside what `time_t` and the C library can place must give no offset rather than a wrong one or
    /// a crash. Specified: the most negative `i64` second gives `None`.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[farhelm_testtrace::test]
    fn an_instant_the_c_library_cannot_place_has_no_offset() {
        assert_eq!(utc_offset_at(i64::MIN), None);
    }

    /// Quiet hours compare this hour with whole-hour bounds, so an offset applied the wrong way or a negative local
    /// time read as a negative hour would quiet the wrong hours. Specified: the hour is the local time of day's hour
    /// for zones east and west of UTC and across midnight in both directions, and a time before the epoch still gives
    /// an hour from 0 to 23.
    #[farhelm_testtrace::test]
    fn the_hour_is_the_local_time_of_days_hour() {
        let noon_utc = 12 * 3_600;
        assert_eq!(hour_at(noon_utc, 0), 12);
        assert_eq!(hour_at(noon_utc, 2 * 3_600), 14);
        assert_eq!(hour_at(noon_utc, -5 * 3_600), 7);
        assert_eq!(hour_at(23 * 3_600, 3_600), 0);
        assert_eq!(hour_at(30 * 60, -3_600), 23);
        assert_eq!(hour_at(-1, 0), 23);
        assert_eq!(hour_at(5 * 3_600 + 30 * 60, 30 * 60), 6);
    }

    /// Quiet hours read this live hour on every send, so reading it through the C library must not fail or panic
    /// whatever the machine's zone. Specified: `hour_now` returns (its value is below 24 by construction).
    #[farhelm_testtrace::test]
    fn the_hour_now_is_an_hour_of_the_day() {
        assert!(hour_now() < 24);
    }
}
