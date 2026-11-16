//! Tests of the list's heading: the order's title and its menu, the order's write and its silent refusal, the count's
//! words, the Templates button, and New.
//!
//! Like the rest of the list's tests, these run the real main view over the helm client's fake, which hands the test
//! every command the window sent, and read controls from GPUI's driver record of the frame by their ids: the title is
//! `session-order`, the menu `session-order-menu`, its items `session-order-<word>`, the count `session-count`, the
//! Templates button `templates-button`, and New `new-session-button` ([`NEW_ID`]). They share the rail's fixtures
//! (`host_rail/tests.rs`): sessions a and b on host 2, a open, over three hosts, with no order stored.
//!
//! GPUI's driver record does not carry a button's expanded state, so whether the title reports its menu open is read
//! where the title takes it from ([`SessionList::order_menu_open`]).

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, KeyedEntry, PREFERENCES_KEY, Preferences,
};
use gpui::{Role, TestAppContext, Toggled, VisualTestContext};
use serde_json::{Value, json};

use super::{COMPACT_DESCRIPTION, COMPACT_ID, Order, TITLE_ID};
use crate::actions::NewSession;
use crate::launcher::launch_tests::{HostChange, start_host_change};
use crate::main_view::MainView;
use crate::session_list::host_rail::tests::{hover_help, label, node, snapshot, texts_within};
use crate::session_list::tests::{change, launcher_drawn, listing_meta, painted, selected, sent};
use crate::session_list::{COUNT_ID, NEW_ID};
use crate::test_support::{self, click, focused_id, frame, keys, meta, started};
use crate::{focus, layers, templates};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The rail's snapshot, connected, in the real main view, with the commands the window sent on its own (remembering the
/// open session, its seen mark) answered, so a test's next look at the fake sees only what its own steps sent. None of
/// those may store an order: the preference row is the helm's, and a window that wrote the order it opened in would
/// store a choice nobody made (the old `sort.spec.ts` case "the default order is never written"). The main view is
/// returned too, to ask its list whether the order menu is open.
fn open(cx: &mut TestAppContext) -> (FakeHelm, gpui::Entity<MainView>, &mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot());
    cx.run_until_parked();
    let (main, window) = test_support::open_view(cx, MainView::new);
    for command in fake.commands() {
        let body = command.body.clone().unwrap_or_default();
        assert!(
            body.get("list_sort").is_none(),
            "opening stored an order: {command:?}"
        );
        fake.reply(command.id, 200, None);
    }
    window.run_until_parked();
    (fake, main, window)
}

/// Whether the order title reports its menu open.
fn menu_open(main: &gpui::Entity<MainView>, cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        let list = main.read(cx).session_list().clone();
        list.update(cx, |list, cx| list.order_menu_open(window, cx))
    })
}

/// A push of the preference row holding `list_sort`, as the helm sends it when the stored order changes.
fn stored_order(fake: &mut FakeHelm, list_sort: Option<&str>, cx: &mut VisualTestContext) {
    let preferences = Preferences {
        visit_history: vec!["a".into()],
        list_sort: list_sort.map(str::to_owned),
        ..Preferences::default()
    };
    fake.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        upsert: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(preferences).unwrap(),
        }],
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// The one order write the window sent since the last look: its envelope id and body. Fails on anything else, so a
/// test that expects the write also proves nothing else went out (no session query that names a sort).
fn one_write(fake: &mut FakeHelm) -> (u64, Value) {
    test_support::one_command(fake, Method::Put, "/api/preferences")
}

/// What the open menu's items report, in their order: (accessible name, role, checked).
fn items(cx: &mut VisualTestContext) -> Vec<(String, Option<Role>, Option<Toggled>)> {
    let nodes = frame(cx);
    Order::ALL
        .iter()
        .map(|order| {
            let item = node(&nodes, order.item_id())
                .unwrap_or_else(|| panic!("{} was not drawn", order.item_id()));
            (
                item.label.as_deref().unwrap_or_default().to_owned(),
                item.role,
                item.toggled,
            )
        })
        .collect()
}

