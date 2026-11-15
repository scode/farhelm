//! Tests of the host settings dialog over the helm client's fake: where focus starts and returns, what each permission
//! switch, the destination and alias editors, the appearance picker and the checkout folder send, and what a refusal
//! leaves on screen.
//!
//! The dialog is opened the way the user opens it: a click on a Hosts panel row's toggle, then on the menu's Settings.
//! The hosts are the reference screens' three (the local host as 1, `build-box` at `dev@build-box` as 2, `gpu-1` as
//! 3). The tests reach the open dialog through [`Opened`], which only test builds record. Every step ends in one of the
//! shared harness's step helpers, so the focus and layer invariants are checked after each.
//!
//! Every opening reads the host's checkout folder, and the fake answers commands only in the order they were sent, so
//! [`open_from_menu`] answers that read before a test sends anything else; the Checkouts tests answer it themselves
//! ([`open_with_folder`]) when the folder matters to them.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{CollectionChanges, HostView};
use gpui::{
    DriverNode, Entity, Global, TestAppContext, Toggled, VisualTestContext, WeakEntity, px, size,
};
use serde_json::{Value, json};

use super::{FOLDER_NOTE, Field, HostSettingsDialog};
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

/// A window tall enough for the whole dialog, every section drawn without scrolling, so every control can be clicked
/// where it is drawn: a control the body scrolled out of view would be clicked at a point that is not on it.
const TALL: (f32, f32) = (1600., 2400.);

/// The checkout folder read's answer for a host with no folder of its own and no default, as the reference stack had.
fn no_folder() -> Value {
    json!({ "root": null, "default": null })
}

/// The Hosts panel over the reference hosts in a [`TALL`] window, and `build-box`'s settings opened from its menu, its
/// checkout folder's read answered with [`no_folder`].
fn open_build_box(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    open_build_box_with_folder(cx, no_folder())
}

/// Open the settings of the host whose toggle is `toggle`, from its menu, check that they opened, and answer the
/// checkout folder's read with [`no_folder`].
fn open_from_menu(helm: &mut FakeHelm, cx: &mut VisualTestContext, toggle: &'static str) {
    open_with_folder(helm, cx, toggle, no_folder());
}

/// [`open_from_menu`] with `folder` as the answer to the checkout folder's read, which must be the one command the
/// opening sent, a `GET` of the host's own route.
fn open_with_folder(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    toggle: &'static str,
    folder: Value,
) {
    click(cx, toggle);
    click(cx, "host-menu-settings");
    assert!(
        node(&frame(cx), "host-settings").is_some(),
        "the premise: the settings opened"
    );
    let host = toggle.trim_start_matches("host-menu-toggle-");
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "the opening read the folder alone: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (
            Method::Get,
            format!("/api/hosts/{host}/checkout-root").as_str()
        )
    );
    helm.reply(sent[0].id, 200, Some(folder));
    settle(cx);
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
    let (mut helm, cx) = open_build_box(cx);
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

    open_from_menu(&mut helm, cx, "host-menu-toggle-1");
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
    let (mut helm, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(TALL.0), px(TALL.1)));
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    settle(cx);
    for close in [
        "escape",
        "host-settings-close",
        "host-settings-footer-close",
    ] {
        open_from_menu(&mut helm, cx, "host-menu-toggle-2");
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
    let (mut helm, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(TALL.0), px(TALL.1)));
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    settle(cx);
    open_from_menu(&mut helm, cx, "host-menu-toggle-2");
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

// ---- Checkouts -------------------------------------------------------------------------------------------------------

/// The route `build-box`'s checkout folder reads and saves.
const FOLDER_ROUTE: &str = "/api/hosts/2/checkout-root";

/// [`open_build_box`] with `folder` as the answer to the checkout folder's read.
fn open_build_box_with_folder(
    cx: &mut TestAppContext,
    folder: Value,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(TALL.0), px(TALL.1)));
    settle(cx);
    open_with_folder(&mut helm, cx, "host-menu-toggle-2", folder);
    (helm, cx)
}

