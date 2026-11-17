//! Tests of the Hosts panel as it draws: the heading's count and add button, a row's name, mark and phase for every
//! host, the rows as the list's items, the Details switch and the rows that open by themselves, the local host with no
//! supervisor, the read states of the host list, host text drawn escaped, and (with the real text system) a long name
//! giving way before the phase words.
//!
//! The panel runs over the helm client's fake, connected with pushed hosts and host progress built here, and the
//! tests read what it drew from GPUI's driver record of the frame, by element id (`host-row-<id>`,
//! `host-status-<id>`, `host-detail-<id>-<n>`, `host-details-toggle`). The fixtures at the top are shared with the
//! module's other tests and the screenshot goldens.

use std::sync::Arc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::http::LOCAL_SUPERVISOR_NOT_RUNNING;
use farhelm_proto::provisioning::{ProvisioningOperation, ProvisioningView, RunStatus, StepStatus};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, ConnectionStage,
    HostStateView, HostView, KeyedEntry, RefreshView,
};
use gpui::{
    AppContext as _, Context, DriverNode, Entity, HeadlessAppContext, IntoElement,
    ParentElement as _, Render, Role, Styled as _, TestAppContext, Toggled, VisualTestContext,
    Window, div, px, size,
};
use gpui_component::Root;

use super::HostsPanel;
use crate::host_text::tests::SPOOFING_CORPUS;
use crate::operations::{Command, OpKey, Work};
use crate::test_support::{self, frame, meta, started};
use crate::{assets, bootstrap, root, session};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// A host as the helm pushes it: registry id `id`, the wire kind `kind`, in `state`. A local host is the registry's
/// own "this machine" with the red laptop; any other is unaliased at `dev@build-box.invalid` (a fixture name that
/// cannot resolve) with the default cloud.
pub(crate) fn host(id: i64, kind: &str, state: HostStateView) -> HostView {
    let local = kind == "local";
    let destination = (!local).then(|| "dev@build-box.invalid".to_owned());
    HostView {
        id,
        entry_uuid: String::new(),
        kind: kind.to_owned(),
        is_session_owned: false,
        tensorlake: None,
        environment: None,
        container_check: None,
        name: destination
            .clone()
            .unwrap_or_else(|| "this machine".to_owned()),
        destination,
        alias: None,
        identity: None,
        remote_farhelm: None,
        remote_state_dir: None,
        yolo_without_asking: false,
        commands_without_asking: false,
        own_sessions_without_asking: false,
        links_without_asking: false,
        local_links_without_asking: false,
        icon: if local {
            HostIcon::Laptop
        } else {
            HostIcon::Cloud
        },
        color: if local {
            HostColor::Red
        } else {
            HostColor::Default
        },
        state,
        incarnation: 1,
        trash_epoch: 0,
    }
}

/// `host` under the alias `alias` at `destination`: the helm then names it by the alias.
pub(crate) fn aliased(mut host: HostView, alias: &str, destination: &str) -> HostView {
    host.alias = Some(alias.to_owned());
    host.name = alias.to_owned();
    host.destination = Some(destination.to_owned());
    host
}

/// A connected host at this fixture's build, `0.16.0`, as current as the helm, with identity `id-1` and three sessions.
pub(crate) fn connected() -> HostStateView {
    connected_with("0.16.0", "id-1", 3)
}

/// A connected, current host at `build` with `identity` and `sessions` sessions.
pub(crate) fn connected_with(build: &str, identity: &str, sessions: usize) -> HostStateView {
    HostStateView::Connected {
        identity: Some(identity.to_owned()),
        build_version: build.to_owned(),
        old_version: false,
        newer_version: false,
        refresh: RefreshView::Ok { sessions },
    }
}

/// A host on connection attempt `attempt`, waiting for its next try, after `error` if the last one failed.
pub(crate) fn connecting(attempt: u32, error: Option<&str>) -> HostStateView {
    HostStateView::Connecting {
        attempt,
        stage: ConnectionStage::WaitingToConnect,
        last_error: error.map(str::to_owned),
    }
}

/// The helm's dial failure for its own machine with no supervisor running, exactly as it reaches the window. Outermost
/// first: the host manager's stage context for the dial (`stage_failure_context`, "connecting to the local
/// supervisor"), the helm's `classify_local_dial` context, `farhelm_supervisor::service::connect`'s context naming the
/// socket, then the operating system's error; joined the way the manager records an error (`{error:#}`) and then
/// Debug-quoted by its `peer_text`, which is why the value carries the surrounding double quotes. It names no
/// hand-start command: the dial leaves remedies to its callers, and the helm adds none for its own machine, whose
/// supervisor the desktop app starts when it opens.
pub(crate) const LOCAL_DIAL_FAILURE: &str = "\"connecting to the local supervisor: no supervisor is running on this \
     machine: supervisor does not appear to be running (socket /srv/state/supervisor.sock is not accepting \
     connections): Connection refused (os error 111)\"";

/// The helm's own machine with no supervisor running, the helm's dial failure reading `error`.
pub(crate) fn local_down(error: &str) -> HostStateView {
    HostStateView::Unreachable {
        cause: LOCAL_SUPERVISOR_NOT_RUNNING.to_owned(),
        last_error: error.to_owned(),
    }
}

/// A host refused at the handshake: it speaks protocol `peer` with build `9.0.0`, the helm speaks `ours` with build
/// `0.16.0`, and the helm's remedy is "update the host".
pub(crate) fn skew(peer: u32, ours: u32) -> HostStateView {
    HostStateView::VersionSkew {
        peer_protocol: peer,
        peer_build: "9.0.0".into(),
        our_protocol: ours,
        our_build: "0.16.0".into(),
        remediation: "update the host".into(),
    }
}

/// Host `host_id`'s retained run of `operation`, in `status`.
pub(crate) fn run(
    host_id: i64,
    operation: ProvisioningOperation,
    status: RunStatus,
) -> ProvisioningView {
    ProvisioningView {
        host_id,
        run_id: Some(format!("run-{host_id}")),
        operation: Some(operation),
        status,
        steps: Vec::new(),
        message: None,
        outcome_unknown: false,
    }
}

/// The collection `kind` holding `entries` in that order, each filed under the registry id `id` gives it (hosts and
/// host progress are both keyed by a host's id).
fn collection<T: serde::Serialize>(
    kind: Collection,
    entries: &[T],
    id: impl Fn(&T) -> i64,
) -> CollectionState {
    let keys: Vec<String> = entries
        .iter()
        .map(|entry| HostView::key(id(entry)))
        .collect();
    let order: Vec<&str> = keys.iter().map(String::as_str).collect();
    CollectionState {
        collection: kind,
        meta: meta(&order),
        entries: entries
            .iter()
            .zip(&keys)
            .map(|(entry, key)| KeyedEntry {
                key: key.clone(),
                value: serde_json::to_value(entry).unwrap(),
            })
            .collect(),
    }
}

/// The hosts collection holding `listed`, in that (registry) order.
pub(crate) fn hosts(listed: &[HostView]) -> CollectionState {
    collection(Collection::Hosts, listed, |host| host.id)
}

/// The host progress collection holding `runs`, each under its host's id.
pub(crate) fn progress(runs: &[ProvisioningView]) -> CollectionState {
    collection(Collection::HostProgress, runs, |run| run.host_id)
}

/// The reference screens' three hosts (`hosts-panel`): the local host and two aliased remotes, all connected and
/// current, with fixture identities and session counts.
pub(crate) fn reference_hosts() -> Vec<HostView> {
    let mut local = host(
        1,
        "local",
        connected_with(
            "0.0.0-unreleased",
            "7a6052f7-5897-4a34-b2c7-f0c9e9ea6c89",
            3,
        ),
    );
    local.icon = HostIcon::Laptop;
    vec![
        local,
        aliased(
            host(
                2,
                "ssh",
                connected_with(
                    "0.0.0-unreleased",
                    "c1182bd3-a604-4d1e-855d-ccbf6c992b0b",
                    1,
                ),
            ),
            "build-box",
            "dev@build-box",
        ),
        aliased(
            host(
                3,
                "ssh",
                connected_with(
                    "0.0.0-unreleased",
                    "df06c6e5-a058-4377-93bb-541c9e68e940",
                    1,
                ),
            ),
            "gpu-1",
            "dev@gpu-1",
        ),
    ]
}

