//! Tests of a host's actions menu as the Hosts panel opens it: what it offers on a remote host and on the helm's own
//! machine, what its greyed Update does (nothing), Settings and Remove waiting for the host's own work, which hosts
//! offer Remove, and where focus goes.
//!
//! The panel runs over the helm client's fake with the reference screens' three hosts (the local host as 1,
//! `build-box` as 2, `gpu-1` as 3), and the menu is opened by clicking a row's toggle. Every step ends in one of the
//! shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::{ApiPath, Method};
use gpui::{DriverNode, Role, TestAppContext, VisualTestContext, px, size};

use crate::hosts::tests::{
    connected, host, hosts, open_panel, open_panel_in_column, reference_hosts,
};
use crate::layers;
use crate::operations::{Command, OpKey, Work};
use crate::session;
use crate::test_support::{self, bounds, center, click, focused_id, frame, keys};

/// The panel over the reference hosts in a window as large as the reference screens', with nothing open.
fn open_hosts(
    cx: &mut TestAppContext,
) -> (farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext) {
    let (helm, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    test_support::settle(cx);
    (helm, cx)
}

/// The node drawn under `id`, if any.
fn node(nodes: &[DriverNode], id: &str) -> Option<DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .cloned()
}

/// The accessible name of what was drawn under `id`; fails if nothing was.
fn label(nodes: &[DriverNode], id: &str) -> String {
    match node(nodes, id).and_then(|node| node.label) {
        Some(label) => label.to_string(),
        None => panic!("{id} was not drawn with a name"),
    }
}

/// Every line of text the window painted, joined with single spaces.
fn painted(nodes: &[DriverNode]) -> String {
    let lines: Vec<String> = nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|line| line.text.to_string()))
        .collect();
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn any_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// Why this matters: the menu is how a user reaches a host's settings and removes it, and what it lists must be what the
/// offers rule says for that host, so a remote host shows no command this build cannot carry out yet. Its header names
/// the host and where it is, and focus starts on its first command, as the old menu's did. Specified: a click on
/// `build-box`'s toggle opens the menu beside it, named "Host actions for build-box", its header reading
/// "dev@build-box · Farhelm 0.0.0-unreleased", with Settings holding focus, then Remove ("Forget this host"), and no
/// Update; Escape closes it with focus on the toggle. (That Remove acts is `hosts::remove`'s tests'.)
#[gpui::test]
fn a_remote_hosts_toggle_opens_a_menu_offering_settings_and_remove(cx: &mut TestAppContext) {
    let (_helm, cx) = open_hosts(cx);
    assert_eq!(
        label(&frame(cx), "host-menu-toggle-2"),
        "Host actions for build-box"
    );
    click(cx, "host-menu-toggle-2");
    let nodes = frame(cx);
    assert!(node(&nodes, "host-menu").is_some(), "the menu opened");
    assert_eq!(
        label(&nodes, "host-menu-items"),
        "Host actions for build-box"
    );
    assert_eq!(
        label(&nodes, "host-menu-summary-2"),
        "dev@build-box · Farhelm 0.0.0-unreleased"
    );
    let settings = node(&nodes, "host-menu-settings").expect("Settings is offered");
    assert_eq!(
        (settings.role, settings.label.map(|label| label.to_string())),
        (Some(Role::MenuItem), Some("Settings".to_owned()))
    );
    let remove = node(&nodes, "host-menu-remove").expect("Remove is offered");
    assert_eq!(
        (remove.role, remove.label.map(|label| label.to_string())),
        (Some(Role::MenuItem), Some("Remove".to_owned()))
    );
    assert!(painted(&nodes).contains("Forget this host"));
    assert!(
        bounds(cx, "host-menu-remove").origin.y > bounds(cx, "host-menu-settings").origin.y,
        "Remove comes after Settings"
    );
    assert!(node(&nodes, "host-menu-update").is_none(), "no Update yet");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-settings"));

    keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed it");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
}

