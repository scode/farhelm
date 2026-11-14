//! Tests of the host settings dialog over the helm client's fake: where focus starts and returns, what each permission
//! switch, the destination and alias editors and the appearance picker send, and what a refusal leaves on screen.
//!
//! The dialog is opened the way the user opens it: a click on a Hosts panel row's toggle, then on the menu's Settings.
//! The hosts are the reference screens' three (the local host as 1, `build-box` at `dev@build-box` as 2, `gpu-1` as
//! 3). The tests reach the open dialog through [`Opened`], which only test builds record. Every step ends in one of the
//! shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{CollectionChanges, HostView};
use gpui::{
    DriverNode, Entity, Global, TestAppContext, Toggled, VisualTestContext, WeakEntity, px, size,
};
use serde_json::{Value, json};

use super::{Field, HostSettingsDialog};
use crate::focus;
use crate::hosts::tests::{hosts, open_panel, reference_hosts};
use crate::layers;
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{click, focused_id, frame, keys, settle};

/// The dialog [`super::open_settings`] opened last, recorded in test builds only, so a test can read its fields.
pub(super) struct Opened(pub(super) WeakEntity<HostSettingsDialog>);

impl Global for Opened {}

/// Kit's select-all in a text field on this platform.
const SELECT_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a"
} else {
    "ctrl-a"
};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The Hosts panel over the reference hosts in a window tall enough for the whole dialog, so every control can be
/// clicked where it is drawn, and `build-box`'s settings opened from its menu.
fn open_build_box(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let (helm, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(1600.), px(1600.)));
    settle(cx);
    open_from_menu(cx, "host-menu-toggle-2");
    (helm, cx)
}

/// Open the settings of the host whose toggle is `toggle`, from its menu, and check that they opened.
fn open_from_menu(cx: &mut VisualTestContext, toggle: &'static str) {
    click(cx, toggle);
    click(cx, "host-menu-settings");
    assert!(
        node(&frame(cx), "host-settings").is_some(),
        "the premise: the settings opened"
    );
}

/// The dialog opened last, which must still be open.
fn dialog(cx: &mut VisualTestContext) -> Entity<HostSettingsDialog> {
    let dialog = cx.update(|_, cx| {
        cx.try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
    });
    dialog.expect("the dialog is open")
}

/// The node drawn under `id`, if any.
fn node(nodes: &[DriverNode], id: &str) -> Option<DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .cloned()
}

/// The accessible name of what was drawn under `id`, if anything was.
fn label(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    node(&frame(cx), id).and_then(|node| node.label.map(|label| label.to_string()))
}

/// The field the dialog opened last has open, if any.
fn editing(cx: &mut VisualTestContext) -> Option<Field> {
    let dialog = dialog(cx);
    cx.update(|_, cx| dialog.read(cx).editing)
}

/// Whether `field`'s text field holds focus in the dialog opened last.
fn field_focused(cx: &mut VisualTestContext, field: Field) -> bool {
    let dialog = dialog(cx);
    cx.update(|window, cx| {
        dialog
            .read(cx)
            .input(field)
            .input()
            .focus_handle(cx)
            .is_focused(window)
    })
}

/// Whether anything was drawn under `id`.
fn drawn(cx: &mut VisualTestContext, id: &str) -> bool {
    node(&frame(cx), id).is_some()
}

/// Whether the switch or choice drawn under `id` shows on (pressed); `None` when it is not drawn.
fn on(cx: &mut VisualTestContext, id: &str) -> Option<bool> {
    node(&frame(cx), id).map(|node| node.toggled == Some(Toggled::True))
}

/// The one command sent since the last call: its id, path and body.
fn one_command(helm: &mut FakeHelm) -> (u64, String, Value) {
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!(sent[0].method, Method::Post);
    (
        sent[0].id,
        sent[0].path.clone(),
        sent[0].body.clone().unwrap_or_default(),
    )
}

/// The work in flight on `build-box`'s key.
fn host_work(cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).in_flight(&OpKey::host("2")))
    })
}

/// The helm pushes `build-box` changed by `change`, as it does before replying to a save that changed it.
fn push_build_box(
    helm: &mut FakeHelm,
    change: impl FnOnce(&mut HostView),
    cx: &mut VisualTestContext,
) {
    let mut listed = reference_hosts();
    change(&mut listed[1]);
    let state = hosts(&listed);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
}