// ---- The harness ---------------------------------------------------------------------------------------------------

/// A root view holding one panel, wired as every root view is, filling the window as the sidebar's column would.
struct Harness {
    panel: Entity<HostsPanel>,
    root: root::Attachment,
    /// The panel's width, as the sidebar's column gives it, or `None` to fill the window.
    column: Option<gpui::Pixels>,
}

impl Render for Harness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = match self.column {
            Some(width) => div().w(width).h_full().child(self.panel.clone()),
            None => div().size_full().child(self.panel.clone()),
        };
        root::wrap(panel, &self.root, window, cx)
    }
}

/// The panel over the fake helm, connected with `snapshot`, in an active window, checked once settled. Shared with the
/// host menu's and the host settings dialog's tests, which open both from a row.
pub(crate) fn open_panel(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, Entity<HostsPanel>, &mut VisualTestContext) {
    open_panel_with(cx, snapshot, None)
}

/// [`open_panel`] in a column of the sidebar's width (less the rail's), as the app draws it, so a row's menu opens beside
/// the panel with room to spare instead of being pushed back over the panel's right edge by the window's.
pub(crate) fn open_panel_in_column(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, Entity<HostsPanel>, &mut VisualTestContext) {
    open_panel_with(
        cx,
        snapshot,
        Some(crate::theme::metrics::SIDEBAR_WIDTH - px(56.)),
    )
}

/// [`open_panel`] with the panel `column` wide, or filling the window.
fn open_panel_with(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
    column: Option<gpui::Pixels>,
) -> (FakeHelm, Entity<HostsPanel>, &mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot);
    cx.run_until_parked();
    let (harness, window) = test_support::open_view(cx, |window, cx| Harness {
        panel: cx.new(HostsPanel::new),
        root: root::attach(window, cx),
        column,
    });
    let panel = window.update(|_, cx| harness.read(cx).panel.clone());
    (fake, panel, window)
}

/// Start `work` under `key` in the current operation store, as the launcher, a session's Restart, Replace or Stop, or a
/// preference save would, and return the id of the one command it sent, which the fake helm leaves unanswered until the
/// test replies: the work stays in flight until then. Shared by the hosts module's tests of refusals during a launch.
pub(crate) fn start_work(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    work: Work,
    key: OpKey,
) -> u64 {
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("a UI session");
        let command = Command::new(
            work,
            Method::Post,
            ApiPath::api(&["sessions"]).expect("a valid path"),
            Some(serde_json::json!({})),
            "POST /api/sessions",
        );
        operations.update(cx, |operations, cx| {
            operations
                .start(&key, command, cx, |_, _| {})
                .expect("the work started");
        });
    });
    test_support::settle(cx);
    let sent = helm.commands();
    assert_eq!(
        sent.len(),
        1,
        "the premise: {work:?} is in flight on the wire"
    );
    sent[0].id
}

/// Whether no key in the current operation store has work in flight: a step's premise that the work an earlier step
/// started has settled.
pub(crate) fn nothing_in_flight(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| {
        session::current_operations(cx)
            .is_some_and(|operations| !operations.read(cx).any_in_flight(|_| true))
    })
}

/// The node drawn under `id`, if any.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// The accessible name of the node drawn under `id`; fails if nothing was drawn there with one.
fn label(nodes: &[DriverNode], id: &str) -> String {
    match node(nodes, id).and_then(|node| node.label.as_deref()) {
        Some(label) => label.to_owned(),
        None => panic!("{id} was not drawn with a name"),
    }
}

/// The names of `host`'s detail lines in a frame, in order: empty while its row is closed.
pub(crate) fn details(nodes: &[DriverNode], host: i64) -> Vec<String> {
    (0..)
        .map_while(|n| node(nodes, &format!("host-detail-{host}-{n}")))
        .map(|node| node.label.as_deref().unwrap_or_default().to_owned())
        .collect()
}

/// Whether the Details switch is on, as assistive technology hears it; fails if it is not drawn with a state. Shared
/// with the other hosts test modules.
pub(crate) fn details_switch(nodes: &[DriverNode]) -> bool {
    match node(nodes, "host-details-toggle").and_then(|node| node.toggled) {
        Some(state) => state == Toggled::True,
        None => panic!("the Details switch was not drawn with a state"),
    }
}

// ---- The heading and the rows --------------------------------------------------------------------------------------

/// Why this matters: the heading's count is how the panel says how many hosts the helm manages, and the old markup kept
/// the number first so a screen reader reads "3 hosts" while the screen shows "Hosts 3"; before the list is read there
/// is no number to give. Specified: the count names "3 hosts" over three hosts (session-owned ones count, as they are
/// listed) and "1 host" over one; with no hosts collection yet it is "Hosts" and the panel says it is loading; the
/// panel is the region named "Host management".
#[gpui::test]
fn the_heading_counts_the_hosts_the_helm_lists(cx: &mut TestAppContext) {
    let mut owned = host(4, "podman", connected());
    owned.is_session_owned = true;
    let mut listed = reference_hosts();
    listed.truncate(2);
    listed.push(owned);
    let (mut fake, _, cx) = open_panel(cx, vec![hosts(&listed)]);
    let nodes = frame(cx);
    assert_eq!(label(&nodes, "host-count"), "3 hosts");
    assert_eq!(
        node(&nodes, "hosts-panel").and_then(|node| node.role),
        Some(Role::Region)
    );
    assert_eq!(label(&nodes, "hosts-panel"), "Host management");
    assert!(
        node(&nodes, "host-row-4").is_some(),
        "the session-owned host has its row"
    );

    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1"]),
        upsert: Vec::new(),
        remove: vec!["2".into(), "4".into()],
    });
    test_support::settle(cx);
    assert_eq!(label(&frame(cx), "host-count"), "1 host");
}

/// Why this matters: before the first snapshot the panel knows no hosts and must not claim a count or invent rows.
/// Specified: with no hosts collection, the count reads "Hosts" and the panel says "Loading hosts…" with no rows.
#[gpui::test]
fn a_host_list_not_read_yet_says_it_is_loading(cx: &mut TestAppContext) {
    let (_fake, _, cx) = open_panel(cx, Vec::new());
    let nodes = frame(cx);
    assert_eq!(label(&nodes, "host-count"), "Hosts");
    assert_eq!(label(&nodes, "hosts-status"), "Loading hosts…");
    assert!(!nodes.iter().any(|node| {
        node.element_id
            .as_deref()
            .is_some_and(|id| id.starts_with("host-row-"))
    }));
}

/// Why this matters: a failed read is no evidence that a host went (SPEC.md: a failed registry refresh retains the
/// hosts, a failed first read invents none), and the user must be told that what they see may be stale. Specified: a
/// refresh that fails over listed hosts keeps every row under "Showing the last state this client read; the refresh
/// failed: <error>"; a failed first read lists no rows and says "The hosts list could not be loaded: <error>".
#[gpui::test]
fn a_failed_read_keeps_the_rows_it_had_and_says_so(cx: &mut TestAppContext) {
    let (mut fake, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    let failed = |order: &[&str]| CollectionMeta {
        availability: Availability::Unavailable {
            error: "registry locked".into(),
        },
        ..meta(order)
    };
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: failed(&["1", "2", "3"]),
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    test_support::settle(cx);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "hosts-load-error"),
        "Showing the last state this client read; the refresh failed: registry locked"
    );
    for id in 1..=3 {
        assert!(
            node(&nodes, &format!("host-row-{id}")).is_some(),
            "row {id} stays"
        );
    }

    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: failed(&[]),
        upsert: Vec::new(),
        remove: vec!["1".into(), "2".into(), "3".into()],
    });
    test_support::settle(cx);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "hosts-load-error"),
        "The hosts list could not be loaded: registry locked"
    );
    assert!(node(&nodes, "host-row-1").is_none());
}