/// Why this matters: the toggle is a disclosure control, so a second press on it while its menu is open must close the
/// menu, as the old toggle's did, rather than leave it open or close and reopen it in one click. The panel sits in a
/// sidebar-wide column, as in the app: filling a wide window, it would put the toggle at the window's edge, where the
/// menu, kept inside the window, covers it and takes the second press. Specified: a click on `build-box`'s toggle opens
/// its menu, beside and clear of the toggle; a second click on the toggle closes it.
#[gpui::test]
fn a_second_press_on_the_toggle_closes_its_menu(cx: &mut TestAppContext) {
    let (_helm, _, cx) = open_panel_in_column(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    test_support::settle(cx);
    click(cx, "host-menu-toggle-2");
    assert!(any_open(cx), "the premise: the first press opened the menu");
    let (toggle, menu) = (center(cx, "host-menu-toggle-2"), bounds(cx, "host-menu"));
    assert!(
        !menu.contains(&toggle),
        "the premise: the menu {menu:?} leaves the toggle's center {toggle:?} clear"
    );
    click(cx, "host-menu-toggle-2");
    assert!(!any_open(cx), "the second press closed it");
}

/// Why this matters: SPEC.md, "Opening and closing": closing something the user opened returns focus to where it was
/// before it opened, on every platform, including those where clicking the button that opened it did not move focus to
/// the button. On macOS a click leaves the toggle unfocused, so the menu must still give focus back to it, which it does
/// by naming the toggle as its return target rather than focusing it before opening. Specified: under macOS's click
/// rule, with focus away from `build-box`'s toggle, a click on the toggle opens its menu and Escape closes it with
/// focus on the toggle.
#[gpui::test]
fn escape_on_the_menu_returns_focus_to_its_toggle_under_the_macos_click_rule(
    cx: &mut TestAppContext,
) {
    let (_helm, _, cx) = open_panel_in_column(cx, vec![hosts(&reference_hosts())]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::settle(cx);
    assert_ne!(
        focused_id(cx).as_deref(),
        Some("host-menu-toggle-2"),
        "the premise: the toggle is unfocused before the click"
    );
    click(cx, "host-menu-toggle-2");
    assert!(any_open(cx), "the premise: the menu opened");
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
}

/// Why this matters: the helm refuses to update its own machine, which is updated by running the installer again, so
/// the local row's Update is shown greyed with that reason and must never send anything (`provisioning.spec.ts`, "the
/// local row's update is greyed out, points at the installer, and sends nothing"). Specified: the local host's menu
/// reads "Local host · Farhelm ..." and offers Update, then Settings; focus starts on Update; its line says "Run the
/// installer again to update this machine"; clicking it and pressing Enter on it send nothing and leave the menu open.
#[gpui::test]
fn the_local_hosts_greyed_update_says_why_and_sends_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx);
    click(cx, "host-menu-toggle-1");
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-menu-summary-1"),
        "Local host · Farhelm 0.0.0-unreleased"
    );
    assert!(node(&nodes, "host-menu-update").is_some());
    assert!(node(&nodes, "host-menu-settings").is_some());
    assert!(
        node(&nodes, "host-menu-remove").is_none(),
        "the helm's own machine is never removed"
    );
    assert!(painted(&nodes).contains("Run the installer again to update this machine"));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-update"));

    click(cx, "host-menu-update");
    keys(cx, "enter");
    assert!(helm.commands().is_empty(), "the greyed Update sent nothing");
    assert!(any_open(cx), "and the menu stayed open");
}

/// Why this matters: a host's settings must not change under its own work in flight (an Update's plan, a save), as the
/// old menu held Settings unavailable then (`provisioning.spec.ts`, "Update planning keeps only its own Settings
/// unavailable"), and the old menu held Remove unavailable while the host was busy too; another host's work must not
/// hold them. Specified: while a save is in flight on `build-box`, its menu's Remove and Settings each do nothing when
/// clicked (no question, no dialog, nothing sent), the menu staying; `gpu-1`'s Settings opens its dialog meanwhile;
/// once the save is answered, `build-box`'s Settings opens its dialog.
#[gpui::test]
fn settings_waits_for_the_hosts_own_work_in_flight(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_hosts(cx);
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("a UI session");
        let command = Command::new(
            Work::Save,
            Method::Post,
            ApiPath::api(&["hosts", "2", "alias"]).unwrap(),
            Some(serde_json::json!({ "alias": "x" })),
            "POST /api/hosts/[host]/alias",
        );
        operations.update(cx, |operations, cx| {
            operations
                .start(&OpKey::host("2"), command, cx, |_, _| {})
                .unwrap();
        });
    });
    test_support::settle(cx);
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "the premise: work on build-box is in flight");

    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-remove");
    assert!(node(&frame(cx), "prompt").is_none(), "no question");
    assert!(helm.commands().is_empty(), "nothing sent");
    click(cx, "host-menu-settings");
    assert!(node(&frame(cx), "host-settings").is_none(), "no dialog");
    assert!(node(&frame(cx), "host-menu").is_some(), "the menu stayed");
    keys(cx, "escape");

    click(cx, "host-menu-toggle-3");
    click(cx, "host-menu-settings");
    assert!(
        node(&frame(cx), "host-settings").is_some(),
        "gpu-1's opened"
    );
    keys(cx, "escape");

    helm.reply(sent[0].id, 200, None);
    test_support::settle(cx);
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-settings");
    assert!(
        node(&frame(cx), "host-settings").is_some(),
        "build-box's opened"
    );
}

/// Why this matters: removing a Tensorlake sandbox's host deletes the sandbox, its files and its sessions (SPEC.md,
/// "Topology": the panel calls that Delete sandbox, and it always asks); that command is not ported yet, so the menu must
/// not offer the ordinary Remove in its place, whose question promises the host keeps running. Specified: a connected
/// Tensorlake host's menu offers Settings and no Remove.
#[gpui::test]
fn a_sandbox_hosts_menu_offers_no_remove(cx: &mut TestAppContext) {
    let mut listed = reference_hosts();
    listed.push(host(4, "tensorlake", connected()));
    let (_helm, _, cx) = open_panel_in_column(cx, vec![hosts(&listed)]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    test_support::settle(cx);
    click(cx, "host-menu-toggle-4");
    let nodes = frame(cx);
    assert!(
        node(&nodes, "host-menu-settings").is_some(),
        "the premise: its menu opened"
    );
    assert!(node(&nodes, "host-menu-remove").is_none());
}
