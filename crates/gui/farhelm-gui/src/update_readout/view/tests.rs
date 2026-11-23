//! Tests of the version readout while an updater runs: each state's control, words and marks, the update menu, the
//! inline status's timing and dismissal, the spinner's life, what a screenshot golden holds still (a rested pointer and
//! a held step), and focus across the check-to-toggle swap. They draw the
//! readout entity over the fake updater host (`desktop::updates::fake`), kept as the app's updater so its commands
//! reach the fake, beside one other control for the focus cases; one test draws it in the real header. Without an
//! updater the header draws its plain build text, which the sidebar's own header test covers.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AppContext as _, Entity, FocusHandle, IntoElement as _, Modifiers, ParentElement as _,
    Styled as _, TestAppContext, VisualTestContext, div, point, px,
};

use super::reminder::{ARROW_ROOM, BURST_TICKS};
use super::{Look, RELEASE_NOTES_URL, UP_TO_DATE_FOR, VersionReadout, check_look};
use crate::desktop::updates::fake::{Feed, Seen, fake_host, readout_saying};
use crate::desktop::updates::{self, Updater};
use crate::test_support;
use crate::update_readout::{Activity, Readout, inline_presentation};
use crate::{theme, ticker};

/// The user-facing timing contract, independent of the scheduler's constants.
const FIFTEEN_MINUTES: Duration = Duration::from_secs(15 * 60);
const FIVE_MINUTES: Duration = Duration::from_secs(5 * 60);

/// A readout in `activity`, with an update waiting when `ready`, under request `notice_id`.
fn readout_in(activity: Activity, ready: bool, notice_id: u64) -> Readout {
    Readout {
        update_ready: ready,
        activity,
        notice_id,
        ..readout_saying("the hover")
    }
}

/// Keep a fake updater whose first readout is `first` as the app's updater, returning its feed and its record.
fn keep_fake(cx: &mut TestAppContext, first: Readout) -> (Rc<Feed>, Rc<Seen>) {
    let (feed, seen) = (Rc::new(Feed::default()), Rc::new(Seen::default()));
    let mut host = fake_host(&feed, &seen, false);
    host.first_readout = first;
    cx.update(|cx| updates::keep(Some(host), cx));
    (feed, seen)
}

/// What a test drives: the window, the readout, the feed to publish into, what the fake's commands saw, and the other
/// control's focus handle.
type Opened<'a> = (
    &'a mut VisualTestContext,
    Entity<VersionReadout>,
    Rc<Feed>,
    Rc<Seen>,
    FocusHandle,
);

/// The readout under test, over a kept fake updater whose first readout is `first`, drawn in an active test window
/// beside one other control, `other`.
fn open(cx: &mut TestAppContext, first: Readout) -> Opened<'_> {
    test_support::install(cx);
    let (feed, seen) = keep_fake(cx, first);
    let readout: Entity<VersionReadout> = cx.update(|cx| {
        let updater: Option<Entity<Updater>> = updates::entity(cx);
        let Some(updater) = updater else {
            panic!("the premise: an updater is kept");
        };
        cx.new(|cx| VersionReadout::new(updater, cx))
    });
    let other = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (drawn_readout, drawn_other) = (readout.clone(), other.clone());
    let window = test_support::open(cx, move |_, _| {
        div()
            .child(drawn_readout.clone())
            .child(test_support::button("other", &drawn_other))
            .into_any_element()
    });
    (window, readout, feed, seen, other)
}

/// Publish `readout` from the fake updater and let the window catch up.
fn publish(cx: &mut VisualTestContext, feed: &Feed, readout: Readout) {
    feed.publish(Some(readout));
    test_support::settle(cx);
}

/// The words painted in the window now.
fn painted(cx: &mut VisualTestContext) -> Vec<String> {
    crate::session_list::tests::painted(&test_support::frame(cx))
}

/// Whether `words` are painted now.
fn shows(cx: &mut VisualTestContext, words: &str) -> bool {
    painted(cx).iter().any(|text| text == words)
}

