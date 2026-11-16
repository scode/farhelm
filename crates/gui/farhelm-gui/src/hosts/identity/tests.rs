//! Tests of a host's identity mark, as pure functions: each color's tint, each shape's bundled drawing, and the local
//! wording of a name the helm sent with a session. The display name is pinned through the rail's identity rule, in the
//! rail's tests (`session_list/host_rail/tests.rs`), the phase words in `identity/phase/tests.rs`, and the times the
//! phase details give in `local_time_tests.rs` and the phase tests.

use farhelm_proto::host_appearance::{HostColor, HostIcon};

use super::{named, tint};
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

/// The session pane's details bar names a session's host from the name the helm sent with the session, and only the
/// local host's own registry name may read as `Local (this machine)`: a remote host the user happened to call `this
/// machine` must not pass for the helm's own machine. Specified: `this machine` on a host known to be local reads
/// `Local (this machine)`; the same name on a host not known to be local, and any other name on the local host, read as
/// given.
#[test]
fn only_the_local_hosts_own_name_reads_local() {
    assert_eq!(named("this machine", true).raw(), "Local (this machine)");
    assert_eq!(named("this machine", false).raw(), "this machine");
    assert_eq!(named("laptop", true).raw(), "laptop");
}