/// Why this matters: the row names a host the way the user knows it (the alias, or "Local (this machine)" for the
/// helm's own machine under the registry's name), with its identity mark saying where it is, and its phase words are
/// its status at a glance. The words for every phase are pinned by `identity/phase/tests.rs`; this proves the row draws
/// them. Specified: the local host, an aliased remote with an old build and a connecting unaliased remote are each
/// named by their shown name, their marks by "Local" or "Remote", and their statuses by their phase words.
#[gpui::test]
fn every_row_shows_its_name_mark_and_phase_words(cx: &mut TestAppContext) {
    let mut old = connected();
    if let HostStateView::Connected { old_version, .. } = &mut old {
        *old_version = true;
    }
    let listed = vec![
        host(1, "local", connected()),
        aliased(host(2, "ssh", old), "build-box", "dev@build-box.invalid"),
        host(3, "ssh", connecting(2, Some("refused"))),
    ];
    let (_fake, _, cx) = open_panel(cx, vec![hosts(&listed)]);
    let nodes = frame(cx);
    let row = |id: i64| {
        [
            label(&nodes, &format!("host-row-{id}")),
            label(&nodes, &format!("host-mark-{id}")),
            label(&nodes, &format!("host-status-{id}")),
        ]
    };
    assert_eq!(row(1), ["Local (this machine)", "Local", "Connected"]);
    assert_eq!(row(2), ["build-box", "Remote", "Old version"]);
    assert_eq!(row(3), ["dev@build-box.invalid", "Remote", "Connecting"]);
}

/// Why this matters: assistive technology reads the rows as the items of the list "Hosts", as the old markup's `ul`
/// and `li` made them; a row drawn outside the list, or without the item role, reads as loose text and controls. The
/// test bridge's snapshot builds its tree from the same parents. Specified: every row is drawn with the list-item role
/// and named by its host's shown name, inside the element with the list role named "Hosts".
#[gpui::test]
fn every_row_is_an_item_of_the_hosts_list(cx: &mut TestAppContext) {
    let (_fake, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    let nodes = frame(cx);
    let list = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some("host-list"))
        .expect("the list was drawn");
    assert_eq!(
        (nodes[list].role, nodes[list].label.as_deref()),
        (Some(Role::List), Some("Hosts"))
    );
    let inside = |mut at: usize| loop {
        match nodes[at].parent {
            Some(parent) if parent == list => return true,
            Some(parent) => at = parent,
            None => return false,
        }
    };
    for (id, name) in [(1, "Local (this machine)"), (2, "build-box"), (3, "gpu-1")] {
        let row = nodes
            .iter()
            .position(|node| node.element_id.as_deref() == Some(&format!("host-row-{id}")))
            .unwrap_or_else(|| panic!("row {id} was drawn"));
        assert_eq!(
            (nodes[row].role, nodes[row].label.as_deref()),
            (Some(Role::ListItem), Some(name))
        );
        assert!(inside(row), "row {id} is drawn inside the list");
    }
}

/// Why this matters: the heading's add button is the panel's own way to add a host, beside the host rail's Add host,
/// and closing the dialog it opens must return focus to it on every platform (SPEC.md, "Opening and closing"), also on
/// macOS, where the click leaves the button unfocused. Specified: the heading draws a button named "Add host"; under
/// macOS's click rule a click on it opens the Add host dialog, and Escape closes it with focus on the button.
#[gpui::test]
fn the_headings_add_button_opens_add_host_and_gets_focus_back(cx: &mut TestAppContext) {
    let (_fake, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    let nodes = frame(cx);
    assert_eq!(
        node(&nodes, "hosts-add").map(|node| (node.role, node.label.as_deref())),
        Some((Some(Role::Button), Some("Add host")))
    );
    test_support::click(cx, "hosts-add");
    assert!(
        node(&frame(cx), "add-host").is_some(),
        "the Add host dialog opened"
    );
    test_support::keys(cx, "escape");
    assert!(node(&frame(cx), "add-host").is_none(), "Escape closed it");
    assert_eq!(test_support::focused_id(cx).as_deref(), Some("hosts-add"));
}

// ---- Disclosure ----------------------------------------------------------------------------------------------------

/// Why this matters: Details is how the user sees the evidence and remedies behind every phase (SPEC.md: it "reveals
/// the version, identity, session count, remedies, diagnostics ... under every row"), it starts off and is never saved,
/// and the sidebar keeps one panel so the choice survives a swap to Sessions and back. Specified: with the switch off no
/// row shows details; one click turns it on for every row (the aliased host's destination first, then its version,
/// identity and sessions); drawing the same panel again keeps it on; another click closes every row.
#[gpui::test]
fn details_open_and_close_every_row(cx: &mut TestAppContext) {
    let (_fake, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    let nodes = frame(cx);
    assert!(!details_switch(&nodes), "the premise: Details starts off");
    for id in 1..=3 {
        assert_eq!(
            details(&nodes, id),
            Vec::<String>::new(),
            "row {id} is closed"
        );
    }

    test_support::click(cx, "host-details-toggle");
    let nodes = frame(cx);
    assert!(details_switch(&nodes));
    assert_eq!(
        details(&nodes, 1),
        ["Farhelm 0.0.0-unreleased; identity 7a6052f7-5897-4a34-b2c7-f0c9e9ea6c89; 3 sessions"]
    );
    assert_eq!(
        details(&nodes, 2),
        [
            "Destination: dev@build-box",
            "Farhelm 0.0.0-unreleased; identity c1182bd3-a604-4d1e-855d-ccbf6c992b0b; 1 session",
        ]
    );
    assert_eq!(details(&nodes, 3).len(), 2);
    test_support::redraw(cx);
    assert!(details_switch(&frame(cx)), "a redraw keeps the choice");

    test_support::click(cx, "host-details-toggle");
    let nodes = frame(cx);
    assert!(!details_switch(&nodes));
    for id in 1..=3 {
        assert_eq!(
            details(&nodes, id),
            Vec::<String>::new(),
            "row {id} closed again"
        );
    }
}

/// Why this matters: a failure must show its evidence without the user having to find the switch (the goal: a row
/// "opens itself on a failure and shows remedies and diagnostics"), but only that row, and the global switch stays the
/// user's. Specified: with Details off, a host whose last run failed opens with its details, a host whose Update is
/// still running stays closed (SPEC.md: "While an update is running, its host row stays folded"), and the switch stays
/// off; once a later run of the failed host completes, its row closes.
#[gpui::test]
fn a_failed_run_opens_only_its_own_row(cx: &mut TestAppContext) {
    let listed = reference_hosts();
    let (mut fake, _, cx) = open_panel(
        cx,
        vec![
            hosts(&listed),
            progress(&[
                run(2, ProvisioningOperation::Update, RunStatus::Failed),
                run(3, ProvisioningOperation::Update, RunStatus::Running),
            ]),
        ],
    );
    let nodes = frame(cx);
    assert!(!details_switch(&nodes), "nothing turned the switch on");
    assert_eq!(details(&nodes, 1), Vec::<String>::new());
    assert_eq!(details(&nodes, 2).len(), 2, "the failed host's row is open");
    assert_eq!(
        details(&nodes, 3),
        Vec::<String>::new(),
        "a running Update stays folded"
    );

    fake.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&["2", "3"]),
        upsert: vec![KeyedEntry {
            key: "2".into(),
            value: serde_json::to_value(run(
                2,
                ProvisioningOperation::Update,
                RunStatus::Completed,
            ))
            .unwrap(),
        }],
        remove: Vec::new(),
    });
    test_support::settle(cx);
    assert_eq!(
        details(&frame(cx), 2),
        Vec::<String>::new(),
        "success closes it"
    );
}

