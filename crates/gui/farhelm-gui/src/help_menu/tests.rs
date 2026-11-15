//! Tests of the `?` button and its Help menu: the items and their order for every combination of updater, shown
//! announcements and platform, the button's place and names at the rail's foot, opening and closing the menu, where
//! focus goes, and what each item does.
//!
//! Like the session list's tests, these run the real main view over the helm client's fake and read controls from
//! GPUI's driver record of the frame by their ids. Links are checked through GPUI's test platform, which records the
//! URL the app asked it to open, so nothing reaches a browser. The menu's order is a pure function of whether an
//! updater runs, whether this helm has shown an announcement and whether this is Linux, tested as such for every
//! combination.

use std::cell::Cell;
use std::rc::Rc;

use farhelm_proto::ui_channel::CollectionState;
use gpui::{DriverNode, Modifiers, Role, TestAppContext, VisualTestContext, px, size};

use super::{BUTTON_ID, Item, items};
use crate::desktop::updates::{self, UpdaterHost};
use crate::layers;
use crate::session_list::tests::{listing, open_main, preferences, session};
use crate::test_support::{self, any_open, focused_id, frame};
use crate::update_readout::{Activity, Readout};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The Settings gear's element id, the control the `?` button sits under.
const GEAR: &str = "app-settings-toggle";

/// The open menu's panel id.
const MENU: &str = "help-menu";

/// A window with two sessions and no host registry read, enough for the rail's foot to draw.
fn snapshot() -> Vec<CollectionState> {
    vec![
        listing(&["a", "b"], &[session("a"), session("b")]),
        preferences(&["a"], None),
    ]
}

/// The node drawn under `id`, if any.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// The menu's items as drawn, top to bottom, by element id.
fn drawn_items(cx: &mut VisualTestContext) -> Vec<String> {
    let mut drawn: Vec<_> = frame(cx)
        .into_iter()
        .filter(|node| node.role == Some(Role::MenuItem))
        .filter_map(|node| {
            let id = node.element_id.clone()?;
            id.starts_with("help-menu-")
                .then_some((node.bounds.top(), id))
        })
        .collect();
    drawn.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    drawn.into_iter().map(|(_, id)| id.to_string()).collect()
}

/// The ids [`items`] gives for this platform, with or without an updater, before any announcement was shown: what
/// the menu should draw.
fn expected(updater: bool) -> Vec<String> {
    items(updater, false, cfg!(target_os = "linux"))
        .into_iter()
        .map(|item| item.id().to_owned())
        .collect()
}

