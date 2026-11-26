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
use crate::launcher::launch_tests::{HostChange, start_host_change};
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
        upgrade_notes: None,
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
/// shortcuts**, **Release notes** and **Welcome**", "Announcements appears directly after Release
/// notes", "Check for updates last"), and the Linux-only notices item has no place in SPEC.md, so it goes at the very
/// end where it moves nothing else. An item out of place is a menu the documentation no longer describes. Specified,
/// for all eight combinations: Send feedback, Discord, Documentation, Keyboard shortcuts, Release notes and
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
/// bottom 48 pixels above the window's (the rail's Expand control and the old foot's padding below it); its hover help
/// names the menu's contents without the update check while no updater runs.
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
    assert_eq!(at.bottom(), px(700. - 48.));
    let gear = node(&nodes, GEAR).expect("the gear is drawn").bounds;
    assert_eq!(
        gear.bottom(),
        at.top(),
        "the ? button sits directly below the gear"
    );
    assert_eq!(
        hover_help(cx, BUTTON_ID).as_deref(),
        Some(
            "Help: send feedback, open the documentation or release notes, chat on Discord, or show Welcome"
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
            "Help: send feedback, open the documentation or release notes, chat on Discord, show Welcome, or check for updates"
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
/// directly after Release notes. Hover and Up place each item's help outside the menu so its neighbors stay visible.
/// Choosing Announcements opens the history; Escape closes it with focus back on the `?` button.
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
    cx.simulate_resize(size(px(1200.), px(700.)));
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
    // With Announcements reachable, neither its pointer help nor the next item reached by Up may cover the panel.
    let menu = test_support::bounds(cx, MENU);
    let shows_beside = |cx: &mut VisualTestContext| {
        let shown = cx
            .update(|window, cx| layers::shown_tooltip(window, cx))
            .expect("the focused or hovered item shows help");
        assert_eq!(shown.side(), layers::TooltipSide::Beside);
        assert!(
            shown.ring().is_none(),
            "the active menu item already has a highlight"
        );
        assert!(test_support::bounds(cx, "tooltip-card").left() > menu.right());
    };
    assert_eq!(
        hover_help(cx, "help-menu-announcements").as_deref(),
        Some(Item::Announcements.help()),
    );
    shows_beside(cx);
    test_support::keys(cx, "up");
    assert_eq!(focused_id(cx).as_deref(), Some("help-menu-release-notes"));
    test_support::advance(cx, layers::HOVER_DELAY);
    assert_eq!(
        cx.update(|window, cx| layers::shown_tooltip(window, cx))
            .map(|shown| shown.text.to_string())
            .as_deref(),
        Some(Item::ReleaseNotes.help()),
    );
    shows_beside(cx);

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

/// Help opens Welcome on request. Both close routes return to Help even under the Mac click rule, where clicking the
/// menu button leaves focus elsewhere. Opening and closing Welcome sends the helm nothing.
#[gpui::test]
fn welcome_closes_to_help_without_writing(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    assert!(!any_open(cx), "premise: no dialog at startup");
    let _ = fake.commands();
    open_menu(cx);
    test_support::click(cx, "help-menu-welcome");
    assert!(node(&frame(cx), MENU).is_none(), "the menu closed");
    assert!(node(&frame(cx), "welcome").is_some(), "the dialog opened");
    assert_eq!(focused_id(cx).as_deref(), Some("welcome-dialog-start"));
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed the dialog");
    assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));

    open_menu(cx);
    test_support::click(cx, "help-menu-welcome");
    test_support::click(cx, "welcome-close");
    assert!(!any_open(cx), "Close closed the dialog");
    assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));
    assert!(fake.commands().is_empty(), "Welcome writes nothing");
}

/// The dialog and the pane have distinct controls even over an empty fleet. The guide leaves the dialog up, and
/// opening either form replaces Welcome and returns focus to Help when the form closes.
#[gpui::test]
fn welcome_dialog_and_pane_can_coexist_and_the_links_return_to_help(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, vec![listing(&[], &[]), preferences(&[], None)]);
    let _ = fake.commands();
    open_menu(cx);
    test_support::click(cx, "help-menu-welcome");
    let shown = frame(cx);
    assert!(node(&shown, "welcome-pane-get-started").is_some());
    assert!(node(&shown, "welcome-dialog-get-started").is_some());
    test_support::click(cx, "welcome-dialog-get-started");
    assert_eq!(
        cx.opened_url().as_deref(),
        Some(crate::welcome::GET_STARTED_URL)
    );
    assert!(node(&frame(cx), "welcome").is_some());

    for (link, form) in [
        ("welcome-dialog-add-host", "add-host-cancel"),
        ("welcome-dialog-feedback", "feedback-cancel"),
    ] {
        test_support::click(cx, link);
        let shown = frame(cx);
        assert!(node(&shown, "welcome").is_none());
        assert!(node(&shown, form).is_some());
        test_support::keys(cx, "escape");
        assert_eq!(focused_id(cx).as_deref(), Some(BUTTON_ID));
        open_menu(cx);
        test_support::click(cx, "help-menu-welcome");
    }
    test_support::keys(cx, "escape");
    assert!(
        fake.commands().is_empty(),
        "opening a form or guide does not write"
    );
}

