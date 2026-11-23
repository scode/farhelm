//! The platform's time zone, read for the GPUI app's clock times (`farhelm_gui::local_time`).
//!
//! The GUI crates never query the platform, so this binary hands the app its time zone, as it hands over Tab's reach.
//! On macOS and Linux that is the C library's local time: `tzset` reads the zone setting (the `TZ` variable, else
//! `/etc/localtime`, which macOS's System Settings and Linux's `timedatectl` both rewrite), and
//! `localtime_r` turns an instant into a local time with the zone data it loaded, daylight saving time included. Here's
//! the TLDR of why it is shaped this way:
//!
//! - **`refresh` is the only read.** glibc's `localtime_r` uses the rules loaded once and never looks at the setting
//!   again on its own, so the app's once-a-minute `refresh` (`tzset`) is what makes a zone change show; glibc rereads
//!   `/etc/localtime` there only when the file changed. macOS's `localtime_r` also notices a change by itself, through
//!   a system notification, at no cost to check. Either way `offset_at`, called for every time drawn, reads no file.
//! - **No new dependency.** `libc` is already this binary's, and these are two plain POSIX calls; a date crate would
//!   bring a second copy of the zone database logic for the same answer.
//! - **Elsewhere, no zone.** On any other platform [`platform`] hands the app nothing and it writes UTC, as the status
//!   sounds have no player there; the desktop app ships for macOS and Linux only.
//!
//! NOTE: Both calls read the process environment. That is sound only while nothing sets environment variables as the
//! app runs, which holds: neither the desktop app, its embedded helm nor the GUI sets any after startup, and tests may
//! not either (`.agents/test-authoring.md`).

use farhelm_gui::local_time::TimeZone;

/// The time zone for this platform, for the bootstrap to follow; `None` where the app has none, which writes UTC.
pub(crate) fn platform() -> Option<&'static dyn TimeZone> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let zone: Option<&'static dyn TimeZone> = Some(&PLATFORM);
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let zone: Option<&'static dyn TimeZone> = None;
    zone
}

/// The one platform time zone: stateless, so a `static` gives the `'static` reference the GUI keeps.
#[cfg(any(target_os = "macos", target_os = "linux"))]
static PLATFORM: Platform = Platform;

/// The C library's local time zone, as [`TimeZone`] asks for it.
#[cfg(any(target_os = "macos", target_os = "linux"))]
struct Platform;

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl TimeZone for Platform {
    fn refresh(&self) {
        // SAFETY: `tzset` takes no arguments and only updates the C library's own zone state, which it locks; it reads
        // the environment, which nothing in this process writes while the app runs (the module docs).
        unsafe { tzset() }
    }

    fn offset_at(&self, seconds: i64) -> Option<i32> {
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
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
unsafe extern "C" {
    /// POSIX `tzset`, which `libc` does not declare on unix: reads the time zone setting into the C library's state.
    fn tzset();
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use farhelm_gui::local_time::TimeZone as _;

    use super::PLATFORM;

    /// The app writes every clock time through this read, and an `offset_at` that failed, or answered something no real
    /// zone does, would leave the app writing UTC with no zone named. The test cannot know the machine's zone, so it
    /// checks only what holds in every zone. Specified: after a refresh, the offset at the epoch, at a recent instant
    /// and a second before the epoch is known and less than a day from UTC, and a refresh in between changes nothing
    /// while the setting stays as it is.
    #[farhelm_testtrace::test]
    fn the_platforms_offsets_are_known_and_less_than_a_day() {
        PLATFORM.refresh();
        let instants = [0, 1_800_000_000, -1];
        let first: Vec<Option<i32>> = instants.iter().map(|at| PLATFORM.offset_at(*at)).collect();
        for (at, offset) in instants.iter().zip(&first) {
            let offset = offset.unwrap_or_else(|| panic!("no offset at {at}"));
            assert!(offset.unsigned_abs() < 86_400, "{offset} s at {at}");
        }
        PLATFORM.refresh();
        let again: Vec<Option<i32>> = instants.iter().map(|at| PLATFORM.offset_at(*at)).collect();
        assert_eq!(first, again);
    }

    /// An instant far outside what `time_t` and the C library can place must give no offset rather than a wrong one or
    /// a crash. Specified: the most negative `i64` second gives `None`.
    #[farhelm_testtrace::test]
    fn an_instant_the_c_library_cannot_place_has_no_offset() {
        assert_eq!(PLATFORM.offset_at(i64::MIN), None);
    }
}