/// Why this matters: when the helm refuses work on a host (a save of its settings, an Update's plan), the refusal is
/// shown where the host is, with the row open so its remedies are beside it, and it stays until the next work on the
/// host replaces it. Specified: before the refusal the row is closed; after the helm refuses work under the host's key,
/// the row shows the helm's words (escaped) and opens its details; the other rows stay closed.
#[gpui::test]
fn a_refusal_shows_under_its_row_and_opens_it(cx: &mut TestAppContext) {
    let (mut fake, _, cx) = open_panel(cx, vec![hosts(&reference_hosts())]);
    assert_eq!(
        details(&frame(cx), 2),
        Vec::<String>::new(),
        "the premise: closed"
    );
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("a UI session");
        let command = Command::new(
            Work::Save,
            Method::Patch,
            ApiPath::api(&["hosts", "2"]).unwrap(),
            Some(serde_json::json!({ "alias": "x" })),
            "PATCH /api/hosts/[host]",
        );
        operations.update(cx, |operations, cx| {
            operations
                .start(&OpKey::host("2"), command, cx, |_, _| {})
                .unwrap();
        });
    });
    test_support::settle(cx);
    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "the premise: the save went out");
    fake.reply(
        sent[0].id,
        409,
        Some(serde_json::json!("alias\u{202E} taken")),
    );
    test_support::settle(cx);
    let nodes = frame(cx);
    assert_eq!(label(&nodes, "host-error-2"), "alias<U+202E> taken");
    assert_eq!(details(&nodes, 2).len(), 2, "the refused host's row opened");
    assert_eq!(details(&nodes, 1), Vec::<String>::new());
    assert!(node(&nodes, "host-error-1").is_none());
}

// ---- Setup progress ------------------------------------------------------------------------------------------------

/// Host 3's setup `run-3` in `status`, with three steps in `steps`' statuses: "Create directories", "Install the
/// farhelm binary" and "Connect to the supervisor".
fn setup_run(status: RunStatus, steps: [StepStatus; 3]) -> ProvisioningView {
    let mut view = run(3, ProvisioningOperation::Add, status);
    view.steps = [
        "Create directories",
        "Install the farhelm binary",
        "Connect to the supervisor",
    ]
    .into_iter()
    .zip(steps)
    .map(|(name, status)| crate::hosts::progress::tests::step(name, status))
    .collect();
    view
}

/// Push `view` as host 3's progress, replacing whatever was there, and settle.
fn push_run(fake: &mut FakeHelm, view: &ProvisioningView, cx: &mut VisualTestContext) {
    push(fake, progress(std::slice::from_ref(view)), cx);
}

/// Push `state` as the helm pushes a change: every entry it holds upserted, in its order, nothing removed; then settle.
fn push(fake: &mut FakeHelm, state: CollectionState, cx: &mut VisualTestContext) {
    fake.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// Every line of text the window painted, joined with single spaces.
fn painted(nodes: &[DriverNode]) -> String {
    let lines: Vec<String> = nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|line| line.text.to_string()))
        .collect();
    lines.join(" ")
}

/// Why this matters: SPEC.md, "Topology": "the running row expands once to show the helm's steps ... The user may fold
/// that run ... the inline status still says setting up ... and shows the step count, current step and elapsed time.
/// Success folds the row". The drive of the first Add host port found a new host's row showing no progress at all.
/// Specified: with Details off, a running setup opens its row with "Setup provisioning Running", each step named with
/// its result word, and Collapse; the inline status in place of the phase words reads 1 of 3 steps done with the
/// running step named; Collapse closes the row and leaves the status; the same run pushed again stays folded; its
/// success takes the status and the steps away and the row stays closed, back to its phase words.
#[gpui::test]
fn a_running_setup_opens_its_row_once_and_collapse_folds_it(cx: &mut TestAppContext) {
    let running = setup_run(
        RunStatus::Running,
        [
            StepStatus::Completed,
            StepStatus::Running,
            StepStatus::Pending,
        ],
    );
    let (mut fake, _, cx) = open_panel(
        cx,
        vec![
            hosts(&reference_hosts()),
            progress(std::slice::from_ref(&running)),
        ],
    );
    let nodes = frame(cx);
    assert!(!details_switch(&nodes), "the premise: Details is off");
    assert_eq!(
        label(&nodes, "host-run-title-3"),
        "Setup provisioning Running"
    );
    let steps: Vec<String> = (0..3)
        .map(|n| label(&nodes, &format!("host-step-3-{n}")))
        .collect();
    assert_eq!(
        steps,
        [
            "Create directories: Completed",
            "Install the farhelm binary: Running",
            "Connect to the supervisor: Pending",
        ]
    );
    assert_eq!(
        label(&nodes, "host-status-3"),
        "Setting up: 1 of 3 steps done, now Install the farhelm binary"
    );
    assert!(
        node(&nodes, "host-run-fold-3").is_some(),
        "Collapse is offered"
    );
    assert!(
        node(&nodes, "host-run-2").is_none(),
        "no other row shows a run"
    );

    test_support::click(cx, "host-run-fold-3");
    let nodes = frame(cx);
    assert!(node(&nodes, "host-run-3").is_none(), "folded");
    assert_eq!(details(&nodes, 3), Vec::<String>::new(), "the row closed");
    assert!(
        label(&nodes, "host-status-3").starts_with("Setting up: 1 of 3"),
        "the inline status stays"
    );
    let mut later = running.clone();
    later.steps[1].status = StepStatus::Completed;
    later.steps[2].status = StepStatus::Running;
    push_run(&mut fake, &later, cx);
    assert!(
        node(&frame(cx), "host-run-3").is_none(),
        "the run's progress keeps its fold"
    );

    push_run(
        &mut fake,
        &setup_run(
            RunStatus::Completed,
            [
                StepStatus::Completed,
                StepStatus::Completed,
                StepStatus::Completed,
            ],
        ),
        cx,
    );
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-status-3"),
        "Connected",
        "the phase words are back"
    );
    assert!(node(&nodes, "host-run-3").is_none());
    assert_eq!(
        details(&nodes, 3),
        Vec::<String>::new(),
        "success leaves it folded"
    );
}

/// Why this matters: SPEC.md, "Topology": "failure opens it for the step and its reason", whatever the user folded, and
/// the drive of the first Add host port found a failed setup's reason ("disk full") shown nowhere. Specified: a running
/// setup the user folded, once pushed failed with its step's reason and the run's message, opens its row with
/// "Setup provisioning Failed", the failed step named "Failed", its reason and the run's message under the steps, and
/// no Collapse; the inline status gives way to the phase words.
#[gpui::test]
fn a_failed_setup_opens_its_row_with_the_step_and_the_reason(cx: &mut TestAppContext) {
    let running = setup_run(
        RunStatus::Running,
        [
            StepStatus::Completed,
            StepStatus::Running,
            StepStatus::Pending,
        ],
    );
    let (mut fake, _, cx) = open_panel(
        cx,
        vec![
            hosts(&reference_hosts()),
            progress(std::slice::from_ref(&running)),
        ],
    );
    test_support::click(cx, "host-run-fold-3");
    assert!(
        node(&frame(cx), "host-run-3").is_none(),
        "the premise: folded"
    );

    let mut failed = setup_run(
        RunStatus::Failed,
        [
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Pending,
        ],
    );
    failed.steps[1].message = Some("disk full".into());
    failed.message =
        Some("step 2 (install-farhelm) failed: disk full; rerun provisioning to continue".into());
    push_run(&mut fake, &failed, cx);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-run-title-3"),
        "Setup provisioning Failed"
    );
    assert_eq!(
        label(&nodes, "host-step-3-1"),
        "Install the farhelm binary: Failed"
    );
    assert_eq!(label(&nodes, "host-step-3-1-message"), "disk full");
    assert_eq!(
        label(&nodes, "host-run-message-3"),
        "step 2 (install-farhelm) failed: disk full; rerun provisioning to continue"
    );
    assert!(
        node(&nodes, "host-run-fold-3").is_none(),
        "a failure cannot be folded"
    );
    assert_eq!(label(&nodes, "host-status-3"), "Connected");
    assert!(!details(&nodes, 3).is_empty(), "the row's details show too");
}

