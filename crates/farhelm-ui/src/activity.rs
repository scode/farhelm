//! How OLD a session's last-known activity is, in the two forms the chrome
//! shows it: a short relative age beside the status ("2m", "3h") and the
//! full absolute stamp behind it on hover.
//!
//! ## Why the age is shown at all
//!
//! The list's default order is "recently active" (SPEC.md's Session list),
//! and until this existed that order was invisible: rows sorted by a number
//! nothing on screen printed, so a fleet where everything had been quiet for
//! a day looked exactly like one where three agents were working. The age is
//! what makes the ordering legible — and it is also what the status dot
//! deliberately stopped saying, since a dot can carry a state but not a
//! duration.
//!
//! ## Which clock this is measured against
//!
//! The client's own wall clock, against a stamp the SESSION'S HOST wrote.
//! Those are two different machines whenever the helm is remote, and in a
//! multi-host fleet the stamps in one list come from several hosts at once —
//! so this difference is only as accurate as the clocks behind it. Nothing
//! here tries to correct for that, deliberately: correcting to the helm's
//! clock (the one reference a client could obtain, from an HTTP `Date`
//! header) would fix at most one of the N edges involved and would lend the
//! rest a precision they do not have. What the code does instead is refuse
//! to print nonsense — a stamp in the future reads as `now` rather than as a
//! negative age — and keep the raw stamp one hover away, so a reader who
//! doubts the number can see what it was derived from.
//!
//! The viewer's half of that subtraction can also be MISSING — a platform
//! that will not answer, or a clock sitting at or before the epoch — and
//! that is carried as an absent value rather than as a zero. The
//! distinction is not pedantic: a zero would subtract into a clamped `now`
//! on every row at once, which is a fleet-wide lie no reader could tell
//! from the truth.
//!
//! Note that `farhelm-ui`'s `skew` module is about a different skew
//! entirely: which BUILD the helm is running, not what time it thinks it is.
//!
//! ## The tick
//!
//! [`ACTIVITY_NOW`] is the coarse "now" every age on screen is computed
//! against, republished by [`ActivityClock`] every [`TICK_MS`]. It is a
//! signal rather than a per-render `Date.now()` because a render is not
//! scheduled by time passing: without a tick, a row that says `2m` says it
//! until something unrelated re-renders it, which on a quiet fleet can be a
//! very long time.

use dioxus::prelude::*;

use crate::reader::sleep_ms;

/// How often the shared "now" is republished.
///
/// Thirty seconds, which buys honesty at the MINUTE-and-larger buckets: the
/// finest boundary from `1m` upwards is a minute wide, so a page is at most
/// half a bucket behind there. The sub-minute boundaries are explicitly NOT
/// covered by that claim — `now` ends at ten seconds, but a stamp that
/// lands just after a tick keeps reading `now` until the next one, so the
/// word can survive to nearly forty seconds. That lag is accepted rather
/// than fixed: the alternative is a faster tick paid by every page all the
/// time to sharpen the one bucket whose exact width nobody acts on.
///
/// The fallback listing poll (`api::POLL_INTERVAL_MS`, 3s) was the obvious
/// thing to piggyback on and is the wrong one: it runs only while the
/// invalidation feed is DOWN, so on a healthy page — the normal case — it
/// never fires at all, and when it does it fires ten times more often than
/// this needs.
///
/// What a tick actually costs is the two components that READ the clock:
/// `ListView` re-renders and recomputes each row's `RowState`, and an open
/// `SessionView` re-renders its header. The rows themselves are memoized on
/// that state, so a row whose formatted age has not moved is diffed and not
/// re-rendered — which is exactly why the age is formatted by the list
/// rather than by the row (see [`ActivityStamp`]).
const TICK_MS: u64 = 30_000;

/// The current wall-clock second, as everything rendering an age reads it,
/// or `None` while the viewer's own clock cannot be trusted to answer.
///
/// A global rather than state threaded from the root for `skew`'s reason:
/// the writers and readers are in different subtrees (`ActivityClock` at the
/// root, the session rows and the open session's header at two leaves), and
/// threading a signal through every intermediate component to reach them
/// would put the plumbing everywhere and the meaning nowhere.
///
/// Reading it SUBSCRIBES the reading scope, which is the point: that is what
/// turns a tick into a re-render of exactly the components that print an age.
pub(crate) static ACTIVITY_NOW: GlobalSignal<Option<i64>> = Signal::global(client_now_secs);

