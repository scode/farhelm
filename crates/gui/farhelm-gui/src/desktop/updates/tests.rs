//! Tests of the window's side of the updater: the one "does an updater run?" answer, the feed of readouts, and the
//! commands, against a fake updater host made of plain closures (the real updater is the desktop crate's and has its
//! own tests). They run on GPUI's fake platform.

use std::cell::Cell;
use std::rc::Rc;

use gpui::TestAppContext;

use super::fake::{Feed, Seen, fake_host, readout_saying};
use super::{
    automatic_updates, check_now, entity, keep, readout, restart_with, runs, set_automatic_updates,
};
use crate::test_support;

/// Every update surface asks one question before it draws anything, so an app with no updater (a devbuild, an agent
/// build, the Linux app) must answer it "no" everywhere and act on nothing. Specified: with no host updater, nothing
/// runs, there is no readout and no automatic-updates state, a check does nothing, and setting automatic updates is
/// refused rather than reported as saved.
#[gpui::test]
fn with_no_updater_nothing_runs_and_nothing_is_offered(cx: &mut TestAppContext) {
    test_support::install(cx);
    cx.update(|cx| {
        keep(None, cx);
        assert!(!runs(cx));
        assert_eq!(readout(cx), None);
        assert_eq!(automatic_updates(cx), None);
        check_now(cx);
        assert!(set_automatic_updates(true, cx).is_err());
    });
}

/// The readout is the updater's, so the window must show the latest one it published and redraw when it changes; a
/// readout that stalled at the first value would hide an update that is ready. Specified: the first readout shows at
/// once; each later one replaces it and notifies the entity's observers, an unchanged one does not; and once the feed
/// ends, nothing more is read.
#[gpui::test]
fn the_window_shows_the_latest_readout_the_updater_published(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (feed, seen) = (Rc::new(Feed::default()), Rc::new(Seen::default()));
    cx.update(|cx| keep(Some(fake_host(&feed, &seen, false)), cx));
    cx.run_until_parked();
    let shown = |cx: &mut TestAppContext| cx.update(|cx| readout(cx).map(|shown| shown.tooltip));
    assert!(cx.update(|cx| runs(cx)));
    assert_eq!(shown(cx).as_deref(), Some("first"));
    let Some(updater) = cx.update(|cx| entity(cx)) else {
        panic!("the premise: an updater is kept");
    };
    let notified = Rc::new(Cell::new(0));
    let counted = notified.clone();
    let _observing =
        cx.update(|cx| cx.observe(&updater, move |_, _| counted.set(counted.get() + 1)));

    feed.publish(Some(readout_saying("second")));
    cx.run_until_parked();
    assert_eq!(shown(cx).as_deref(), Some("second"));
    assert_eq!(notified.get(), 1, "a new readout notifies its observers");
    feed.publish(Some(readout_saying("second")));
    cx.run_until_parked();
    assert_eq!(notified.get(), 1, "the same readout again redraws nothing");

    feed.publish(None);
    cx.run_until_parked();
    feed.publish(Some(readout_saying("after the end")));
    cx.run_until_parked();
    assert_eq!(
        shown(cx).as_deref(),
        Some("second"),
        "after its end the feed is not read again"
    );
}

/// Each update surface reaches the one updater through these entry points, so each must reach the matching command,
/// and a failure to save the automatic-updates choice must come back to the switch rather than be lost. Specified: a
/// check reaches the updater's check; the automatic-updates state starts as the updater's; a saved change returns `Ok`
/// and is what the state then reads; a failed one returns the updater's reason and leaves the state as it was.
#[gpui::test]
fn the_commands_reach_the_updater_and_its_failures_come_back(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (feed, seen) = (Rc::new(Feed::default()), Rc::new(Seen::default()));
    cx.update(|cx| keep(Some(fake_host(&feed, &seen, false)), cx));
    cx.update(|cx| {
        check_now(cx);
        check_now(cx);
        assert_eq!(seen.checks.get(), 2);
        assert_eq!(automatic_updates(cx), Some(false));
        assert_eq!(set_automatic_updates(true, cx), Ok(()));
        assert_eq!(automatic_updates(cx), Some(true), "a saved change is kept");
        assert_eq!(
            set_automatic_updates(false, cx),
            Err("not saved".to_string())
        );
        assert_eq!(
            automatic_updates(cx),
            Some(true),
            "a failed change leaves it as it was"
        );
    });
}

/// Restart to update quits the app so the reopened one can take over, but quitting when the restart could not start
/// would leave the app closed (SPEC.md, "Installation and updates"). Specified: the app quits once when the updater
/// started its restart, and not at all when it did not. The quit is passed in because GPUI's test platform ignores a
/// real one.
#[gpui::test]
fn restart_to_update_quits_only_when_the_restart_started(cx: &mut TestAppContext) {
    test_support::install(cx);
    for starts in [false, true] {
        let (feed, seen) = (Rc::new(Feed::default()), Rc::new(Seen::default()));
        let quits = Rc::new(Cell::new(0));
        let counted = quits.clone();
        cx.update(|cx| {
            keep(Some(fake_host(&feed, &seen, starts)), cx);
            restart_with(move |_| counted.set(counted.get() + 1), cx);
        });
        assert_eq!(
            seen.restarts.get(),
            1,
            "the premise: the restart was asked for"
        );
        assert_eq!(quits.get(), u32::from(starts));
    }
}