/// Why this matters: SPEC.md, "Topology": Prepare snapshot shows "with the explanation **Only the first time, and then
/// at most once every 24 hours or after Farhelm updates.** The explanation stays visible while waiting", a wait long
/// enough to make the user wonder whether every new host costs it. Every other step's explanation belongs to the
/// setup's confirmation, and the old panel left it out of the step list (`progress_step_explanation`). Specified: in
/// an open row's step list, a running Prepare snapshot step (the helm's `prepare-snapshot`) draws its explanation
/// under its name and above its message; a completed step whose plan carried an explanation draws none.
#[gpui::test]
fn the_step_list_explains_snapshot_preparation_and_no_other_step(cx: &mut TestAppContext) {
    const PREPARATION: &str =
        "Only the first time, and then at most once every 24 hours or after Farhelm updates.";
    const CONFIRMATION_ONLY: &str = "Creates ~/.local/share/farhelm and keeps what is there.";
    let step = crate::hosts::progress::tests::step;
    let mut running = run(3, ProvisioningOperation::Add, RunStatus::Running);
    let mut done = step("Create directories", StepStatus::Completed);
    done.explanation = Some(CONFIRMATION_ONLY.into());
    let mut preparing = step("Prepare snapshot", StepStatus::Running);
    assert_eq!(
        preparing.step, "prepare-snapshot",
        "the premise: the helm's id"
    );
    preparing.explanation = Some(PREPARATION.into());
    preparing.message = Some("Building the recipe's snapshot".into());
    running.steps = vec![done, preparing];
    let (_fake, _, cx) = open_panel(cx, vec![hosts(&reference_hosts()), progress(&[running])]);
    let nodes = frame(cx);
    assert_eq!(label(&nodes, "host-step-3-1-explanation"), PREPARATION);
    assert_eq!(
        label(&nodes, "host-step-3-1-message"),
        "Building the recipe's snapshot"
    );
    // Both lines are drawn (the labels above), so their tops compare; by the driver's nodes, since a host sentence has
    // an id but no debug selector.
    let top = |id: &str| node(&nodes, id).map(|node| node.bounds.top());
    assert!(
        top("host-step-3-1-explanation") < top("host-step-3-1-message"),
        "explanation first"
    );
    assert!(node(&nodes, "host-step-3-0-explanation").is_none());
    assert!(
        !painted(&nodes).contains(CONFIRMATION_ONLY),
        "the confirmation's words stay out"
    );
}

/// Why this matters: the helm registers a host when it accepts its setup, before the setup runs, so the new host's row
/// must appear with the run's steps as they go, and its menu must hold what would change the host under the run
/// (`provisioning.spec.ts`, "accepted ADD registers before execution and releases the page lock while running"; the
/// rail's half is the rail's, and what the menu offers once the run ends is `offers/tests.rs`'
/// `a_running_run_holds_the_host_until_it_ends`). Specified: a pushed new host 4 with a running setup run gets a row
/// showing "Setup provisioning Running", whose menu offers no Update and no Re-run, and whose Settings does nothing
/// when clicked (no dialog, nothing sent, the menu staying).
#[gpui::test]
fn an_accepted_setups_host_appears_with_its_run_and_its_menu_waits_for_it(cx: &mut TestAppContext) {
    let (mut fake, _, cx) = open_panel(cx, vec![hosts(&reference_hosts()), progress(&[])]);
    // Room for the fourth row with its steps open, and for its menu beside it.
    cx.simulate_resize(size(px(1600.), px(1000.)));
    test_support::settle(cx);
    let mut listed = reference_hosts();
    listed.push(aliased(
        host(4, "ssh", connecting(1, None)),
        "new-box",
        "dev@new-box",
    ));
    let mut setup = setup_run(
        RunStatus::Running,
        [
            StepStatus::Completed,
            StepStatus::Running,
            StepStatus::Pending,
        ],
    );
    setup.host_id = 4;
    setup.run_id = Some("run-4".into());
    push(&mut fake, hosts(&listed), cx);
    push(&mut fake, progress(std::slice::from_ref(&setup)), cx);
    assert_eq!(label(&frame(cx), "host-row-4"), "new-box");
    assert_eq!(
        label(&frame(cx), "host-run-title-4"),
        "Setup provisioning Running"
    );
    test_support::click(cx, "host-menu-toggle-4");
    let nodes = frame(cx);
    assert!(node(&nodes, "host-menu-update").is_none());
    assert!(node(&nodes, "host-menu-rerun").is_none());
    test_support::click(cx, "host-menu-settings");
    assert!(
        node(&frame(cx), "host-settings").is_none() && fake.commands().is_empty(),
        "Settings waits for the run"
    );
    assert!(test_support::any_open(cx), "the menu stayed");
}

/// The text the window painted in its last drawn frame, without drawing another: so only a redraw the app asked for by
/// itself can have changed it. Call [`frame`] once first, which turns the window's driver record on.
fn last_painted(cx: &mut VisualTestContext) -> String {
    painted(&cx.update(|window, _| window.driver_nodes().to_vec()))
}

/// How often the Hosts panel rendered since the render counter was last reset.
fn panel_renders(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| crate::render_count::count("hosts-panel", cx))
}

/// Why this matters: the elapsed time is measured by the client (SPEC.md: "client-measured elapsed time"), and it must
/// keep counting on its own while nothing else changes, in an inactive window (often still on screen) and under
/// reduced motion too, where no pulse redraws the panel. The ticker's one-second clock, which the panel holds while a
/// run's time shows, is the only thing that redraws it then, so this reads what the window last drew rather than
/// drawing a frame of its own. Specified: in an inactive window under reduced motion, a setup running when the panel
/// first sees it reads "0:00"; after 7 seconds on the executor's clock, with nothing pushed, the panel has redrawn by
/// itself and reads "0:07", and after 63 more, "1:10"; once the run completes, the panel lets go of the clock, and 5
/// more seconds redraw it not once.
#[gpui::test]
fn the_elapsed_time_counts_from_when_the_panel_first_saw_the_run(cx: &mut TestAppContext) {
    let running = setup_run(
        RunStatus::Running,
        [
            StepStatus::Running,
            StepStatus::Pending,
            StepStatus::Pending,
        ],
    );
    let (mut fake, _, cx) = open_panel(
        cx,
        vec![
            hosts(&reference_hosts()),
            progress(std::slice::from_ref(&running)),
        ],
    );
    cx.update(|_, cx| crate::ticker::set_reduced_motion(true, cx));
    cx.deactivate_window();
    assert!(
        !cx.update(|window, cx| crate::ticker::is_active(window, cx)),
        "the premise: the window counts as inactive"
    );
    let first = painted(&frame(cx));
    assert!(first.contains("0:00"), "0:00 is painted: {first}");
    for (wait, clock) in [(7, "0:07"), (63, "1:10")] {
        cx.update(|_, cx| crate::render_count::reset(cx));
        test_support::advance(cx, std::time::Duration::from_secs(wait));
        assert!(panel_renders(cx) > 0, "the panel redrew by itself");
        let text = last_painted(cx);
        assert!(text.contains(clock), "{clock} is painted: {text}");
    }

    push_run(
        &mut fake,
        &setup_run(
            RunStatus::Completed,
            [
                StepStatus::Completed,
                StepStatus::Completed,
                StepStatus::Completed,
            ],
        ),
        cx,
    );
    cx.update(|_, cx| crate::render_count::reset(cx));
    test_support::advance(cx, std::time::Duration::from_secs(5));
    assert_eq!(panel_renders(cx), 0, "no clock is left to keep");
}