/// The hover help the control drawn under `selector` shows once the pointer rests on it.
fn hover_help(cx: &mut VisualTestContext, selector: &'static str) -> Option<String> {
    let at = test_support::center(cx, selector);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// An updater that counts the checks asked of it and otherwise does nothing: its readout never changes.
fn counting_updater(checks: &Rc<Cell<u32>>) -> UpdaterHost {
    let counted = checks.clone();
    UpdaterHost {
        first_readout: Readout {
            update_ready: false,
            needs_reinstall: false,
            withdrawn: false,
            tooltip: "Up to date".to_string(),
            activity: Activity::Idle,
            notice_id: 0,
            user_update_ready: false,
        },
        next_readout: Box::new(|| Box::pin(std::future::pending())),
        check_now: Box::new(move || counted.set(counted.get() + 1)),
        restart_to_update: Box::new(|| false),
        automatic_updates: Box::new(|| false),
        set_automatic_updates: Box::new(|_| Ok(())),
    }
}

/// Open the menu with a click on the `?` button, checking that it opened.
fn open_menu(cx: &mut VisualTestContext) {
    test_support::click(cx, BUTTON_ID);
    assert!(
        node(&frame(cx), MENU).is_some(),
        "the click opened the Help menu"
    );
}

// ---- The order of the items ----------------------------------------------------------------------------------------

/// SPEC.md fixes the menu's order ("in order: **Send feedback**, **Chat on Discord**, **Documentation**, **Keyboard
/// shortcuts**, **Release notes**, **SSH needs 2FA?** and **Welcome**", "Announcements appears directly after Release
/// notes", "Check for updates last"), and the Linux-only notices item has no place in SPEC.md, so it goes at the very
/// end where it moves nothing else. An item out of place is a menu the documentation no longer describes. Specified,
/// for all eight combinations: Send feedback, Discord, Documentation, Keyboard shortcuts, Release notes, SSH help and
/// Welcome always, in that order, Send feedback first; a shown announcement adds Announcements right after Release
/// notes; an updater adds Check for updates after the fixed items; Linux adds Open-Source Notices after everything,
/// including after Check for updates.
#[test]
fn the_items_keep_specs_order_with_the_update_check_and_the_linux_notices_last() {
    for updater in [false, true] {
        for announced in [false, true] {
            for linux in [false, true] {
                let mut expected = vec![
                    Item::SendFeedback,
                    Item::ChatOnDiscord,
                    Item::Documentation,
                    Item::KeyboardShortcuts,
                    Item::ReleaseNotes,
                ];
                if announced {
                    expected.push(Item::Announcements);
                }
                expected.push(Item::SshHelp);
                expected.push(Item::Welcome);
                if updater {
                    expected.push(Item::CheckForUpdates);
                }
                if linux {
                    expected.push(Item::OpenSourceNotices);
                }
                assert_eq!(
                    items(updater, announced, linux),
                    expected,
                    "updater {updater}, announced {announced}, linux {linux}"
                );
            }
        }
    }
}

// ---- The button at the rail's foot ---------------------------------------------------------------------------------

/// SPEC.md: "The host rail's foot has a **?** button below the settings gear." A button that drifted or lost its name
/// would leave keyboard and screen reader users without the way into help. Specified: in a 1200 by 700 window, the
/// button named "Help" is a 40-pixel square 7.5 pixels from the window's left edge, directly below the gear, with its
/// bottom 8 pixels above the window's (the old foot's padding); its hover help names the menu's contents
/// without the update check while no updater runs.
#[gpui::test]
fn the_help_button_sits_below_the_gear_with_its_name_and_help(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    cx.simulate_resize(size(px(1200.), px(700.)));
    test_support::settle(cx);
    let nodes = frame(cx);
    let button = node(&nodes, BUTTON_ID).expect("the ? button is drawn");
    assert_eq!(button.label.as_deref(), Some("Help"));
    assert_eq!(button.role, Some(Role::Button));
    let at = button.bounds;
    assert_eq!((at.size.width, at.size.height), (px(40.), px(40.)));
    assert_eq!(at.left(), px(7.5));
    assert_eq!(at.bottom(), px(700. - 8.));
    let gear = node(&nodes, GEAR).expect("the gear is drawn").bounds;
    assert_eq!(
        gear.bottom(),
        at.top(),
        "the ? button sits directly below the gear"
    );
    assert_eq!(
        hover_help(cx, BUTTON_ID).as_deref(),
        Some(
            "Help: send feedback, open the documentation or release notes, chat on Discord, get SSH 2FA help, or show Welcome"
        )
    );
}

// ---- Opening and closing -------------------------------------------------------------------------------------------

/// SPEC.md's "Opening and closing": closing something the user opened returns focus to where it was, "including those
/// where clicking the button that opened it did not move focus to the button", which is macOS, so this runs under that
/// click rule. The menu is the user's way to every help surface, so its header, its items and their order are its
/// interface. GPUI's driver record has no expanded field, so the button's expanded state is not read back here; the
/// button's `aria-expanded` wiring is the one every expanded button shares. Specified: with focus away from the `?`
/// button, a click on it opens the menu beside it (its left edge right of the button's right edge), headed "Help" over
/// "Farhelm guides and feedback", its items the ones [`items`] gives for this platform without an updater, in that
/// order, and focus starts on the first, Send feedback; Down moves focus to the second; Escape closes the menu with
/// focus on the `?` button.
#[gpui::test]
fn the_menu_opens_beside_the_button_and_escape_returns_focus_to_it(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    assert_ne!(
        focused_id(cx).as_deref(),
        Some(BUTTON_ID),
        "premise: focus is away from the ? button, so returning to it is the menu's doing"
    );
    open_menu(cx);
    assert!(
        test_support::bounds(cx, MENU).left() > test_support::bounds(cx, BUTTON_ID).right(),
        "the menu opens beside the button"
    );
    let header: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|text| text.text.to_string()))
        .filter(|text| text == "Help" || text == "Farhelm guides and feedback")
        .collect();
    assert!(
        header.contains(&"Help".to_owned())
            && header.contains(&"Farhelm guides and feedback".to_owned()),
        "the menu is headed Help over its summary: {header:?}"
    );
    assert_eq!(drawn_items(cx), expected(false));
    assert_eq!(focused_id(cx).as_deref(), Some("help-menu-send-feedback"));
    test_support::keys(cx, "down");
    assert_eq!(focused_id(cx).as_deref(), Some("help-menu-discord"));
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed the menu");
    assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));
}

