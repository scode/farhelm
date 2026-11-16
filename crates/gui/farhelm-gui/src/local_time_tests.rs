//! Tests of local time: the clock text a fixed offset gives (across midnight both ways, before 1970, on leap days), a
//! platform zone's per-instant offsets and the answers it falls back from, and when the app asks the platform to read
//! its time zone setting again (once when it starts following it, then at most once a minute, from a draw).
//!
//! No test reads the machine's own time zone: the formatting tests take a [`Zone`] directly, and the app tests hand the
//! app a stand-in platform whose answers the test sets.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{IntoElement as _, ParentElement as _, TestAppContext, div};

use crate::local_time::{self, REFRESH_PERIOD, TimeZone, Zone, clock, date, minute, stamp};
use crate::test_support;

/// 2023-11-14 22:13:20 UTC, an instant less than two hours before midnight in UTC.
const NEAR_MIDNIGHT: i64 = 1_700_000_000;

/// One hour, in seconds east of UTC.
const HOUR: i32 = 3_600;

// ---- Clock text ------------------------------------------------------------------------------------------------------

/// Why this matters: every clock time the app shows goes through these functions, and an offset applied the wrong way
/// round, or one that forgets to move the date when it crosses midnight, misdates every stamp at once. Specified: a
/// zone east of UTC moves 22:13:20 two hours on, into the next day; one west of UTC moves an early-morning time back
/// into the previous day; UTC leaves the instant as it is.
#[test]
fn an_offset_moves_the_time_and_the_date_across_midnight_both_ways() {
    assert_eq!(stamp(NEAR_MIDNIGHT, Zone::UTC), "2023-11-14 22:13:20");
    assert_eq!(
        stamp(NEAR_MIDNIGHT, Zone::fixed(2 * HOUR)),
        "2023-11-15 00:13:20"
    );
    // 2023-11-15 03:00:00 UTC is still the 14th in New York's winter time.
    assert_eq!(
        stamp(1_700_017_200, Zone::fixed(-5 * HOUR)),
        "2023-11-14 22:00:00"
    );
    // A half-hour zone moves the minutes too.
    assert_eq!(
        stamp(NEAR_MIDNIGHT, Zone::fixed(5 * HOUR + 1_800)),
        "2023-11-15 03:43:20"
    );
}

/// Why this matters: the helm sends whatever a host recorded, and the old UTC helpers wrote a time before 1970 as it was
/// rather than refusing it or wrapping it around the epoch; local time must not lose that. Specified: one second before
/// the epoch is 1969-12-31 23:59:59 in UTC, the epoch itself is still 1969 an hour west of UTC, and one second before
/// the epoch an hour east of UTC is 1970-01-01 00:59:59.
#[test]
fn times_before_1970_land_on_their_own_day() {
    assert_eq!(stamp(-1, Zone::UTC), "1969-12-31 23:59:59");
    assert_eq!(stamp(0, Zone::fixed(-HOUR)), "1969-12-31 23:00:00");
    assert_eq!(stamp(-1, Zone::fixed(HOUR)), "1970-01-01 00:59:59");
}

/// Why this matters: an offset that pushes a time across midnight on the last day of February lands on a leap day or
/// skips it, and the calendar arithmetic underneath must get both right in local time as it did in UTC. Specified: an
/// hour east of 2024-02-28 23:30 UTC is 2024-02-29; an hour east of 2023-02-28 23:30 UTC is 2023-03-01; and 2100, which
/// is not a leap year, goes from February 28 to March 1.
#[test]
fn an_offset_across_the_end_of_february_lands_on_leap_days_only_in_leap_years() {
    assert_eq!(minute(1_709_163_000, Zone::fixed(HOUR)), "2024-02-29 00:30");
    assert_eq!(minute(1_677_627_000, Zone::fixed(HOUR)), "2023-03-01 00:30");
    assert_eq!(minute(4_107_540_600, Zone::fixed(HOUR)), "2100-03-01 00:30");
}

