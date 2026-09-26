//! Calendar arithmetic shared by every crate that prints a UTC date.
//!
//! The browser formats session ages, and the supervisor names checkout
//! directories and reads conversation files by date. None of them wants a date
//! crate for one conversion (the wasm bundle least of all), so the conversion
//! lives here once rather than as three hand-carried copies.

/// Days since the Unix epoch to a proleptic Gregorian `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days`, the standard closed-form inverse of the
/// day count and what a date crate would run here. Valid for every day count
/// up to `i64::MAX - 719_468`, negative days (before 1970) included; above that
/// the first shift overflows. Every caller derives its day count by dividing
/// Unix seconds by 86,400, which stays far inside the range.
///
/// Two of the magic numbers have a plain meaning: 719468 shifts the epoch to
/// 0000-03-01 (March-first years make the leap day the LAST day, so no month
/// length depends on it), and 146097 is the number of days in a 400-year
/// Gregorian era. The 1460 / 36524 / 146096 divisors in the year-of-era line
/// do NOT: they are that formula's leap-day corrections over a zero-based
/// day-of-era count, and reading them as cycle lengths is wrong (a Gregorian
/// four-year cycle is 1461 days, not 1460). Copy them from Hinnant rather than
/// re-deriving them from a calendar.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097); // [0, 146096]
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
    (
        if month <= 2 { year + 1 } else { year },
        month as u32,
        day as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    /// Why this matters: session ages, checkout directory names, and dated
    /// conversation lookups all print through this one conversion, and an
    /// off-by-one at a month or leap boundary would misdate all three at once.
    ///
    /// Specification: known day counts map to their calendar dates, including
    /// the epoch, the day before it, leap days in a century leap year and a
    /// plain leap year, the day after a non-leap century's February, and the
    /// last day of a year.
    #[farhelm_testtrace::test]
    fn day_counts_map_to_their_calendar_dates() {
        for (days, date) in [
            (0, (1970, 1, 1)),
            (-1, (1969, 12, 31)),
            (-719_468, (0, 3, 1)),
            (11_016, (2000, 2, 29)),
            (11_017, (2000, 3, 1)),
            (19_416, (2023, 2, 28)),
            (19_782, (2024, 2, 29)),
            (47_541, (2100, 3, 1)),
            (20_088, (2024, 12, 31)),
        ] {
            assert_eq!(civil_from_days(days), date, "day {days}");
        }
    }
}
