//! Tests of a retired containerized session in the list (`retired.rs`): its row's title in either density, its menu's
//! one command and the header's reason, no bell and no seen toggle, the Rename shortcut and the automatic seen mark
//! leaving it alone, the marked bar leaving it out of every batch, and an ordinary container session keeping every
//! ordinary command. The open session's view of one
//! is `session_pane/retired_tests.rs`'s.
//!
//! Everything runs in the real main view over the helm client's fake, as the list's other tests do, and reads what the
//! window sent from the fake and what it drew from GPUI's driver record. A retired session is seeded as the helm
//! stores one, an agent launch whose selection still names the old container ([`retired_launch`]); nothing about its
//! host differs from an ordinary session's.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HostView, PREFERENCES_KEY, Preferences,
};
use farhelm_proto::{EnvironmentHostView, LaunchTarget, Place, RETIRED_CONTAINER_REASON};
use gpui::{Modifiers, Role, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::host_rail::tests::{hosts, local, remote};
use super::marking::{COUNT_ID, MENU_TOGGLE_ID};
use super::menu_tests::{drawn, label_of};
use super::tests::{Fixture, TEST_NOW, listing, open_main, painted, preferences_state, session};
use crate::actions::RENAME_SESSION_CHORD;
use crate::test_support::{self, focused_id, frame, meta};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// A launch as the retired model stored it: a Codex agent launch whose selection names the old Podman container, its
/// archive under the legacy `image_file` key, as the old app's own spec seeded one.
pub(crate) fn retired_launch() -> Value {
    json!({
        "kind": "agent",
        "selection": {"harness": "codex", "model": null, "effort": null, "permissions": null,
            "container": {"engine": "podman", "image_file": "legacy.tar", "api_proxy_port": 43210}},
        "start": ["codex", "{farhelm_args}"],
        "resume": null,
    })
}

/// `key` as a retired containerized session: running as last listed, so a live session's controls would show.
pub(crate) fn retired(key: &'static str) -> Fixture {
    session(key).launch(retired_launch())
}

/// The registry with host 2, the fixtures' `buildbox`, as a container a session owns: session-owned, and described as
/// a recipe's container on the local host, the shape the helm lists a new container session's host in.
pub(crate) fn container_registry() -> CollectionState {
    let mut registry = hosts(&[local(), remote(2, "buildbox")]);
    let entry = &mut registry.entries[1];
    let mut view: HostView = serde_json::from_value(entry.value.clone()).unwrap();
    assert_eq!(view.id, 2, "the premise: the sessions' host");
    view.kind = "podman".into();
    view.is_session_owned = true;
    view.environment = Some(EnvironmentHostView {
        name: "owned-container".into(),
        place_name: Some("Build machine".into()),
        target: LaunchTarget::Environment {
            recipe: "dev".into(),
            place: Place::PodmanHost(1),
        },
        tunnels: None,
        credential_warning: None,
    });
    entry.value = serde_json::to_value(&view).unwrap();
    registry
}

/// The preferences with the first of `order` open, the compact preference as `compact` says, the Welcome card seen
/// (it would open over the main view otherwise), and the sidebar at its widest: the retired prefix keeps its width and
/// the title gives way, and at the default width the fixtures' rows (their ages read thousands of days) leave the
/// title no room to show.
fn preferences(order: &[&str], compact: Option<bool>) -> Preferences {
    Preferences {
        visit_history: order.iter().take(1).map(|key| key.to_string()).collect(),
        compact,
        sidebar_width: Some(*farhelm_proto::ui_channel::SIDEBAR_WIDTH_RANGE.end()),
        welcome_seen: Some(true),
        ..Preferences::default()
    }
}

/// The real main view over `rows` in that order, the first one open, with the compact preference `compact` and the
/// `extra` collections (a registry).
fn open<'a>(
    cx: &'a mut TestAppContext,
    rows: &[Fixture],
    compact: Option<bool>,
    extra: Vec<CollectionState>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    let mut snapshot = vec![
        listing(&order, rows),
        preferences_state(preferences(&order, compact)),
    ];
    snapshot.extend(extra);
    open_main(cx, snapshot)
}

/// The ids of the menu items drawn in a fresh frame, top to bottom.
fn menu_items(cx: &mut VisualTestContext) -> Vec<String> {
    let mut items: Vec<_> = frame(cx)
        .into_iter()
        .filter(|node| node.role == Some(Role::MenuItem))
        .filter_map(|node| Some((node.bounds.origin.y, node.element_id?.to_string())))
        .collect();
    items.sort_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)));
    items.into_iter().map(|(_, id)| id).collect()
}