/// Whether an element with id `id` is drawn now.
fn drawn(cx: &mut VisualTestContext, id: &str) -> bool {
    test_support::frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

/// The accessible name of the element drawn with id `id`.
fn label(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    test_support::frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// The look the check button was last drawn with.
fn look(cx: &mut VisualTestContext, readout: &Entity<VersionReadout>) -> Option<Look> {
    cx.update(|_, cx| readout.read(cx).drawn_look.get())
}

/// Whether the readout holds its spinner (it does only while something spins and the window is active).
fn holds_spinner(cx: &mut VisualTestContext, readout: &Entity<VersionReadout>) -> bool {
    cx.update(|_, cx| readout.read(cx).spinner.is_some())
}

/// Observe actual reminder handles so a visually quiet row cannot hide a repeating timer or retained ticker.
fn reminder_state(
    cx: &mut VisualTestContext,
    readout: &Entity<VersionReadout>,
) -> (Option<u64>, bool, bool) {
    cx.update(|_, cx| {
        let reminder = &readout.read(cx).reminder;
        (
            reminder.burst.as_ref().map(|burst| burst.count(cx)),
            reminder.deadline.is_some(),
            reminder.is_due(),
        )
    })
}

/// Drive the shared test executor and let the readout act on the deadline or draw its next frame.
fn advance_reminder(cx: &mut VisualTestContext, duration: Duration) {
    cx.executor().advance_clock(duration);
    test_support::settle(cx);
}

/// Advance the remaining burst ticks through their own frames, like an active window receiving the ticker.
fn complete_reminder(cx: &mut VisualTestContext, readout: &Entity<VersionReadout>) -> Duration {
    let remaining = BURST_TICKS - reminder_state(cx, readout).0.expect("the burst is playing");
    for _ in 0..remaining {
        advance_reminder(cx, ticker::TICK);
    }
    ticker::TICK * u32::try_from(remaining).unwrap()
}

/// A new ready update plays once. Re-publishing it does not reset the burst or its deadline. A recent key press
/// qualifies the next reminder exactly at the interval, with no animation handle in the quiet period.
#[gpui::test]
fn reminder_starts_on_arrival_and_repeats_with_recent_input(cx: &mut TestAppContext) {
    let (cx, readout, feed, _, _) = open(cx, readout_in(Activity::Idle, false, 1));
    assert_eq!(reminder_state(cx, &readout), (None, false, false));
    publish(cx, &feed, readout_in(Activity::Idle, true, 2));
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));
    advance_reminder(cx, ticker::TICK);
    let raised = reminder_state(cx, &readout);
    assert_eq!(raised, (Some(1), true, false));
    publish(cx, &feed, readout_in(Activity::Idle, true, 3));
    assert_eq!(reminder_state(cx, &readout), raised);
    let spent = ticker::TICK + complete_reminder(cx, &readout);
    assert_eq!(reminder_state(cx, &readout), (None, true, false));

    advance_reminder(cx, FIFTEEN_MINUTES - spent - Duration::from_millis(1));
    assert_eq!(reminder_state(cx, &readout), (None, true, false));
    test_support::keys(cx, "tab");
    assert!(cx.update(|window, cx| crate::focus::last_activity(window, cx).is_some()));
    advance_reminder(cx, Duration::from_millis(1));
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));
}

/// Time passing alone never shows another burst in an idle window. A mouse press consumes the due reminder and
/// starts the next interval without reviving a second scheduler.
#[gpui::test]
fn idle_reminder_waits_for_a_press_without_retrying(cx: &mut TestAppContext) {
    let (cx, readout, feed, _, _) = open(cx, readout_in(Activity::Idle, false, 1));
    publish(cx, &feed, readout_in(Activity::Idle, true, 2));
    test_support::keys(cx, "tab");
    assert!(cx.update(|window, cx| crate::focus::last_activity(window, cx).is_some()));
    let spent = complete_reminder(cx, &readout);
    assert!(cx.update(|window, cx| ticker::is_active(window, cx)));
    assert_eq!(reminder_state(cx, &readout), (None, true, false));
    advance_reminder(cx, FIFTEEN_MINUTES - spent);
    assert_eq!(reminder_state(cx, &readout), (None, false, true));
    advance_reminder(cx, FIFTEEN_MINUTES);
    assert_eq!(reminder_state(cx, &readout), (None, false, true));
    test_support::click(cx, "other");
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));
}

/// Activation counts as returning even without a recent input. It wakes both an update delivered to a hidden window
/// and a periodic reminder that expires while the window is inactive.
#[gpui::test]
fn activation_plays_a_reminder_due_while_inactive(cx: &mut TestAppContext) {
    let (cx, readout, feed, _, _) = open(cx, readout_in(Activity::Idle, false, 1));
    cx.deactivate_window();
    assert!(!cx.update(|window, cx| ticker::is_active(window, cx)));
    publish(cx, &feed, readout_in(Activity::Idle, true, 2));
    assert_eq!(reminder_state(cx, &readout), (None, false, true));
    cx.update(|window, _| window.activate_window());
    test_support::settle(cx);
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));
    let spent = complete_reminder(cx, &readout);
    cx.deactivate_window();
    assert!(!cx.update(|window, cx| ticker::is_active(window, cx)));
    advance_reminder(cx, FIFTEEN_MINUTES - spent);
    assert_eq!(reminder_state(cx, &readout), (None, false, true));
    assert!(cx.update(|window, cx| crate::focus::last_activity(window, cx).is_none()));
    cx.update(|window, _| window.activate_window());
    test_support::settle(cx);
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));
}

