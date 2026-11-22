//! Golden scenes for the other dialogs and the sidebar controls that open them: rail notices and the notices bell, a
//! row's notification list, the Help menu, announcements, Welcome, Send feedback, Keyboard shortcuts and Trash. The
//! scene list itself, `SCENES` with each scene's name and size, stays in `mod.rs`.

use crate::focus::{self, Mark};
use crate::session_list::SessionList;
use crate::session_list::tests::{TEST_NOW, preferences_state};
use crate::theme;
use crate::{session, test_support, ticker};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::announcements::{Announcement, AnnouncementLink, AnnouncementQueue};
use farhelm_proto::ui_channel::{
    ANNOUNCEMENTS_KEY, Collection, CollectionState, KeyedEntry, Preferences,
};
use gpui::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement, Styled, Window, div,
    point, px,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{
    SceneState, fleet, rest_pointer_at, session_at_age, session_list_scene_holding,
    session_list_scene_opening, session_list_then,
};

/// The Help menu, open beside the host rail foot's `?` button with its first item highlighted (reference `help-menu`):
/// the items this platform offers with no updater running, the Linux golden with Open-Source Notices last.
pub(super) fn help_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| {
            let help = list.read(cx).help_button().clone();
            help.update(cx, |help, cx| help.toggle(window, cx));
        },
        cx,
    );
}

/// Show the reported Announcements hover with enough room outside the menu for the card. The menu adds the item
/// only after the helm supplies a shown announcement.
pub(super) fn help_menu_announcements_hover_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    window.set_driver_enabled(true);
    let rows = fleet();
    session_list_scene_holding(
        state,
        &rows,
        Some(rows[0].key),
        vec![announcement_history()],
        cx,
    );
    state.after.set(Some(open_help_with_history));
}

/// Wait for the Help menu's own history oracle. The list may draw before the fake helm's snapshot arrives.
fn open_help_with_history(
    list: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    if !crate::announcements::has_history(cx) {
        state.after.set(Some(open_help_with_history));
        return;
    }
    let help = list.read(cx).help_button().clone();
    help.update(cx, |help, cx| help.toggle(window, cx));
    state.after.set(Some(hover_announcements_item));
}

/// Require a visible item before warming and dispatching its hover. The following frame verifies the scene's
/// real help because two matching screenshots alone can accept a stable, empty tooltip.
fn hover_announcements_item(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let nodes = window.driver_nodes();
    let bounds = |id| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} is drawn before the hover"))
            .bounds
    };
    let at = bounds("help-menu-announcements").center();
    assert!(
        bounds("help-menu").contains(&at),
        "Announcements is visible in the menu"
    );
    rest_pointer_at(state, at, window, cx);
    state.after.set(Some(|_, _, window, cx| {
        let shown =
            crate::layers::shown_tooltip(window, cx).expect("the Announcements tooltip shows");
        assert!(
            shown.text.starts_with("Announcements:"),
            "the hovered item replaced the warming tooltip"
        );
        assert_eq!(shown.side(), crate::layers::TooltipSide::Beside);
    }));
    window.refresh();
}

/// The fleet of [`session_list_setup`] with `held` as the helm's notices, and the rail's notices panel open beside the
/// bell at the top of the rail's foot when `open`.
fn rail_notices_scene(state: &SceneState, held: &[serde_json::Value], open: bool, cx: &mut App) {
    let keys: Vec<String> = held.iter().map(|notice| notice["id"].to_string()).collect();
    let order: Vec<&str> = keys.iter().map(String::as_str).collect();
    let notices = CollectionState {
        collection: Collection::Notices,
        meta: crate::test_support::meta(&order),
        entries: held
            .iter()
            .map(|notice| KeyedEntry {
                key: notice["id"].to_string(),
                value: notice.clone(),
            })
            .collect(),
    };
    let rows = fleet();
    session_list_scene_holding(state, &rows, Some(rows[0].key), vec![notices], cx);
    if !open {
        return;
    }
    state.after.set(Some(|list, _, window, cx| {
        let bell = list.read(cx).notices_button().clone();
        bell.update(cx, |bell, cx| bell.toggle(window, cx));
    }));
}