/// The order whose item the open menu reports checked; fails unless exactly one is.
fn checked(cx: &mut VisualTestContext) -> Order {
    let checked: Vec<Order> = Order::ALL
        .into_iter()
        .zip(items(cx))
        .filter(|(_, (_, _, toggled))| *toggled == Some(Toggled::True))
        .map(|(order, _)| order)
        .collect();
    assert_eq!(checked.len(), 1, "one checked order: {checked:?}");
    checked[0]
}

// ---- The title -----------------------------------------------------------------------------------------------------

/// The title is the only place the window says which order the list is in, and its name and help are what a screen
/// reader user hears for it (the old `aria-label` and `data-tooltip`); an unknown stored word must read as the helm's
/// default, as the channel serves it. Specified: with no order stored the title paints "Recently active", is a button
/// named "Sort: Recently active" with the hover help "Sort: the order of the session list, shared by every client";
/// each stored word the helm pushes ("created", "title", "activity", and the unknown "most-recent") renames it ("Sort:
/// Newest created", "Sort: Title A–Z", "Sort: Recently active" twice), and none of those pushes sends anything.
#[gpui::test]
fn the_title_names_the_stored_order_and_follows_a_pushed_change(cx: &mut TestAppContext) {
    let (mut fake, _main, cx) = open(cx);
    let nodes = frame(cx);
    assert_eq!(label(&nodes, TITLE_ID), "Sort: Recently active");
    assert_eq!(node(&nodes, TITLE_ID).unwrap().role, Some(Role::Button));
    assert!(
        texts_within(&nodes, TITLE_ID)
            .iter()
            .any(|text| text.text.as_ref() == "Recently active"),
        "the title paints the order's name"
    );
    assert_eq!(
        hover_help(cx, "session-order").as_deref(),
        Some("Sort: the order of the session list, shared by every client")
    );

    for (word, name) in [
        ("created", "Sort: Newest created"),
        ("title", "Sort: Title A–Z"),
        ("activity", "Sort: Recently active"),
        ("most-recent", "Sort: Recently active"),
    ] {
        stored_order(&mut fake, Some(word), cx);
        assert_eq!(label(&frame(cx), TITLE_ID), name, "stored {word}");
    }
    assert_eq!(sent(&mut fake), [], "a pushed order sends nothing");
}

// ---- The menu ------------------------------------------------------------------------------------------------------