/// Whether an element with the id `id` was drawn in a fresh frame.
fn has_id(cx: &mut VisualTestContext, id: &str) -> bool {
    frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

/// The paths of the commands the window sent since the last call, each answered with success at once: the fake answers
/// in order, so a test that answers a later command (a Delete) answers these first.
fn answered(fake: &mut FakeHelm) -> Vec<String> {
    fake.commands()
        .into_iter()
        .map(|command| {
            fake.reply(command.id, 200, None);
            command.path
        })
        .collect()
}

/// The seen marks among [`answered`]'s commands.
fn seen_marks(fake: &mut FakeHelm) -> Vec<String> {
    answered(fake)
        .into_iter()
        .filter(|path| path.ends_with("/seen"))
        .collect()
}

// ---- The row -------------------------------------------------------------------------------------------------------

/// SPEC.md, "Retired containerized sessions", leaves such a session Delete only; the old row said so in its title in
/// every density, because a compact row has no other line that could. Without the prefix a retired row looks like any
/// other whose every action fails. Specified: a retired session titled `historical` is named and painted
/// "Retired container · historical" and an ordinary one keeps its plain title; once the helm pushes the compact
/// preference on (premise: a's last line, its `~/src/a`, is gone), the retired row still reads the same.
#[gpui::test]
fn a_retired_rows_title_says_so_in_either_density(cx: &mut TestAppContext) {
    let rows = [session("b"), retired("a").title("historical")];
    let (mut fake, cx) = open(cx, &rows, None, Vec::new());
    for compact in [false, true] {
        assert_eq!(
            label_of(cx, "session-row-a").as_deref(),
            Some("Retired container · historical"),
            "compact: {compact}"
        );
        assert_eq!(label_of(cx, "session-row-b").as_deref(), Some("b"));
        let texts = painted(&frame(cx));
        assert_eq!(
            texts.iter().any(|text| text == "~/src/a"),
            !compact,
            "the premise: a's last line shows only in the normal density"
        );
        for words in ["Retired container ·", "historical"] {
            assert!(
                texts.iter().any(|text| text == words),
                "{words} is painted (compact: {compact}): {texts:?}"
            );
        }
        fake.push(CollectionChanges {
            collection: Collection::Preferences,
            meta: meta(&[PREFERENCES_KEY]),
            upsert: vec![
                preferences_state(preferences(&["b", "a"], Some(true)))
                    .entries
                    .remove(0),
            ],
            remove: Vec::new(),
        });
        test_support::settle(cx);
    }
}

/// The old menu packed a retired row's commands down to Delete, and its header said why: offering Rename, Pin, Clone
/// or Stop would only collect the helm's refusals. Specified: with an ordinary b open, a click on retired a's button
/// opens a menu whose one command is Delete, with focus on it; its header reads "Codex · Running · " and the retired
/// reason; Delete asks, and Confirm delete sends the ordinary unguarded `DELETE /api/sessions/a`; the helm's refusal
/// shows on a's row with its words.
#[gpui::test]
fn a_retired_rows_menu_holds_delete_alone_and_says_why(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[session("b"), retired("a")], None, Vec::new());
    answered(&mut fake);
    test_support::click(cx, "session-menu-a");
    assert_eq!(menu_items(cx), ["session-menu-delete"]);
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-delete"));
    assert_eq!(
        label_of(cx, "session-menu-summary-a"),
        Some(format!("Codex · Running · {RETIRED_CONTAINER_REASON}"))
    );
    assert_eq!(
        RETIRED_CONTAINER_REASON,
        "Containerized sessions are retired and support Delete only. Create a new container session from a recipe."
    );

    test_support::click(cx, "session-menu-delete");
    assert!(drawn(cx, "prompt"), "the ordinary Delete question");
    assert_eq!(
        answered(&mut fake),
        Vec::<String>::new(),
        "nothing before the answer"
    );
    test_support::click(cx, "prompt-confirm");
    let (id, _) = test_support::one_command(&mut fake, Method::Delete, "/api/sessions/a");
    fake.reply(id, 409, Some(json!("retired fixture retained for retry")));
    test_support::settle(cx);
    assert_eq!(
        label_of(cx, "session-outcome-a").as_deref(),
        Some("retired fixture retained for retry")
    );
}

/// Closing a bell writes a read mark and the status dot's click writes a seen stamp, both metadata the old app kept
/// unchanged on a retired session. Specified: retired a and ordinary b both have an unread notification and unseen
/// output; b draws its bell and its dot as a Mark read toggle, a draws neither (its dot is a bare mark).
#[gpui::test]
fn a_retired_row_draws_no_bell_and_no_seen_toggle(cx: &mut TestAppContext) {
    let note = json!([{ "seq": 1, "at": TEST_NOW - 60, "text": "build finished" }]);
    let unseen = |fixture: Fixture| {
        fixture
            .activity(200)
            .seen(Some(100))
            .notifications(note.clone(), 0)
    };
    let (_fake, cx) = open(
        cx,
        &[session("c"), unseen(session("b")), unseen(retired("a"))],
        None,
        Vec::new(),
    );
    assert!(has_id(cx, "session-bell-b"), "the premise: b's bell");
    assert!(has_id(cx, "session-seen-b"), "the premise: b's toggle");
    assert!(!has_id(cx, "session-bell-a"));
    assert!(!has_id(cx, "session-seen-a"));
}