/// How a test changes the pushed host to what the helm stores after a save.
type Stores = fn(&mut HostView);

fn any_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

// ---- Opening, closing and focus ------------------------------------------------------------------------------------

/// Why this matters: the old dialog put focus on its first editable setting, the destination's Edit, or the alias's on
/// a host that has no destination to edit, so a keyboard user starts at the top. The helm's own machine has no SSH
/// destination, so its dialog must not offer one. Specified: `build-box`'s settings, opened from its menu, hold focus
/// on "Edit destination" and are named "Host settings · build-box"; the local host's draw no destination row and start on "Edit
/// alias".
#[gpui::test]
fn focus_starts_on_the_first_edit(cx: &mut TestAppContext) {
    let (_helm, cx) = open_build_box(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-settings-edit-destination")
    );
    assert_eq!(
        label(cx, "host-settings-edit-destination").as_deref(),
        Some("Edit destination")
    );
    assert_eq!(
        label(cx, "host-settings").as_deref(),
        Some("Host settings · build-box")
    );
    keys(cx, "escape");

    open_from_menu(cx, "host-menu-toggle-1");
    assert!(!drawn(cx, "host-settings-edit-destination"));
    assert_eq!(focused_id(cx).as_deref(), Some("host-settings-edit-alias"));
    assert_eq!(
        label(cx, "host-settings-edit-alias").as_deref(),
        Some("Edit alias")
    );
}

/// Why this matters: closing the dialog must put focus back on the row's toggle it was opened from, as the old dialog
/// did, also on macOS, where a click leaves a button unfocused and focus would otherwise go back to whatever held it
/// before (a terminal). Run under macOS's click rule for that reason. Specified: Escape, the header's "Close host
/// settings" and the footer's Close each close the dialog, and leave focus on `build-box`'s toggle.
#[gpui::test]
fn escape_and_both_close_controls_return_focus_to_the_toggle(cx: &mut TestAppContext) {
    let (helm, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(1600.), px(1600.)));
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    settle(cx);
    for close in [
        "escape",
        "host-settings-close",
        "host-settings-footer-close",
    ] {
        open_from_menu(cx, "host-menu-toggle-2");
        if close == "escape" {
            keys(cx, "escape");
        } else {
            click(cx, close);
        }
        assert!(!any_open(cx), "{close} closed the dialog");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("host-menu-toggle-2"),
            "{close} returned focus to the toggle"
        );
    }
    drop(helm);
}

/// Why this matters: a dialog about a host that is gone would save into nothing, and SPEC.md has a dialog go away when
/// the thing it is about is gone (the old dialog closed when its host disappeared, `provisioning.spec.ts`). A failed
/// refresh proves nothing, so it must not close it. Specified: with `build-box`'s settings open, a hosts list that
/// fails to refresh leaves it open; a list read whole without `build-box` closes it.
#[gpui::test]
fn the_dialog_closes_when_the_registry_no_longer_lists_its_host(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    let mut failed = hosts(&reference_hosts()).meta;
    failed.availability = farhelm_proto::ui_channel::Availability::Unavailable {
        error: "fixture refresh failure".into(),
    };
    helm.push(CollectionChanges {
        collection: farhelm_proto::ui_channel::Collection::Hosts,
        meta: failed,
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    settle(cx);
    assert!(any_open(cx), "a failed refresh proves nothing");

    let mut listed = reference_hosts();
    listed.remove(1);
    let state = hosts(&listed);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: vec!["2".into()],
    });
    settle(cx);
    assert!(!any_open(cx), "the dialog closed with its host");
}

/// Why this matters: the old field took focus as it opened, on every platform, and on macOS a click leaves a button
/// unfocused, so the move into the field must come from Edit's action rather than from wherever focus was. With the
/// dialog fresh, focus sits on the destination's Edit, so a click on the alias's Edit must not leave it there either.
/// Run under macOS's click rule. Specified: a click on the alias's Edit, with focus on the destination's, puts focus in
/// the alias field; Escape closes the field and puts focus on the alias's Edit; a click on the destination's Edit puts
/// focus in the destination field.
#[gpui::test]
fn a_click_on_edit_focuses_its_field_under_the_macos_click_rule(cx: &mut TestAppContext) {
    let (_helm, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(1600.), px(1600.)));
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    settle(cx);
    open_from_menu(cx, "host-menu-toggle-2");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-settings-edit-destination"),
        "the premise: focus starts on the destination's Edit"
    );

    click(cx, "host-settings-edit-alias");
    assert!(
        field_focused(cx, Field::Alias),
        "the alias field took focus"
    );
    keys(cx, "escape");
    assert_eq!(editing(cx), None);
    assert_eq!(focused_id(cx).as_deref(), Some("host-settings-edit-alias"));

    click(cx, "host-settings-edit-destination");
    assert!(
        field_focused(cx, Field::Destination),
        "the destination field took focus"
    );
}