/// Enter in Help's Welcome opens the existing session launcher. This needs its own test because New refuses while a
/// dialog still holds the keyboard.
#[gpui::test]
fn welcome_dialog_start_hands_off_to_the_session_launcher(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, vec![listing(&[], &[]), preferences(&[], None)]);
    open_menu(cx);
    test_support::click(cx, "help-menu-welcome");
    assert_eq!(focused_id(cx).as_deref(), Some("welcome-dialog-start"));
    test_support::keys(cx, "enter");
    let shown = frame(cx);
    assert!(node(&shown, "welcome").is_none());
    assert!(node(&shown, "launcher-launch").is_some());
}

/// A pending host probe makes New unavailable. Welcome must start on a usable control so the keyboard can still
/// reach the help links while the probe has no answer.
#[gpui::test]
fn welcome_dialog_focuses_add_host_while_start_is_unavailable(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, vec![listing(&[], &[]), preferences(&[], None)]);
    let path = start_host_change(cx, HostChange::Probe);
    assert!(
        fake.commands().iter().any(|command| command.path == path),
        "premise: the probe is waiting for a reply"
    );
    assert!(
        !cx.update(|_, cx| crate::launcher::entry::new_available(cx)),
        "premise: New is unavailable"
    );

    open_menu(cx);
    test_support::click(cx, "help-menu-welcome");
    let shown = frame(cx);
    let start = node(&shown, "welcome-dialog-start").expect("Start a session is drawn");
    assert!(!start.flags.focusable);
    assert_eq!(focused_id(cx).as_deref(), Some("welcome-dialog-add-host"));
    assert!(node(&shown, "welcome").is_some());
}

// ---- A menu taller than the window --------------------------------------------------------------------------------

/// A menu that runs past the window's edge leaves its last items where nobody can reach them, and one that scrolls
/// without following the keyboard hides the item focus lands on. With every item, the menu is taller than the 720 by
/// 560 minimum window on both platforms; the old panel was capped and scrolled (`.host-row-menu-flyout >
/// .host-row-menu-panel`, `overflow-y: auto`). A 300-pixel window makes even the shortest menu overflow, whatever the
/// text system. Specified: in a 720 by 300 window, the open menu lies inside the window with focus on its first item,
/// and its last item lies below the menu's bottom edge; Down to the last item focuses it and shows it wholly inside the
/// menu.
#[gpui::test]
fn a_menu_taller_than_the_window_fits_in_it_and_scrolls_to_the_focused_item(
    cx: &mut TestAppContext,
) {
    let (_fake, cx) = open_main(cx, snapshot());
    cx.simulate_resize(size(px(720.), px(300.)));
    test_support::settle(cx);
    open_menu(cx);
    let shown = expected(false);
    let (first, last) = (shown[0].as_str(), shown[shown.len() - 1].as_str());
    assert_eq!(focused_id(cx).as_deref(), Some(first));
    let nodes = frame(cx);
    let menu = node(&nodes, MENU).expect("the menu is open").bounds;
    assert!(
        menu.top() >= px(0.) && menu.bottom() <= px(300.),
        "inside the window: {menu:?}"
    );
    let item = node(&nodes, last).expect("the last item is drawn").bounds;
    assert!(
        item.top() >= menu.bottom(),
        "premise: the last item lies below the menu: {item:?}"
    );
    test_support::keys(cx, &vec!["down"; shown.len() - 1].join(" "));
    assert_eq!(focused_id(cx).as_deref(), Some(last));
    let nodes = frame(cx);
    let menu = node(&nodes, MENU).expect("the menu is open").bounds;
    let item = node(&nodes, last).expect("the last item is drawn").bounds;
    assert!(
        menu.top() <= item.top() && item.bottom() <= menu.bottom(),
        "{item:?} in {menu:?}"
    );
}