/// Why this matters: the app's displays pick one of four forms (to the second, to the minute, the date alone, or hours
/// and minutes alone), and each must zero-pad so a column of times lines up. Specified: the same instant two hours east
/// of UTC reads `2023-11-15 00:13:20`, `2023-11-15 00:13`, `2023-11-15` and `00:13`.
#[test]
fn the_four_forms_pad_every_part() {
    let zone = Zone::fixed(2 * HOUR);
    assert_eq!(stamp(NEAR_MIDNIGHT, zone), "2023-11-15 00:13:20");
    assert_eq!(minute(NEAR_MIDNIGHT, zone), "2023-11-15 00:13");
    assert_eq!(date(NEAR_MIDNIGHT, zone), "2023-11-15");
    assert_eq!(clock(NEAR_MIDNIGHT, zone), "00:13");
}

// ---- The platform's zone ---------------------------------------------------------------------------------------------

/// A stand-in for the host's time zone, for every test of the host's seam: it answers `current` (`None` when it cannot
/// say) until a refresh takes up `pending`, as the platform answers from the setting it last read, and counts its
/// refreshes. Leaked, since the app keeps a zone for its whole life.
pub(crate) struct StandIn {
    pub(crate) pending: Cell<Option<i32>>,
    current: Cell<Option<i32>>,
    pub(crate) refreshes: Cell<u32>,
}

impl TimeZone for StandIn {
    fn refresh(&self) {
        self.current.set(self.pending.get());
        self.refreshes.set(self.refreshes.get() + 1);
    }

    fn offset_at(&self, _: i64) -> Option<i32> {
        self.current.get()
    }
}

/// A [`StandIn`] whose setting is `offset` from the start, for the app's whole life.
pub(crate) fn stand_in(offset: Option<i32>) -> &'static StandIn {
    Box::leak(Box::new(StandIn {
        pending: Cell::new(offset),
        current: Cell::new(None),
        refreshes: Cell::new(0),
    }))
}

/// A zone with summer time from `SUMMER` on: an hour east of UTC before it, two hours after. The one stand-in whose
/// answer depends on the instant.
struct Seasons;

/// When [`Seasons`] changes to summer time: 2024-03-31 01:00 UTC, when central Europe's clocks go forward.
const SUMMER: i64 = 1_711_846_800;

impl TimeZone for Seasons {
    fn refresh(&self) {}

    fn offset_at(&self, seconds: i64) -> Option<i32> {
        Some(if seconds < SUMMER { HOUR } else { 2 * HOUR })
    }
}

/// The zone the app hands a draw after following `platform`, for a test that only formats with it.
fn followed(platform: &'static dyn TimeZone, cx: &mut TestAppContext) -> Zone {
    test_support::install(cx);
    cx.update(|cx| {
        local_time::follow_platform(platform, cx);
        local_time::zone(cx)
    })
}

/// Why this matters: one offset applied to every stamp would put every time from the other side of a daylight saving
/// change an hour off (a credential saved in summer, looked at in winter); the platform's zone answers per instant, as
/// a browser's or `date`'s does. Specified: a second before the change reads in winter time and the change itself in
/// summer time, so 01:59:59 is followed by 03:00:00.
#[gpui::test]
fn each_instant_takes_its_own_offset_from_the_platform(cx: &mut TestAppContext) {
    static SEASONS: Seasons = Seasons;
    let zone = followed(&SEASONS, cx);
    assert_eq!(stamp(SUMMER - 1, zone), "2024-03-31 01:59:59");
    assert_eq!(stamp(SUMMER, zone), "2024-03-31 03:00:00");
}

