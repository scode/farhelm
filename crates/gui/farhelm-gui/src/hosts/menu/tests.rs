//! Tests of a host's actions menu as the Hosts panel opens it: what it offers on a remote host and on the helm's own
//! machine, what its greyed Update does (nothing) and what that Update does in the installed desktop app (asks the
//! app's updater to check), the header of a host that reports no build, Settings and Remove
//! waiting for the host's own work, which hosts offer Remove, where focus goes, also when Down or Up on the closed
//! toggle opens the menu, and an open menu following its row through what the helm pushes until its host goes.
//!
//! The panel runs over the helm client's fake, most tests with the reference screens' three hosts (the local host as
//! 1, `build-box` as 2, `gpu-1` as 3), and the menu is opened by clicking a row's toggle. Every step ends in one of
//! the shared harness's step helpers, so the focus and layer invariants are checked after each.
//!
//! The last section runs the real main view instead ([`main_view`]), for what the menu does beside the session list
//! and the host rail: Add host and the rail's Settings gear opened over an open menu, and the dialogs an avatar's
//! menu opens.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_input_mapping::Platform;
use farhelm_proto::provisioning::{
    PlanStepPath, ProvisioningOperation, ProvisioningView, RunStatus, StepStatus,
};
use farhelm_proto::ui_channel::{Availability, Collection, CollectionChanges, HostView};
use gpui::{DriverNode, Role, TestAppContext, VisualTestContext, px, size};

use crate::actions::THIS_PLATFORM;
use crate::hosts::progress::tests::step;
use crate::hosts::tests::{
    aliased, connected, connected_with, connecting, host, hosts, local_down, open_panel,
    open_panel_in_column, progress, reference_hosts, run, start_work,
};
use crate::operations::{Command, OpKey, Work};
use crate::session;
use crate::test_support::{self, any_open, bounds, center, click, focused_id, frame, keys};

/// The greyed Update's line on this build's platform, written out rather than read from
/// [`crate::hosts::offers::Update::description`], so a menu that drew another platform's words fails on whichever
/// platform runs the test.
fn greyed_reason() -> &'static str {
    match THIS_PLATFORM {
        Platform::Mac => "Run the installer again to update this machine",
        Platform::Linux => "Install a newer Farhelm on this machine to update it",
    }
}

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

/// Why this matters: the menu is how a user reaches a host's settings, updates it and removes it, and what it lists
/// must be what the offers rule says for that host, so a remote host shows no command this build cannot carry out yet.
/// Its header names the host and where it is, and focus starts on its first command, Retry, as the old menu's did.
/// Specified: a click on `build-box`'s toggle opens the menu beside it, named "Host actions for build-box", its header
/// reading "dev@build-box · Farhelm 0.0.0-unreleased", offering Retry, which holds focus, Update, then Settings, then
/// Uninstall ("Remove Farhelm from this host, keeping its data") and Remove ("Forget this host"); Escape closes it with
/// focus on the toggle. (That Update and Remove act is `hosts::update`'s and
/// `hosts::remove`'s tests'.)
#[gpui::test]
fn a_remote_hosts_toggle_opens_a_menu_starting_on_retry(cx: &mut TestAppContext) {
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
    let uninstall = node(&nodes, "host-menu-uninstall").expect("Uninstall is offered");
    assert_eq!(
        (
            uninstall.role,
            uninstall.label.map(|label| label.to_string())
        ),
        (Some(Role::MenuItem), Some("Uninstall".to_owned()))
    );
    assert!(painted(&nodes).contains("Remove Farhelm from this host, keeping its data"));
    assert!(
        bounds(cx, "host-menu-uninstall").origin.y > bounds(cx, "host-menu-settings").origin.y
            && bounds(cx, "host-menu-remove").origin.y > bounds(cx, "host-menu-uninstall").origin.y,
        "Uninstall and then Remove come after Settings"
    );
    let update = node(&nodes, "host-menu-update").expect("Update is offered");
    assert_eq!(
        (update.role, update.label.map(|label| label.to_string())),
        (Some(Role::MenuItem), Some("Update".to_owned()))
    );
    assert!(
        bounds(cx, "host-menu-update").origin.y < bounds(cx, "host-menu-settings").origin.y,
        "Update comes before Settings"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));

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
/// the button. On macOS a click leaves the toggle unfocused, so the menu must still give focus back to it, which it
/// does by naming the toggle as its return target rather than focusing it before opening. Specified: under macOS's
/// click rule, with focus away from `build-box`'s toggle, a click on the toggle opens its menu and Escape closes it
/// with focus on the toggle.
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
/// local row's update is greyed out, points at the installer, and sends nothing"). Focus must not start on it: a greyed
/// command draws no highlight, so a menu opening there showed no mark of where Enter would go (the old menu opened on
/// Retry, ahead of it, as this one does). Specified: the local host's menu reads "Local host · Farhelm ..." and offers
/// Retry, Update, then Settings, and neither Remove nor Uninstall; focus starts on Retry, and Down reaches the greyed
/// Update, as the old menu's arrows reached it; its line gives this platform's way to update the machine, "Run the
/// installer again to update this machine" on macOS and "Install a newer Farhelm on this machine to update it" on
/// Linux; clicking it and pressing Enter on it send nothing and leave the menu open.
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
    assert!(
        node(&nodes, "host-menu-uninstall").is_none(),
        "nor uninstalled from the panel: `farhelm uninstall` there does it"
    );
    assert!(painted(&nodes).contains(greyed_reason()));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));
    keys(cx, "down");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-update"));

    click(cx, "host-menu-update");
    keys(cx, "enter");
    assert!(helm.commands().is_empty(), "the greyed Update sent nothing");
    assert!(any_open(cx), "and the menu stayed open");
}