/// A click is how most users reach the orders, and on macOS that click leaves the title unfocused, so the title must be
/// named as where closing returns focus or Escape would leave focus wherever it was. The menu's words are the old
/// menu's. Specified, under the macOS click rule: a click on the title opens the menu with focus on its first order,
/// Recently active, the title reporting it open; the menu paints the header "Session list" over "Order and density,
/// shared by every client", and its items sit in a menu named "Sort" as radio items named Recently active, Newest
/// created and Title A–Z, only Recently active (the stored default) checked, each with its old description as its
/// hover help; Escape closes it with focus on the title, which reports it closed.
#[gpui::test]
fn a_click_opens_the_menu_on_its_first_order_and_escape_returns_to_the_title(
    cx: &mut TestAppContext,
) {
    let (_fake, main, cx) = open(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    assert!(!menu_open(&main, cx), "the premise: the menu is closed");

    click(cx, "session-order");
    assert!(menu_open(&main, cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-order-activity"));
    let nodes = frame(cx);
    let words = painted(&nodes);
    for line in ["Session list", "Order and density, shared by every client"] {
        assert!(words.iter().any(|text| text == line), "{line} in {words:?}");
    }
    let menu = node(&nodes, "session-order-items").unwrap();
    assert_eq!(
        (menu.role, menu.label.as_deref()),
        (Some(Role::Menu), Some("Sort"))
    );
    let radio = |name: &str, checked| (name.to_owned(), Some(Role::MenuItemRadio), Some(checked));
    assert_eq!(
        items(cx),
        [
            radio("Recently active", Toggled::True),
            radio("Newest created", Toggled::False),
            radio("Title A–Z", Toggled::False),
        ]
    );
    for (order, description) in [
        (
            "session-order-activity",
            "Active sessions first, ordered by when work started",
        ),
        (
            "session-order-created",
            "Most recently created sessions first",
        ),
        (
            "session-order-title",
            "Session titles in alphabetical order",
        ),
    ] {
        assert_eq!(hover_help(cx, order).as_deref(), Some(description));
    }

    keys(cx, "escape");
    assert!(!menu_open(&main, cx));
    assert_eq!(focused_id(cx).as_deref(), Some(TITLE_ID));
}

/// The old title opened its menu from the keyboard as a menu button does, and the old menu took Home and End; a
/// keyboard user who could not reach the last item without walking the list would lose what the old app gave. The
/// arrows inside are the layer manager's. Specified: with focus on the closed title, Down opens the menu on Recently
/// active; Down moves to Newest created; End goes to Compact, the last item, and Home back to Recently active; Escape
/// closes it, and Up on the closed title opens it on Compact; Tab from there keeps the menu open with focus on one of
/// its items (SPEC.md, "Keyboard focus": "Inside a dialog or menu, focus stays inside", where the old menu closed on
/// Tab).
#[gpui::test]
fn down_and_up_open_the_menu_at_either_end_and_home_and_end_walk_it(cx: &mut TestAppContext) {
    let (_fake, main, cx) = open(cx);
    click(cx, "session-order");
    keys(cx, "escape");
    assert!(!menu_open(&main, cx));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(TITLE_ID),
        "the premise: the closed title has focus"
    );

    keys(cx, "down");
    assert!(menu_open(&main, cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-order-activity"));
    keys(cx, "down");
    assert_eq!(focused_id(cx).as_deref(), Some("session-order-created"));
    keys(cx, "end");
    assert_eq!(focused_id(cx).as_deref(), Some(COMPACT_ID));
    keys(cx, "home");
    assert_eq!(focused_id(cx).as_deref(), Some("session-order-activity"));

    keys(cx, "escape");
    assert!(!menu_open(&main, cx));
    keys(cx, "up");
    assert!(menu_open(&main, cx));
    assert_eq!(focused_id(cx).as_deref(), Some(COMPACT_ID));
    keys(cx, "tab");
    assert!(menu_open(&main, cx), "Tab leaves the menu open");
    let focused = focused_id(cx);
    assert!(
        Order::ALL
            .iter()
            .map(|order| order.item_id())
            .chain([COMPACT_ID])
            .any(|item| focused.as_deref() == Some(item)),
        "Tab keeps focus on an item: {focused:?}"
    );
}

// ---- Choosing ------------------------------------------------------------------------------------------------------

/// The helm sorts the pushed list by the stored order, so choosing is one sparse preference write and nothing else: a
/// session query naming a sort would stop the channel following the shared order, and a body naming other fields could
/// put back a stale copy of them. Re-choosing writes nothing (the old `apply_sort`). Specified: Down, Down and Enter on
/// the title send exactly one command, `PUT /api/preferences` with `{"list_sort": "created"}`, close the menu and leave
/// focus on the title, which names Newest created while the write is in flight; once the helm pushes the stored order
/// and answers, the title still names Newest created, focus is still on it and the open session is still open;
/// reopening the menu shows Newest created checked, and choosing it again sends nothing and closes the menu with focus
/// on the title.
#[gpui::test]
fn choosing_an_order_sends_only_its_sparse_write_and_rechoosing_sends_nothing(
    cx: &mut TestAppContext,
) {
    let (mut fake, main, cx) = open(cx);
    let open_session = selected(cx);
    click(cx, "session-order");
    keys(cx, "escape");
    keys(cx, "down down enter");
    let (write, body) = one_write(&mut fake);
    assert_eq!(body, json!({ "list_sort": "created" }));
    assert!(!menu_open(&main, cx));
    assert_eq!(focused_id(cx).as_deref(), Some(TITLE_ID));
    assert_eq!(
        label(&frame(cx), TITLE_ID),
        "Sort: Newest created",
        "while the write is in flight"
    );

    stored_order(&mut fake, Some("created"), cx);
    fake.reply(write, 200, None);
    test_support::settle(cx);
    assert_eq!(label(&frame(cx), TITLE_ID), "Sort: Newest created");
    assert_eq!(focused_id(cx).as_deref(), Some(TITLE_ID));
    assert_eq!(selected(cx), open_session);

    keys(cx, "down");
    assert_eq!(checked(cx), Order::Created);
    click(cx, "session-order-created");
    assert_eq!(sent(&mut fake), [], "the order in force sends nothing");
    assert!(!menu_open(&main, cx));
    assert_eq!(focused_id(cx).as_deref(), Some(TITLE_ID));
}

/// SPEC.md, "Errors and diagnostics", makes the list's order a best-effort preference: a failed save is logged and
/// stays silent, and the choice lasts only until the next read from the helm, which under the channel is at once (the
/// helm pushes the list in the stored order). So a refused write must leave the title on the stored order and add
/// nothing to the window (decided by the migration's orchestrator on 2026-11-14). Specified: choosing Title A–Z by
/// click sends `{"list_sort": "title"}`; once the helm refuses it with words of its own, the title names Recently
/// active again, the frame paints exactly what it painted before the choice (none of the helm's words), nothing is
/// sent again (SPEC.md: a failed save "is never resent"), and the menu checks Recently active; choosing Newest created
/// then still sends its own write.
#[gpui::test]
fn a_refused_write_keeps_the_stored_order_and_shows_nothing(cx: &mut TestAppContext) {
    let (mut fake, _main, cx) = open(cx);
    let before = painted(&frame(cx));
    click(cx, "session-order");
    click(cx, "session-order-title");
    let (write, body) = one_write(&mut fake);
    assert_eq!(body, json!({ "list_sort": "title" }));
    fake.reply(write, 400, Some(json!("no title order on this helm")));
    test_support::settle(cx);
    assert_eq!(sent(&mut fake), [], "a refused write is not resent");

    let nodes = frame(cx);
    assert_eq!(label(&nodes, TITLE_ID), "Sort: Recently active");
    assert_eq!(
        painted(&nodes),
        before,
        "the refusal adds nothing to the window"
    );
    click(cx, "session-order");
    assert_eq!(checked(cx), Order::Activity);
    click(cx, "session-order-created");
    let (_, body) = one_write(&mut fake);
    assert_eq!(
        body,
        json!({ "list_sort": "created" }),
        "a later choice still sends"
    );
}

/// An open menu must not go on checking an order a push replaced, since the next choice is judged against what it
/// shows. Specified: with the menu open over the stored Recently active, a push storing "title" moves the check to
/// Title A–Z, and a push storing nothing moves it back to Recently active.
#[gpui::test]
fn the_open_menu_follows_a_pushed_order(cx: &mut TestAppContext) {
    let (mut fake, _main, cx) = open(cx);
    click(cx, "session-order");
    assert_eq!(checked(cx), Order::Activity);
    stored_order(&mut fake, Some("title"), cx);
    assert_eq!(checked(cx), Order::Title);
    stored_order(&mut fake, None, cx);
    assert_eq!(checked(cx), Order::Activity);
}

// ---- Compact -------------------------------------------------------------------------------------------------------

/// A push of the preference row with `compact` (`None` for unset), as the helm sends it when the stored density
/// changes.
fn stored_compact(fake: &mut FakeHelm, compact: Option<bool>, cx: &mut VisualTestContext) {
    let preferences = Preferences {
        visit_history: vec!["a".into()],
        compact,
        welcome_seen: Some(true),
        ..Preferences::default()
    };
    fake.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        upsert: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(preferences).unwrap(),
        }],
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// Whether the open menu reports Compact checked.
fn compact_checked(cx: &mut VisualTestContext) -> bool {
    let nodes = frame(cx);
    let item = node(&nodes, COMPACT_ID).expect("the menu draws Compact");
    item.toggled == Some(Toggled::True)
}

/// Whether the rows are compact: b's row does not paint its last line's directory (`~/src/b`). Row b, not the open a,
/// whose directory the session pane's details bar paints too.
fn rows_compact(cx: &mut VisualTestContext) -> bool {
    !texts_within(&frame(cx), "session-item-b")
        .iter()
        .any(|text| text.text.as_ref() == "~/src/b")
}

/// SPEC.md, "Session list": the title's menu offers the orders "and **Compact**", and the old menu drew it last, after
/// a separator, as a checkbox, so a screen reader hears it as a choice that is on or off rather than a fourth order.
/// Specified: the open menu's Compact is a checkbox item named Compact, below Title A–Z, unchecked while the preference
/// is unset, its hover help `One line per session, with the folder shown on hover`; and with the preference pushed on,
/// the open menu checks it, the orders' check unchanged.
#[gpui::test]
fn compact_is_a_checkbox_after_the_orders_checked_as_the_preference_is(cx: &mut TestAppContext) {
    let (mut fake, _main, cx) = open(cx);
    click(cx, "session-order");
    let nodes = frame(cx);
    let compact = node(&nodes, COMPACT_ID).expect("the menu draws Compact");
    assert_eq!(
        (compact.role, compact.label.as_deref()),
        (Some(Role::MenuItemCheckBox), Some("Compact"))
    );
    assert!(
        compact.bounds.top() > node(&nodes, "session-order-title").unwrap().bounds.bottom(),
        "Compact comes after the orders"
    );
    assert!(!compact_checked(cx), "unset is off");
    assert_eq!(
        hover_help(cx, COMPACT_ID).as_deref(),
        Some(COMPACT_DESCRIPTION)
    );
    assert_eq!(
        COMPACT_DESCRIPTION,
        "One line per session, with the folder shown on hover"
    );

    stored_compact(&mut fake, Some(true), cx);
    assert!(compact_checked(cx));
    assert_eq!(checked(cx), Order::Activity);
}

/// Compact is a write of the shared preference row, like the order: sparse, so it never puts back a stale copy of any
/// other field, and shown at once while it is in flight. Turning it off must store `false` rather than leave the field
/// as it was (the old `compact hides the second line and persists across client seeds`). Specified: choosing Compact
/// sends exactly one command, `PUT /api/preferences` with `{"compact": true}`, closes the menu with focus on the title,
/// and the rows are compact while the write is in flight, the reopened menu checking it; once the helm pushes the
/// stored preference and answers, the rows stay compact; choosing Compact again sends `{"compact": false}`, and the
/// rows show their last lines.
#[gpui::test]
fn choosing_compact_sends_its_sparse_write_and_the_rows_follow_it(cx: &mut TestAppContext) {
    let (mut fake, main, cx) = open(cx);
    assert!(
        !rows_compact(cx),
        "the premise: the rows show their last lines"
    );
    click(cx, "session-order");
    click(cx, COMPACT_ID);
    let (write, body) = one_write(&mut fake);
    assert_eq!(body, json!({ "compact": true }));
    assert!(!menu_open(&main, cx));
    assert_eq!(focused_id(cx).as_deref(), Some(TITLE_ID));
    assert!(rows_compact(cx), "while the write is in flight");
    click(cx, "session-order");
    assert!(compact_checked(cx), "the open menu checks it in flight");
    keys(cx, "escape");

    stored_compact(&mut fake, Some(true), cx);
    fake.reply(write, 200, None);
    test_support::settle(cx);
    assert!(rows_compact(cx));

    click(cx, "session-order");
    click(cx, COMPACT_ID);
    let (write, body) = one_write(&mut fake);
    assert_eq!(body, json!({ "compact": false }));
    stored_compact(&mut fake, Some(false), cx);
    fake.reply(write, 200, None);
    test_support::settle(cx);
    assert!(!rows_compact(cx));
}

/// SPEC.md, "Errors and diagnostics", names the "compact layout" among the best-effort preferences: a failed save is
/// logged but stays silent, the choice lasts only until the next read from the helm, which under the channel is at
/// once, and it "is never resent" (the order's rule, decided by the migration's orchestrator on 2026-11-14 for the
/// order). Specified: choosing Compact sends `{"compact": true}`; once the helm refuses it with words of its own, the
/// frame paints exactly what it painted before the choice (the rows' last lines, none of the helm's words), nothing is
/// sent again, and the menu shows Compact unchecked; choosing it again still sends its write.
#[gpui::test]
fn a_refused_compact_write_keeps_the_stored_density_and_shows_nothing(cx: &mut TestAppContext) {
    let (mut fake, _main, cx) = open(cx);
    let before = painted(&frame(cx));
    click(cx, "session-order");
    click(cx, COMPACT_ID);
    let (write, body) = one_write(&mut fake);
    assert_eq!(body, json!({ "compact": true }));
    fake.reply(write, 400, Some(json!("no compact rows on this helm")));
    test_support::settle(cx);
    assert_eq!(sent(&mut fake), [], "a refused write is not resent");
    assert_eq!(
        painted(&frame(cx)),
        before,
        "the refusal adds nothing and the rows are as stored"
    );

    click(cx, "session-order");
    assert!(!compact_checked(cx));
    click(cx, COMPACT_ID);
    let (_, body) = one_write(&mut fake);
    assert_eq!(
        body,
        json!({ "compact": true }),
        "a later choice still sends"
    );
}

// ---- The count -----------------------------------------------------------------------------------------------------

/// A bare number beside the title says nothing to a screen reader, so the old count was named, and helped on hover,
/// "Sessions: <count>"; a cut listing's warning took the same name in place of the count. Specified: over two sessions
/// the count is named "Sessions: 2" and shows that as its hover help; over a listing the helm's cap cut at one of five
/// sessions, the warning is named "Sessions: Showing 1 of 5 sessions — could not read the list to the end".
#[gpui::test]
fn the_count_is_named_and_helped_as_sessions_and_its_text(cx: &mut TestAppContext) {
    let (mut fake, _main, cx) = open(cx);
    assert_eq!(label(&frame(cx), COUNT_ID), "Sessions: 2");
    assert_eq!(
        hover_help(cx, "session-count").as_deref(),
        Some("Sessions: 2")
    );

    fake.push(change(listing_meta(&["a"], 5, true), &[], &["b"]));
    test_support::settle(cx);
    assert_eq!(
        label(&frame(cx), COUNT_ID),
        "Sessions: Showing 1 of 5 sessions — could not read the list to the end"
    );
}

// ---- Templates -----------------------------------------------------------------------------------------------------

/// The heading's Templates button is how the window reaches the Templates dialog without a shortcut, and SPEC.md's
/// "Opening and closing" wants focus back where it was when the dialog closes. Its help is the Templates module's
/// (`templates::button_help`), whose words that module's tests pin; here it is enough that the help is that one.
/// Specified: the button is named "Templates", sits right of the title, and its hover help starts "Templates: " and
/// names the ⚡ chord as the module spells it; under the Linux click rule a click on it opens the Templates dialog (its
/// list drawn), and Escape closes it with focus back on the button.
#[gpui::test]
fn templates_opens_its_dialog_and_closing_returns_focus_to_the_button(cx: &mut TestAppContext) {
    let (_fake, _main, cx) = open(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    let nodes = frame(cx);
    assert_eq!(label(&nodes, "templates-button"), "Templates");
    assert!(
        node(&nodes, "templates-button").unwrap().bounds.left()
            > node(&nodes, TITLE_ID).unwrap().bounds.right()
    );
    let help = hover_help(cx, "templates-button").expect("the button has hover help");
    assert!(
        help.starts_with("Templates: ") && help.contains(templates::QUICK_LAUNCH_SHORTCUT_NAME),
        "the Templates module's help: {help}"
    );

    click(cx, "templates-button");
    assert!(
        cx.update(|window, cx| layers::any_open(window, cx)),
        "the dialog opened"
    );
    assert!(
        node(&frame(cx), "templates-list").is_some(),
        "its list is drawn"
    );
    keys(cx, "escape");
    assert!(!cx.update(|window, cx| layers::any_open(window, cx)));
    assert_eq!(focused_id(cx).as_deref(), Some("templates-button"));
}

// ---- New -----------------------------------------------------------------------------------------------------------

/// Start Add host's probe, which makes New unavailable, and return its envelope id, to answer it by.
fn probe_in_flight(fake: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    let path = start_host_change(cx, HostChange::Probe);
    fake.commands()
        .into_iter()
        .find(|command| command.path == path)
        .expect("the premise: the probe is in flight")
        .id
}

/// The heading's New is the window's way into the launcher without a shortcut, and the sidebar's one filled control
/// (SPEC.md, "Session list": "Trash with its count, Templates and New are compact icon buttons beside the title ... New
/// remains the sidebar's one filled control"). Its words are the old button's, which a screen reader user hears in
/// place of the plus. Specified: the button is named "New session", sits right of Templates, takes clicks and focus,
/// and its hover help is New's help word for word ("New session: start an agent or a command on any host", with " (⌘N)"
/// on a Mac); under the Linux click rule a click on it opens the launcher, and Escape closes the launcher with focus
/// back on the button.
#[gpui::test]
fn new_opens_the_launcher_and_closing_returns_focus_to_the_button(cx: &mut TestAppContext) {
    let (_fake, _main, cx) = open(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(true, cx));
    let nodes = frame(cx);
    assert_eq!(label(&nodes, NEW_ID), "New session");
    let new = node(&nodes, NEW_ID).unwrap();
    assert!(
        new.bounds.left() > node(&nodes, "templates-button").unwrap().bounds.right(),
        "New sits after Templates"
    );
    assert!(new.flags.focusable && new.flags.click, "enabled");
    let expected = if cfg!(target_os = "macos") {
        "New session: start an agent or a command on any host (⌘N)"
    } else {
        "New session: start an agent or a command on any host"
    };
    assert_eq!(hover_help(cx, NEW_ID).as_deref(), Some(expected));

    click(cx, NEW_ID);
    assert!(launcher_drawn(cx), "the click opened the launcher");
    keys(cx, "escape");
    assert!(!cx.update(|window, cx| layers::any_open(window, cx)));
    assert_eq!(focused_id(cx).as_deref(), Some(NEW_ID));
}

/// New is unavailable while a launch is pending or a host change is in flight (SPEC.md, "Creation": "Launches and
/// changes to the host registry still exclude each other"), and the old button said so by being disabled; a button
/// that looked ready and did nothing would read as broken. A host probe stands in for both, since it is the cheaper to
/// stage and the button reads the one answer for either (`launcher::entry::new_available`). Specified: with Add host's
/// probe in flight the button takes neither focus nor clicks, and a click on it opens nothing; once the probe is
/// answered the button takes both again.
#[gpui::test]
fn new_is_disabled_while_a_host_change_is_in_flight(cx: &mut TestAppContext) {
    let (mut fake, _main, cx) = open(cx);
    let probe = probe_in_flight(&mut fake, cx);
    let new = node(&frame(cx), NEW_ID).cloned().expect("New is drawn");
    assert!(!new.flags.focusable && !new.flags.click, "disabled");
    click(cx, NEW_ID);
    assert!(
        !launcher_drawn(cx),
        "a click on the disabled button opens nothing"
    );

    fake.reply(probe, 200, Some(json!({})));
    test_support::settle(cx);
    let new = node(&frame(cx), NEW_ID).cloned().expect("New is drawn");
    assert!(new.flags.focusable && new.flags.click, "enabled again");
}

/// New is reached from the heading only while Sessions shows, but the New shortcut works with the Hosts panel open too,
/// and SPEC.md wants an accepted New to bring Sessions back ("All or a host avatar returns to Sessions, as does an
/// accepted quick-switcher pick or New handoff; a refused handoff leaves the panel as it was"). Specified: with the
/// Hosts panel shown and a probe in flight, the New action opens nothing and the panel stays; once the probe is
/// answered, the action opens the launcher and the list's column shows again in place of the panel.
#[gpui::test]
fn the_new_action_leaves_the_hosts_panel_only_when_new_opens(cx: &mut TestAppContext) {
    let (mut fake, _main, cx) = open(cx);
    click(cx, "hosts-panel-toggle");
    assert!(
        node(&frame(cx), "hosts-panel").is_some(),
        "the premise: the Hosts panel shows"
    );
    let probe = probe_in_flight(&mut fake, cx);
    cx.dispatch_action(NewSession);
    test_support::settle(cx);
    assert!(!launcher_drawn(cx), "New was refused");
    assert!(
        node(&frame(cx), "hosts-panel").is_some(),
        "a refused New leaves the panel as it was"
    );

    fake.reply(probe, 200, Some(json!({})));
    test_support::settle(cx);
    cx.dispatch_action(NewSession);
    test_support::settle(cx);
    assert!(launcher_drawn(cx), "New opened");
    let nodes = frame(cx);
    assert!(node(&nodes, "hosts-panel").is_none(), "the panel went");
    assert!(
        node(&nodes, NEW_ID).is_some(),
        "and the heading shows again"
    );
}