/// Why this matters: a time is still worth drawing when the platform cannot place it, and an absurd offset from a
/// broken platform must not wrap the hour around or shift the date by days. Specified: a platform answering `None`, or
/// 24 hours east or west, gives the UTC time; one answering just under a day still applies it.
#[gpui::test]
fn a_platform_that_cannot_say_counts_as_utc(cx: &mut TestAppContext) {
    for (answer, expected) in [
        (None, "2023-11-14 22:13:20"),
        (Some(86_400), "2023-11-14 22:13:20"),
        (Some(-86_400), "2023-11-14 22:13:20"),
        (Some(86_399), "2023-11-15 22:13:19"),
    ] {
        let zone = followed(stand_in(answer), cx);
        assert_eq!(stamp(NEAR_MIDNIGHT, zone), expected, "{answer:?}");
    }
}

// ---- Reading the setting again ---------------------------------------------------------------------------------------

/// Why this matters: tests and the goldens run without a host (the goldens pin nothing), and their times must not
/// depend on the machine they run on; and a test that pins an offset must see exactly that offset. Specified: the
/// bootstrap alone writes times in UTC, and a pinned offset replaces it.
#[gpui::test]
fn without_the_hosts_zone_times_are_utc_until_a_test_pins_one(cx: &mut TestAppContext) {
    test_support::install(cx);
    let utc = cx.update(|cx| stamp(NEAR_MIDNIGHT, local_time::zone(cx)));
    assert_eq!(utc, "2023-11-14 22:13:20");
    let pinned = cx.update(|cx| {
        local_time::pin(-5 * HOUR, cx);
        stamp(NEAR_MIDNIGHT, local_time::zone(cx))
    });
    assert_eq!(pinned, "2023-11-14 17:13:20");
}

/// Why this matters: a zone changed while the app runs (travel, or the user picking another one) must show without a
/// restart, but reading the setting can read a file, so it must not happen on every frame, and the migration's
/// orchestrator decided on 2026-11-16 that it must not start a timer that wakes an idle app either. Specified: following
/// the platform reads its setting once; a window drawn again and again within [`REFRESH_PERIOD`] reads it no more, and
/// keeps showing the old offset after the setting changed; the first draw once the period has passed (and not one a
/// second before) reads it again and shows the new offset; and letting time pass with nothing drawn reads nothing.
#[gpui::test]
fn the_setting_is_read_again_on_the_first_draw_a_minute_on(cx: &mut TestAppContext) {
    let platform = stand_in(Some(HOUR));
    let shown = Rc::new(RefCell::new(Vec::new()));
    let drawn = shown.clone();
    // `open` bootstraps the app, which installs UTC, so the platform is followed once the window is up.
    let cx = test_support::open(cx, move |_, cx| {
        let text = stamp(NEAR_MIDNIGHT, local_time::zone(cx));
        drawn.borrow_mut().push(text.clone());
        div().child(text).into_any_element()
    });
    cx.update(|_, cx| local_time::follow_platform(platform, cx));
    assert_eq!(platform.refreshes.get(), 1, "read once when followed");
    test_support::redraw(cx);
    let last = |shown: &Rc<RefCell<Vec<String>>>| shown.borrow().last().cloned();
    assert_eq!(last(&shown).as_deref(), Some("2023-11-14 23:13:20"));

    platform.pending.set(Some(3 * HOUR));
    for _ in 0..5 {
        test_support::redraw(cx);
    }
    test_support::advance(cx, REFRESH_PERIOD - Duration::from_secs(1));
    test_support::redraw(cx);
    assert_eq!(
        platform.refreshes.get(),
        1,
        "no draw within the period reads the setting"
    );
    assert_eq!(
        last(&shown).as_deref(),
        Some("2023-11-14 23:13:20"),
        "the old offset holds until the setting is read again"
    );

    let draws = shown.borrow().len();
    test_support::advance(cx, Duration::from_secs(1));
    assert_eq!(
        (platform.refreshes.get(), shown.borrow().len()),
        (1, draws),
        "time passing with nothing drawn reads nothing"
    );
    test_support::redraw(cx);
    assert_eq!(
        platform.refreshes.get(),
        2,
        "the first draw a minute on reads it"
    );
    assert_eq!(last(&shown).as_deref(), Some("2023-11-15 01:13:20"));
}