/// Why this matters: in the installed desktop app the local row's Update is the app's own update check (SPEC.md,
/// "Session list": "there the local row always offers Update, and choosing it checks for a newer release and installs
/// it"), one of the on-demand routes "Installation and updates" lists beside the version readout and the ? menu. It
/// must reach the updater and never the helm, which refuses to update its own machine. Specified: with a fake updater
/// kept as the app's, the local host's menu shows "Check for a newer Farhelm and install it" in place of the installer
/// line, and Down from Retry, where focus starts, reaches Update; Enter on it asks the updater for one check, sends
/// the helm nothing, and closes the menu with focus back on its toggle.
#[gpui::test]
fn in_the_desktop_app_the_local_hosts_update_asks_the_updater_to_check(cx: &mut TestAppContext) {
    use crate::desktop::updates::fake::{Feed, Seen, fake_host};
    let (mut helm, cx) = open_hosts(cx);
    let (feed, seen) = (
        std::rc::Rc::new(Feed::default()),
        std::rc::Rc::new(Seen::default()),
    );
    cx.update(|_, cx| crate::desktop::updates::keep(Some(fake_host(&feed, &seen, false)), cx));
    click(cx, "host-menu-toggle-1");
    let nodes = frame(cx);
    assert!(node(&nodes, "host-menu-update").is_some());
    let text = painted(&nodes);
    assert!(text.contains("Check for a newer Farhelm and install it"));
    assert!(!text.contains(greyed_reason()));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));
    keys(cx, "down");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-update"));

    keys(cx, "enter");
    assert_eq!(seen.checks.get(), 1, "the updater was asked to check once");
    assert!(helm.commands().is_empty(), "the helm was sent nothing");
    assert!(!any_open(cx), "the menu closed");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-1"));
}

