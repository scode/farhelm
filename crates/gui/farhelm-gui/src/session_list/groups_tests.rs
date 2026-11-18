//! The group layout and controls read the same pushed metadata as a real
//! window. The fake holds writes so the tests can observe pending choices.

use farhelm_helm_client::{
    Method,
    fake::{FakeCommand, FakeHelm},
};
use farhelm_proto::ui_channel::{CollectionState, SessionGroup};
use gpui::TestAppContext;
use serde_json::json;

use super::menu_tests::{drawn, label_of, snapshot};
use super::tests::{Fixture, open_main, row_order, session};
use crate::test_support::{self, frame};

/// A named group as stored by the helm. A filtered fixture can name a
/// member total larger than the number of rows it sends.
pub(super) fn group(id: i64, name: &str, members: u64, collapsed: bool) -> SessionGroup {
    SessionGroup {
        id,
        name: name.to_owned(),
        position: id,
        collapsed,
        members,
    }
}

pub(super) fn grouped_snapshot(
    rows: &[Fixture],
    groups: Vec<SessionGroup>,
) -> Vec<CollectionState> {
    let rows: Vec<_> = rows.iter().cloned().map(|row| (row, 0)).collect();
    let mut state = snapshot(&rows, false);
    state[0].meta.session_groups = groups;
    state
}

/// The test drives just the group writes. Answer the main view's usual
/// selection and read marks as they come in so they cannot hold it up.
fn group_commands(helm: &mut FakeHelm) -> Vec<FakeCommand> {
    helm.commands()
        .into_iter()
        .filter(|command| {
            if command.path.starts_with("/api/session-groups") || command.path.ends_with("/group") {
                true
            } else {
                helm.reply(command.id, 200, None);
                false
            }
        })
        .collect()
}

/// Collapse hides all rows while leaving the muted live wait eligible for
/// the header's dot. Reversing a pending expand stays visible and persists
/// after the first write, even while the pointer holds the row order.
#[gpui::test]
fn grouped_layout_and_collapse_queue_follow_the_latest_choice(cx: &mut TestAppContext) {
    let state = grouped_snapshot(
        &[
            session("loose"),
            session("run").grouped(10),
            session("wait")
                .title("Waiting member")
                .status(json!({"state": "waiting"}))
                .muted(None)
                .pinned(5)
                .grouped(10),
            session("global").pinned(12),
        ],
        vec![group(10, "Project", 2, true)],
    );
    let (mut fake, cx) = open_main(cx, state);
    let _ = group_commands(&mut fake);
    assert_eq!(
        row_order(&frame(cx)),
        ["global", "loose"],
        "premise: the stored group is collapsed"
    );
    let attention = label_of(cx, "session-group-10-attention").expect("one attention mark");
    assert!(
        attention.contains("Waiting member") && attention.ends_with("Waiting"),
        "{attention}"
    );
    test_support::click(cx, "session-group-10-toggle");
    assert_eq!(row_order(&frame(cx)), ["global", "wait", "run", "loose"]);
    assert!(!drawn(cx, "session-group-10-attention"));
    let first = group_commands(&mut fake);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].method, Method::Patch);
    assert_eq!(first[0].path, "/api/session-groups/10");
    assert_eq!(first[0].body, Some(json!({"collapsed": false})));
    test_support::click(cx, "session-group-10-toggle");
    assert_eq!(row_order(&frame(cx)), ["global", "loose"]);
    assert!(group_commands(&mut fake).is_empty());
    fake.reply(first[0].id, 200, None);
    test_support::settle(cx);
    let second = group_commands(&mut fake);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].body, Some(json!({"collapsed": true})));
}

/// The full roster drives a menu even if this list has no matching member
/// of the destination. The same destination is offered by its stored ID.
#[gpui::test]
fn row_menu_can_move_to_a_group_with_no_displayed_members(cx: &mut TestAppContext) {
    let state = grouped_snapshot(&[session("a")], vec![group(10, "Offscreen", 2, false)]);
    let (mut fake, cx) = open_main(cx, state);
    let _ = group_commands(&mut fake);
    assert_eq!(row_order(&frame(cx)), ["a"]);
    assert!(!drawn(cx, "session-group-10-toggle"));
    test_support::click(cx, "session-menu-a");
    test_support::click(cx, "session-menu-group");
    assert_eq!(
        label_of(cx, "session-menu-group-10").as_deref(),
        Some("Offscreen")
    );
    test_support::click(cx, "session-menu-group-10");
    let written = group_commands(&mut fake);
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].path, "/api/sessions/a/group");
    assert_eq!(written[0].body, Some(json!({"target": {"group": 10}})));
}

/// Marked New group opens one draft for the selection. Cancel preserves
/// the marks and returns to their menu. A save sends one request and clears
/// them at dispatch. After a refusal, closing the editor must land focus
/// somewhere Tab can work, since the marked menu is already gone.
#[gpui::test]
fn marked_new_group_sends_one_request_and_clears_marks_at_dispatch(cx: &mut TestAppContext) {
    use super::marking::{Gesture, MENU_TOGGLE_ID};
    let state = grouped_snapshot(&[session("a"), session("b")], Vec::new());
    let (mut fake, list, cx) = super::marking_tests::open_list(cx, state);
    let _ = group_commands(&mut fake);
    cx.update(|_, cx| {
        list.update(cx, |list, cx| {
            list.row_clicked("a", Gesture::Toggle, cx);
            list.row_clicked("b", Gesture::Toggle, cx);
        })
    });
    test_support::settle(cx);
    assert!(
        drawn(cx, MENU_TOGGLE_ID),
        "premise: two session rows are marked"
    );
    let open = |cx: &mut gpui::VisualTestContext| {
        test_support::click(cx, MENU_TOGGLE_ID);
        test_support::click(cx, "session-marked-menu-group");
        test_support::click(cx, "session-marked-menu-group-new");
        assert!(drawn(cx, "rename-field"));
    };
    open(cx);
    test_support::keys(cx, "escape");
    assert!(drawn(cx, MENU_TOGGLE_ID));
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some(MENU_TOGGLE_ID)
    );
    assert!(group_commands(&mut fake).is_empty());
    open(cx);
    test_support::keys(cx, "w o r k enter");
    let [create] = &group_commands(&mut fake)[..] else {
        panic!("one request for both sessions")
    };
    assert_eq!(create.path, "/api/session-groups");
    assert_eq!(create.method, Method::Post);
    assert_eq!(
        create.body,
        Some(json!({"name": "work", "sessions": ["a", "b"]}))
    );
    assert!(!drawn(cx, MENU_TOGGLE_ID));
    fake.reply(create.id, 400, Some(json!("a group already has that name")));
    test_support::settle(cx);
    assert!(drawn(cx, "rename-field"));
    assert_eq!(
        label_of(cx, "rename-refusal").as_deref(),
        Some("a group already has that name")
    );
    test_support::keys(cx, "escape");
    assert!(!drawn(cx, "rename-field"));
    assert!(!drawn(cx, MENU_TOGGLE_ID));
    test_support::keys(cx, "tab");
    assert!(
        test_support::focused_id(cx).is_some(),
        "Tab can reach a drawn control after the editor closes"
    );
}