// ---- Permissions ---------------------------------------------------------------------------------------------------

/// Why this matters: each permission is its own stored value on its own route (SPEC.md, "Agent-spawned sessions"), and
/// agents act on them, so a switch must save exactly its own value, at once, under the host's key. Specified: for each
/// switch, a click sends `POST /api/hosts/2/<route>` with its field set to `true` alone (`allowed` for the two link
/// permissions), the host's key holds a save meanwhile, and once the helm pushes the new value and answers, the switch
/// shows on and the key is free.
#[gpui::test]
fn each_permission_saves_its_own_route_and_field(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    let table: [(&'static str, &str, &str, Stores); 5] = [
        (
            "host-yolo-without-asking",
            "yolo-without-asking",
            "yolo_without_asking",
            |host| host.yolo_without_asking = true,
        ),
        (
            "host-own-sessions-without-asking",
            "own-sessions-without-asking",
            "own_sessions_without_asking",
            |host| host.own_sessions_without_asking = true,
        ),
        (
            "host-commands-without-asking",
            "commands-without-asking",
            "commands_without_asking",
            |host| host.commands_without_asking = true,
        ),
        (
            "host-links-without-asking",
            "links-without-asking",
            "allowed",
            |host| host.links_without_asking = true,
        ),
        (
            "host-local-links-without-asking",
            "local-links-without-asking",
            "allowed",
            |host| host.local_links_without_asking = true,
        ),
    ];
    let mut stored: Vec<Stores> = Vec::new();
    for (switch, route, field, change) in table {
        assert_eq!(on(cx, switch), Some(false), "{route}: the premise");
        click(cx, switch);
        let (id, path, body) = one_command(&mut helm);
        assert_eq!(path, format!("/api/hosts/2/{route}"));
        assert_eq!(body, json!({ field: true }), "{route}: only its field");
        assert_eq!(host_work(cx), Some(Work::Save), "{route}: under the key");
        assert_eq!(on(cx, switch), Some(true), "{route}: shown saving");

        stored.push(change);
        let all = stored.clone();
        push_build_box(
            &mut helm,
            move |host| all.iter().for_each(|change| change(host)),
            cx,
        );
        helm.reply(id, 200, None);
        settle(cx);
        assert_eq!(host_work(cx), None, "{route}: the key is free");
        assert_eq!(on(cx, switch), Some(true), "{route}: stored");
    }
}

/// Why this matters: SPEC.md, "A failed save restores the last known value and shows an adjacent error", and with no
/// optimistic copy a switch that kept its refused value would claim a permission the helm does not hold. Specified: the
/// YOLO switch shows on while its save is in flight; the helm's refusal puts it back off with the helm's words, escaped,
/// in the line under it; the next save that succeeds takes the line away.
#[gpui::test]
fn a_refused_permission_shows_the_stored_value_and_the_helms_words(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    let yolo = "host-yolo-without-asking";
    click(cx, yolo);
    let (id, _, _) = one_command(&mut helm);
    assert_eq!(on(cx, yolo), Some(true), "the pending value shows");
    helm.reply(id, 409, Some(json!("fixture\u{202E} refusal")));
    settle(cx);
    assert_eq!(on(cx, yolo), Some(false), "the stored value is back");
    assert_eq!(
        label(cx, "host-yolo-without-asking-error").as_deref(),
        Some("fixture<U+202E> refusal")
    );

    click(cx, yolo);
    let (id, _, _) = one_command(&mut helm);
    push_build_box(&mut helm, |host| host.yolo_without_asking = true, cx);
    helm.reply(id, 200, None);
    settle(cx);
    assert!(label(cx, "host-yolo-without-asking-error").is_none());
}

// ---- Connection ------------------------------------------------------------------------------------------------------

/// Why this matters: the destination is where Farhelm dials the host, and the old flow keeps a refused draft in place
/// with the helm's reason so the user can correct it, including the helm's answer that the change waits for a setup
/// or Update running on the host. Specified: Edit swaps in a field holding the stored destination, with focus; what is
/// typed goes as `POST /api/hosts/2/destination` `{"ssh": ...}`; a refusal keeps the field and the draft with the
/// helm's words under the row, a 504 likewise; a save that succeeds closes the field with focus back on Edit, and the row
/// shows the new destination.
#[gpui::test]
fn the_destination_saves_as_typed_and_a_refusal_stays_in_place(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    click(cx, "host-settings-edit-destination");
    let editor = dialog(cx);
    let (seeded, focused) = cx.update(|window, cx| {
        let field = &editor.read(cx).destination;
        (
            field.draft().value().clone(),
            field.input().focus_handle(cx).is_focused(window),
        )
    });
    assert_eq!(seeded, "dev@build-box");
    assert!(focused, "focus went into the field");

    keys(cx, &format!("{SELECT_ALL} n e w b o x enter"));
    let (id, path, body) = one_command(&mut helm);
    assert_eq!(path, "/api/hosts/2/destination");
    assert_eq!(body, json!({ "ssh": "newbox" }));
    helm.reply(id, 409, Some(json!("another host is already at newbox")));
    settle(cx);
    assert_eq!(
        label(cx, "host-settings-destination-error").as_deref(),
        Some("another host is already at newbox")
    );
    assert_eq!(editing(cx), Some(Field::Destination), "still open");

    keys(cx, "enter");
    let (id, _, body) = one_command(&mut helm);
    assert_eq!(body, json!({ "ssh": "newbox" }), "the draft was kept");
    let waiting = "the host did not take the new destination within 20 seconds, most likely because a setup or \
                   update is running on it";
    helm.reply(id, 504, Some(json!(waiting)));
    settle(cx);
    assert_eq!(
        label(cx, "host-settings-destination-error").as_deref(),
        Some(waiting)
    );

    keys(cx, "enter");
    let (id, _, _) = one_command(&mut helm);
    push_build_box(
        &mut helm,
        |host| host.destination = Some("newbox".into()),
        cx,
    );
    helm.reply(id, 200, None);
    settle(cx);
    assert_eq!(editing(cx), None, "closed");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-settings-edit-destination")
    );
    let painted: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect();
    assert!(painted.iter().any(|line| line == "newbox"), "{painted:?}");
}