/// Why this matters: the header's second line says which Farhelm a host runs, and only a build the host reports is one.
/// The helm also pushes where Farhelm is installed on the host, a path, which the old header showed in the build's
/// place when the host reported none, so it read as a version. Specified: a host on its second connection attempt, with
/// an install path recorded, has its header read "dev@build-box.invalid · Farhelm not connected".
#[gpui::test]
fn a_host_that_reports_no_build_reads_not_connected(cx: &mut TestAppContext) {
    let mut down = host(2, "ssh", connecting(2, Some("connection refused")));
    down.remote_farhelm = Some("~/.local/bin/farhelm".to_owned());
    let (_helm, _, cx) = open_panel(cx, vec![hosts(&[host(1, "local", connected()), down])]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    test_support::settle(cx);
    click(cx, "host-menu-toggle-2");
    assert_eq!(
        label(&frame(cx), "host-menu-summary-2"),
        "dev@build-box.invalid · Farhelm not connected"
    );
}

/// Why this matters: a host's settings must not change under its own work in flight (an Update's plan, a save), as the
/// old menu held Settings unavailable then (`provisioning.spec.ts`, "Update planning keeps only its own Settings
/// unavailable"), and the old menu held Remove unavailable while the host was busy too; another host's work must not
/// hold them. Specified: while a save is in flight on `build-box`, its menu opens on Retry, and its Remove and Settings
/// each do nothing when clicked (no question, no dialog, nothing sent), the menu staying; `gpu-1`'s Settings opens its
/// dialog meanwhile; once the save is answered, `build-box`'s Settings opens its dialog.
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
    // Every item is unavailable while the host is busy, so the menu falls back to the first one it draws.
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));
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
/// "Topology": the panel calls that Delete sandbox, and it always asks), so the menu must not offer the ordinary Remove
/// beside or instead of it, whose question promises the host keeps running. Specified: a connected Tensorlake host's
/// menu offers Settings and Delete sandbox, and neither Remove nor Uninstall.
#[gpui::test]
fn a_sandbox_hosts_menu_offers_delete_sandbox_and_no_remove(cx: &mut TestAppContext) {
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
    assert!(node(&nodes, "host-menu-delete").is_some());
    assert!(node(&nodes, "host-menu-remove").is_none());
    assert!(
        node(&nodes, "host-menu-uninstall").is_none(),
        "SPEC.md: the panel \"offers no Uninstall for these hosts\""
    );
}

/// Why this matters: the old toggle opened its menu from the keyboard with Down on the first command and Up on the
/// last, as every menu toggle in the app does (`components::menu_keys`), so a keyboard user reaches a host's Remove or
/// Delete in one key rather than walking past every other command. The last command differs by host kind, which is
/// why both are checked. Specified: with focus on the closed toggle of `build-box` (a remote host), Down opens its
/// menu with focus on Retry, and after Escape, Up opens it with focus on Remove; on a connected Tensorlake host, Up
/// opens it on Delete sandbox. Escape returns focus to the toggle each time.
#[gpui::test]
fn down_and_up_on_a_closed_toggle_open_the_menu_on_its_first_or_last_command(
    cx: &mut TestAppContext,
) {
    let mut listed = reference_hosts();
    listed.push(host(4, "tensorlake", connected()));
    let (_helm, _, cx) = open_panel_in_column(cx, vec![hosts(&listed)]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    test_support::settle(cx);
    for (toggle, last) in [
        ("host-menu-toggle-2", "host-menu-remove"),
        ("host-menu-toggle-4", "host-menu-delete"),
    ] {
        // A click and Escape leave focus on the closed toggle on every platform, since Escape returns it there.
        click(cx, toggle);
        keys(cx, "escape");
        assert!(!any_open(cx), "the premise: the menu is closed");
        assert_eq!(focused_id(cx).as_deref(), Some(toggle), "the premise");

        keys(cx, "down");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("host-menu-retry"),
            "Down on {toggle}"
        );
        keys(cx, "escape");
        assert_eq!(focused_id(cx).as_deref(), Some(toggle));

        keys(cx, "up");
        assert!(any_open(cx), "Up on {toggle} opened the menu");
        assert_eq!(focused_id(cx).as_deref(), Some(last), "Up on {toggle}");
        keys(cx, "escape");
        assert_eq!(focused_id(cx).as_deref(), Some(toggle));
    }
}

// ---- The menu through changes underneath ---------------------------------------------------------------------------

/// The remote hosts of [`an_open_menu_follows_its_row_and_closes_when_its_host_goes`], by id: aliased `a`, `b` and
/// `c`, so no two share a destination.
const A: i64 = 2;
const B: i64 = 3;
const C: i64 = 4;

/// A build long enough that its details line wraps over several lines of the panel's column; every other remote host
/// reports `0.16.0`.
const LONG_BUILD: &str = "balanced-detail-sentinel visible detail words visible detail words visible \
                          detail words visible detail words visible detail words";

/// A step message, and a local dial failure, that wrap likewise.
const LONG_MESSAGE: &str = "/opt/fixture/retained-payload-directory/retained-payload-directory/retained-payload-\
                            directory/retained-payload-directory/farhelm already has the requested payload";

/// Host `id`'s toggle, as the step helpers name it.
fn toggle(id: i64) -> &'static str {
    [
        "host-menu-toggle-2",
        "host-menu-toggle-3",
        "host-menu-toggle-4",
    ][(id - A) as usize]
}