/// Why this matters: an Update's inline status shows its elapsed time just as a setup's does, so the panel must hold
/// the one-second clock for an Update's run too; and since the sidebar keeps its one panel for the window's life, the
/// panel must let go of that clock when the sidebar hides it through its `hidden` hook, or the clock's loop would keep
/// waking every second for a panel nobody sees. (That the sidebar calls the hook on every way out of Hosts is pinned by
/// the pulse tests below.) Specified: under reduced motion, so that no pulse redraws the panel, with host 3's Update
/// running and its status read out as an Update's, one second on the executor's clock redraws the panel by itself to
/// read "0:01"; once `hidden` is called, 3 more seconds redraw it not once.
#[gpui::test]
fn an_updates_clock_counts_until_the_panel_is_hidden(cx: &mut TestAppContext) {
    let mut updating = run(3, ProvisioningOperation::Update, RunStatus::Running);
    updating.steps = vec![
        crate::hosts::progress::tests::step("Install the farhelm binary", StepStatus::Running),
        crate::hosts::progress::tests::step("Restart the supervisor", StepStatus::Pending),
    ];
    let (_fake, panel, cx) = open_panel(
        cx,
        vec![
            hosts(&reference_hosts()),
            progress(std::slice::from_ref(&updating)),
        ],
    );
    cx.update(|_, cx| crate::ticker::set_reduced_motion(true, cx));
    let nodes = frame(cx);
    assert!(
        label(&nodes, "host-status-3").starts_with("Updating: 0 of 2 steps done"),
        "the premise: the row shows the Update's run"
    );

    cx.update(|_, cx| crate::render_count::reset(cx));
    test_support::advance(cx, std::time::Duration::from_secs(1));
    assert!(
        panel_renders(cx) > 0,
        "the clock redrew the panel by itself"
    );
    let text = last_painted(cx);
    assert!(text.contains("0:01"), "0:01 is painted: {text}");

    panel.update(cx, |panel, cx| panel.hidden(cx));
    cx.update(|_, cx| crate::render_count::reset(cx));
    test_support::advance(cx, std::time::Duration::from_secs(3));
    assert_eq!(
        panel_renders(cx),
        0,
        "a hidden panel holds no clock to redraw it"
    );
}

/// The main window over a fleet with no sessions, `reference_hosts` and `runs` as the host progress, Sessions showing:
/// the sidebar as the app draws it, with its one Hosts panel made but not drawn.
fn main_with_runs<'a>(
    cx: &'a mut TestAppContext,
    runs: &[ProvisioningView],
) -> (FakeHelm, &'a mut VisualTestContext) {
    crate::session_list::tests::open_main(cx, main_snapshot(runs))
}

/// [`main_with_runs`]'s snapshot.
fn main_snapshot(runs: &[ProvisioningView]) -> Vec<CollectionState> {
    vec![
        crate::session_list::tests::listing(&[], &[]),
        crate::session_list::tests::preferences(&[], None),
        hosts(&reference_hosts()),
        progress(runs),
    ]
}

/// Why this matters: the panel keeps a run's first sight from the moment the run is pushed, whether or not Hosts shows
/// (that is what its store subscription is for), because SPEC.md's elapsed time counts the run, not how long the panel
/// has been open; and what it keeps belongs to one UI session, so a reset starts it over, as the old app's reload did.
/// Specified: with Sessions showing, a setup pushed running and 5 seconds later Hosts pressed shows the row open with
/// "0:05"; after Collapse and a reset that brings the same run back, the row is open again and reads "0:00".
#[gpui::test]
fn a_run_keeps_its_clock_while_hosts_is_closed_and_a_reset_starts_it_over(cx: &mut TestAppContext) {
    let (mut fake, cx) = main_with_runs(cx, &[]);
    assert!(
        node(&frame(cx), "hosts-panel").is_none(),
        "the premise: Sessions shows"
    );
    let running = setup_run(
        RunStatus::Running,
        [
            StepStatus::Running,
            StepStatus::Pending,
            StepStatus::Pending,
        ],
    );
    push_run(&mut fake, &running, cx);
    test_support::advance(cx, std::time::Duration::from_secs(5));
    test_support::click(cx, "hosts-panel-toggle");
    let nodes = frame(cx);
    assert!(
        node(&nodes, "host-run-3").is_some(),
        "the row opened for the run"
    );
    let text = painted(&nodes);
    assert!(
        text.contains("0:05"),
        "the clock counted while Hosts was closed: {text}"
    );

    test_support::click(cx, "host-run-fold-3");
    assert!(
        node(&frame(cx), "host-run-3").is_none(),
        "the premise: folded"
    );
    fake.break_channel();
    test_support::settle(cx);
    fake.connect(main_snapshot(std::slice::from_ref(&running)));
    test_support::settle(cx);
    // Whether Hosts still shows after a reset is the sidebar's own per-session state; this is about the panel.
    if node(&frame(cx), "hosts-panel").is_none() {
        test_support::click(cx, "hosts-panel-toggle");
    }
    let nodes = frame(cx);
    assert!(
        node(&nodes, "host-run-3").is_some(),
        "the fold went with the old UI session"
    );
    let text = painted(&nodes);
    assert!(text.contains("0:00"), "the clock started over: {text}");
}

/// The ticker's count of delivered ticks.
fn ticks(cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| crate::ticker::ticks(cx))
}

/// The main window with a setup running on `gpu-1` and Hosts shown, after checking the premise every variant below
/// needs: the setup's pulse makes the ticker tick.
fn pulsing_hosts(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let running = setup_run(
        RunStatus::Running,
        [
            StepStatus::Running,
            StepStatus::Pending,
            StepStatus::Pending,
        ],
    );
    let (fake, cx) = main_with_runs(cx, std::slice::from_ref(&running));
    test_support::click(cx, "hosts-panel-toggle");
    let before = ticks(cx);
    test_support::advance(cx, std::time::Duration::from_millis(500));
    assert!(
        ticks(cx) > before,
        "the premise: the pulse ticks while Hosts shows"
    );
    (fake, cx)
}

/// Check that Sessions shows and that half a second on the executor's clock delivers no tick.
fn no_ticks_with_sessions_shown(cx: &mut VisualTestContext) {
    assert!(
        node(&frame(cx), "hosts-panel").is_none(),
        "the premise: Sessions shows"
    );
    let hidden = ticks(cx);
    test_support::advance(cx, std::time::Duration::from_millis(500));
    assert_eq!(ticks(cx), hidden, "nothing ticks for a panel nobody sees");
}

/// Why this matters: the ticker's rule is that no view holds the running pulse while it draws no pulsing mark, because
/// a held pulse keeps the tick loop waking ten times a second; the sidebar keeps its one Hosts panel for the window's
/// life and only stops drawing it when Sessions shows, so the panel must be told where the sidebar hides it.
/// Specified: with a setup running and Hosts showing, the ticker ticks; once Hosts is pressed again and Sessions shows,
/// half a second on the executor's clock delivers no tick.
#[gpui::test]
fn hiding_hosts_stops_the_setup_pulse(cx: &mut TestAppContext) {
    let (_fake, cx) = pulsing_hosts(cx);
    test_support::click(cx, "hosts-panel-toggle");
    no_ticks_with_sessions_shown(cx);
}

/// Why this matters: Hosts also gives way to Sessions without its own control being pressed: a host choice on the rail
/// returns to Sessions (SPEC.md: "All or a host avatar returns to Sessions"), and so does a reset, which starts the
/// sidebar's state over. Either must let go of the pulse too, the first through the sidebar's other hiding path and the
/// second through the panel's own following of the UI session. Specified: with a setup running and Hosts showing, a
/// click on the rail's All shows Sessions and no tick follows; then, with Hosts shown again, a reset that brings the
/// same run back leaves Sessions showing and no tick follows either.
#[gpui::test]
fn a_host_choice_and_a_reset_stop_the_setup_pulse_too(cx: &mut TestAppContext) {
    let (mut fake, cx) = pulsing_hosts(cx);
    test_support::click(cx, "host-rail-all");
    no_ticks_with_sessions_shown(cx);

    test_support::click(cx, "hosts-panel-toggle");
    let before = ticks(cx);
    test_support::advance(cx, std::time::Duration::from_millis(500));
    assert!(
        ticks(cx) > before,
        "the premise: it ticks again with Hosts shown"
    );
    let running = setup_run(
        RunStatus::Running,
        [
            StepStatus::Running,
            StepStatus::Pending,
            StepStatus::Pending,
        ],
    );
    fake.break_channel();
    test_support::settle(cx);
    fake.connect(main_snapshot(std::slice::from_ref(&running)));
    test_support::settle(cx);
    no_ticks_with_sessions_shown(cx);
}

