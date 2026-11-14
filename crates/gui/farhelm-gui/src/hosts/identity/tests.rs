//! Tests of a host's identity mark and of the UTC stamp, as pure functions: each color's tint, each shape's bundled
//! drawing, and the stamp's format. The display name is pinned through the rail's identity rule, in the rail's tests
//! (`session_list/host_rail/tests.rs`), and the phase words in `identity/phase/tests.rs`.

use farhelm_proto::host_appearance::{HostColor, HostIcon};

use super::{tint, utc_stamp};
use crate::{assets, theme};

/// A host's color names a palette token by its place in the color list (SPEC.md: host colors identify hosts), and the
/// default color is the ordinary foreground. A tint written per color must stay in step with that list. Specified: for
/// every color but the default, the tint is the palette's host entry at its place in `HostColor::ALL` less one, in both
/// appearances.
#[test]
fn each_host_color_tints_with_its_palette_entry() {
    for palette in [&theme::DARK, &theme::LIGHT] {
        assert_eq!(tint(HostColor::Default, palette), palette.fg_0);
        for (place, color) in HostColor::ALL.iter().enumerate().skip(1) {
            assert_eq!(tint(*color, palette), palette.host[place - 1], "{color:?}");
        }
    }
}

/// Every shape a user can pick draws its own bundled drawing, so no two hosts' shapes collapse into one and none is
/// missing. Specified: the sixteen `HostIcon`s map to sixteen distinct bundled SVGs.
#[test]
fn every_host_shape_has_its_own_bundled_drawing() {
    let paths: std::collections::HashSet<&str> = HostIcon::ALL
        .iter()
        .map(|icon| super::glyph(*icon).path())
        .collect();
    assert_eq!(paths.len(), HostIcon::ALL.len());
    for path in paths {
        assert!(
            gpui::AssetSource::load(&assets::Assets, path)
                .unwrap()
                .is_some(),
            "{path} is bundled"
        );
    }
}

/// Why this matters: the times a host's details show (and the session row's age, which is to use the same stamp) must
/// read the same on every machine, so they are written in UTC with the zone named, since the GUI carries no time zone
/// database. Specified: a known instant reads as its UTC date and time to the second, and one second before the epoch
/// lands on 1969-12-31 rather than wrapping around 1970.
#[test]
fn utc_stamps_name_the_zone_and_hold_before_the_epoch() {
    assert_eq!(utc_stamp(1_700_000_000), "2023-11-14 22:13:20 UTC");
    assert_eq!(utc_stamp(-1), "1969-12-31 23:59:59 UTC");
}
