//! Tests of the Hosts panel as it draws: the heading's count, a row's name, mark and phase for every host, the Details
//! switch and the rows that open by themselves, the local host with no supervisor, the read states of the host list,
//! host text drawn escaped, and (with the real text system) a long name giving way before the phase words.
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
use farhelm_proto::provisioning::{ProvisioningOperation, ProvisioningView, RunStatus};
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
}

impl Render for Harness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        root::wrap(
            div().size_full().child(self.panel.clone()),
            &self.root,
            window,
            cx,
        )
    }
}

/// The panel over the fake helm, connected with `snapshot`, in an active window, checked once settled.
fn open_panel(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, Entity<HostsPanel>, &mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot);
    cx.run_until_parked();
    let (harness, window) = test_support::open_view(cx, |window, cx| Harness {
        panel: cx.new(HostsPanel::new),
        root: root::attach(window, cx),
    });
    let panel = window.update(|_, cx| harness.read(cx).panel.clone());
    (fake, panel, window)
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
fn details(nodes: &[DriverNode], host: i64) -> Vec<String> {
    (0..)
        .map_while(|n| node(nodes, &format!("host-detail-{host}-{n}")))
        .map(|node| node.label.as_deref().unwrap_or_default().to_owned())
        .collect()
}

/// Whether the Details switch is on, as assistive technology hears it.
fn details_switch(nodes: &[DriverNode]) -> bool {
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
/// its status at a glance. The words for every phase are pinned by `phase/tests.rs`; this proves the row draws them.
/// Specified: the local host, an aliased remote with an old build and a connecting unaliased remote are each named by
/// their shown name, their marks by "Local" or "Remote", and their statuses by their phase words.
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
/// user's. Specified: with Details off, a host whose last run failed opens with its details, a host whose run is still
/// running stays closed, and the switch stays off; once a later run of the failed host completes, its row closes.
#[gpui::test]
fn a_failed_run_opens_only_its_own_row(cx: &mut TestAppContext) {
    let listed = reference_hosts();
    let (mut fake, _, cx) = open_panel(
        cx,
        vec![
            hosts(&listed),
            progress(&[
                run(2, ProvisioningOperation::Update, RunStatus::Failed),
                run(3, ProvisioningOperation::Add, RunStatus::Running),
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
        "a running run opens nothing yet"
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

// ---- The local host ------------------------------------------------------------------------------------------------

/// Why this matters: on the helm's own machine the panel never installs a supervisor, and until one exists the local
/// host must appear with what to do, "not as a phantom unreachable host" (SPEC.md, "Topology"). The helm's own words
/// name the state directory only the helm knows. Nothing else opens this row (no run fails, nothing is refused), so it
/// has to open itself, with the user's Details switch left off. Specified: with Details off, the local host with no
/// supervisor is "Local (this machine)" reading "Unreachable, retrying", and its row is open with "No supervisor is
/// running on this machine" and the helm's remedy; once the helm pushes it connected, the row closes.
#[gpui::test]
fn the_local_host_without_a_supervisor_opens_its_row_to_say_what_to_do(cx: &mut TestAppContext) {
    let down = host(
        1,
        "local",
        local_down("start it with `farhelm supervisor run --state-dir /srv/state`"),
    );
    let (mut fake, _, cx) = open_panel(cx, vec![hosts(std::slice::from_ref(&down))]);
    let nodes = frame(cx);
    assert!(!details_switch(&nodes), "the premise: Details is off");
    assert_eq!(label(&nodes, "host-row-1"), "Local (this machine)");
    assert_eq!(label(&nodes, "host-status-1"), "Unreachable, retrying");
    assert_eq!(
        details(&nodes, 1),
        [
            "No supervisor is running on this machine",
            "The helm reports: start it with `farhelm supervisor run --state-dir /srv/state`",
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

// ---- Layout, with the real text system -----------------------------------------------------------------------------

/// Why this matters: a host's name is unbounded user data, and the old row let the name give way first (its 4em floor
/// and ellipsis) so the phase words and the toggle's place never left the sidebar (`.host-row-main`'s `nowrap`, a
/// fixed bug of the old app). Only the real text system measures that, so this lays the panel out with the bundled
/// fonts in a headless app. Specified: in the sidebar column's 340 pixels, a host with a very long alias keeps its
/// whole phase words inside the panel, its name cut short with "…" and ending before the words begin.
#[test]
fn a_long_name_gives_way_before_the_phase_words() {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| session::start(handle, events, None, cx));
    let long = aliased(
        host(2, "ssh", connecting(1, None)),
        "a build machine with a deliberately long alias that cannot fit",
        "dev@build-box.invalid",
    );
    fake.connect(vec![hosts(&[long])]);
    cx.run_until_parked();
    let window = cx
        .open_window(size(px(340.), px(400.)), |window, cx| {
            let panel = cx.new(HostsPanel::new);
            let harness = cx.new(|cx| Harness {
                panel,
                root: root::attach(window, cx),
            });
            cx.new(|cx| Root::new(harness, window, cx))
        })
        .unwrap();
    let nodes = cx
        .update_window(window.into(), |_, window, cx| {
            window.set_driver_enabled(true);
            window.draw(cx).clear(cx);
            window.driver_nodes().to_vec()
        })
        .unwrap();
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