/// Every line of text the frame painted, joined with single spaces, so a sentence a wrap split still matches.
fn painted(cx: &mut VisualTestContext) -> String {
    let lines: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect();
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The text in the checkout folder field of the dialog opened last.
fn folder_text(cx: &mut VisualTestContext) -> String {
    let dialog = dialog(cx);
    cx.update(|_, cx| dialog.read(cx).checkout.read(cx).text(cx))
}

/// Click into the checkout folder field, check that the click put focus there, and type `text` over what it holds.
fn type_folder(cx: &mut VisualTestContext, text: &str) {
    click(cx, "checkout-folder");
    let dialog = dialog(cx);
    let focused = cx.update(|window, cx| dialog.read(cx).checkout.read(cx).focused(window, cx));
    assert!(focused, "the premise: the click put focus in the field");
    keys(cx, SELECT_ALL);
    cx.simulate_input(text);
    settle(cx);
}

/// The one command sent since the last call, which must be `method` on the folder's route: its id and its body.
fn one_folder_command(helm: &mut FakeHelm, method: Method) -> (u64, Value) {
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (method, FOLDER_ROUTE)
    );
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// Why this matters: SPEC.md, "Topology", puts every host's own checkout folder in its settings dialog, in a
/// Checkouts section between Appearance and Permissions "for every host kind", with the default it inherits shown as
/// the empty field's placeholder; the old dialog had it there with its own note, and the footer already says the folder
/// waits for Save. Specified: `build-box`'s dialog reads `/api/hosts/2/checkout-root` as it opens (the open helper
/// checks that it is the one command sent) and draws a group named "Checkouts" below Appearance and above Permissions,
/// whose empty field shows "Default: ~/src", the old note under it, and no Use default; the local host's dialog draws
/// the section too, and with no default set says so.
#[gpui::test]
fn the_checkouts_section_sits_between_appearance_and_permissions_on_every_host(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) =
        open_build_box_with_folder(cx, json!({ "root": null, "default": "~/src" }));
    let nodes = frame(cx);
    let top = |id: &str| {
        node(&nodes, id)
            .unwrap_or_else(|| panic!("{id} is drawn"))
            .bounds
            .top()
    };
    assert!(top("host-settings-appearance") < top("host-settings-checkouts"));
    assert!(top("host-settings-checkouts") < top("host-settings-permissions"));
    assert_eq!(
        label(cx, "host-settings-checkouts").as_deref(),
        Some("Checkouts")
    );
    assert_eq!(folder_text(cx), "");
    let shown = painted(cx);
    assert!(shown.contains("Default: ~/src"), "{shown}");
    assert!(shown.contains(FOLDER_NOTE), "the old note, whole: {shown}");
    assert!(!drawn(cx, "checkout-folder-use-default"));
    keys(cx, "escape");

    open_from_menu(&mut helm, cx, "host-menu-toggle-1");
    assert!(drawn(cx, "host-settings-checkouts"), "the local host's too");
    assert!(painted(cx).contains("No default set"), "{}", painted(cx));
}