/// What the registry holds: the remote hosts' order, which of them reports [`LONG_BUILD`], and the local host's dial
/// failure while its supervisor is down; with the save [`Push::Save`] sent, for [`Push::Refuse`] to answer.
struct Registry {
    order: Vec<i64>,
    long: i64,
    local: Option<&'static str>,
    saved: Option<u64>,
}

impl Registry {
    /// The hosts as the helm pushes them: the local host first, then the remote hosts in order.
    fn listed(&self) -> Vec<HostView> {
        let local = match self.local {
            Some(failure) => host(1, "local", local_down(failure)),
            None => host(1, "local", connected()),
        };
        let remote = self.order.iter().map(|&id| {
            let build = if id == self.long {
                LONG_BUILD
            } else {
                "0.16.0"
            };
            let alias = ["a", "b", "c"][(id - A) as usize];
            let view = host(id, "ssh", connected_with(build, "id-1", 0));
            aliased(view, alias, &format!("dev@{alias}.invalid"))
        });
        std::iter::once(local).chain(remote).collect()
    }
}

/// What one case changes, before it opens the menu or while the menu is open.
enum Push {
    /// The registry with the remote hosts in this order.
    Order([i64; 3]),
    /// The registry with this host's build the long one.
    LongBuild(i64),
    /// The registry with the local host's supervisor down, its dial failure reading this.
    LocalFailure(&'static str),
    /// A registry read that failed: the rows stay, under a line that says so.
    ReadFails,
    /// This host's retained Update run.
    Run(ProvisioningView),
    /// A save under this host's key, as the settings dialog's, left in flight, as when the dialog was closed first.
    Save(i64),
    /// The helm's refusal of that save, which shows under `b`'s row.
    Refuse,
    /// The registry without this host.
    Gone(i64),
}

/// An Update of host `id`, its upload step `upload` with `message`, its install step `install`, and the run done once
/// the install is.
fn update(
    id: i64,
    upload: StepStatus,
    install: StepStatus,
    message: Option<&str>,
) -> ProvisioningView {
    let status = match install {
        StepStatus::Completed => RunStatus::Completed,
        _ => RunStatus::Running,
    };
    let mut view = run(id, ProvisioningOperation::Update, status);
    let mut uploaded = step("Upload the farhelm binary", upload);
    uploaded.message = message.map(str::to_owned);
    view.steps = vec![
        step("Create directories", StepStatus::Completed),
        uploaded,
        step("Install the farhelm binary", install),
    ];
    view
}

/// Make `push` happen, settle, and check that the panel draws it: the rows in the registry's order, and the line the
/// change adds or grows, if any.
fn apply(push: Push, now: &mut Registry, helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    let (mut registry, mut failed, mut remove) = (true, false, Vec::new());
    // The line the change draws, by id, and words it must hold.
    let mut shows: Option<(String, String)> = None;
    match push {
        Push::Order(order) => now.order = order.to_vec(),
        Push::LongBuild(id) => {
            (now.long, shows) = (id, Some((format!("host-detail-{id}-1"), LONG_BUILD.into())))
        }
        Push::LocalFailure(failure) => {
            (now.local, shows) = (
                Some(failure),
                Some(("host-detail-1-0".into(), failure.into())),
            )
        }
        Push::ReadFails => {
            (failed, shows) = (true, Some(("hosts-load-error".into(), "locked".into())))
        }
        Push::Gone(id) => {
            now.order.retain(|&listed| listed != id);
            remove.push(HostView::key(id));
        }
        Push::Run(view) => {
            let (id, step) = (view.host_id, &view.steps[1]);
            // Without a message, the step's own name: the result words of the statuses these runs give it there,
            // Running and Completed, are their names (`progress.rs`, `look`).
            let drawn = match &step.message {
                Some(message) => (format!("host-step-{id}-1-message"), message.clone()),
                None => (format!("host-step-{id}-1"), format!("{:?}", step.status)),
            };
            helm.push(CollectionChanges {
                collection: Collection::HostProgress,
                // Both hosts that ever get a run here, as the helm orders every run it holds.
                meta: test_support::meta(&["3", "4"]),
                upsert: progress(&[view]).entries,
                remove: Vec::new(),
            });
            (registry, shows) = (false, Some(drawn));
        }
        Push::Save(id) => {
            now.saved = Some(start_work(
                helm,
                cx,
                Work::Save,
                OpKey::host(&HostView::key(id)),
            ));
            registry = false;
        }
        Push::Refuse => {
            let refusal = serde_json::json!("another host already has this alias");
            helm.reply(
                now.saved.take().expect("the premise: a save is out"),
                409,
                Some(refusal),
            );
            (registry, shows) = (
                false,
                Some((format!("host-error-{B}"), "has this alias".into())),
            );
        }
    }
    if registry {
        let mut state = hosts(&now.listed());
        if failed {
            state.meta.availability = Availability::Unavailable {
                error: "locked".into(),
            };
            state.entries.clear();
        }
        let (meta, upsert) = (state.meta, state.entries);
        helm.push(CollectionChanges {
            collection: Collection::Hosts,
            meta,
            upsert,
            remove,
        });
    }
    test_support::settle(cx);
    let nodes = frame(cx);
    let rows = nodes.iter().filter_map(|node| {
        node.element_id
            .as_deref()?
            .strip_prefix("host-row-")?
            .parse()
            .ok()
    });
    let drawn: Vec<i64> = std::iter::once(1)
        .chain(now.order.iter().copied())
        .collect();
    assert_eq!(
        rows.collect::<Vec<i64>>(),
        drawn,
        "the premise: the registry's rows"
    );
    if let Some((id, words)) = shows {
        assert!(
            label(&nodes, &id).contains(&words),
            "the premise: {id} draws the change"
        );
    }
}

/// Why this matters: SPEC.md, "Things changing underneath": "a focused row, or its open menu, moves with its session",
/// and nothing closes a menu it did not open; a host's menu likewise goes only when its host does. The old app closed
/// an open host menu whenever its toggle moved (`popover-host-movement.spec.ts` and
/// `popover-expanded-host-movement.spec.ts` at reference commit `24bc0580`, inverted by that rule); each of their host
/// menu cases is one case here. Run under Linux's click rule, where the click that opens a menu focuses its toggle, so
/// in the last case the menu's return target and what held focus before it opened go with the row together.
/// Specified: in a tall window, with Details on and the remote hosts a, b and c after the local host (c with the long
/// build), each case makes its premise, opens its host's menu with a click, and makes its change, which the panel
/// draws; the menu stays open on Retry with the same commands and moves exactly as far as its toggle, down where the
/// case says so and not at all otherwise. In the last case the registry stops listing the host, and its menu closes
/// with focus on something still drawn.
#[gpui::test]
fn an_open_menu_follows_its_row_and_closes_when_its_host_goes(cx: &mut TestAppContext) {
    use Push::*;
    use StepStatus::{Completed, Pending, Running, Skipped};
    use std::cmp::Ordering::{self, Equal, Greater};

    let mut now = Registry {
        order: vec![A, B, C],
        long: C,
        local: None,
        saved: None,
    };
    let (mut helm, _, cx) = open_panel_in_column(cx, vec![hosts(&now.listed()), progress(&[])]);
    cx.simulate_resize(size(px(1600.), px(1400.)));
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(true, cx));
    click(cx, "host-details-toggle");