/// The rail's notices panel with nothing in it (reference `notices-empty`): the header, "No notifications", and the
/// footer with Clear resolved unavailable and Close; the bell quiet, in the open look.
pub(super) fn notices_empty_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    rail_notices_scene(state, &[], true, cx);
}

/// The rail's notices panel with one unread trash cleanup notice about the local host (reference `notices-alerting`):
/// its host and what happened, the checkout's repository and size, the reason in red, when it last failed, the promise
/// of a retry, and Retry now unavailable (the host's trash read is never answered here, so no listing shows the
/// checkout stuck) beside Open Trash.
pub(super) fn notices_alerting_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    rail_notices_scene(state, &[alerting_notice()], true, cx);
}

/// The rail with that unread notice and the panel closed: the bell filled, in the strong red, with its count "1" over
/// its corner, the look nothing else pictures (the open bell wears the open look's inverse ink instead).
pub(super) fn notices_bell_loud_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    rail_notices_scene(state, &[alerting_notice()], false, cx);
}

/// The one unread trash cleanup notice about the local host that the alerting scenes show.
fn alerting_notice() -> serde_json::Value {
    serde_json::json!({
            "id": 1,
            "host": 1,
            "raised_at": 1_794_596_700_i64,
            "read": false,
            "resolved": false,
            "content": {
                "kind": "stuck_trash",
                "checkout_id": "archived-work",
                "name": "archived-work",
                "repository": "example/project",
                "bytes": 4096,
                "reason": "Permission denied",
                "at": 1_794_596_700_i64,
            },
    })
}

/// A row's notification list, open beside its bell (no reference: the old app's screenshots have no open bell). The
/// open row has nothing to tell; the row below has three notifications, newest first: one new since the list was last
/// opened (the red edge and New), whose text ends with a docs link, one already read, and one resolved (greyed and
/// marked Resolved). Its bell is loud, with one unread. The bell is pressed as a click would press it, and the pointer
/// then leaves for an empty corner, so neither a hover fill nor a tooltip is in the picture ([`press_bell_b`]).
pub(super) fn session_bell_list_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    let notifications = serde_json::json!([
        {
            "seq": 7,
            "at": TEST_NOW - 3 * 60,
            "text": "Farhelm ended this session's old terminal when it started, so its agent is no longer \
                     running. Restart starts it again in a new terminal. See \
                     https://farhelm.io/docs/using/new-terminals/",
        },
        {
            "seq": 6,
            "at": TEST_NOW - 2 * 3_600,
            "text": "Farhelm could not add its conversation hook to this launch, so Restart cannot resume it.",
        },
        {
            "seq": 5,
            "at": TEST_NOW - 26 * 3_600,
            "text": "When Farhelm checked, Codex's own record of this conversation was missing or no longer \
                     matched, so Restart stopped offering to resume it.",
            "resolved": true,
        },
    ]);
    let rows = [
        session_at_age("a", "Refactor the API client", 2 * 60).cwd("/home/user/src/api"),
        session_at_age("b", "Release notes for 0.40", 5 * 60)
            .cwd("/home/user/src/farhelm")
            .notifications(notifications, 6),
        session_at_age("c", "Benchmark on the build box", 3 * 3_600).cwd("/home/user/bench"),
    ];
    session_list_scene_opening(state, &rows, Some("a"), cx);
    state.after.set(Some(press_bell_b));
}

/// Press row `b`'s bell as a click would, where the last frame drew it, then move the pointer to the empty corner at
/// the top left (see [`session_bell_list_setup`]). The bell's place comes from GPUI's driver record, which the setup
/// turns on and which only reads what a frame drew, so the app keeps nothing for the scene. The store's first snapshot
/// reaches the list only once the executor has run, a frame or more after the list is first drawn, so until a frame has
/// drawn the bell this waits for the next frame the scene's settling draws; it asks for none itself, since a frame asked
/// for here would be drawn before the executor runs again and could never show the bell.
fn press_bell_b(_: &Entity<SessionList>, state: &SceneState, window: &mut Window, cx: &mut App) {
    let Some(bell) = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some("session-bell-b"))
        .map(|node| node.bounds.center())
    else {
        state.after.set(Some(press_bell_b));
        return;
    };
    let pressed = |down: bool| {
        if down {
            gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                button: gpui::MouseButton::Left,
                position: bell,
                modifiers: gpui::Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            })
        } else {
            gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                button: gpui::MouseButton::Left,
                position: bell,
                modifiers: gpui::Modifiers::default(),
                click_count: 1,
            })
        }
    };
    window.dispatch_event(pressed(true), cx);
    window.dispatch_event(pressed(false), cx);
    window.dispatch_event(
        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: point(px(2.), px(2.)),
            pressed_button: None,
            modifiers: gpui::Modifiers::default(),
        }),
        cx,
    );
}