/// Reduced motion skips arrival and cancels any ongoing burst and future reminder without hiding the update action.
#[gpui::test]
fn reduced_motion_leaves_the_pill_steady(cx: &mut TestAppContext) {
    let (cx, readout, feed, _, _) = open(cx, readout_in(Activity::Idle, false, 1));
    cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    publish(cx, &feed, readout_in(Activity::Idle, true, 2));
    assert!(drawn(cx, "app-version-toggle"));
    assert_eq!(reminder_state(cx, &readout), (None, false, false));
    advance_reminder(cx, FIFTEEN_MINUTES);
    assert_eq!(reminder_state(cx, &readout), (None, false, false));

    cx.update(|_, cx| ticker::set_reduced_motion(false, cx));
    // Let the live preference notification drive the frame without requesting one from the test.
    test_support::settle(cx);
    assert_eq!(reminder_state(cx, &readout), (None, true, false));
    advance_reminder(cx, FIFTEEN_MINUTES - FIVE_MINUTES + Duration::from_secs(1));
    test_support::keys(cx, "tab");
    advance_reminder(cx, FIVE_MINUTES - Duration::from_secs(1));
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));
    cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    test_support::settle(cx);
    assert_eq!(reminder_state(cx, &readout), (None, false, false));
    assert!(drawn(cx, "app-version-toggle"));
}

/// Exercise the production gap beside a wordmark at the narrow Mac header budget and at a wider width. The pill
/// still pulses when the arrow would overlap the wordmark or when an inline status competes for the space.
#[gpui::test]
fn narrow_and_busy_headers_keep_the_pulse_without_an_arrow(cx: &mut TestAppContext) {
    test_support::install(cx);
    let (feed, _) = keep_fake(cx, readout_in(Activity::Idle, false, 1));
    let readout = cx.update(|cx| {
        let updater = updates::entity(cx).expect("the fixture set up an updater");
        cx.new(|cx| VersionReadout::new(updater, cx))
    });
    let width = Rc::new(Cell::new(px(296.)));
    let traffic_lights = Rc::new(Cell::new(px(88.)));
    let (drawn_readout, drawn_width, drawn_inset) =
        (readout.clone(), width.clone(), traffic_lights.clone());
    let cx = test_support::open(cx, move |_, cx| {
        div()
            .w(drawn_width.get())
            .child(crate::sidebar::header_row(
                Some(drawn_readout.clone()),
                Some(drawn_inset.get()),
                cx,
            ))
            .into_any_element()
    });
    publish(cx, &feed, readout_in(Activity::Idle, true, 2));
    assert!(drawn(cx, "app-version-toggle"));
    let gap = |cx: &mut VisualTestContext| {
        test_support::bounds(cx, "app-version-toggle").left()
            - test_support::bounds(cx, "app-wordmark").right()
    };
    let initial_gap = gap(cx);
    if initial_gap >= ARROW_ROOM {
        // A release's build string can be shorter. Keep the 296-pixel width and consume enough of the extra room
        // in the inset to exercise the same narrow-gap boundary with the production layout.
        traffic_lights.set(traffic_lights.get() + initial_gap - ARROW_ROOM + px(1.));
        test_support::redraw(cx);
    }
    assert!(gap(cx) < ARROW_ROOM);
    for _ in 0..3 {
        advance_reminder(cx, ticker::TICK);
    }
    let pulses = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            readout
                .read(cx)
                .reminder
                .frame(cx)
                .is_some_and(|frame| frame.pulsing)
        })
    };
    assert!(pulses(cx));
    assert!(!drawn(cx, "upgrade-reminder-arrow"));

    width.set(px(380.));
    test_support::redraw(cx);
    assert!(drawn(cx, "upgrade-reminder-arrow"));
    let before = test_support::bounds(cx, "app-version-toggle");
    let wordmark = test_support::bounds(cx, "app-wordmark");
    let arrow = test_support::bounds(cx, "upgrade-reminder-arrow");
    assert!(before.left() - wordmark.right() >= ARROW_ROOM);
    assert!(arrow.left() >= wordmark.right() && arrow.right() <= before.left());
    advance_reminder(cx, ticker::TICK * 2);
    assert_ne!(
        test_support::bounds(cx, "upgrade-reminder-arrow").left(),
        arrow.left()
    );
    assert_eq!(test_support::bounds(cx, "app-version-toggle"), before);
    publish(cx, &feed, readout_in(Activity::Checking, true, 3));
    assert!(drawn(cx, "app-update-status"));
    assert!(pulses(cx));
    assert!(!drawn(cx, "upgrade-reminder-arrow"));
}

