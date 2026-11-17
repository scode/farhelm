//! How a host is named and marked: the one home of a registered host's display name ([`display_name`], and [`named`]
//! for a name the helm sent with a session), its identity mark, its chosen shape in its chosen color ([`host_mark`]),
//! and what its phase is called ([`phase`]).
//!
//! The rail, the Hosts panel, Add host, the launcher, and the host menu and settings read these from here, so that no
//! area reaches into another's private module for them and every surface names and marks a host the same way.
//!
//! [`phase`] holds a phase's words, its tone (the color group of its dot and words), its hover, and the detail lines
//! under a host's row. It is `pub(crate)` so that the launcher's host picker can show the phase words; the detail
//! lines are the Hosts panel's, and the session pane's unavailable card reads two of their parts, the evidence and the
//! remedy ([`phase::evidence`], [`phase::remedy`]), so that a session on an unreachable host says what the host's row
//! says.

pub(crate) mod phase;

#[cfg(test)]
pub(crate) mod tests;

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
    named(&host.name, host.is_local())
}

/// A registered host's short name, for a surface too small for its full name (the progress card's header and the
/// attention list's rows): the alias, or the destination without its user and domain, or "Local" for the local host,
/// by the protocol crate's one rule (`host_short_name`, which the old app's `Host::short_name` used too), and the
/// registry's full name when that rule finds nothing. Host text, as [`display_name`] is.
///
/// A host the registry describes as a disposable environment (`environment` set) reads by its full name, the one the
/// Hosts panel shows, unless it has an alias, and its destination is never consulted. A container's destination is the
/// helm's registry coordinate, `podman:<container>`, not an SSH hostname, so SPEC.md's rule ("the first label of its SSH
/// hostname") has nothing to derive from it. A sandbox's destination is a real SSH destination
/// (`<sandbox>@<sandbox>.sandbox.tensorlake.ai`), but the helm names it `<sandbox>`, the same as the label the rule
/// would derive, so it reads the same either way. The old app derived the short name from the destination for both,
/// and its attention list named a container `podman:<container>`; a real container run on 2026-11-16 found the GPUI
/// list doing the same.
pub(crate) fn short_name(host: &HostView) -> HostText {
    let destination = host
        .destination
        .as_deref()
        .filter(|_| host.environment.is_none());
    let short = farhelm_proto::host_name::host_short_name(
        host.is_local(),
        destination,
        host.alias.as_deref(),
    );
    if short.is_empty() {
        HostText::from_host(host.name.as_str())
    } else {
        HostText::from_host(short)
    }
}

/// [`display_name`]'s rule applied to a name the helm sent elsewhere than in the registry: `name` is the helm's name
/// for a host, and `is_local` whether the registry places that host on the helm's own machine. The session pane's
/// details bar names a session's host this way, from the name the helm sent with the session (the old app's
/// `gui_host_name`), so it still names a host the registry does not list, while the local host reads the same as
/// everywhere else. A name that only matches [`LOCAL_NAME`] on a host not known to be local stays as it is.
pub(crate) fn named(name: &str, is_local: bool) -> HostText {
    if is_local && name == LOCAL_NAME {
        HostText::from_host("Local (this machine)")
    } else {
        HostText::from_host(name)
    }
}

/// A host's mark: its chosen shape `icon` at `size`, tinted with its chosen `color` (the ordinary foreground for the
/// default color). Decorative: what it stands for is in the accessible name of the control it sits in. Every host mark
/// in the app goes through this, the session list's rows and rail and the quick switcher included, so they cannot drift.
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