    let running = |id| Run(update(id, Running, Pending, None));
    let grown = |id| Run(update(id, Skipped, Running, Some(LONG_MESSAGE)));
    let finished = || update(B, Skipped, Completed, Some("already current"));
    // Only what the row does not draw changes: the run's id, a step's id, its paths and its explanation.
    let mut renamed = finished();
    renamed.run_id = Some("run-3-again".into());
    renamed.steps[0].step = "create-directories-new-metadata".into();
    renamed.steps[0].paths = vec![PlanStepPath {
        full: "/opt/fixture/full".into(),
        short: "~/full".into(),
    }];
    renamed.steps[0].explanation = Some("drawn on the progress card alone".into());
    let (down, still) = (Some(Greater), Some(Equal));
    // (the case, what makes its premise, whose menu, the change while it is open, and `None` where the menu closes).
    // One case per line, so the table reads down its columns.
    type Case = (&'static str, Vec<Push>, i64, Push, Option<Ordering>);
    #[rustfmt::skip]
    let cases: Vec<Case> = vec![
        ("a reorder moves b down", vec![], B, Order([A, C, B]), down),
        ("a reorder below a", vec![], A, Order([A, B, C]), still),
        ("a permutation above c", vec![], C, Order([B, A, C]), still),
        ("b's detail above a grows, c's below shrinks", vec![], A, LongBuild(B), down),
        ("the details above c trade lengths", vec![], C, LongBuild(A), still),
        ("the details below b trade lengths", vec![], B, LongBuild(C), still),
        ("b's Update above a grows a step message", vec![running(B)], A, grown(B), down),
        ("c's Update below a moves on a step", vec![running(C)], A, Run(update(C, Completed, Running, None)), still),
        ("the local host's dial failure grows", vec![LocalFailure("refused")], A, LocalFailure(LONG_MESSAGE), down),
        ("b's Update grows after a failed registry read", vec![ReadFails, running(B)], A, grown(B), down),
        ("b's finished run changes only undrawn fields", vec![Run(finished())], A, Run(renamed), still),
        ("a late refusal of b's save shows under b", vec![Save(B)], A, Refuse, down),
        ("the registry stops listing c", vec![], C, Gone(C), None),
    ];
    let at = |nodes: &[DriverNode], id: &str| node(nodes, id).map(|node| node.bounds.top());
    let offered = |nodes: &[DriverNode]| {
        nodes
            .iter()
            .filter(|node| node.role == Some(Role::MenuItem))
            .filter_map(|node| node.element_id.clone())
            .collect::<Vec<_>>()
    };
    for (case, before, target, push, way) in cases {
        for premise in before {
            apply(premise, &mut now, &mut helm, cx);
        }
        click(cx, toggle(target));
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("host-menu-retry"),
            "{case}: the premise"
        );
        let nodes = frame(cx);
        let (button, menu, commands) = (
            at(&nodes, toggle(target)),
            at(&nodes, "host-menu"),
            offered(&nodes),
        );

        apply(push, &mut now, &mut helm, cx);
        let nodes = frame(cx);
        let Some(way) = way else {
            assert!(!any_open(cx), "{case}: the menu closed");
            assert!(
                cx.update(|window, cx| window.focused(cx).is_some()),
                "{case}: focus stayed"
            );
            continue;
        };
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("host-menu-retry"),
            "{case}: still open"
        );
        assert_eq!(offered(&nodes), commands, "{case}: the same commands");
        let moved = at(&nodes, toggle(target)).unwrap() - button.unwrap();
        let followed = at(&nodes, "host-menu").unwrap() - menu.unwrap();
        assert!(
            (followed - moved).abs() < px(1.),
            "{case}: {followed:?} for {moved:?}"
        );
        assert_eq!(
            moved.round().partial_cmp(&px(0.)),
            Some(way),
            "{case}: {moved:?}"
        );
        // Closed for the next case, which may click this toggle again.
        keys(cx, "escape");
    }
}