// ---- The local host ------------------------------------------------------------------------------------------------

/// Why this matters: on the helm's own machine the panel never installs a supervisor, and until one exists the local
/// host must appear with what to do, "not as a phantom unreachable host" (SPEC.md, "Topology"): reopen the Farhelm app,
/// which starts that machine's supervisor. Nothing else opens this row (no run fails, nothing is refused), so it has to
/// open itself, with the user's Details switch left off. Specified: with Details off, the local host with no
/// supervisor is "Local (this machine)" reading "Unreachable, retrying", and its row is open with the helm's dial
/// failure ([`LOCAL_DIAL_FAILURE`]) and then the app's remedy; once the helm pushes it connected, the row closes.
#[gpui::test]
fn the_local_host_without_a_supervisor_opens_its_row_to_say_what_to_do(cx: &mut TestAppContext) {
    let down = host(1, "local", local_down(LOCAL_DIAL_FAILURE));
    let (mut fake, _, cx) = open_panel(cx, vec![hosts(std::slice::from_ref(&down))]);
    let nodes = frame(cx);
    assert!(!details_switch(&nodes), "the premise: Details is off");
    assert_eq!(label(&nodes, "host-row-1"), "Local (this machine)");
    assert_eq!(label(&nodes, "host-status-1"), "Unreachable, retrying");
    assert_eq!(
        details(&nodes, 1),
        [
            LOCAL_DIAL_FAILURE,
            "Quit Farhelm and open it again; it starts this machine's supervisor when it opens"
        ]
    );

    let up = host(1, "local", connected());
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1"]),
        upsert: hosts(std::slice::from_ref(&up)).entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
    assert_eq!(
        details(&frame(cx), 1),
        Vec::<String>::new(),
        "a running supervisor closes it"
    );
}

// ---- Host text -----------------------------------------------------------------------------------------------------

/// Why this matters: a host's name, its builds and its errors come from the host or the helm, and a hidden or
/// direction-changing character in one must show as itself everywhere it is drawn and named (SPEC.md, 2026-10-10), or
/// a name could pose as another host and an error could reorder the words around it. Specified: for every string of
/// the spoofing corpus as a host's alias and as its connection error, the row's name and the detail line show the
/// escaped spelling, never the raw one.
#[gpui::test]
fn host_text_is_drawn_escaped(cx: &mut TestAppContext) {
    let listed: Vec<HostView> = SPOOFING_CORPUS
        .iter()
        .enumerate()
        .map(|(n, (raw, _))| {
            let id = n as i64 + 10;
            let mut row = host(id, "ssh", connecting(1, Some(raw)));
            row.alias = Some((*raw).to_owned());
            row.name = (*raw).to_owned();
            row
        })
        .collect();
    let (_fake, panel, cx) = open_panel(cx, vec![hosts(&listed)]);
    cx.update(|_, cx| panel.update(cx, |panel, cx| panel.show_details(true, cx)));
    let nodes = frame(cx);
    for (n, (raw, shown)) in SPOOFING_CORPUS.iter().enumerate() {
        let id = n as i64 + 10;
        assert_eq!(label(&nodes, &format!("host-row-{id}")), *shown, "{raw:?}");
        let lines = details(&nodes, id);
        let evidence = lines
            .iter()
            .find(|line| line.starts_with("Attempt 1"))
            .unwrap_or_else(|| panic!("{raw:?}: no evidence line in {lines:?}"));
        assert!(
            evidence.ends_with(&format!("the last one failed: {shown}")),
            "{raw:?}: {evidence:?}"
        );
        if raw != shown {
            assert!(!evidence.contains(raw), "{raw:?} drawn raw");
        }
    }
}

// ---- Revealing a host ----------------------------------------------------------------------------------------------

/// Why this matters: the attention list's Details opens Hosts "with that host's row in view and its menu button
/// focused" (SPEC.md, "Session list"), which only the panel can do, since it alone draws the row; and a reveal that
/// outlives its row must not land on a later host that reuses the id. Specified: in a short window over thirty hosts,
/// revealing the last one focuses its menu toggle and scrolls it into the panel's visible part, with the Details switch
/// still off and no menu open; revealing a host the registry does not list changes nothing, and nothing happens either
/// when a host with that id is then added.
#[gpui::test]
fn reveal_focuses_a_far_rows_toggle_and_scrolls_it_into_view(cx: &mut TestAppContext) {
    let mut listed: Vec<HostView> = (10..40).map(|id| host(id, "ssh", connected())).collect();
    let (mut fake, panel, cx) = open_panel_in_column(cx, vec![hosts(&listed)]);
    cx.simulate_resize(size(px(800.), px(400.)));
    test_support::settle(cx);
    let in_view = |cx: &mut VisualTestContext| {
        let shown = test_support::bounds(cx, "hosts-panel");
        let toggle = test_support::bounds(cx, "host-menu-toggle-39");
        toggle.top() >= shown.top() && toggle.bottom() <= shown.bottom()
    };
    assert!(!in_view(cx), "the premise: the last row is below the fold");
    let reveal = |host: i64, cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            let mark = crate::focus::Mark::take(cx);
            panel.update(cx, |panel, cx| panel.reveal(host, &mark, window, cx));
        });
        test_support::settle(cx);
    };

    reveal(39, cx);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("host-menu-toggle-39")
    );
    assert!(in_view(cx));
    assert!(!details_switch(&frame(cx)));
    assert!(!test_support::any_open(cx));

    reveal(99, cx);
    listed.push(host(99, "ssh", connected()));
    let state = hosts(&listed);
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("host-menu-toggle-39"),
        "the reveal of a host without a row was dropped"
    );
}

// ---- Layout, with the real text system -----------------------------------------------------------------------------

/// Why this matters: a host's name is unbounded user data, and the old row let the name give way first (its 4em floor
/// and ellipsis) so the phase words and the toggle's place never left the sidebar (`.host-row-main`'s `nowrap`, a
/// fixed bug of the old app). Only the real text system measures that ([`laid_out`]). Specified: in the sidebar
/// column's 340 pixels, a host with a very long alias keeps its whole phase words inside the panel, its name cut short
/// with "…" and ending before the words begin.
#[test]
fn a_long_name_gives_way_before_the_phase_words() {
    let long = aliased(
        host(2, "ssh", connecting(1, None)),
        "a build machine with a deliberately long alias that cannot fit",
        "dev@build-box.invalid",
    );
    let nodes = laid_out(&[long], 340., |_| {});
    let status = node(&nodes, "host-status-2").expect("the status was drawn");
    assert!(
        status.bounds.right() <= px(340.),
        "the status stays inside the panel: {:?}",
        status.bounds
    );
    let texts: Vec<_> = nodes.iter().flat_map(|node| node.texts.iter()).collect();
    let words = texts
        .iter()
        .find(|text| text.text.as_ref() == "Connecting")
        .expect("the phase words are painted whole");
    let name = texts
        .iter()
        .find(|text| text.text.starts_with("a build machine"))
        .expect("the name is painted");
    assert!(name.text.ends_with('…'), "cut short: {:?}", name.text);
    assert!(
        name.bounds.right() <= words.bounds.left(),
        "the name ends before the words: {:?} {:?}",
        name.bounds,
        words.bounds
    );
}