/// The old toggle closed its menu when pressed again, and a press that both closed and reopened it would leave the
/// user unable to close the menu with the button they opened it with. Specified: with the menu open, a second click on
/// the `?` button closes it, and a third opens it again.
#[gpui::test]
fn a_second_press_on_the_button_closes_the_menu(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    test_support::check(cx);
    open_menu(cx);
    test_support::click(cx, BUTTON_ID);
    assert!(!any_open(cx), "the second click closed the menu");
    open_menu(cx);
}

// ---- What the items do ---------------------------------------------------------------------------------------------

/// SPEC.md names each link's address and says they open in the system browser through the app's one opener, whether or
/// not an update waits; a wrong address sends users somewhere else. Each item also closes the menu, leaving focus on
/// the `?` button. Specified: Chat on Discord opens `https://discord.gg/G3bukxTjXR`, Documentation
/// `https://farhelm.io/docs/`, Release notes `https://farhelm.io/docs/release-notes/`; with focus away from the `?`
/// button to begin with, after each, no layer is open and focus is on the `?` button.
#[gpui::test]
fn the_link_items_open_their_pages_and_close_the_menu(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    assert_ne!(
        focused_id(cx).as_deref(),
        Some(BUTTON_ID),
        "premise: focus is away from the ? button, so returning to it is the menu's doing"
    );
    for (item, url) in [
        ("help-menu-discord", "https://discord.gg/G3bukxTjXR"),
        ("help-menu-documentation", "https://farhelm.io/docs/"),
        (
            "help-menu-release-notes",
            "https://farhelm.io/docs/release-notes/",
        ),
    ] {
        open_menu(cx);
        test_support::click(cx, item);
        assert_eq!(cx.opened_url().as_deref(), Some(url), "{item}");
        assert!(!any_open(cx), "{item} closed the menu");
        assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID), "{item}");
    }
}

/// SPEC.md's SSH needs 2FA? opens the SSH help dialog, and the old app returned focus to its Help toggle when it
/// closed. (Closing the menu has already put focus on the button when SSH help opens, so this pins where focus ends up,
/// not which opener the dialog was given.) Specified: choosing SSH needs 2FA? from the keyboard closes the menu and
/// opens SSH help with focus inside it; Escape closes the dialog and leaves focus on the `?` button.
#[gpui::test]
fn ssh_help_opens_from_the_menu_and_returns_focus_to_the_button(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    open_menu(cx);
    test_support::keys(cx, "down down down down down");
    assert_eq!(focused_id(cx).as_deref(), Some("help-menu-ssh-help"));
    test_support::keys(cx, "enter");
    assert!(node(&frame(cx), MENU).is_none(), "the menu closed");
    assert!(any_open(cx), "SSH help opened");
    assert_ne!(
        focused_id(cx).as_deref(),
        Some(BUTTON_ID),
        "focus is inside SSH help"
    );
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed SSH help");
    assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));
}