// ---- The menu in the real main view --------------------------------------------------------------------------------

/// The real main view over the reference hosts, `build-box` (2) with a failed setup run, so it offers Re-run; one
/// session, `a`, open; the Welcome card seen; in a 1200 by 700 window with every command sent at startup answered. The
/// tests here need what only the main view draws together: the Hosts panel or the session list, beside the host rail
/// and its foot.
fn main_view(
    cx: &mut TestAppContext,
) -> (farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext) {
    use crate::hosts::tests::{progress, run};
    use crate::session_list::host_rail::tests::answer_everything;
    use crate::session_list::tests::{listing, open_main, preferences_state, session};
    use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus};
    let snapshot = vec![
        listing(&["a"], &[session("a")]),
        preferences_state(farhelm_proto::ui_channel::Preferences {
            visit_history: vec!["a".to_owned()],
            welcome_seen: Some(true),
            ..Default::default()
        }),
        hosts(&reference_hosts()),
        progress(&[run(2, ProvisioningOperation::Add, RunStatus::Failed)]),
    ];
    let (mut fake, cx) = open_main(cx, snapshot);
    cx.simulate_resize(size(px(1200.), px(700.)));
    answer_everything(&mut fake);
    test_support::settle(cx);
    (fake, cx)
}

/// Answer the one command the window sent to `path` since the last look with `status` and `body`, and any other it
/// sent meanwhile (the list's own bookkeeping) as done; fails if none went to `path`. It checks nothing about those
/// others, so a stray request among them goes unnoticed: a test that must show something was not sent looks itself.
fn answer(
    fake: &mut farhelm_helm_client::fake::FakeHelm,
    path: &str,
    status: u16,
    body: Option<serde_json::Value>,
) {
    let mut found = false;
    for command in fake.commands() {
        if command.path == path && !found {
            fake.reply(command.id, status, body.clone());
            found = true;
        } else {
            fake.reply(command.id, 200, None);
        }
    }
    assert!(found, "a command went to {path}");
}