/// The Hosts panel over `listed`, `width` pixels wide, laid out once with the bundled fonts in a headless app, after
/// `before` has run against the app with the hosts in its store: the frame's driver record. The ordinary test windows
/// measure every glyph alike, so whether a row's words fit beside its controls is only decided here.
fn laid_out(
    listed: &[HostView],
    width: f32,
    before: impl FnOnce(&mut gpui::App),
) -> Vec<DriverNode> {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| session::start(handle, events, None, cx));
    fake.connect(vec![hosts(listed)]);
    cx.run_until_parked();
    cx.update(before);
    cx.run_until_parked();
    let window = cx
        .open_window(size(px(width), px(400.)), |window, cx| {
            let panel = cx.new(HostsPanel::new);
            let harness = cx.new(|cx| Harness {
                panel,
                root: root::attach(window, cx),
                column: None,
            });
            cx.new(|cx| Root::new(harness, window, cx))
        })
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        window.set_driver_enabled(true);
        window.draw(cx).clear(cx);
        window.driver_nodes().to_vec()
    })
    .unwrap()
}

/// Whether `a` and `b` share a horizontal band, as boxes on one line do: a box wrapped onto a line of its own has no
/// vertical overlap with the ones it left. The boxes need not coincide, since the row centers controls of different
/// heights.
fn on_one_line(a: gpui::Bounds<gpui::Pixels>, b: gpui::Bounds<gpui::Pixels>) -> bool {
    a.top() < b.bottom() && b.top() < a.bottom()
}

/// The first text painted in `nodes` that is `whole`, or a beginning of it cut short with "…", as text that does not
/// fit is drawn; fails, listing every painted text, if none is.
fn painted_as<'a>(nodes: &'a [DriverNode], whole: &str) -> &'a gpui::DriverText {
    let texts = move || nodes.iter().flat_map(|node| node.texts.iter());
    let cut_from_whole = |text: &str| {
        text.strip_suffix('…')
            .is_some_and(|kept| !kept.is_empty() && whole.starts_with(kept.trim_end()))
    };
    match texts().find(|text| text.text.as_ref() == whole || cut_from_whole(&text.text)) {
        Some(text) => text,
        None => {
            let painted: Vec<&str> = texts().map(|text| text.text.as_ref()).collect();
            panic!("{whole:?} is painted, whole or cut short: {painted:?}")
        }
    }
}

/// Host 2's menu toggle in `nodes`, checked to lie inside the panel, `width` wide, on one line with each of `beside`
/// (the header's other parts).
fn toggle_on_the_line(
    nodes: &[DriverNode],
    width: f32,
    beside: &[gpui::Bounds<gpui::Pixels>],
) -> gpui::Bounds<gpui::Pixels> {
    let toggle = node(nodes, "host-menu-toggle-2")
        .expect("the toggle is drawn")
        .bounds;
    assert!(
        toggle.left() >= px(0.) && toggle.right() <= px(width),
        "the toggle lies inside the panel: {toggle:?} in {width}"
    );
    for part in beside {
        assert!(
            on_one_line(toggle, *part),
            "the toggle shares the header's line: {toggle:?}, {part:?}"
        );
    }
    toggle
}

/// Why this matters: the toggle is the row's only way to its host's actions, and the longest phase words the panel
/// draws, "Unreachable, retrying", are long enough to push it off the header's line in a narrow sidebar, where a
/// wrapped toggle would sit apart from its row or outside the sidebar (the old `terminal-multihost.spec.ts`,
/// "host-menu-survives-the-longest-phase-word", and `ui-plan-regressions.spec.ts`, "an unreachable host keeps its menu
/// inside the minimum sidebar"). The words and the host's name may be cut short there, as the old `.host-status-label`
/// and `.host-name` were (`overflow: hidden` with an ellipsis, the name keeping a 4em floor), and the status keeps the
/// words whole as its name. Specified: in the panel's column at the sidebar's narrowest (the stored width's floor, 296
/// pixels, less the rail's 56), an unreachable `build-box`'s status is named "Unreachable, retrying"; the words and the
/// name are each painted whole or cut short with "…"; and its menu toggle lies inside the panel on the same line as
/// its name and its words.
#[test]
fn an_unreachable_hosts_toggle_stays_on_its_line_in_the_narrowest_sidebar() {
    let width = *farhelm_proto::ui_channel::SIDEBAR_WIDTH_RANGE.start() as f32 - 56.;
    let unreachable = HostStateView::Unreachable {
        cause: "transport-failure".to_owned(),
        last_error: "timed out".to_owned(),
    };
    let listed = [aliased(
        host(2, "ssh", unreachable),
        "build-box",
        "dev@build-box.invalid",
    )];
    let nodes = laid_out(&listed, width, |_| {});
    let status = node(&nodes, "host-status-2").expect("the status is drawn");
    assert_eq!(
        status.label.as_deref(),
        Some("Unreachable, retrying"),
        "the premise: the longest phase words are the status's name"
    );
    let words = painted_as(&nodes, "Unreachable, retrying").bounds;
    let name = painted_as(&nodes, "build-box").bounds;
    toggle_on_the_line(&nodes, width, &[name, words]);
}

/// Why this matters: the inline `↑ Update` sits on a row's header line between the name and the menu toggle, and its
/// progress takes the button's place once pressed; a long name must give way to both rather than wrap either one, or
/// the toggle would move away from its row (the old `terminal-multihost.spec.ts`, "host-inline-update: ${activation}
/// starts progress and keeps the header on one line", whose host had a very long name). Specified: in the panel's
/// column at the sidebar's default width, an older host under a long alias paints its name cut short with "…" and
/// draws its Update button inside the panel, on one line with the name and the menu toggle; with the Update's plan
/// request in flight it paints "Updating…" inside the panel, the name still cut short and on the toggle's line.
#[test]
fn the_inline_update_and_its_progress_keep_the_header_on_one_line() {
    const ALIAS: &str = "a build machine with a deliberately long alias that cannot fit";
    let width = f32::from(crate::theme::metrics::SIDEBAR_WIDTH) - 56.;
    let old = HostStateView::Connected {
        identity: Some("id-old".into()),
        build_version: "0.15.0".into(),
        old_version: true,
        newer_version: false,
        refresh: RefreshView::Ok { sessions: 1 },
    };
    let listed = [aliased(host(2, "ssh", old), ALIAS, "dev@build-box.invalid")];
    let cut_name = |nodes: &[DriverNode]| {
        let name = painted_as(nodes, ALIAS);
        assert!(
            name.text.ends_with('…'),
            "the name gives way: {:?}",
            name.text
        );
        name.bounds
    };

    let nodes = laid_out(&listed, width, |_| {});
    let button = node(&nodes, "host-update-2")
        .expect("the premise: the inline Update is drawn")
        .bounds;
    assert!(
        button.right() <= px(width),
        "the button lies inside: {button:?}"
    );
    toggle_on_the_line(&nodes, width, &[cut_name(&nodes), button]);

    // The plan request in flight on the host's key, set up directly because this headless app takes no clicks; that
    // the button's press starts this very work is `hosts::update`'s
    // `the_inline_update_shows_on_an_older_remote_host_and_starts_the_update`.
    let nodes = laid_out(&listed, width, |cx| {
        let Some(operations) = session::current_operations(cx) else {
            panic!("the premise: a UI session");
        };
        let path = ApiPath::api(&["hosts", "2", "update"]).unwrap();
        let command = Command::new(
            Work::UpdateHost,
            Method::Post,
            path,
            None,
            "POST /api/hosts/2/update",
        );
        operations.update(cx, |operations, cx| {
            operations
                .start(&OpKey::host("2"), command, cx, |_, _| {})
                .unwrap();
        });
    });
    assert!(
        node(&nodes, "host-update-2").is_none(),
        "the premise: progress replaced the button"
    );
    toggle_on_the_line(&nodes, width, &[cut_name(&nodes)]);
    let progress = painted_as(&nodes, "Updating…");
    assert_eq!(
        progress.text.as_ref(),
        "Updating…",
        "the progress is painted whole"
    );
    let progress = progress.bounds;
    assert!(
        progress.right() <= px(width),
        "the progress lies inside: {progress:?}"
    );
}
