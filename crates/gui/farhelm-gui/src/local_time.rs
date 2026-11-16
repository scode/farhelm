//! Local time: the one place the GUI turns an instant into clock text, in the viewer's own time zone.
//!
//! The maintainer decided on 2026-11-15 that every clock time the app shows (a session's last activity, when a
//! credential was saved, when a snapshot was built, "checked HH:MM") is the viewer's local time, not UTC. SPEC.md asked
//! for local time in one place already ("None found · checked HH:MM"); the old app wrote the rest in UTC and said so,
//! because a browser stamp formatted in Rust had no time zone to use. Times are written as they were, without the
//! "UTC" label, except "checked", which is hours and minutes, as the old list showed it.
//!
//! Here's the TLDR of how it works:
//!
//! - **The platform is the host's.** The GUI crates never query the platform (see the ticker on reduced motion), so the
//!   process that embeds the GUI hands in its [`TimeZone`] (`bootstrap::Host::time_zone`). Without one, as in tests and
//!   the screenshot goldens, every time is UTC, which makes no test depend on the machine's time zone.
//! - **Each instant gets its own offset.** A stamp from July shown in November keeps July's offset, as `date` or a
//!   browser would show it. One offset applied to every stamp would put every time from the other side of a daylight
//!   saving change an hour off.
//! - **The zone is read again at most once a minute, from a draw.** [`zone`] asks the platform to read its time zone
//!   setting again when the last read is [`REFRESH_PERIOD`] old, so a zone changed while the app runs shows within a
//!   minute of the next draw. It starts no timer: the migration's orchestrator decided on 2026-11-16 that this re-read
//!   must not wake an idle app, so a view whose text nothing redraws keeps its old times until something draws it
//!   again. Between reads, turning an instant into an offset touches no file (the platform's zone data is already in
//!   memory).
//! - **Formatting is pure.** [`stamp`], [`minute`], [`date`] and [`clock`] take seconds since the epoch and a [`Zone`],
//!   so a view reads the zone once per draw and its pure helpers stay testable with a [`Zone::fixed`] offset.

use std::cell::Cell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{App, Global};

use crate::bootstrap::installed;
use crate::ticker;

/// How old the platform's last reading of its time zone may get before the next draw asks it to read the setting again.
/// A minute keeps a zone change (travel, or the user picking another zone) from showing stale times for long, while a
/// window that draws ten times a second reads the setting once a minute, not on every frame.
pub(crate) const REFRESH_PERIOD: Duration = Duration::from_secs(60);

/// The platform's time zone, as the process that embeds the GUI reads it.
///
/// Both methods are called on the main thread, from a draw, so both must return quickly and never fail outright.
pub trait TimeZone {
    /// Read the platform's time zone setting again, so a change made while the app runs shows. Called once when the
    /// host's zone is installed and then at most once per [`REFRESH_PERIOD`], from a draw. It may read a file.
    fn refresh(&self);

    /// The offset east of UTC, in seconds, that the setting last read gives the instant `seconds` since the epoch
    /// (daylight saving time included), or `None` when the platform cannot say. Called for every time drawn, so it
    /// must not read a file.
    fn offset_at(&self, seconds: i64) -> Option<i32>;
}

/// The time zone clock text is written in: a fixed offset, or the platform's zone, which gives each instant its own.
///
/// It is `Copy`, so a view reads it once per draw ([`zone`]) and hands it to every row, the way it hands them the
/// time now.
#[derive(Clone, Copy)]
pub(crate) struct Zone(Kind);

/// Where a [`Zone`]'s offsets come from. The platform's zone is `'static` because the host's lives as long as the app.
#[derive(Clone, Copy)]
enum Kind {
    Fixed(i32),
    Platform(&'static dyn TimeZone),
}

impl Zone {
    /// UTC: what every time is written in without the host's zone, as in tests and the goldens.
    pub(crate) const UTC: Zone = Zone(Kind::Fixed(0));

    /// A zone `seconds_east` of UTC all year round, for tests that pin the viewer's zone.
    #[cfg(test)]
    pub(crate) const fn fixed(seconds_east: i32) -> Zone {
        Zone(Kind::Fixed(seconds_east))
    }