/// The Announcements dialog as the `?` menu's history opens it: two shown announcements, newest first, the first with a
/// detail line, two paragraphs and a labelled link, the second plain. The old app's announcements dialog was never
/// captured as a reference scene, so SPEC.md and the old code are this picture's reference.
pub(super) fn announcements_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![announcement_history()]);
    // The queue reaches the store after this setup returns. Open history once the connection has a UI session.
    let handle = window.window_handle();
    let opener = cx.focus_handle().tab_stop(true);
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let opener = opener.clone();
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::announcements::open_history(&opener, &mark, window, cx);
        });
    });
}

/// Both the conditional Help menu item and its history dialog need a previously shown announcement.
fn announcement_history() -> CollectionState {
    let newest = Announcement {
        id: "2".to_owned(),
        title: "Sessions now restart in their own terminal".to_owned(),
        body: "Restart keeps the session's history and opens a fresh terminal beside it.\n\nNothing changes for \
               sessions you do not restart."
            .to_owned(),
        detail: Some("Since 0.40.0".to_owned()),
        link: Some(AnnouncementLink {
            label: "Release notes".to_owned(),
            url: "https://farhelm.io/docs/release-notes/".to_owned(),
        }),
    };
    let older = Announcement {
        id: "1".to_owned(),
        title: "Hosts update on their own".to_owned(),
        body: "Remote hosts now update when the app does.".to_owned(),
        detail: None,
        link: None,
    };
    CollectionState {
        collection: Collection::Announcements,
        meta: test_support::meta(&[ANNOUNCEMENTS_KEY]),
        entries: vec![KeyedEntry {
            key: ANNOUNCEMENTS_KEY.into(),
            value: serde_json::to_value(AnnouncementQueue {
                pending: vec![],
                history: vec![newest, older],
            })
            .unwrap(),
        }],
    }
}

/// The Welcome card (reference `welcome-dialog`): the title and its ×, the introduction, the two bordered pointers with
/// Read Get started and Send feedback now, the footer's note and Got it with focus. Opened as the `?` menu's Welcome item
/// opens it, which draws the same card as the window start does; the scene's ring handle stands in for the `?` button.
pub(super) fn welcome_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    crate::welcome::open(&state.ring, &mark, window, cx);
}

/// The upgrade dialog gets two finalized releases with different categories, styled runs and one screenshot link.
/// Use the real dialog so the picture covers the scrollable body, pinned count and all three actions together.
pub(super) fn whats_new_dialog_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let releases = crate::whats_new::tests::highlights();
    crate::whats_new::open(
        releases[0].version.clone(),
        releases,
        &Mark::take(cx),
        window,
        cx,
    );
}

/// The Send feedback dialog as the Help menu opens it (reference `feedback-dialog`), over a preference row
/// remembering `contact`: focus in the empty message, the contact field holding `contact`, and, when it is not empty,
/// Re-use checked under it; then what is sent with it, the footer's note, Cancel and Send, unavailable while the
/// message is empty. The opener is the scene's ring handle, which stands in for the `?` button. The fake and the
/// connection are kept for the scene's life, like the Rename editor's.
fn feedback_dialog_scene(
    state: &SceneState,
    contact: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![preferences_state(Preferences {
        feedback_contact: contact.map(str::to_owned),
        ..Preferences::default()
    })]);
    // The dialog seeds its contact from the store, which the snapshot reaches after this setup returns: open once the
    // connection has a UI session, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::feedback::open(&opener, &mark, window, cx);
        });
    });
}