/// Republishes [`ACTIVITY_NOW`] forever. Renders nothing.
///
/// Mounted at PAGE level (beside `feed::FleetFeed`) rather than inside
/// either pane. The sidebar's `ListView` would in fact survive a selection
/// change — it stays mounted — but the keyed `SessionView` does not, and
/// putting the clock in either place would mean one surface owning a fact
/// the other also needs. Page ownership gives both readers the same clock
/// and keeps it running across every right-pane remount.
#[component]
pub(crate) fn ActivityClock() -> Element {
    // Publish once, synchronously, before the loop below has even been
    // spawned — because [`ACTIVITY_NOW`] outlives THIS component. The
    // global is initialized once per page and keeps whatever value it last
    // held, while this clock is unmounted and remounted whenever the
    // authenticated tree is rebuilt (`auth`'s credential exchange remounts
    // it). Without this line the page would spend its first tick's worth of
    // time — thirty seconds, and after a long-idle reauthentication a value
    // that could be hours stale — showing ages measured against the clock
    // reading from before the unmount.
    use_hook(|| {
        *ACTIVITY_NOW.write() = client_now_secs();
    });
    use_future(|| async {
        loop {
            sleep_ms(TICK_MS).await;
            // Written unconditionally rather than compared first. The point
            // is not that the reading must have changed — a wall clock can
            // be stepped backwards or corrected onto the same second — but
            // that every tick republishes the LATEST reading, corrections
            // included, which is the only thing the ages on screen are
            // entitled to be measured against.
            *ACTIVITY_NOW.write() = client_now_secs();
        }
    });
    rsx! {}
}

/// Seconds since the Unix epoch on whatever machine this UI is rendering on,
/// or `None` when that machine has no usable answer.
///
/// A `cfg` pair because `std::time::SystemTime` is not usable on
/// wasm32-unknown-unknown, and the browser's own answer is `Date.now()`.
/// Deliberately the WALL clock, not a monotonic one — the value it is
/// compared against is an absolute epoch stamp, so a monotonic reading
/// would have nothing to subtract from.
///
/// Anything at or below zero is `None`, not a number to subtract from. The
/// platform failing to answer and a viewer clock sitting at or before the
/// epoch are the same situation for this module's purposes, and both must
/// be distinguished from a real reading: subtracting a valid host stamp
/// from a zero "now" clamps to `now` (see [`relative_age`]), which would
/// paint every session in the fleet as having just been active.
fn client_now_secs() -> Option<i64> {
    #[cfg(not(target_arch = "wasm32"))]
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64);
    #[cfg(target_arch = "wasm32")]
    // Milliseconds, and possibly fractional; `as i64` truncates toward
    // zero, which for a positive epoch is the floor we want.
    let now = (web_sys::js_sys::Date::now() / 1000.0) as i64;
    (now > 0).then_some(now)
}

/// One session's last-known activity, ready to render: the short age the row
/// prints and the absolute stamp its tooltip carries.
///
/// Built by the LIST rather than by each row (see `list::shared::RowState`),
/// which is what keeps the 30-second tick from re-rendering a row whose
/// displayed age has not actually changed — an `8h` row survives sixty ticks
/// with an equal `RowState` and no render at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivityStamp {
    /// The relative age, as one unpadded token: `now`, `<1m`, `59m`, `23h`,
    /// `400d`. Deliberately UNBOUNDED in length (see [`relative_age`]) — a
    /// long-quiet session reads `1000d` rather than being capped, so nothing
    /// downstream may size itself on an assumed maximum width.
    pub(crate) age: String,
    /// The same instant spelled out in full, for the `title` attribute.
    /// Always present when `age` is: an abbreviation is never the only place
    /// a value is recorded (SPEC.md's Session list rule, which the row
    /// already applies to its directory and invocation).
    pub(crate) absolute: String,
}

