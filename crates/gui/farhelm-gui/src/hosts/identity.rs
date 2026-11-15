//! How a host is named and marked: the one home of a registered host's display name ([`display_name`]), its identity
//! mark, its chosen shape in its chosen color ([`host_mark`]), and what its phase is called ([`phase`]).
//!
//! The rail, the Hosts panel, Add host, the launcher, and the host menu and settings read these from here, so that no
//! area reaches into another's private module for them and every surface names and marks a host the same way.
//!
//! [`phase`] holds a phase's words, its tone (the color group of its dot and words), its hover, and the detail lines
//! under a host's row. It is `pub(crate)` so that the launcher's host picker can show the phase words; the detail
//! lines are the Hosts panel's, and no other area reads them.
//!
//! [`utc_stamp`] is here too: the phase details print their times through it, and the session row's age is to use it
//! as well, importing it from here rather than from inside [`phase`].

pub(crate) mod phase;

#[cfg(test)]
mod tests;

use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::HostView;
use gpui::{Pixels, Styled, Svg, svg};

use crate::assets::Icon;
use crate::host_text::HostText;
use crate::theme::{self, Palette, Token};

/// The name the registry gives the local host when the user gave it no alias. Only this exact name is shown as
/// "Local (this machine)": an alias stays the user's word, and a remote host that happens to carry this name is not
/// the local one.
pub(crate) const LOCAL_NAME: &str = "this machine";

/// A registered host's full display name, the one rule the rail's avatar and chip, the Hosts panel's rows, Add host and
/// the launcher share: the registry's name for it (its alias, else its destination), or "Local (this machine)" for the
/// local host under the registry's own name. An alias stays the user's word, even on the local host. Host text; the
/// app's own words for the local host travel the same way, and escaping them changes nothing.
pub(crate) fn display_name(host: &HostView) -> HostText {
    if host.is_local() && host.name == LOCAL_NAME {
        HostText::from_host("Local (this machine)")
    } else {
        HostText::from_host(host.name.as_str())
    }
}

/// A host's mark: its chosen shape `icon` at `size`, tinted with its chosen `color` (the ordinary foreground for the
/// default color). Decorative: what it stands for is in the accessible name of the control it sits in. The session
/// list's rows and the quick switcher will draw a host's mark through this too.
pub(crate) fn host_mark(icon: HostIcon, color: HostColor, size: Pixels, palette: &Palette) -> Svg {
    svg()
        .path(glyph(icon).path())
        .size(size)
        .flex_none()
        .text_color(theme::color(tint(color, palette)))
}

/// The drawing of each host shape.
fn glyph(icon: HostIcon) -> Icon {
    match icon {
        HostIcon::Cloud => Icon::HostCloud,
        HostIcon::House => Icon::HostHouse,
        HostIcon::Flask => Icon::HostFlask,
        HostIcon::Database => Icon::HostDatabase,
        HostIcon::Chip => Icon::HostChip,
        HostIcon::Rocket => Icon::HostRocket,
        HostIcon::Gear => Icon::HostGear,
        HostIcon::Gem => Icon::HostGem,
        HostIcon::Hexagon => Icon::HostHexagon,
        HostIcon::Triangle => Icon::HostTriangle,
        HostIcon::Ring => Icon::HostRing,
        HostIcon::Square => Icon::HostSquare,
        HostIcon::Bug => Icon::HostBug,
        HostIcon::Factory => Icon::HostFactory,
        HostIcon::Castle => Icon::HostCastle,
        HostIcon::Laptop => Icon::HostLaptop,
    }
}

/// The token a host color tints with: the palette's `host` entry at the color's place in `HostColor::ALL` less one
/// (`--host-icon-<color>`), and the ordinary foreground for the default color, as the old mark's inline style did. The
/// host settings' color swatches fill with it too, so a swatch and the mark it picks never disagree.
pub(crate) fn tint(color: HostColor, palette: &Palette) -> Token {
    match color {
        HostColor::Default => palette.fg_0,
        HostColor::Lavender => palette.host[0],
        HostColor::Orchid => palette.host[1],
        HostColor::Teal => palette.host[2],
        HostColor::Steel => palette.host[3],
        HostColor::Sand => palette.host[4],
        HostColor::Copper => palette.host[5],
        HostColor::Red => palette.host[6],
    }
}

/// `secs` since the epoch as `YYYY-MM-DD HH:MM:SS UTC`, the old app's unambiguous stamp (`activity::utc_stamp`): a
/// zone-qualified time needs no time zone database, which the GUI does not carry. It is the app's one timestamp form:
/// the Hosts panel's detail lines ([`phase::detail_lines`]) print when a suspension was noticed or a provider was asked
/// through it, and a session row's age shows its last activity in it, as the old app's did. Seconds, not the
/// milliseconds the helm pushes: callers divide first. Times before 1970 are written as they are, not refused.
pub(crate) fn utc_stamp(secs: i64) -> String {
    const SECS_PER_DAY: i64 = 86_400;
    // Euclidean division, so a stamp before 1970 still lands on its own day rather than reflecting around the epoch.
    let (year, month, day) = farhelm_proto::time::civil_from_days(secs.div_euclid(SECS_PER_DAY));
    let of_day = secs.rem_euclid(SECS_PER_DAY);
    let (hour, minute, second) = (of_day / 3_600, (of_day % 3_600) / 60, of_day % 60);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}