/// Why this matters: the alias is the name a host is shown under, and clearing it must be possible (the helm then names
/// the host by its destination); Escape in an open field must close the field, not the whole dialog, as the old
/// dialog's did. Specified: Escape in the alias field closes it with nothing sent, the dialog open and focus on Edit
/// alias; a new alias goes as `{"alias": "box"}`; an emptied field goes as `{"alias": null}`.
#[gpui::test]
fn the_alias_saves_and_clears_and_escape_closes_only_its_field(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    click(cx, "host-settings-edit-alias");
    assert!(
        field_focused(cx, Field::Alias),
        "the premise: focus went into the field"
    );
    keys(cx, "x escape");
    assert!(helm.commands().is_empty(), "Escape sent nothing");
    assert!(any_open(cx), "the dialog stayed open");
    assert_eq!(editing(cx), None, "the field closed");
    assert_eq!(focused_id(cx).as_deref(), Some("host-settings-edit-alias"));

    keys(cx, "enter");
    keys(cx, &format!("{SELECT_ALL} b o x enter"));
    let (id, path, body) = one_command(&mut helm);
    assert_eq!(path, "/api/hosts/2/alias");
    assert_eq!(body, json!({ "alias": "box" }));
    push_build_box(&mut helm, |host| host.alias = Some("box".into()), cx);
    helm.reply(id, 200, None);
    settle(cx);

    keys(cx, "enter");
    keys(cx, &format!("{SELECT_ALL} backspace enter"));
    let (_, _, body) = one_command(&mut helm);
    assert_eq!(body, json!({ "alias": null }));
}

