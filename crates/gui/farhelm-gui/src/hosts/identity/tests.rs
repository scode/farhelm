//! Tests of a host's identity mark, as pure functions: each color's tint, each shape's bundled drawing, and the local
//! wording of a name the helm sent with a session, and a sandbox's or container's short name. The display name is pinned
//! through the rail's identity rule, in the rail's tests (`session_list/host_rail/tests.rs`), the phase words in
//! `identity/phase/tests.rs`, and the times the phase details give in `local_time_tests.rs` and the phase tests.

use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::HostView;

use super::{named, short_name, tint};
use crate::hosts::tests::{connected, host};
use crate::session_list::environment_tests::{Provider, environment_host};
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

/// Host 2 as the helm lists the session-owned environment `provider` made, named `farhelm-sess-dev-1` and unaliased:
/// its destination is the one the helm stores (a registry coordinate for a container, an SSH destination for a
/// sandbox), and its name, like its environment's, the machine's own, the one the Hosts panel shows.
pub(crate) fn listed_environment(provider: Provider) -> HostView {
    let mut host = environment_host(provider, connected());
    host.destination = Some(
        match provider {
            Provider::Sandbox => "farhelm-sess-dev-1@farhelm-sess-dev-1.sandbox.tensorlake.ai",
            Provider::Container => "podman:farhelm-sess-dev-1",
        }
        .to_owned(),
    );
    host.name = "farhelm-sess-dev-1".to_owned();
    if let Some(environment) = host.environment.as_mut() {
        environment.name = host.name.clone();
    }
    host
}

/// A container's short name once read `podman:<container>` in the attention list, the helm's registry coordinate,
/// where Hosts named the same host `<container>` (SPEC.md derives a short name only from an SSH hostname). Specified:
/// an unaliased container and an unaliased sandbox, as the helm lists them, read `farhelm-sess-dev-1` (the sandbox
/// read so before this rule too, since its derived label is its name: it is here as the specification, not a guard);
/// an aliased container reads its alias; an ordinary host at `dev@build-box.invalid` still reads `build-box`.
#[test]
fn an_environment_hosts_short_name_is_its_name_not_its_coordinate() {
    for provider in [Provider::Container, Provider::Sandbox] {
        assert_eq!(
            short_name(&listed_environment(provider)).raw(),
            "farhelm-sess-dev-1",
            "{provider:?}"
        );
    }
    let mut aliased = listed_environment(Provider::Container);
    aliased.alias = Some("Builder".to_owned());
    assert_eq!(short_name(&aliased).raw(), "Builder");
    assert_eq!(short_name(&host(3, "ssh", connected())).raw(), "build-box");
}