/// SPEC.md's Send feedback opens the feedback dialog, and the old app returned focus to its Help toggle when it closed,
/// whether it was dismissed or closed itself after a send. (Closing the menu has already put focus on the button when
/// the dialog opens, so this pins where focus ends up; the dialog's own tests open it with focus elsewhere.) Specified:
/// choosing Send feedback, the menu's first item, from the keyboard closes the menu and opens the dialog with focus
/// inside it (in its message field, which the dialog's own tests pin); Escape closes the dialog and leaves focus on the
/// `?` button.
#[gpui::test]
fn send_feedback_opens_the_dialog_and_returns_focus_to_the_button(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    open_menu(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("help-menu-send-feedback"));
    test_support::keys(cx, "enter");
    assert!(node(&frame(cx), MENU).is_none(), "the menu closed");
    assert!(
        node(&frame(cx), "feedback").is_some(),
        "the feedback dialog opened"
    );
    assert_ne!(
        focused_id(cx).as_deref(),
        Some(BUTTON_ID),
        "focus is inside the dialog"
    );
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed the dialog");
    assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));
}

/// SPEC.md: "The installed desktop app's menu adds **Check for updates** last", and every update surface asks whether
/// an updater runs before drawing anything. Without that check a devbuild or the Linux app would offer an item that
/// does nothing; with it, the item must reach the updater. Specified: with an updater, the menu draws the items
/// [`items`] gives with an updater, the button's hover help names the update check, and choosing Check for updates asks
/// the updater for one check and closes the menu.
#[gpui::test]
fn check_for_updates_is_offered_only_with_an_updater_and_asks_it_to_check(cx: &mut TestAppContext) {
    let checks = Rc::new(Cell::new(0));
    let host = counting_updater(&checks);
    cx.update(|cx| updates::keep(Some(host), cx));
    let (_fake, cx) = open_main(cx, snapshot());
    test_support::check(cx);
    assert_eq!(
        hover_help(cx, BUTTON_ID).as_deref(),
        Some(
            "Help: send feedback, open the documentation or release notes, chat on Discord, get SSH 2FA help, show Welcome, or check for updates"
        )
    );
    open_menu(cx);
    assert_eq!(drawn_items(cx), expected(true));
    test_support::click(cx, "help-menu-check-for-updates");
    assert_eq!(checks.get(), 1, "one check was asked for");
    assert!(!any_open(cx), "the item closed the menu");
}

/// Linux has no menu bar, so the Help menu is the only way to the Open-Source Notices window the Mac app menu opens;
/// the item must dispatch the same action the Mac menu's item does, which the desktop integration handles app-wide.
/// Specified: on Linux, choosing Open-Source Notices dispatches `OpenSourceNotices` once and closes the menu.
#[cfg(target_os = "linux")]
#[gpui::test]
fn open_source_notices_dispatches_the_app_menus_action_on_linux(cx: &mut TestAppContext) {
    // Imported here, inside the Linux-only test that alone uses it: at the top of the file it is an unused import on
    // macOS.
    use crate::actions::OpenSourceNotices;

    let (_fake, cx) = open_main(cx, snapshot());
    let dispatched = Rc::new(Cell::new(0));
    let counted = dispatched.clone();
    // Registered after the app's own handler: GPUI calls an action's app-wide handlers newest first and stops at the
    // first, so this one sees the action instead of the desktop integration's window opener.
    cx.update(|_, cx| {
        cx.on_action(move |_: &OpenSourceNotices, _| counted.set(counted.get() + 1));
    });
    test_support::check(cx);
    open_menu(cx);
    test_support::click(cx, "help-menu-open-source-notices");
    assert_eq!(dispatched.get(), 1);
    assert!(!any_open(cx), "the item closed the menu");
}

// ---- Announcements -------------------------------------------------------------------------------------------------