impl ActivityStamp {
    /// `None` whenever EITHER end of the subtraction is unavailable, and
    /// callers then render no element at all — the same "say nothing rather
    /// than guess" rule `status_badge` applies to an unclassified status.
    ///
    /// The two ends fail for unrelated reasons and both have to be caught
    /// here. A nonpositive `activity_secs` is a helm predating the field
    /// rather than a session last active in 1970 (see
    /// `Session::effective_activity`). A `None` `now_secs` is the VIEWER's
    /// own clock declining to answer ([`client_now_secs`]) — and that one is
    /// the more dangerous of the two, because a zero passed through as if it
    /// were a reading would clamp every age in the fleet to `now` and paint
    /// a dormant fleet as a busy one.
    pub(crate) fn new(now_secs: Option<i64>, activity_secs: i64) -> Option<Self> {
        let now_secs = now_secs?;
        if activity_secs <= 0 {
            return None;
        }
        Some(ActivityStamp {
            age: relative_age(now_secs, activity_secs),
            absolute: absolute_stamp(activity_secs),
        })
    }
}

/// How long ago `activity_secs` was, in one bucket's worth of characters.
///
/// The buckets are deliberately coarse and deliberately unpadded — `now`,
/// `<1m`, `Nm`, `Nh`, `Nd` — because this sits in a dense sidebar row where
/// every character competes with the session's title. Rounding is always
/// DOWN (integer division), so the number never claims more elapsed time
/// than has actually passed.
///
/// Two edges are worth stating outright:
///
/// - A stamp in the FUTURE reads as `now`. The clocks behind the two
///   operands are not synchronized (see the module docs), and a negative
///   age is a statement about clock skew that no user can act on.
/// - `now` covers the first ten seconds rather than only the exact instant,
///   because the tick is 30 seconds wide: with a narrower window the word
///   would frequently never be seen at all.
///
/// There is no upper bound. A session last active 400 days ago reads `400d`,
/// which is wide but true; capping it would need a "more than" marker to stay
/// honest, and a fleet with such rows has a bigger problem than a wide badge.
fn relative_age(now_secs: i64, activity_secs: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;

    let elapsed = (now_secs - activity_secs).max(0);
    if elapsed < 10 {
        "now".to_string()
    } else if elapsed < MINUTE {
        "<1m".to_string()
    } else if elapsed < HOUR {
        format!("{}m", elapsed / MINUTE)
    } else if elapsed < DAY {
        format!("{}h", elapsed / HOUR)
    } else {
        format!("{}d", elapsed / DAY)
    }
}