/// Why this matters: a save in flight must not be abandoned by Escape, or the field would close while the helm may
/// still refuse the value, and its words would have nowhere to show; the old dialog's Escape followed its field's
/// Cancel, which is unavailable mid-save. Specified: with the destination's save in flight, Escape sends nothing more,
/// leaves the dialog and the field open, and the field closes only once the helm answers. The Escape was a key press,
/// so the Save's own move back to Edit does not run (the user moved on since the Save); focus, left on the field that
/// went, is recovered to the closest control left, the destination's Edit drawn in its place.
#[gpui::test]
fn escape_while_a_save_is_in_flight_keeps_the_field_open(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    click(cx, "host-settings-edit-destination");
    keys(cx, &format!("{SELECT_ALL} n e w b o x enter"));
    let (id, _, _) = one_command(&mut helm);
    assert_eq!(host_work(cx), Some(Work::Save), "the premise: in flight");

    keys(cx, "escape");
    assert!(helm.commands().is_empty(), "Escape sent nothing");
    assert!(any_open(cx), "the dialog stayed open");
    assert_eq!(
        editing(cx),
        Some(Field::Destination),
        "the field stayed open"
    );

    push_build_box(
        &mut helm,
        |host| host.destination = Some("newbox".into()),
        cx,
    );
    helm.reply(id, 200, None);
    settle(cx);
    assert_eq!(editing(cx), None, "the answer closed it");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-settings-edit-destination"),
        "the coordinator's recovery, not the Save's move"
    );
}

// ---- Appearance ------------------------------------------------------------------------------------------------------

/// Why this matters: the helm stores a host's icon and color together and refuses a body without both, and a click must
/// save at once (SPEC.md, "Topology"). Specified: a click on Rocket sends `{"icon": "rocket", "color": "default"}` and
/// shows Rocket pressed while in flight; the helm's refusal puts Cloud back with its words under the picker; a click on
/// Teal sends `{"icon": "cloud", "color": "teal"}`.
#[gpui::test]
fn an_appearance_choice_saves_icon_and_color_together(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    assert_eq!(on(cx, "host-icon-cloud"), Some(true), "the premise");
    click(cx, "host-icon-rocket");
    let (id, path, body) = one_command(&mut helm);
    assert_eq!(path, "/api/hosts/2/appearance");
    assert_eq!(
        body,
        json!({ "icon": HostIcon::Rocket.as_str(), "color": HostColor::Default.as_str() })
    );
    assert_eq!(on(cx, "host-icon-rocket"), Some(true));
    helm.reply(id, 409, Some(json!("fixture appearance refusal")));
    settle(cx);
    assert_eq!(
        on(cx, "host-icon-cloud"),
        Some(true),
        "the stored icon is back"
    );
    assert_eq!(
        label(cx, "host-settings-appearance-error").as_deref(),
        Some("fixture appearance refusal")
    );

    click(cx, "host-color-teal");
    let (_, _, body) = one_command(&mut helm);
    assert_eq!(body, json!({ "icon": "cloud", "color": "teal" }));
}

/// Why this matters: SPEC.md, "Reaching everything by keyboard": every choice must be reachable and usable without a
/// pointer, and the old picker's choices were buttons Tab reached one by one (not a radio group the arrows walk, which
/// would save at every step). Specified: from the destination's Edit, two Tabs reach Cloud and one more House; Enter
/// there sends House with the current color; fifteen more Tabs reach the Default color and one more Lavender, where
/// Space sends Lavender with the current icon.
#[gpui::test]
fn the_picker_is_reached_and_used_by_keyboard(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    keys(cx, "tab tab");
    assert_eq!(focused_id(cx).as_deref(), Some("host-icon-cloud"));
    keys(cx, "tab enter");
    assert_eq!(focused_id(cx).as_deref(), Some("host-icon-house"));
    let (id, _, body) = one_command(&mut helm);
    assert_eq!(body, json!({ "icon": "house", "color": "default" }));
    helm.reply(id, 409, None);
    settle(cx);

    keys(cx, &vec!["tab"; 15].join(" "));
    assert_eq!(focused_id(cx).as_deref(), Some("host-color-default"));
    keys(cx, "tab space");
    assert_eq!(focused_id(cx).as_deref(), Some("host-color-lavender"));
    let (_, _, body) = one_command(&mut helm);
    assert_eq!(body, json!({ "icon": "cloud", "color": "lavender" }));
}