/// Open a host's menu from its rail avatar, drawn under `avatar` (`host-rail-host-<id>`), with a right press, as a
/// secondary click delivers it, and check that it opened.
fn open_from_avatar(cx: &mut VisualTestContext, avatar: &'static str) {
    let avatar = bounds(cx, avatar);
    cx.simulate_mouse_down(
        avatar.center(),
        gpui::MouseButton::Right,
        gpui::Modifiers::none(),
    );
    cx.simulate_mouse_up(
        avatar.center(),
        gpui::MouseButton::Right,
        gpui::Modifiers::none(),
    );
    test_support::settle(cx);
    assert!(
        node(&frame(cx), "host-menu").is_some(),
        "the premise: the avatar opened its host's menu"
    );
}

/// Whether the Hosts panel shows in the list's place; fails unless exactly one of the panel and the session list is
/// drawn, so `!hosts_shown` also says the list is.
fn hosts_shown(cx: &mut VisualTestContext) -> bool {
    let nodes = frame(cx);
    let panel = node(&nodes, "hosts-panel").is_some();
    assert_ne!(
        panel,
        node(&nodes, "session-list").is_some(),
        "exactly one of the panel and the list is drawn"
    );
    panel
}

/// Why this matters: the host menu is a popover with no veil, so the page around it stays live, and the gear on the
/// host rail's foot must still open Settings in one press while a row's menu is open; a menu left up under Settings, or
/// keeping focus from it, would leave the user with two layers to dismiss and focus outside the dialog they opened. The
/// old spec `settings.spec.ts`, "settings dismisses a row menu, isolates the page and returns focus on Escape", opened
/// Settings over an open Hosts row menu and pinned that the menu went, focus started on Settings' first switch, and
/// Escape returned it to the gear.
/// Specified: with Hosts shown and `build-box`'s row menu open from its toggle, focus on Retry, a click on the gear
/// closes the menu and opens Settings with focus on Skip session confirmations; Escape closes Settings with focus on
/// the gear and nothing open.
#[gpui::test]
fn the_gear_closes_an_open_host_menu_and_opens_settings_at_its_first_switch(
    cx: &mut TestAppContext,
) {
    let (_fake, cx) = main_view(cx);
    click(cx, "hosts-panel-toggle");
    click(cx, "host-menu-toggle-2");
    assert!(
        node(&frame(cx), "host-menu").is_some(),
        "the premise: the row's menu is open"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-menu-retry"),
        "the premise: focus is in the menu"
    );

    click(cx, "app-settings-toggle");
    let nodes = frame(cx);
    assert!(node(&nodes, "host-menu").is_none(), "the menu went");
    assert!(
        node(&nodes, "settings-sections").is_some(),
        "the gear opened Settings"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("settings-skip-session-confirmations"),
        "focus starts on Settings' first switch"
    );
    keys(cx, "escape");
    assert!(
        !any_open(cx),
        "Escape closed Settings and nothing is left open"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("app-settings-toggle"));
}

/// Why this matters: a row's menu is a popover with no veil, so the controls around it stay live, and Add host must
/// open in one press with a host's or a session's menu open, taking that menu away rather than leaving it up under the
/// dialog or pointing at a row the dialog covers (the old "add-host-form-closes-a-stray-host-menu" and
/// "add-host-form-closes-a-stray-session-menu"). Specified: with Hosts shown and `build-box`'s menu open, a click on
/// the heading's Add host closes the menu and opens Add host, and Escape returns focus to that button; back on
/// Sessions with session `a`'s menu open, a click on the rail's Add host does the same, Escape returning focus there.
#[gpui::test]
fn add_host_takes_an_open_host_or_session_menu_away(cx: &mut TestAppContext) {
    let (_fake, cx) = main_view(cx);
    click(cx, "hosts-panel-toggle");
    click(cx, "host-menu-toggle-2");
    assert!(
        node(&frame(cx), "host-menu").is_some(),
        "the premise: the host's menu is open"
    );
    click(cx, "hosts-add");
    let nodes = frame(cx);
    assert!(node(&nodes, "host-menu").is_none(), "the host's menu went");
    assert!(node(&nodes, "add-host").is_some(), "Add host opened");
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("hosts-add"));

    click(cx, "hosts-panel-toggle");
    assert!(!hosts_shown(cx), "the premise: Sessions shows");
    click(cx, "session-menu-a");
    assert!(
        node(&frame(cx), "session-menu").is_some(),
        "the premise: the session's menu is open"
    );
    click(cx, "host-rail-add");
    let nodes = frame(cx);
    assert!(
        node(&nodes, "session-menu").is_none(),
        "the session's menu went"
    );
    assert!(node(&nodes, "add-host").is_some(), "Add host opened");
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-add"));
}