/// The main window keeps its sidebar across a channel break. Hiding the header must release the held animation
/// and deadline even though the readout will not render again until reconnect.
#[gpui::test]
fn disconnected_header_releases_the_reminder(cx: &mut TestAppContext) {
    let (mut helm, _) = test_support::started(cx);
    let (feed, _) = keep_fake(cx, readout_in(Activity::Idle, false, 1));
    helm.connect(Vec::new());
    cx.run_until_parked();
    let (main, cx) = test_support::open_view(cx, crate::main_view::MainView::new);
    let readout = cx
        .update(|_, cx| main.read(cx).update_readout(cx))
        .expect("the sidebar has an updater");
    publish(cx, &feed, readout_in(Activity::Idle, true, 2));
    assert!(drawn(cx, "app-version-toggle"));
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));

    helm.break_channel();
    test_support::settle(cx);
    assert!(!drawn(cx, "app-version-toggle"));
    assert_eq!(reminder_state(cx, &readout), (None, false, false));
    advance_reminder(cx, FIFTEEN_MINUTES * 2);
    assert_eq!(reminder_state(cx, &readout), (None, false, false));

    helm.connect(Vec::new());
    test_support::settle(cx);
    assert!(drawn(cx, "app-version-toggle"));
    assert_eq!(reminder_state(cx, &readout), (None, true, false));

    // An arrival already due survives the disconnected screen, even if the window activates before reconnecting.
    publish(cx, &feed, readout_in(Activity::Idle, false, 3));
    cx.deactivate_window();
    assert!(!cx.update(|window, cx| ticker::is_active(window, cx)));
    publish(cx, &feed, readout_in(Activity::Idle, true, 4));
    assert_eq!(reminder_state(cx, &readout), (None, false, true));
    helm.break_channel();
    test_support::settle(cx);
    cx.update(|window, _| window.activate_window());
    test_support::settle(cx);
    assert!(!drawn(cx, "app-version-toggle"));
    assert_eq!(reminder_state(cx, &readout), (None, false, true));
    helm.connect(Vec::new());
    test_support::settle(cx);
    assert!(drawn(cx, "app-version-toggle"));
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));

    // An idle periodic reminder remains due across a connection change. The connection alone is not new input.
    let spent = complete_reminder(cx, &readout);
    advance_reminder(cx, FIFTEEN_MINUTES - spent);
    assert_eq!(reminder_state(cx, &readout), (None, false, true));
    helm.break_channel();
    test_support::settle(cx);
    helm.connect(Vec::new());
    test_support::settle(cx);
    assert!(drawn(cx, "app-version-toggle"));
    assert_eq!(reminder_state(cx, &readout), (None, false, true));
    test_support::keys(cx, "tab");
    assert_eq!(reminder_state(cx, &readout), (Some(0), true, false));
}

/// Move keyboard focus to the check button with Tab, asserting it got there. The second Tab is for Linux, where an
/// earlier click on the button focused it (on macOS a click focuses no button), so the first Tab moved on from it.
fn tab_to_check(cx: &mut VisualTestContext) {
    test_support::keys(cx, "tab");
    if test_support::focused_id(cx).as_deref() != Some("app-version-check") {
        test_support::keys(cx, "tab");
    }
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("app-version-check"),
        "the premise: keyboard focus is on the check button"
    );
}

// ---- The look ------------------------------------------------------------------------------------------------------

/// SPEC.md: the version's hint "stays dim at rest", and "hover and keyboard focus brighten it and turn it a quarter
/// turn"; a check in progress spins the glyph instead, and a reinstall warning keeps the retry action with the
/// version in the warning ink. Specified as the pure look the check button draws: at rest the version is `--fg-1` and
/// the glyph `--fg-2`, unturned; lit, both `--fg-0` and the glyph two eighths turned; spinning, the glyph `--fg-0` at
/// its step whether lit or not; with a warning, the version is `--warn` at rest and `--fg-0` lit.
#[test]
fn hover_and_keyboard_focus_brighten_the_version_and_turn_the_glyph() {
    let palette = &theme::DARK;
    let rest = check_look(palette, false, None, false);
    assert_eq!(
        (rest.version, rest.glyph, rest.eighths),
        (palette.fg_1, palette.fg_2, 0)
    );
    let lit = check_look(palette, true, None, false);
    assert_eq!(
        (lit.version, lit.glyph, lit.eighths),
        (palette.fg_0, palette.fg_0, 2)
    );
    for lit in [false, true] {
        let spinning = check_look(palette, lit, Some(5), false);
        assert_eq!(
            (spinning.glyph, spinning.eighths),
            (palette.fg_0, 5),
            "lit: {lit}"
        );
    }
    assert_eq!(check_look(palette, false, None, true).version, palette.warn);
    assert_eq!(check_look(palette, true, None, true).version, palette.fg_0);
}

// ---- The check button ----------------------------------------------------------------------------------------------