/// SPEC.md: "After at least one successful showing, Announcements stays in the ? menu directly after Release notes",
/// and it opens the history, "returning focus to Help" when that closes. The item reads the pushed queue when the menu
/// draws, so it appears as soon as the helm pushes a history. Specified: with nothing shown, the menu has no
/// Announcements item; once the helm pushes a queue whose history holds one announcement, the menu draws Announcements
/// directly after Release notes; choosing it closes the menu and opens the history showing that announcement; Escape
/// closes it with focus back on the `?` button.
#[gpui::test]
fn announcements_appear_after_a_showing_and_open_the_history(cx: &mut TestAppContext) {
    use farhelm_proto::announcements::{Announcement, AnnouncementQueue};
    use farhelm_proto::ui_channel::{ANNOUNCEMENTS_KEY, Collection, CollectionChanges, KeyedEntry};
    let entry = |history: Vec<Announcement>| KeyedEntry {
        key: ANNOUNCEMENTS_KEY.into(),
        value: serde_json::to_value(AnnouncementQueue {
            pending: vec![],
            history,
        })
        .unwrap(),
    };
    let mut nothing_shown = snapshot();
    nothing_shown.push(CollectionState {
        collection: Collection::Announcements,
        meta: test_support::meta(&[ANNOUNCEMENTS_KEY]),
        entries: vec![entry(vec![])],
    });
    let (mut fake, cx) = open_main(cx, nothing_shown);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    open_menu(cx);
    assert!(
        !drawn_items(cx).contains(&"help-menu-announcements".to_owned()),
        "nothing shown yet, so no Announcements item"
    );
    test_support::keys(cx, "escape");

    fake.push(CollectionChanges {
        collection: Collection::Announcements,
        meta: test_support::meta(&[ANNOUNCEMENTS_KEY]),
        upsert: vec![entry(vec![Announcement {
            id: "1".to_owned(),
            title: "Shown before".to_owned(),
            body: "Its body.".to_owned(),
            detail: None,
            link: None,
        }])],
        remove: vec![],
    });
    test_support::settle(cx);
    open_menu(cx);
    let drawn = drawn_items(cx);
    let at = |id: &str| {
        drawn
            .iter()
            .position(|item| item == id)
            .unwrap_or_else(|| panic!("{id} was not drawn: {drawn:?}"))
    };
    assert_eq!(
        at("help-menu-announcements"),
        at("help-menu-release-notes") + 1,
        "Announcements directly after Release notes: {drawn:?}"
    );
    test_support::click(cx, "help-menu-announcements");
    assert!(node(&frame(cx), MENU).is_none(), "the menu closed");
    assert!(any_open(cx), "the history opened");
    assert!(
        frame(cx).iter().any(|node| node
            .texts
            .iter()
            .any(|text| text.text.as_ref() == "Shown before")),
        "the history shows the shown announcement"
    );
    test_support::keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));
}

/// SPEC.md: "Welcome shows the welcome card again", and "Reopening does not reset or rewrite seen. Closing a reopened
/// card returns focus to the help toggle". The real main view's window-start owner sits beside the menu here, so this
/// also shows that a card reopened over a helm that has already seen it does not start the window-start flow. Specified:
/// over preferences that say the card was seen, nothing opens by itself; choosing Welcome, the menu's last fixed item,
/// closes the menu and opens the card with focus on Got it; Escape closes it with focus on the `?` button, and the helm
/// received no command.
#[gpui::test]
fn welcome_reopens_the_card_without_writing_and_returns_focus_to_the_button(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot());
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    assert!(
        !any_open(cx),
        "premise: a seen card does not open by itself"
    );
    let _ = fake.commands();
    open_menu(cx);
    test_support::click(cx, "help-menu-welcome");
    assert!(node(&frame(cx), MENU).is_none(), "the menu closed");
    assert!(node(&frame(cx), "welcome").is_some(), "the card opened");
    assert_eq!(focused_id(cx).as_deref(), Some("welcome-got-it"));
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed the card");
    assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));
    assert!(fake.commands().is_empty(), "reopening writes nothing");
}