/// [`feedback_dialog_scene`] as the dialog first opens with nothing remembered (reference `feedback-dialog`).
pub(super) fn feedback_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    feedback_dialog_scene(state, None, window, cx);
}

/// [`feedback_dialog_scene`] with a remembered contact, so Re-use for future feedback shows, checked.
pub(super) fn feedback_dialog_reuse_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    feedback_dialog_scene(state, Some("you@example.test"), window, cx);
}

/// The Keyboard shortcuts overview as the `?` menu opens it, in this platform's forms (Linux, where the goldens run),
/// over a preference row with ⚡ on `deploy` and number keys 1 and 0 assigned, so both kinds of template row show. The
/// old app's overview was never captured as a reference scene, so SPEC.md and the old code are this picture's
/// reference. The opener is the scene's ring handle, which stands in for the `?` button; the fake and the connection
/// are kept for the scene's life, like the feedback dialog's.
pub(super) fn keyboard_shortcuts_dialog_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![preferences_state(Preferences {
        quick_launch_template: Some("deploy".to_owned()),
        template_shortcuts: [("1", "api review"), ("0", "nightly build")]
            .into_iter()
            .map(|(key, name)| (key.to_owned(), name.to_owned()))
            .collect(),
        ..Preferences::default()
    })]);
    // The rows read the store, which the snapshot reaches after this setup returns: open once the connection has a UI
    // session, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::shortcuts::open_from_menu(&opener, &mark, window, cx);
        });
    });
}

/// A capable host's trash listing as the trash scenes answer it: `checkouts` (each its id, name, repository, archive
/// time and size), in `root`'s `.farhelm-trash`, with removal active when `removing`.
fn trash_listing(
    root: &str,
    checkouts: &[(&str, &str, &str, i64, u64)],
    removing: bool,
) -> serde_json::Value {
    let checkouts: Vec<serde_json::Value> = checkouts
        .iter()
        .map(|(id, name, repository, archived_at, bytes)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "repository": repository,
                "path": format!("{root}/.farhelm-trash/{name}"),
                "archived_at": archived_at,
                "bytes": bytes,
            })
        })
        .collect();
    serde_json::json!({
        "checkouts": checkouts,
        "issues": [],
        "removing": removing,
        "reports_retired": true,
    })
}

/// The reference screens' trash (`checkout-trash-dialog`): two archives on this machine and one on `build-box`, with
/// their sizes, repositories and archive times; `gpu-1`'s trash is empty.
fn trash_reference(host: i64) -> serde_json::Value {
    match host {
        1 => trash_listing(
            "/Users/you/work",
            &[
                (
                    "web-fix-login",
                    "web-fix-login-20261113T174055Z",
                    "acme/web",
                    1_794_591_655,
                    48_234_496,
                ),
                (
                    "api-2",
                    "api-2-20261110T191055Z",
                    "acme/api",
                    1_794_337_855,
                    212_965_785,
                ),
            ],
            false,
        ),
        2 => trash_listing(
            "/home/dev/work",
            &[(
                "api-1",
                "api-1-20261112T191055Z",
                "acme/api",
                1_794_510_655,
                186_961_100,
            )],
            false,
        ),
        _ => trash_listing("/home/dev/work", &[], false),
    }
}

/// How far a trash scene goes once every host has answered.
#[derive(Clone, Copy)]
enum TrashOpen {
    /// The button alone.
    Button,
    /// The dialog open.
    Dialog,
    /// The dialog open and, once its measured reads are answered too, asking to delete everything on the host (`Some`,
    /// its "Delete all on") or on every reachable host (`None`, Delete all).
    Asking(Option<i64>),
}