/// The same instant as an unambiguous absolute stamp, for the tooltip.
///
/// UTC, and it says so, rather than the viewer's local time. Local rendering
/// would mean either a `cfg` pair (the browser's `toLocaleString` on one
/// side, a date crate on the other) or a timezone database this crate does
/// not carry — and the tooltip's job is to be the unambiguous record behind
/// an abbreviation, which a zone-qualified UTC stamp does better than a
/// local time whose offset is not printed.
fn absolute_stamp(secs: i64) -> String {
    const SECS_PER_DAY: i64 = 86_400;
    // Euclidean rather than truncating division so a pre-1970 stamp still
    // lands on the right day. Nothing should produce one — `ActivityStamp`
    // rejects everything at or below zero — but a formatter that silently
    // reflects around the epoch is a trap for whoever calls this next.
    let (year, month, day) = civil_from_days(secs.div_euclid(SECS_PER_DAY));
    let seconds_of_day = secs.rem_euclid(SECS_PER_DAY);
    let (hour, minute, second) = (
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60,
    );
    format!("last activity {year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

/// Days since the Unix epoch to a proleptic Gregorian `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days`, which is the standard closed-form
/// inverse of the day count and is what a date crate would run here. Carried
/// as ten lines rather than as a dependency because this is the only calendar
/// arithmetic in the whole UI, and pulling `chrono`/`time` into the wasm
/// bundle to format one tooltip is not a trade worth making.
///
/// Two of the magic numbers have a plain meaning: 719468 shifts the epoch to
/// 0000-03-01 (March-first years make the leap day the LAST day, so no month
/// length depends on it), and 146097 is the number of days in a 400-year
/// Gregorian era. The 1460 / 36524 / 146096 divisors in the year-of-era line
/// do NOT: they are that formula's leap-day corrections over a zero-based
/// day-of-era count, and reading them as cycle lengths is wrong (a Gregorian
/// four-year cycle is 1461 days, not 1460). Copy them from Hinnant rather
/// than re-deriving them from a calendar.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // [0, 399]
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let month_prime = (5 * day_of_year + 2) / 153; // [0, 11], March = 0
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1; // [1, 31]
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    // January and February belong to the NEXT calendar year in the
    // March-first numbering the algorithm works in.
    (
        if month <= 2 { year + 1 } else { year },
        month as u32,
        day as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins every bucket boundary, because the boundaries ARE the design:
    /// the whole value of the age is that a glance separates "this agent is
    /// working" from "this has been quiet since yesterday", and a bucket that
    /// silently widened (or an off-by-one that made `60s` read `0m`) would
    /// degrade that without breaking anything loudly.
    ///
    /// Each case is expressed as an elapsed duration from a fixed `now` so
    /// the test reads as the specification it is rather than as arithmetic.
    #[test]
    fn each_relative_bucket_starts_and_ends_where_it_says() {
        let now = 1_700_000_000;
        let age = |elapsed: i64| relative_age(now, now - elapsed);

        assert_eq!(age(0), "now");
        assert_eq!(age(9), "now");
        assert_eq!(age(10), "<1m");
        assert_eq!(age(59), "<1m");
        assert_eq!(age(60), "1m");
        assert_eq!(age(119), "1m");
        assert_eq!(age(59 * 60 + 59), "59m");
        assert_eq!(age(3_600), "1h");
        assert_eq!(age(23 * 3_600 + 59 * 60), "23h");
        assert_eq!(age(86_400), "1d");
        assert_eq!(age(400 * 86_400), "400d", "nothing caps the day count");
        assert_eq!(
            age(1_000 * 86_400),
            "1000d",
            "and nothing caps its WIDTH either — four characters is not a bound this \
             formatter promises, so no layout downstream may assume one"
        );
    }

    /// A stamp from the future must read as `now`, never as a negative age.
    ///
    /// This is not a hypothetical: the stamp is written by the session's HOST
    /// and the "now" is read from the VIEWER's machine, so any clock
    /// disagreement in that direction lands here. The failure it prevents is
    /// an age like `-3m`, which tells a user nothing they can act on and
    /// reads as a rendering bug rather than as the clock disagreement it is.
    ///
    /// The tooltip half is asserted too, and it is the half that keeps the
    /// clamp honest: the RAW future instant must still be recoverable on
    /// hover. Rounding a skewed stamp down to `now` in the abbreviation
    /// while also rewriting the record behind it would erase the only
    /// evidence a reader has that the two machines disagree — the same
    /// "an abbreviation is never the only place a value is recorded" rule
    /// SPEC.md applies to the row's directory and invocation.
    #[test]
    fn a_stamp_from_the_future_reads_as_now_but_keeps_its_real_instant() {
        let now = 1_700_000_000;
        assert_eq!(relative_age(now, now + 5), "now");
        assert_eq!(relative_age(now, now + 86_400), "now");

        let skewed = ActivityStamp::new(Some(now), now + 86_400)
            .expect("a future stamp is still a positive stamp");
        assert_eq!(skewed.age, "now");
        assert_eq!(
            skewed.absolute, "last activity 2023-11-15 22:13:20 UTC",
            "the tooltip records the stamp as it arrived — a day AHEAD of the viewer's now"
        );
    }

    /// A session with no activity stamp gets no element at all, rather than
    /// an age computed from a zero that means "this helm predates the field"
    /// (`Session::effective_activity`). Rendering it would put `56y` beside a
    /// perfectly healthy session.
    #[test]
    fn a_missing_stamp_produces_no_activity_at_all() {
        assert_eq!(ActivityStamp::new(Some(1_700_000_000), 0), None);
        assert_eq!(ActivityStamp::new(Some(1_700_000_000), -1), None);
    }

    /// A viewer clock that cannot answer suppresses the age entirely, on
    /// every session, however good the host's own stamp is.
    ///
    /// This is the failure mode that looks like success: before the clock
    /// was an `Option`, an unavailable reading arrived as `0`, and
    /// `relative_age` clamps a negative elapsed time to zero — so every
    /// session in the fleet would have rendered `now`, and a dormant fleet
    /// would have read as one where everything had just been touched. There
    /// is no visual cue that could distinguish that from the truth, which is
    /// why it has to be refused at the source.
    #[test]
    fn an_unavailable_viewer_clock_renders_no_age_at_all() {
        assert_eq!(ActivityStamp::new(None, 1_700_000_000), None);
        assert_eq!(
            ActivityStamp::new(None, 0),
            None,
            "neither end available is still nothing to render"
        );
    }

    /// The tooltip must name a real instant, complete with the zone it is
    /// expressed in — it exists to be the unambiguous record behind a
    /// three-character abbreviation, and a bare `2026-08-23 14:05` would
    /// leave the reader guessing which machine's idea of the day that is.
    ///
    /// The leap-year and month-boundary cases are here because the calendar
    /// arithmetic is hand-carried (see `civil_from_days`): a date crate would
    /// come with its own tests, and this does not.
    ///
    /// The cases are chosen at the three places the Gregorian leap rule
    /// changes its answer — every-4, not-every-100, but-yes-every-400 — plus
    /// the March-first hinge the algorithm's own year numbering turns on and
    /// the epoch itself. Each expected string is an independent UTC golden
    /// (`date -u -d @N`), not a value re-derived from the same formula the
    /// test is checking.
    #[test]
    fn the_absolute_stamp_names_the_instant_and_its_zone() {
        assert_eq!(
            absolute_stamp(0),
            "last activity 1970-01-01 00:00:00 UTC",
            "the epoch itself, which is what every off-by-one in the day count moves"
        );
        assert_eq!(
            absolute_stamp(1_700_000_000),
            "last activity 2023-11-14 22:13:20 UTC"
        );
        assert_eq!(
            absolute_stamp(1_709_164_800),
            "last activity 2024-02-29 00:00:00 UTC",
            "a leap day the March-first year numbering has to place correctly"
        );
        assert_eq!(
            absolute_stamp(1_709_251_199),
            "last activity 2024-02-29 23:59:59 UTC",
            "the last second before a month rolls over"
        );
        assert_eq!(
            absolute_stamp(1_709_251_200),
            "last activity 2024-03-01 00:00:00 UTC",
            "and the first second of the hinge month the whole algorithm counts from"
        );
        assert_eq!(
            absolute_stamp(951_782_400),
            "last activity 2000-02-29 00:00:00 UTC",
            "the divisible-by-400 exception: 2000 IS a leap year, and a formula \
             that only implemented the century rule would land a day off here"
        );
        assert_eq!(
            absolute_stamp(4_107_542_399),
            "last activity 2100-02-28 23:59:59 UTC",
            "the century rule proper: 2100 is NOT a leap year, so February ends here"
        );
        assert_eq!(
            absolute_stamp(4_107_542_400),
            "last activity 2100-03-01 00:00:00 UTC",
            "and March follows it directly, with no 29th in between"
        );
    }

    /// A pre-epoch second still names a pre-epoch instant, rather than
    /// reflecting around 1970 into some day in January.
    ///
    /// Nothing in the product reaches this — `ActivityStamp::new` refuses
    /// every stamp at or below zero — but `absolute_stamp` is a total
    /// function over `i64` and its Euclidean division exists precisely to
    /// hold here. A truncating `/` and `%` would put this second on
    /// 1970-01-01 at a negative time-of-day, which is the trap the next
    /// caller of this formatter would inherit silently.
    #[test]
    fn a_pre_epoch_second_stays_before_the_epoch() {
        assert_eq!(
            absolute_stamp(-1),
            "last activity 1969-12-31 23:59:59 UTC",
            "one second before the epoch is the last second of 1969"
        );
    }

    /// Both halves are built together or not at all, so a row can never show
    /// an age whose tooltip is missing — the abbreviation and the record
    /// behind it are one decision, not two.
    #[test]
    fn a_present_stamp_carries_both_the_age_and_the_record() {
        let stamp = ActivityStamp::new(Some(1_700_000_000), 1_700_000_000 - 7_200)
            .expect("a positive stamp always renders");
        assert_eq!(stamp.age, "2h");
        assert_eq!(stamp.absolute, "last activity 2023-11-14 20:13:20 UTC");
    }

    /// A clock that is MOUNTED must publish immediately, not one tick later.
    ///
    /// [`ACTIVITY_NOW`] is page-global and outlives [`ActivityClock`], which
    /// is unmounted and remounted whenever the authenticated tree is rebuilt
    /// (`auth`). A clock that only wrote after its first sleep would leave
    /// the whole page rendering ages against the reading from BEFORE that
    /// gap — thirty seconds at best, and after a reauthentication that
    /// followed a long idle, a value wrong by hours with nothing on screen
    /// admitting it.
    ///
    /// The stale sentinel below stands in for exactly that leftover value,
    /// which is what makes this a remount test rather than a first-mount
    /// one: it is written into the global BEFORE the clock mounts, the way
    /// a previous mount would have left it.
    #[test]
    fn a_mounted_clock_publishes_now_before_its_first_sleep() {
        /// Far enough in the past that no real reading could be mistaken
        /// for it: 2001-09-09, the nine-digit-epoch second.
        const LEFTOVER: i64 = 1_000_000_000;

        let mut dom = VirtualDom::new(|| rsx! { ActivityClock {} });
        dom.in_runtime(|| {
            *ACTIVITY_NOW.write() = Some(LEFTOVER);
        });

        let before_mount = client_now_secs().expect("this machine's clock answers");
        dom.rebuild_in_place();

        let published = dom
            .in_runtime(|| *ACTIVITY_NOW.read())
            .expect("a mounted clock publishes a reading, not None");
        assert!(
            published >= before_mount,
            "the mount must republish the CURRENT second, not keep the {LEFTOVER} \
             a previous mount left behind (got {published})"
        );
    }

    /// A tick advances what is on screen with nothing else happening on the
    /// page — no feed notice, no listing poll, no user input.
    ///
    /// This is the whole reason [`ACTIVITY_NOW`] is a signal rather than a
    /// per-render `Date.now()`: a render is not scheduled by time passing,
    /// so on a quiet fleet a row that said `59m` would keep saying it until
    /// something unrelated re-rendered it. The chain being pinned here is
    /// read-subscribes → write-dirties → re-render → new bucket; a clock
    /// that was never written, or a reader that did not subscribe, breaks it
    /// silently and looks fine in a screenshot.
    ///
    /// [`ACTIVITY_NOW`] is driven directly instead of by waiting out
    /// [`TICK_MS`]: the cadence belongs to the loop and is asserted nowhere
    /// but there, while what this test is about is what a published value
    /// does once it lands.
    #[test]
    fn a_tick_advances_a_rendered_age_with_nothing_else_happening() {
        use std::cell::RefCell;

        /// When the session under test was last active. Fixed, so every
        /// change in the rendered age below comes from the clock alone.
        const ACTIVE_AT: i64 = 1_700_000_000;

        thread_local! {
            /// Every age this probe has rendered, oldest first — the record
            /// the assertions read. Thread-local rather than a signal so it
            /// records renders WITHOUT participating in the reactivity being
            /// measured.
            static RENDERED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
        }

        /// Stands in for a session row: reads the shared clock, formats one
        /// session's age against it, and records what it drew.
        fn probe() -> Element {
            let age = ActivityStamp::new(*ACTIVITY_NOW.read(), ACTIVE_AT)
                .map(|stamp| stamp.age)
                .unwrap_or_default();
            RENDERED.with(|rendered| rendered.borrow_mut().push(age));
            rsx! {}
        }

        let mut dom = VirtualDom::new(probe);
        dom.in_runtime(|| {
            *ACTIVITY_NOW.write() = Some(ACTIVE_AT + 59 * 60);
        });
        dom.rebuild_in_place();

        // One second short of the hour, then across it: the bucket boundary
        // is what proves the number is being recomputed rather than merely
        // re-rendered.
        dom.in_runtime(|| {
            *ACTIVITY_NOW.write() = Some(ACTIVE_AT + 60 * 60);
        });
        dom.render_immediate(&mut dioxus::core::NoOpMutations);

        RENDERED.with(|rendered| {
            assert_eq!(
                rendered.borrow().as_slice(),
                ["59m", "1h"],
                "the tick alone must move the age a reader sees"
            );
        });
    }
}