/// The old app kept a retired session's stored metadata unchanged even on selection, and its list fenced the Rename
/// shortcut with the menu's Rename. The helm refuses a retired session's seen write too, which is why it stays the one
/// session the automatic mark skips now that stale and ended ones are marked. Specified: with retired a open at startup
/// and its output unseen, no seen mark is sent for it and the Rename shortcut opens no editor; opening b, whose output
/// is unseen too, sends b's mark, so the automatic mark is not off altogether.
#[gpui::test]
fn opening_a_retired_session_stamps_nothing_and_rename_opens_nothing(cx: &mut TestAppContext) {
    let unseen = |fixture: Fixture| fixture.activity(200).seen(Some(100));
    let (mut fake, cx) = open(
        cx,
        &[unseen(retired("a")), unseen(session("b"))],
        None,
        Vec::new(),
    );
    assert!(drawn(cx, "session-retired-card"), "the premise: a is open");
    assert_eq!(seen_marks(&mut fake), Vec::<String>::new());
    test_support::keys(cx, RENAME_SESSION_CHORD);
    assert!(!drawn(cx, "rename"), "no Rename editor");

    test_support::click(cx, "session-row-b");
    assert_eq!(seen_marks(&mut fake), ["/api/sessions/b/seen"]);
}

/// The old marked bar kept a retired row out of every metadata batch ("Delete-only retired rows never enter a
/// metadata batch"), so a Pin of the marks never wrote to one, and a bar over retired rows alone offered no metadata
/// action (its Delete is `bulk_tests`').
/// Specified: with ordinary b open, Ctrl-clicking retired a and ordinary c marks both ("2 marked"), and the marked
/// menu's Pin sends only `PUT /api/sessions/c/pin`; with retired a marked alone ("1 marked"), the bar draws no ⋯, so
/// none of Pin, Unpin, Mark read and Mark unread is offered.
#[gpui::test]
fn the_marked_bar_leaves_a_retired_row_out_of_every_batch(cx: &mut TestAppContext) {
    let unseen = |fixture: Fixture| fixture.activity(200).seen(Some(100));
    let (mut fake, cx) = open(
        cx,
        &[session("b"), unseen(retired("a")), unseen(session("c"))],
        None,
        Vec::new(),
    );
    answered(&mut fake);
    let ctrl = Modifiers {
        control: true,
        ..Modifiers::default()
    };
    let click = |cx: &mut VisualTestContext, selector: &'static str| {
        let at = test_support::center(cx, selector);
        cx.simulate_click(at, ctrl);
        test_support::settle(cx);
    };
    click(cx, "session-row-a");
    click(cx, "session-row-c");
    assert_eq!(label_of(cx, COUNT_ID).as_deref(), Some("2 marked"));
    test_support::click(cx, MENU_TOGGLE_ID);
    test_support::click(cx, "session-marked-menu-pin");
    assert_eq!(answered(&mut fake), ["/api/sessions/c/pin"]);

    click(cx, "session-row-a");
    assert_eq!(label_of(cx, COUNT_ID).as_deref(), Some("1 marked"));
    assert!(!has_id(cx, MENU_TOGGLE_ID), "no metadata action applies");
}

/// Retirement is the stored launch's, never the host's: a new container session runs an ordinary launch on a host its
/// environment owns, and the old spec's other half asserts it keeps every ordinary control. Telling them apart by the
/// host would strand every new container session with Delete alone. Specified: an ordinary session on a session-owned
/// container host is titled plainly and its menu holds Rename, Mark unread, Pin to top, Mute, the environment's Suspend and
/// Stop and suspend, Clone, Replace with…, Replace, Stop and Delete, in that order.
#[gpui::test]
fn an_ordinary_container_sessions_row_keeps_its_commands(cx: &mut TestAppContext) {
    let (_fake, cx) = open(
        cx,
        &[session("b"), session("a")],
        None,
        vec![container_registry()],
    );
    assert_eq!(label_of(cx, "session-row-a").as_deref(), Some("a"));
    test_support::click(cx, "session-menu-a");
    assert_eq!(
        menu_items(cx),
        [
            "session-menu-rename",
            "session-menu-seen",
            "session-menu-pin",
            "session-menu-group",
            "session-menu-mute",
            "session-menu-suspend",
            "session-menu-stop-and-suspend",
            "session-menu-clone",
            "session-menu-replace-with",
            "session-menu-replace",
            "session-menu-stop",
            "session-menu-delete",
        ]
    );
}