    /// The offset east of UTC, in seconds, at the instant `seconds` since the epoch. A platform that cannot say, or
    /// says something a day or more from UTC (no real zone is), counts as UTC, so a time is still drawn.
    fn offset_at(self, seconds: i64) -> i64 {
        let offset = match self.0 {
            Kind::Fixed(offset) => offset,
            Kind::Platform(platform) => platform
                .offset_at(seconds)
                .filter(|offset| offset.unsigned_abs() < 86_400)
                .unwrap_or(0),
        };
        i64::from(offset)
    }
}

// ---- The app's zone --------------------------------------------------------------------------------------------------

/// The zone the app writes times in, and when the platform last read its setting.
struct LocalTime {
    zone: Zone,
    /// When [`TimeZone::refresh`] last ran, by the ticker's wall clock (the epoch for a fixed zone, which never reads
    /// anything). A `Cell`, so [`zone`] can refresh from a draw, which holds the app only by shared reference.
    refreshed_at: Cell<SystemTime>,
}

impl Global for LocalTime {}

/// Make `zone` the app's, last read at `refreshed_at`: what [`install`], [`follow_platform`] and [`pin`] share.
fn set(zone: Zone, refreshed_at: SystemTime, cx: &mut App) {
    cx.set_global(LocalTime {
        zone,
        refreshed_at: Cell::new(refreshed_at),
    });
}

/// Install the app's zone as UTC. The bootstrap calls this with the other app-wide state; [`follow_platform`] then
/// replaces it with the host's zone when there is one.
pub(crate) fn install(cx: &mut App) {
    set(Zone::UTC, UNIX_EPOCH, cx);
}

/// Write every time in the platform's zone from now on, reading its setting now. The bootstrap calls this with the
/// host's [`TimeZone`]. `'static` because the zone lives as long as the app does and is copied into every [`Zone`] a
/// draw hands out.
pub(crate) fn follow_platform(platform: &'static dyn TimeZone, cx: &mut App) {
    platform.refresh();
    let now = ticker::wall_now(cx);
    set(Zone(Kind::Platform(platform)), now, cx);
}

/// Write every time `seconds_east` of UTC, all year round, for tests that show a display's local time.
#[cfg(test)]
pub(crate) fn pin(seconds_east: i32, cx: &mut App) {
    set(Zone::fixed(seconds_east), UNIX_EPOCH, cx);
}

/// The zone to write times in for this draw. Reading it is cheap. When the app follows the platform and its last
/// reading is [`REFRESH_PERIOD`] old by the ticker's wall clock (or the clock went back since), the platform reads its
/// setting again first, so a draw at most once a minute pays for that read and no other draw does.
pub(crate) fn zone(cx: &App) -> Zone {
    let Some(state) = installed::<LocalTime>(cx) else {
        return Zone::UTC;
    };
    if let Kind::Platform(platform) = state.zone.0 {
        let now = ticker::wall_now(cx);
        let due = now
            .duration_since(state.refreshed_at.get())
            .map_or(true, |since| since >= REFRESH_PERIOD);
        if due {
            platform.refresh();
            state.refreshed_at.set(now);
        }
    }
    state.zone
}

// ---- Clock text ------------------------------------------------------------------------------------------------------

/// An instant's calendar date and time of day in a zone, which the public forms below write out.
struct Civil {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

/// The instant `seconds` since the epoch as a date and time of day in `zone`. Times before 1970 land on their own day.
fn civil(seconds: i64, zone: Zone) -> Civil {
    const SECS_PER_DAY: i64 = 86_400;
    // Saturating, so a nonsense stamp near the ends of `i64` still gives some date rather than overflowing.
    let local = seconds.saturating_add(zone.offset_at(seconds));
    // Euclidean division, so a time before 1970 lands on its own day rather than reflecting around the epoch.
    let (year, month, day) = farhelm_proto::time::civil_from_days(local.div_euclid(SECS_PER_DAY));
    let of_day = local.rem_euclid(SECS_PER_DAY) as u32;
    Civil {
        year,
        month,
        day,
        hour: of_day / 3_600,
        minute: of_day % 3_600 / 60,
        second: of_day % 60,
    }
}

/// `YYYY-MM-DD HH:MM:SS` in `zone`: the app's stamp, to the second (a session's last activity, a host's suspension).
pub(crate) fn stamp(seconds: i64, zone: Zone) -> String {
    let at = civil(seconds, zone);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        at.year, at.month, at.day, at.hour, at.minute, at.second
    )
}

/// `YYYY-MM-DD HH:MM` in `zone`: the stamp to the minute, where the old app showed no seconds (a snapshot's build, an
/// archived checkout).
pub(crate) fn minute(seconds: i64, zone: Zone) -> String {
    format!("{} {}", date(seconds, zone), clock(seconds, zone))
}

/// `YYYY-MM-DD` in `zone`, the date alone, for a display that sets the time apart (the credentials list's
/// `2026-11-14, 09:05`).
pub(crate) fn date(seconds: i64, zone: Zone) -> String {
    let at = civil(seconds, zone);
    format!("{:04}-{:02}-{:02}", at.year, at.month, at.day)
}

/// `HH:MM` in `zone`, with no date: a time of today, such as when a list was last checked.
pub(crate) fn clock(seconds: i64, zone: Zone) -> String {
    let at = civil(seconds, zone);
    format!("{:02}:{:02}", at.hour, at.minute)
}