/// The reference screens' three hosts, all connected, with a Trash button over them that reads each host's trash and
/// gets `listing_of(host)` back at once; once every host has answered, the scene goes as far as `open` says, the
/// dialog's settle pause off, and its measured reads are answered the same way. The fake, the connection, the button and
/// the watch that answers are kept for the scene's life.
fn trash_scene(
    state: &SceneState,
    listing_of: fn(i64) -> serde_json::Value,
    open: TrashOpen,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::trash::TrashButton;
    let (helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let helm = Rc::new(RefCell::new(helm));
    helm.borrow_mut().connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let button = cx.new(TrashButton::new);
    let window = window.window_handle();
    let answering = helm.clone();
    let opened = Cell::new(matches!(open, TrashOpen::Button));
    // The question waits for the dialog's own measured reads: a host being read offers nothing to delete.
    let ask = Cell::new(match open {
        TrashOpen::Asking(host) => Some(host),
        TrashOpen::Button | TrashOpen::Dialog => None,
    });
    let watch = cx.observe(&button, move |button, cx| {
        {
            let mut helm = answering.borrow_mut();
            for read in helm.commands() {
                let host = read
                    .body
                    .as_ref()
                    .and_then(|body| body["host"].as_i64())
                    .unwrap_or_default();
                helm.reply(read.id, 200, Some(listing_of(host)));
            }
        }
        let ready = button
            .update(cx, |button, cx| button.snapshot(cx))
            .all_ready();
        if opened.get() {
            if let Some(host) = ask.get().filter(|_| ready) {
                ask.set(None);
                let dialog = button.read(cx).dialog_view();
                let _ = window.update(cx, |_, window, cx| {
                    if let Some(dialog) = dialog {
                        dialog.update(cx, |dialog, cx| dialog.ask(host, window, cx));
                    }
                });
            }
            return;
        }
        if !ready {
            return;
        }
        opened.set(true);
        let _ = window.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            button.update(cx, |button, cx| {
                button.open(focus::ReturnTo::Opener, &mark, window, cx);
                if let Some(dialog) = button.dialog_view() {
                    dialog.update(cx, |dialog, _| dialog.without_pause());
                }
            });
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(button));
    keep.push(Box::new(watch));
}

/// The Trash button a trash scene made, at the top left of the sidebar's raised heading surface.
pub(super) fn trash_scenes(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let button = state.keep.borrow().iter().find_map(|kept| {
        kept.downcast_ref::<Entity<crate::trash::TrashButton>>()
            .cloned()
    });
    div()
        .size_full()
        .flex()
        .items_start()
        .p(px(8.))
        .bg(theme::color(palette.bg_2))
        .children(button)
        .into_any_element()
}

/// The Trash button over hosts whose trash is empty: the outline can, no count.
pub(super) fn trash_button_empty_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    trash_scene(
        state,
        |_| trash_listing("/work", &[], false),
        TrashOpen::Button,
        window,
        cx,
    );
}

/// The Trash button over the reference trash (reference `main-running-session`'s header): the open-lid can and its
/// count, 3.
pub(super) fn trash_button_full_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    trash_scene(state, trash_reference, TrashOpen::Button, window, cx);
}

/// The Trash button while this machine frees space, under reduced motion so the picture holds still: the can upside
/// down, unlifted, with its count.
pub(super) fn trash_button_emptying_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    ticker::set_reduced_motion(true, cx);
    trash_scene(
        state,
        |host| {
            let mut listing = trash_reference(host);
            if host == 1 {
                listing["removing"] = serde_json::json!(true);
            }
            listing
        },
        TrashOpen::Button,
        window,
        cx,
    );
}

/// The Trash dialog over the reference trash (reference `checkout-trash-dialog`): its title with the open can and the
/// total, the intro, each host's "Delete all on", this machine's two archives and build-box's one with their sizes,
/// repositories, archive times and Deletes, each host's folder, and the footer's Close, where focus starts (its ring
/// shows only under keyboard focus, so the picture shows none), and Delete all.
pub(super) fn checkout_trash_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    trash_scene(state, trash_reference, TrashOpen::Dialog, window, cx);
}

/// The Trash dialog over the reference trash asking to delete everything: the question in its red band in the region
/// above the footer, naming the three checkouts on this machine and build-box with their size and the warning, Cancel
/// and Delete permanently (where focus is, its ring hidden as on any pointer-opened question).
pub(super) fn checkout_trash_confirm_all_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    trash_scene(state, trash_reference, TrashOpen::Asking(None), window, cx);
}

/// The Trash dialog over the reference trash asking to delete build-box's one archive: the question in its red band
/// inside build-box's card, under its header.
pub(super) fn checkout_trash_confirm_host_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    trash_scene(
        state,
        trash_reference,
        TrashOpen::Asking(Some(2)),
        window,
        cx,
    );
}
