//! The platform's time zone, read for the GPUI app's clock times (`farhelm_gui::local_time`).
//!
//! The GUI crates never query the platform, so this binary hands the app its time zone, as it hands over Tab's reach.
//! On macOS and Linux that is the C library's local time, read through `farhelm_helm::local_time`, which also gives
//! the embedded helm's ntfy quiet hours their local hour; its module docs say how the zone is read and why. The app
//! calls [`TimeZone::refresh`] once a minute, which is what makes a zone change show on Linux, and
//! [`TimeZone::offset_at`] for every time drawn, which reads no file.
//!
//! Elsewhere there is no zone: [`platform`] hands the app nothing and it writes UTC, as the status sounds have no
//! player there; the desktop app ships for macOS and Linux only.

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
        farhelm_helm::local_time::refresh();
    }

    fn offset_at(&self, seconds: i64) -> Option<i32> {
        farhelm_helm::local_time::utc_offset_at(seconds)
    }
}