/// Why this matters: the folder decides where every new managed checkout on the host goes, so the dialog must change
/// what the helm stores, not a local copy: `host-checkout-folder.spec.ts` saved, refused, read back and cleared it
/// through the host's own route. And the save must be work on the host's key, which the dialog's other saves share, so
/// that two writes to one host never race. The old case also checked that nothing in the field overflowed, which a
/// helm refusal longer than the field once did here. Specified: a refused folder keeps its draft and shows the helm's
/// words, wrapped inside the section however long they are; a saved one goes as `PUT /api/hosts/2/checkout-root`
/// `{"root": ...}` under the host's key, is read back, says "Checkout folder saved" and offers Use default; Use default
/// sends `{"root": null}`, and the read back empties the field, takes Use default away and shows the default as the
/// placeholder again.
#[gpui::test]
fn the_host_folder_saves_under_the_hosts_key_and_use_default_clears_it(cx: &mut TestAppContext) {
    let (mut helm, cx) =
        open_build_box_with_folder(cx, json!({ "root": null, "default": "~/def" }));
    type_folder(cx, "~another-user/work");
    click(cx, "checkout-folder-save");
    let (id, body) = one_folder_command(&mut helm, Method::Put);
    assert_eq!(body, json!({ "root": "~another-user/work" }));
    assert_eq!(host_work(cx), Some(Work::Save), "under the host's key");
    let refusal = "checkout root must be an absolute path (or ~, ~/...), not ~user (fixture refusal, long enough \
                   to need more than one line in the dialog)";
    helm.reply(id, 400, Some(json!(refusal)));
    settle(cx);
    assert_eq!(label(cx, "checkout-folder-error").as_deref(), Some(refusal));
    assert_eq!(folder_text(cx), "~another-user/work", "the draft was kept");
    let nodes = frame(cx);
    let right = |id: &str| {
        node(&nodes, id)
            .unwrap_or_else(|| panic!("{id} is drawn"))
            .bounds
            .right()
    };
    assert!(
        right("checkout-folder-error") <= right("host-settings-checkouts"),
        "the refusal wraps inside its section"
    );

    type_folder(cx, "/srv/box");
    click(cx, "checkout-folder-save");
    let (id, body) = one_folder_command(&mut helm, Method::Put);
    assert_eq!(body, json!({ "root": "/srv/box" }));
    helm.reply(id, 204, None);
    settle(cx);
    let (id, _) = one_folder_command(&mut helm, Method::Get);
    helm.reply(
        id,
        200,
        Some(json!({ "root": "/srv/box", "default": "~/def" })),
    );
    settle(cx);
    assert_eq!(
        label(cx, "checkout-folder-saved").as_deref(),
        Some("Checkout folder saved")
    );
    assert_eq!(folder_text(cx), "/srv/box");
    assert!(drawn(cx, "checkout-folder-use-default"));

    click(cx, "checkout-folder-use-default");
    let (id, body) = one_folder_command(&mut helm, Method::Put);
    assert_eq!(body, json!({ "root": null }));
    helm.reply(id, 204, None);
    settle(cx);
    let (id, _) = one_folder_command(&mut helm, Method::Get);
    helm.reply(id, 200, Some(json!({ "root": null, "default": "~/def" })));
    settle(cx);
    assert_eq!(folder_text(cx), "");
    assert!(!drawn(cx, "checkout-folder-use-default"));
    assert!(painted(cx).contains("Default: ~/def"), "{}", painted(cx));
}

/// Why this matters: the old dialog locked the folder field while any other write to the host ran; this port keeps
/// the field live and leaves the ordering to the operation store's one request per key instead (the module docs'
/// NOTE). That is only sound if the key really does hold the folder's Save back, and the folder's save holds the other
/// controls back, or two writes to one host could cross. Specified: with a permission's save in flight, the field takes
/// typing, and its Save sends nothing and still reads "Save"; once that save is answered, Save sends the folder; with
/// the folder's save in flight, a permission's switch sends nothing and keeps showing the stored value; once the save is
/// done and its read-back answered, the key is free and the field says "Checkout folder saved".
#[gpui::test]
fn saves_on_the_host_wait_for_each_other_without_locking_the_folder(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_build_box(cx);
    let yolo = "host-yolo-without-asking";
    click(cx, yolo);
    let (id, _, _) = one_command(&mut helm);
    assert_eq!(host_work(cx), Some(Work::Save), "the premise: in flight");

    type_folder(cx, "/srv/box");
    assert_eq!(folder_text(cx), "/srv/box", "the field took typing");
    click(cx, "checkout-folder-save");
    assert!(helm.commands().is_empty(), "the key held the folder back");
    assert_eq!(label(cx, "checkout-folder-save").as_deref(), Some("Save"));

    push_build_box(&mut helm, |host| host.yolo_without_asking = true, cx);
    helm.reply(id, 200, None);
    settle(cx);
    click(cx, "checkout-folder-save");
    let (id, body) = one_folder_command(&mut helm, Method::Put);
    assert_eq!(body, json!({ "root": "/srv/box" }));

    click(cx, yolo);
    assert!(helm.commands().is_empty(), "the key held the switch back");
    assert_eq!(on(cx, yolo), Some(true), "the stored value shows");
    helm.reply(id, 204, None);
    settle(cx);
    // The done save reads the folder back; answered, so the test leaves nothing in flight.
    let (id, _) = one_folder_command(&mut helm, Method::Get);
    helm.reply(
        id,
        200,
        Some(json!({ "root": "/srv/box", "default": null })),
    );
    settle(cx);
    assert_eq!(host_work(cx), None, "the key is free again");
    assert_eq!(
        label(cx, "checkout-folder-saved").as_deref(),
        Some("Checkout folder saved")
    );
}