/// Why this matters: the avatar's menu is how the rail reaches a host's actions while Sessions shows, and what they
/// open must not swap the list for the Hosts panel under the user, and closing it must bring focus back to the avatar
/// the user started from, not to a row toggle that is not drawn (the old "rail ${action} modal stays visible over
/// Sessions and returns to its avatar", for setup, uninstall and a refused update). Specified, with Sessions shown:
/// from `build-box`'s avatar, Re-run's probe answered with a plan opens the setup question, and its Cancel; from
/// `gpu-1`'s, Uninstall's plan opens the uninstall question, and its Cancel; and Update's plan refused opens the host
/// refusal dialog, and its Close: each leaves Sessions shown while open and after, and returns focus to the avatar. The
/// same holds for Cancel on a removal asked from `gpu-1`'s avatar, which sends nothing at the ask or at the Cancel (the
/// old "rail removal returns to its avatar on cancel and Hosts on success", its Cancel half; focus after a removal that
/// went through is a recorded deviation).
#[gpui::test]
fn dialogs_from_an_avatars_menu_keep_sessions_and_return_to_the_avatar(cx: &mut TestAppContext) {
    let (mut fake, cx) = main_view(cx);
    assert!(!hosts_shown(cx), "the premise: Sessions shows");
    let plan = |uninstall: bool| {
        let mut plan = serde_json::json!({
            "probe_id": "plan-1",
            "plan": { "state_dir": "/home/dev/.local/state/farhelm" },
            "confirmation": "Farhelm will change this host:\n",
            "steps": { "lead": "Farhelm will change this host:", "steps": [], "kept": null },
        });
        if !uninstall {
            plan["result"] = serde_json::json!("provisionable");
        }
        plan
    };
    let refused = serde_json::json!("this host cannot be updated from here");
    for (avatar, item) in [
        ("host-rail-host-2", "host-menu-rerun"),
        ("host-rail-host-3", "host-menu-uninstall"),
        ("host-rail-host-3", "host-menu-update"),
    ] {
        let (path, status, body, close) = match item {
            "host-menu-rerun" => ("/api/hosts/probe", 200, plan(false), "prompt-cancel"),
            "host-menu-uninstall" => ("/api/hosts/3/uninstall", 200, plan(true), "prompt-cancel"),
            "host-menu-update" => (
                "/api/hosts/3/update",
                409,
                refused.clone(),
                "host-refusal-done",
            ),
            other => unreachable!("no case for {other}"),
        };
        open_from_avatar(cx, avatar);
        click(cx, item);
        answer(&mut fake, path, status, Some(body));
        test_support::settle(cx);
        assert!(any_open(cx), "{item}: its dialog opened");
        assert!(!hosts_shown(cx), "{item}: Sessions still shows");
        click(cx, close);
        assert!(!any_open(cx), "{item}: {close} closed it");
        assert!(!hosts_shown(cx), "{item}: Sessions still shows after");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(avatar),
            "{item}: focus is back on the avatar"
        );
    }

    open_from_avatar(cx, "host-rail-host-3");
    click(cx, "host-menu-remove");
    assert!(any_open(cx), "the premise: the removal question opened");
    assert!(!hosts_shown(cx), "Sessions still shows while it asks");
    click(cx, "prompt-cancel");
    let sent = fake.commands();
    assert!(
        sent.iter().all(|command| command.path != "/api/hosts/3"),
        "no removal was sent: {sent:?}"
    );
    assert!(
        !any_open(cx) && !hosts_shown(cx),
        "Cancel closed it over Sessions"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-3"));
}