/// Selecting the version while an updater runs and no update waits checks for updates (SPEC.md: "Selecting the version
/// checks for updates now"), and the hint brightens and turns under the pointer and on keyboard focus. Specified: the
/// version is drawn as `app-version-check`, named for this build and the check, with no menu toggle; at rest it is not
/// lit; hovering lights it; a click and Enter on the focused button each reach the updater's check once; keyboard
/// focus lights it.
#[gpui::test]
fn the_idle_version_is_a_named_check_button(cx: &mut TestAppContext) {
    let (cx, readout, _feed, seen, _other) = open(cx, readout_in(Activity::Idle, false, 0));
    let palette = *cx.update(|_, cx| theme::palette(cx));
    assert!(drawn(cx, "app-version-check"));
    assert!(!drawn(cx, "app-version-toggle"));
    let name = label(cx, "app-version-check").unwrap_or_default();
    assert!(
        name.starts_with(&format!(
            "Farhelm {}; check for updates.",
            farhelm_proto::BUILD_VERSION
        )),
        "{name}"
    );
    assert_eq!(
        look(cx, &readout).map(|look| look.eighths),
        Some(0),
        "at rest"
    );
    let center = test_support::center(cx, "app-version-check");
    cx.simulate_mouse_move(center, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(
        look(cx, &readout).map(|look| (look.version, look.eighths)),
        Some((palette.fg_0, 2)),
        "hover lights it"
    );
    test_support::click(cx, "app-version-check");
    assert_eq!(seen.checks.get(), 1);
    cx.simulate_mouse_move(point(px(500.), px(500.)), None, Modifiers::none());
    test_support::settle(cx);
    if test_support::focused_id(cx).as_deref() != Some("app-version-check") {
        assert_eq!(
            look(cx, &readout).map(|look| look.eighths),
            Some(0),
            "the premise: with the pointer gone and no focus, it rests"
        );
    }
    tab_to_check(cx);
    assert_eq!(
        look(cx, &readout).map(|look| look.eighths),
        Some(2),
        "keyboard focus lights it"
    );
    test_support::keys(cx, "enter");
    assert_eq!(seen.checks.get(), 2, "Enter checks too");
}

/// A reinstall warning keeps the retry action (SPEC.md: selecting the version still retries) and puts the warning mark
/// before the version, whose ink turns to the warning's (a withdrawal draws the same; its mark is in the toggle's test). Specified: with a reinstall needed the
/// version is still the check button, `⚠` is painted, the look's version ink is `--warn` at rest, and a click checks.
#[gpui::test]
fn a_reinstall_warning_keeps_the_check_and_its_mark(cx: &mut TestAppContext) {
    let (cx, readout, _feed, seen, _other) = open(
        cx,
        Readout {
            needs_reinstall: true,
            ..readout_in(Activity::Idle, false, 0)
        },
    );
    let palette = *cx.update(|_, cx| theme::palette(cx));
    assert!(drawn(cx, "app-version-check"));
    assert!(shows(cx, "⚠"));
    assert_eq!(
        look(cx, &readout).map(|look| look.version),
        Some(palette.warn)
    );
    test_support::click(cx, "app-version-check");
    assert_eq!(seen.checks.get(), 1);
}

// ---- Progress and the spinner --------------------------------------------------------------------------------------

/// While a check the user asked for runs, the readout says so beside the version and the glyph spins, with no second
/// spinner (SPEC.md: "Checking…, Installing…"; one progress indicator). Specified: Checking paints "Checking…" and the
/// check button's glyph advances with the ticker; Installing paints "Installing…"; neither has the separate ring.
#[gpui::test]
fn a_visible_check_spins_the_glyph_and_shows_its_words(cx: &mut TestAppContext) {
    let (cx, readout, feed, _seen, _other) = open(cx, readout_in(Activity::Checking, false, 1));
    assert!(shows(cx, "Checking…"));
    assert!(holds_spinner(cx, &readout), "the glyph spins");
    let first = look(cx, &readout).map(|look| look.eighths);
    test_support::advance(cx, Duration::from_millis(300));
    assert_ne!(
        look(cx, &readout).map(|look| look.eighths),
        first,
        "the glyph advanced"
    );
    let checking = readout_in(Activity::Checking, false, 1);
    assert!(!inline_presentation(&checking, None).separate_progress_spinner);
    assert!(
        cx.debug_bounds("app-update-ring").is_none(),
        "no separate ring beside the spinning glyph"
    );
    publish(
        cx,
        &feed,
        readout_in(Activity::Installing("1.2.4".into()), false, 1),
    );
    assert!(shows(cx, "Installing…"));
}

/// While an update waits, the toggle has no glyph to spin, so progress gets a ring of its own (SPEC.md: "the separate
/// spinner"). Specified: with an update ready and a check running, the separate ring is part of the presentation, the
/// readout holds its spinner, and "Checking…" is painted beside the toggle.
#[gpui::test]
fn a_waiting_update_shows_progress_with_its_own_spinner(cx: &mut TestAppContext) {
    let ready_checking = readout_in(Activity::Checking, true, 1);
    assert!(inline_presentation(&ready_checking, None).separate_progress_spinner);
    let (cx, readout, _feed, _seen, _other) = open(cx, ready_checking);
    assert!(drawn(cx, "app-version-toggle"));
    assert!(shows(cx, "Checking…"));
    assert!(holds_spinner(cx, &readout));
    assert!(
        cx.debug_bounds("app-update-ring").is_some(),
        "the ring is drawn"
    );
}

/// Animation follows the ticker's rules: nothing turns once the work is done, nothing turns in an inactive window, and
/// under reduced motion the glyph stands still while the words stay (SPEC.md; the old spec's reduced-motion case).
/// Specified: idle holds no spinner; Checking holds one until the window goes inactive; under reduced motion the
/// glyph stays at step 0 over a second and "Checking…" is still painted.
#[gpui::test]
fn the_spinner_runs_only_while_something_spins_and_stands_still_under_reduced_motion(
    cx: &mut TestAppContext,
) {
    let (cx, readout, feed, _seen, _other) = open(cx, readout_in(Activity::Idle, false, 0));
    assert!(!holds_spinner(cx, &readout), "idle: nothing spins");
    publish(cx, &feed, readout_in(Activity::Checking, false, 1));
    assert!(holds_spinner(cx, &readout), "the premise: a check spins");
    cx.deactivate_window();
    test_support::settle(cx);
    assert!(
        !holds_spinner(cx, &readout),
        "an inactive window spins nothing"
    );
    cx.update(|window, cx| {
        crate::ticker::set_reduced_motion(true, cx);
        window.activate_window();
    });
    test_support::settle(cx);
    assert!(
        holds_spinner(cx, &readout),
        "the premise: active again, it holds its spinner"
    );
    // Three ticks, not a whole turn, so a glyph that kept turning could not land back on its first step.
    test_support::advance(cx, crate::ticker::TICK * 3);
    assert_eq!(look(cx, &readout).map(|look| look.eighths), Some(0));
    assert!(shows(cx, "Checking…"), "the words stay");
}

// ---- What a screenshot golden holds still --------------------------------------------------------------------------

/// The version readout goldens light the check button with a rested pointer and show a turn with a held step, and
/// they are evidence for the app only while both go through what the app draws from (SPEC_impl.md, "GPUI cookbook:
/// traps", on `cfg(test)` branches). Specified: a rested pointer lights the button exactly as a real hover does, and a
/// real pointer passing over and leaving puts it back at rest, since the rest is the hover listener's own state; a
/// held step turns nothing at rest, and once a check spins the glyph is drawn at that step.
#[gpui::test]
fn a_golden_rests_the_pointer_and_holds_the_step_through_the_drawn_inputs(cx: &mut TestAppContext) {
    let (cx, readout, feed, _seen, _other) = open(cx, readout_in(Activity::Idle, false, 0));
    let palette = *cx.update(|_, cx| theme::palette(cx));
    let lit = Some((palette.fg_0, 2));
    let ink_and_turn =
        |cx: &mut VisualTestContext| look(cx, &readout).map(|look| (look.version, look.eighths));
    assert_eq!(
        ink_and_turn(cx),
        Some((palette.fg_1, 0)),
        "the premise: at rest"
    );
    readout.update(cx, |readout, cx| readout.rest_pointer(cx));
    test_support::settle(cx);
    assert_eq!(ink_and_turn(cx), lit, "a rested pointer lights it");
    // A real pointer enters before it leaves: the hover listener reports a leave only after it saw the pointer over
    // the control.
    let center = test_support::center(cx, "app-version-check");
    cx.simulate_mouse_move(center, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(ink_and_turn(cx), lit, "the pointer entering keeps it lit");
    cx.simulate_mouse_move(point(px(500.), px(500.)), None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(
        ink_and_turn(cx),
        Some((palette.fg_1, 0)),
        "the pointer left"
    );

    readout.update(cx, |readout, cx| {
        readout.step_hold = Some(5);
        cx.notify();
    });
    test_support::settle(cx);
    assert_eq!(
        look(cx, &readout).map(|look| look.eighths),
        Some(0),
        "a held step turns nothing at rest"
    );
    publish(cx, &feed, readout_in(Activity::Checking, false, 1));
    assert_eq!(look(cx, &readout).map(|look| look.eighths), Some(5));
    assert!(
        holds_spinner(cx, &readout),
        "the premise: the turn runs, so the ticks below would move an unheld glyph"
    );
    test_support::advance(cx, crate::ticker::TICK * 3);
    assert_eq!(
        look(cx, &readout).map(|look| look.eighths),
        Some(5),
        "held while the turn runs on"
    );
}

// ---- The update toggle and its menu --------------------------------------------------------------------------------

/// An installed update turns the readout into the red menu toggle with the update arrow, and a warning that stands keeps
/// its mark before the arrow (SPEC.md: "the version readout turns red with an up-arrow"). Specified: with an update
/// ready the toggle is drawn, named for the build and the update, with `↑` painted and no check button; with a
/// withdrawal as well, `⚠` and `↑` are both painted.
#[gpui::test]
fn an_installed_update_turns_the_version_into_the_red_menu_toggle(cx: &mut TestAppContext) {
    let (cx, _readout, feed, _seen, _other) = open(cx, readout_in(Activity::Idle, true, 0));
    assert!(drawn(cx, "app-version-toggle"));
    assert!(!drawn(cx, "app-version-check"));
    let name = label(cx, "app-version-toggle").unwrap_or_default();
    assert!(name.contains("update ready"), "{name}");
    assert!(shows(cx, "↑"));
    publish(
        cx,
        &feed,
        Readout {
            withdrawn: true,
            ..readout_in(Activity::Idle, true, 0)
        },
    );
    assert!(shows(cx, "⚠") && shows(cx, "↑"));
}

/// The toggle's menu offers Restart to update and What's new, and each closes the menu before it acts (SPEC.md: "the
/// menu with Restart to update and What's new"; What's new opens the release notes in the system browser). Specified:
/// a click on the toggle draws the menu; What's new closes it and opens the release notes; opened again, Restart to
/// update closes it and reaches the updater's restart.
#[gpui::test]
fn the_update_menu_offers_restart_and_whats_new(cx: &mut TestAppContext) {
    let (cx, _readout, _feed, seen, _other) = open(cx, readout_in(Activity::Idle, true, 0));
    test_support::click(cx, "app-version-toggle");
    test_support::settle(cx);
    assert!(drawn(cx, "update-menu"), "the menu opened");
    test_support::click(cx, "update-menu-whats-new");
    test_support::settle(cx);
    assert!(!drawn(cx, "update-menu"), "What's new closed it");
    assert_eq!(cx.opened_url().as_deref(), Some(RELEASE_NOTES_URL));
    test_support::click(cx, "app-version-toggle");
    test_support::settle(cx);
    test_support::click(cx, "update-menu-restart");
    test_support::settle(cx);
    assert!(!drawn(cx, "update-menu"), "Restart closed it");
    assert_eq!(seen.restarts.get(), 1);
}

/// The menu belongs to the waiting update: once no update waits (a newer check withdrew it, say) the toggle gives way
/// to the check button, an open menu must not outlive it, and focus inside it must land on the readout rather than
/// fall out of the window. Specified: with the menu open and focus on its first item, a readout without an update
/// waiting closes the menu, draws the check button and leaves focus on it.
#[gpui::test]
fn the_update_menu_closes_when_no_update_waits(cx: &mut TestAppContext) {
    let (cx, _readout, feed, _seen, _other) = open(cx, readout_in(Activity::Idle, true, 0));
    test_support::click(cx, "app-version-toggle");
    test_support::settle(cx);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("update-menu-restart"),
        "the premise: the menu is open with focus inside"
    );
    publish(cx, &feed, readout_in(Activity::Idle, false, 0));
    assert!(!drawn(cx, "update-menu"));
    assert!(drawn(cx, "app-version-check"));
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("app-version-check")
    );
}

// ---- The inline status ---------------------------------------------------------------------------------------------

/// Up to date shows for about five seconds after a check finds nothing newer, and only the result that scheduled it may
/// hide it; a refresh of the same result must neither hide nor stretch it (SPEC.md: "Up to date for about five
/// seconds"). Specified: Up to date is painted; three seconds in, a republished readout with the same result keeps it
/// painted; five seconds after the result it is gone; a new check's result shows it again.
#[gpui::test]
fn up_to_date_shows_for_five_seconds(cx: &mut TestAppContext) {
    let up_to_date = |id| readout_in(Activity::UpToDate("1.2.3".into()), false, id);
    let (cx, _readout, feed, _seen, _other) = open(cx, up_to_date(1));
    assert!(shows(cx, "Up to date"));
    test_support::advance(cx, Duration::from_secs(3));
    publish(
        cx,
        &feed,
        Readout {
            tooltip: "refreshed".into(),
            ..up_to_date(1)
        },
    );
    assert!(shows(cx, "Up to date"), "a refresh keeps it");
    test_support::advance(cx, UP_TO_DATE_FOR - Duration::from_secs(3));
    assert!(
        !shows(cx, "Up to date"),
        "gone five seconds after the result"
    );
    publish(cx, &feed, up_to_date(2));
    assert!(shows(cx, "Up to date"), "a new check shows it again");
}

/// A failed check stays beside the readout until the user dismisses it or checks again, and a refresh of the same
/// failure must not bring a dismissed one back (SPEC.md, the rules for when Check failed stays). Specified: Check failed
/// is a button named for dismissing it; it survives a republish of the same failure; a click hides it; a new check's
/// failure shows it again.
#[gpui::test]
fn check_failed_stays_until_dismissed_or_the_next_check(cx: &mut TestAppContext) {
    let failed = |id| readout_in(Activity::Failed("offline".into()), false, id);
    let (cx, _readout, feed, _seen, _other) = open(cx, failed(1));
    assert_eq!(
        label(cx, "app-update-failure").as_deref(),
        Some("Check failed; dismiss update check status")
    );
    publish(
        cx,
        &feed,
        Readout {
            tooltip: "refreshed".into(),
            ..failed(1)
        },
    );
    assert!(drawn(cx, "app-update-failure"), "a refresh keeps it");
    test_support::click(cx, "app-update-failure");
    test_support::settle(cx);
    assert!(!drawn(cx, "app-update-failure"), "dismissed");
    publish(cx, &feed, failed(2));
    assert!(
        drawn(cx, "app-update-failure"),
        "a new check's failure shows again"
    );
}

/// A failed restart is said in the hover only (SPEC.md), so nothing joins the readout for it. Specified: with the
/// restart failed, the installed update's readout paints the same words. Resting on its toggle reveals the reason.
#[gpui::test]
fn a_failed_restart_shows_nothing_inline(cx: &mut TestAppContext) {
    let reason = "Restart could not finish.";
    let (cx, _readout, feed, _seen, _other) = open(cx, readout_in(Activity::Idle, true, 0));
    assert!(drawn(cx, "app-version-toggle") && shows(cx, "↑"));
    let ready = painted(cx);
    let mut state = readout_in(Activity::RestartFailed, true, 1);
    state.tooltip = reason.into();
    publish(cx, &feed, state);
    assert_eq!(painted(cx), ready, "failure adds no inline text");
    let button = test_support::center(cx, "app-version-toggle");
    cx.simulate_mouse_move(button, None, Modifiers::none());
    test_support::settle(cx);
    test_support::advance(cx, crate::layers::HOVER_DELAY);
    assert!(
        painted(cx).join(" ").contains(reason),
        "the hover help shows the reason"
    );
}

// ---- Focus ---------------------------------------------------------------------------------------------------------

/// When an install makes an update ready, the check button becomes the menu toggle; a user whose focus was on the
/// readout must keep it there, and nobody else's focus may move (the old app's focus hand-off, its three cases).
/// Specified: with nothing focused, the swap focuses nothing; with another control focused, it keeps focus; with
/// keyboard focus on the check button, the toggle that replaces it holds focus.
#[gpui::test]
fn an_installed_update_keeps_focus_on_the_readout_only_if_it_had_it(cx: &mut TestAppContext) {
    let (cx, _readout, feed, _seen, other) = open(cx, readout_in(Activity::Idle, false, 0));
    assert_eq!(
        test_support::focused_id(cx),
        None,
        "the premise: nothing focused"
    );
    publish(cx, &feed, readout_in(Activity::Idle, true, 0));
    assert_eq!(
        test_support::focused_id(cx),
        None,
        "nothing was focused, nothing is"
    );

    publish(cx, &feed, readout_in(Activity::Idle, false, 0));
    cx.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        crate::focus::focus_control(&other, &mark, window, cx)
    });
    test_support::settle(cx);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("other"),
        "the premise: another control holds focus"
    );
    publish(cx, &feed, readout_in(Activity::Idle, true, 0));
    assert_eq!(test_support::focused_id(cx).as_deref(), Some("other"));

    publish(cx, &feed, readout_in(Activity::Idle, false, 0));
    // From the other control back to the readout, which comes first in the tab order.
    test_support::keys(cx, "shift-tab");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("app-version-check"),
        "the premise: keyboard focus is on the check button"
    );
    publish(cx, &feed, readout_in(Activity::Idle, true, 0));
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("app-version-toggle")
    );
}

// ---- In the header -------------------------------------------------------------------------------------------------

/// The readout is the header's version while an updater runs; a header that kept drawing the plain text would hide
/// every update state. Specified on the real main view: with an updater kept before the window opens, the header draws
/// the check button and not the plain `app-version` text.
#[gpui::test]
fn in_the_header_the_readout_replaces_the_plain_version_while_an_updater_runs(
    cx: &mut TestAppContext,
) {
    let (mut fake, _connection) = test_support::started(cx);
    let _kept = keep_fake(cx, readout_in(Activity::Idle, false, 0));
    fake.connect(vec![test_support::sessions(&[])]);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, crate::main_view::MainView::new);
    test_support::settle(window);
    assert!(drawn(window, "app-version-check"));
    assert!(!drawn(window, "app-version"));
}
