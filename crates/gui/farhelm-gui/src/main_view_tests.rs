//! Tests of the main window as a whole that no one view's tests can make: here, that a window left alone sends the helm
//! nothing.
//!
//! The windows are the real main view over the helm client's fake ([`test_support::open_main`]), whose command log
//! records every command the window sends on the UI channel.

use std::time::Duration;

use farhelm_proto::ui_channel::{CollectionState, HostStateView};
use gpui::TestAppContext;

use crate::hosts::tests::{connected, host, hosts};
use crate::session_list::menu_tests::snapshot;
use crate::session_list::tests::{Fixture, session};
use crate::session_pane::unavailable;
use crate::test_support::{self, MainWindow};

/// The helm's state for one idle case: `rows` listed, the first of them open (none with no rows), and a registry
/// holding only the helm fixture's host 2, every fixture session's host, in `state`.
fn fleet(rows: &[Fixture], state: HostStateView) -> Vec<CollectionState> {
    let rows: Vec<(Fixture, usize)> = rows.iter().cloned().map(|row| (row, 0)).collect();
    let mut fleet = snapshot(&rows, false);
    fleet.push(hosts(&[host(2, "ssh", state)]));
    fleet
}

/// The fixture's host as the helm reports it while it cannot be reached.
fn unreachable() -> HostStateView {
    HostStateView::Unreachable {
        cause: "transport-failure".into(),
        last_error: "ssh: connect to host build-box.invalid port 22: Connection refused".into(),
    }
}

/// Answer what the window sends while it settles as a healthy helm would (each host's trash listing empty and removing
/// nothing, every other command a plain success), since those reads are not polls, as the old spec counted from a mark
/// taken once the page had settled; then let ten minutes pass and assert that nothing more was sent.
fn stays_idle(main: &mut MainWindow, case: &str) {
    for _ in 0..10 {
        for command in main.helm.commands() {
            let body = (command.path == "/api/checkout-trash/list")
                .then(|| crate::trash::tests::listing(&[], false));
            main.helm.reply(command.id, 200, body);
        }
        test_support::settle(main.cx);
    }
    // Stops the animation ticks and nothing else; the test's docstring says why.
    main.cx
        .update(|_, cx| crate::ticker::set_reduced_motion(true, cx));
    test_support::advance(main.cx, Duration::from_secs(10 * 60));
    let sent = main.helm.commands();
    assert!(sent.is_empty(), "{case}: an idle window sent {sent:?}");
}

/// Why this matters: the old app polled the helm in four loops (the listing, the host registry, the open session's
/// detail and a stale session's host read), and the UI channel replaced them all with the helm's pushes (SPEC_impl.md,
/// "The UI channel"). A view that took up re-reading on a timer would bring the polling back unseen, since it still
/// draws correctly. This is the old feed spec's "a healthy feed performs no periodic reads" in its three windows.
///
/// Specified: once a window has settled and what it sent is answered, ten minutes pass and it sends the helm nothing,
/// with no session open (an empty fleet, since a listed session is always opened at startup), with a session open on a
/// reachable host and its terminal live, and with one open on an unreachable host, showing the unavailable card. Only
/// the UI channel is watched, not a terminal's own stream. Each window gets an app of its own, so each starts fresh.
///
/// The ten minutes run under reduced motion, which stops only the ten-a-second animation redraws (they made the test
/// take minutes); the ticker's deadlines and clocks, what a re-read would ride on, run all the same (`ticker.rs`).
#[gpui::test]
fn an_idle_window_sends_the_helm_nothing(
    no_session: &mut TestAppContext,
    session_open: &mut TestAppContext,
    unreachable_open: &mut TestAppContext,
) {
    let mut main = test_support::open_main(no_session, fleet(&[], connected()));
    stays_idle(&mut main, "no session open");

    let listed = [session("a"), session("b")];
    let mut main = test_support::open_main(session_open, fleet(&listed, connected()));
    main.terminal_live();
    stays_idle(&mut main, "a session open");

    let stale = [session("a").stale(), session("b")];
    let mut main = test_support::open_main(unreachable_open, fleet(&stale, unreachable()));
    assert!(
        main.cx.debug_bounds(unavailable::CARD_ID).is_some(),
        "the premise: the unavailable card shows"
    );
    stays_idle(&mut main, "an unreachable session open");
}
