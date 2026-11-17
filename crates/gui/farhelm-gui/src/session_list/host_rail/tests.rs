//! Tests of the host rail: its entries and their names, the session query a choice sends and how the helm's answer
//! settles it, the chip, the empty sentence of a host with no sessions, the placeholder of a removed host, the status
//! of a host list never read, a reset, the keyboard, Add host and the keyboard's reach below the rail's fold, the
//! foot's Hosts control and Settings gear, the rail's width in the sidebar, and each host's status on its avatar and
//! the Hosts control's update count. The query's status dimension, and the query's bookkeeping as pure functions, are
//! tested in `filters/tests.rs`, which shares these fixtures. When a host's update badge, live work or warning shows
//! is `hosts::status`'s rule and tested in `hosts/status/tests.rs`; these tests check that the rail draws and names
//! what that module reads, with the marks found by their debug selectors (their colors are the goldens' to check).
//!
//! Like the list's other tests, these run the real main view over the helm client's fake, which hands the test every
//! command the window sent, and read controls from GPUI's driver record of the frame by their ids (`host-rail-all`,
//! `host-rail-host-<id>`, `host-filter-chip`). The rule that is a pure function (a host's identity) is tested as such;
//! a host's mark and its tint are tested where they live, in `hosts/identity/tests.rs`. The one test whose outcome is
//! layout (the rail's width, the list beside it and a caption cut short) uses the real text system, since the test
//! context's fake one gives every character the same width.

use std::sync::Arc;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::provisioning::ProvisioningView;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostStateView,
    HostView, KeyedEntry, ListCounts, RefreshView, SESSION_QUERY_PATH,
};
use gpui::{
    AppContext as _, DriverNode, DriverText, HeadlessAppContext, Modifiers, MouseButton, Role,
    ScrollDelta, ScrollEvent, TestAppContext, Toggled, VisualTestContext, point, px, size,
};
use gpui_component::Root;
use serde_json::{Value, json};

use super::{identity, status};
use crate::host_text::tests::SPOOFING_CORPUS;
use crate::hosts::identity::LOCAL_NAME;
use crate::hosts::tests::{
    aliased, connected, connecting, host as status_host, hosts as status_hosts,
    progress as status_progress, run as status_run, skew,
};
use crate::main_view::MainView;
use crate::session_list::tests::{
    carrying, change, listing, open_main, painted, preferences, session,
};
use crate::test_support::{self, any_open, focused_id, frame, meta};
use crate::{assets, bootstrap, layers};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// One registered host as the helm would push it: the helm's pinned host list's first entry with the fields these
/// tests are about replaced. The screenshot goldens build their rail from it too.
#[derive(Clone)]
pub(crate) struct Host {
    id: i64,
    kind: &'static str,
    name: String,
    destination: Option<String>,
    alias: Option<String>,
    icon: &'static str,
    color: &'static str,
    session_owned: bool,
}

/// The local host as a fresh registry holds it: the registry's own name for it, the red laptop, no alias.
pub(crate) fn local() -> Host {
    Host {
        id: 1,
        kind: "local",
        name: LOCAL_NAME.to_owned(),
        destination: None,
        alias: None,
        icon: "laptop",
        color: "red",
        session_owned: false,
    }
}

/// An unaliased ssh host at `destination`, which the helm then also uses as its name: the default cloud.
pub(crate) fn remote(id: i64, destination: &str) -> Host {
    Host {
        id,
        kind: "ssh",
        name: destination.to_owned(),
        destination: Some(destination.to_owned()),
        alias: None,
        icon: "cloud",
        color: "default",
        session_owned: false,
    }
}

impl Host {
    /// The host under the user's alias, which the helm then also uses as its name.
    pub(crate) fn alias(mut self, alias: &str) -> Self {
        self.alias = Some(alias.to_owned());
        self.name = alias.to_owned();
        self
    }

    /// The host's chosen mark and color, as their stable words.
    pub(crate) fn mark(mut self, icon: &'static str, color: &'static str) -> Self {
        self.icon = icon;
        self.color = color;
        self
    }

    /// A container a session owns: the registry lists it, the rail does not.
    fn session_owned(mut self) -> Self {
        self.kind = "podman";
        self.session_owned = true;
        self
    }

    fn entry(&self) -> KeyedEntry {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../farhelm-helm/http-contract/host-list.json"
        ))
        .unwrap();
        let mut value = fixture["hosts"][0].clone();
        value["id"] = json!(self.id);
        value["kind"] = json!(self.kind);
        value["name"] = json!(self.name);
        value["destination"] = json!(self.destination);
        value["alias"] = json!(self.alias);
        value["icon"] = json!(self.icon);
        value["color"] = json!(self.color);
        value["is_session_owned"] = json!(self.session_owned);
        KeyedEntry {
            key: HostView::key(self.id),
            value,
        }
    }

    fn view(&self) -> HostView {
        serde_json::from_value(self.entry().value).unwrap()
    }
}

/// The hosts collection holding `hosts`, in that (registry) order.
pub(crate) fn hosts(hosts: &[Host]) -> CollectionState {
    let keys: Vec<String> = hosts.iter().map(|host| HostView::key(host.id)).collect();
    let order: Vec<&str> = keys.iter().map(String::as_str).collect();
    CollectionState {
        collection: Collection::Hosts,
        meta: meta(&order),
        entries: hosts.iter().map(Host::entry).collect(),
    }
}

/// The fleet most tests use: the local host, an aliased remote whose alias is far too long for the rail's caption, an
/// unaliased remote, and a container a session owns, in that registry order.
pub(crate) fn fleet_hosts() -> Vec<Host> {
    vec![
        local(),
        remote(3, "deploy@builder-east.example.invalid")
            .alias("Build fleet with a deliberately long caption")
            .mark("database", "teal"),
        remote(2, "ops@render-west.example.invalid"),
        remote(4, "podman:sandbox-1").session_owned(),
    ]
}

/// A snapshot of two sessions, both open-able, over [`fleet_hosts`].
pub(crate) fn snapshot() -> Vec<CollectionState> {
    vec![
        listing(&["a", "b"], &[session("a"), session("b")]),
        preferences(&["a"], None),
        hosts(&fleet_hosts()),
    ]
}

/// The sessions collection's metadata for a host query that matched `matching` of `total` sessions, complete.
pub(crate) fn filtered(order: &[&str], total: u64, matching: u64) -> CollectionMeta {
    let mut meta = meta(order);
    meta.counts = Some(ListCounts {
        total,
        matching: Some(matching),
        truncated: false,
    });
    meta
}

/// Answer, as done, every command the window has sent so far (remembering the selection, a seen mark), so a test's
/// next look at the fake sees only what its own step sent.
pub(crate) fn answer_everything(fake: &mut FakeHelm) {
    for command in fake.commands() {
        fake.reply(command.id, 200, None);
    }
}

/// Every command the window sent since the last look, each checked to be a session query, as (its command, its body).
pub(crate) fn queries(fake: &mut FakeHelm) -> Vec<(FakeCommand, Value)> {
    fake.commands()
        .into_iter()
        .map(|command| {
            assert_eq!(
                command.path, SESSION_QUERY_PATH,
                "only session queries: {command:?}"
            );
            assert_eq!(command.method, Method::Put, "{command:?}");
            let body = command.body.clone().unwrap_or_default();
            (command, body)
        })
        .collect()
}

/// The bodies of the session queries the window sent since the last look, in order. Every command, queries and the
/// rest (the selection remembered, a seen mark), is answered as done, so the next look sees only what came after; for
/// a test that pins which queries a step sent among other commands.
pub(crate) fn sent_queries(fake: &mut FakeHelm) -> Vec<Value> {
    let mut bodies = Vec::new();
    for command in fake.commands() {
        if command.path == SESSION_QUERY_PATH {
            bodies.push(command.body.clone().unwrap_or_default());
        }
        fake.reply(command.id, 204, None);
    }
    bodies
}

/// The one query the window sent since the last look, with its body; fails on none or several.
pub(crate) fn one_query(fake: &mut FakeHelm) -> (FakeCommand, Value) {
    let mut sent = queries(fake);
    assert_eq!(sent.len(), 1, "one query: {sent:?}");
    sent.remove(0)
}

/// The node drawn under `id` in `nodes`.
pub(crate) fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// The nodes drawn under `id`: the node itself and every node inside it, at any depth. Text a node shows is painted by
/// the text element inside it, not by the node, so this is where its text is found.
fn within<'a>(nodes: &'a [DriverNode], id: &str) -> Vec<&'a DriverNode> {
    let Some(top) = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some(id))
    else {
        panic!("{id} was not drawn");
    };
    let inside = |mut index: usize| loop {
        if index == top {
            return true;
        }
        match nodes[index].parent {
            Some(parent) => index = parent,
            None => return false,
        }
    };
    (0..nodes.len())
        .filter(|&index| inside(index))
        .map(|index| &nodes[index])
        .collect()
}

/// The lines of text painted under `id` (see [`within`]), with where each was painted.
pub(crate) fn texts_within(nodes: &[DriverNode], id: &str) -> Vec<DriverText> {
    within(nodes, id)
        .into_iter()
        .flat_map(|node| node.texts.iter().cloned())
        .collect()
}

/// The accessible name of the control drawn under `id`; fails if nothing was drawn under it.
pub(crate) fn label(nodes: &[DriverNode], id: &str) -> String {
    match node(nodes, id).and_then(|node| node.label.as_deref()) {
        Some(label) => label.to_owned(),
        None => panic!("{id} was not drawn with a name"),
    }
}

/// Whether the rail entry drawn under `id` is pressed, as assistive technology hears it.
pub(crate) fn pressed(nodes: &[DriverNode], id: &str) -> bool {
    match node(nodes, id).map(|node| node.toggled) {
        Some(Some(toggled)) => toggled == Toggled::True,
        Some(None) => panic!("{id} reports no pressed state"),
        None => panic!("{id} was not drawn"),
    }
}

/// The rail's entries that the frame drew, top to bottom, by id: every control under a `host-rail-` id but the foot's
/// Expand control, which shares the old prefix (`#host-rail-toggle`) without being an entry.
fn rail_entries(nodes: &[DriverNode]) -> Vec<String> {
    let mut entries: Vec<_> = nodes
        .iter()
        .filter(|node| node.role.is_some())
        .filter_map(|node| {
            let id = node.element_id.as_deref()?;
            (id.starts_with("host-rail-")
                && !id.starts_with("host-rail-caption")
                && id != super::TOGGLE_ID)
                .then(|| (f32::from(node.bounds.origin.y), id.to_owned()))
        })
        .collect();
    entries.sort_by(|a, b| a.0.total_cmp(&b.0));
    entries.into_iter().map(|(_, id)| id).collect()
}

/// The hover help showing after the pointer rests on the control drawn under `selector` for the hover delay.
pub(crate) fn hover_help(cx: &mut VisualTestContext, selector: &'static str) -> Option<String> {
    let at = test_support::center(cx, selector);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

// ---- The rail's entries ---------------------------------------------------------------------------------------------

/// The rail is how a user picks a host, so its order and names are its whole interface: SPEC.md has All first, then
/// every registered host in registry order, and a host a session owns (a sandbox or container made for one session)
/// stays out of it, its sessions under All. An avatar's name and tooltip are its identity (the old `rail_identity`),
/// which a screen reader user hears in place of the mark and the caption. Specified: over the local host, an aliased
/// remote, an unaliased one and a session-owned container, the rail draws All, then the three registered hosts in
/// registry order and nothing for the container, then Add host; All is named "All hosts" and pressed, the avatars are
/// not pressed and are named "Local (this machine)", the alias with its destination in parentheses, and the bare
/// destination (which is also its name, so no suffix), each followed by its state, here ": Connected"; each avatar's
/// caption is a node with no role (hidden from
/// assistive technology), and the local caption paints "Local".
#[gpui::test]
fn the_rail_lists_all_then_registered_hosts_in_order_with_their_names(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    let nodes = frame(cx);
    assert_eq!(
        rail_entries(&nodes),
        [
            "host-rail-all",
            "host-rail-host-1",
            "host-rail-host-3",
            "host-rail-host-2",
            "host-rail-add"
        ]
    );
    assert_eq!(label(&nodes, "host-rail-all"), "All hosts");
    assert!(pressed(&nodes, "host-rail-all"));
    assert_eq!(
        label(&nodes, "host-rail-host-1"),
        "Local (this machine): Connected"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-3"),
        "Build fleet with a deliberately long caption (deploy@builder-east.example.invalid): Connected"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-2"),
        "ops@render-west.example.invalid: Connected"
    );
    for id in ["host-rail-host-1", "host-rail-host-3", "host-rail-host-2"] {
        assert!(!pressed(&nodes, id), "{id} is not chosen");
    }
    for id in [1, 3, 2] {
        assert!(
            within(&nodes, &format!("host-rail-caption-{id}"))
                .iter()
                .all(|node| node.role.is_none()),
            "host {id}'s caption is hidden from assistive technology"
        );
    }
    let local_caption: Vec<String> = texts_within(&nodes, "host-rail-caption-1")
        .into_iter()
        .map(|text| text.text.to_string())
        .collect();
    assert_eq!(local_caption, ["Local"]);
}

/// The avatar's hover help is the same words as its name, so a sighted user learns which host a mark stands for and
/// what state it is in, as the old tooltip said. Specified: resting the pointer on the aliased host's avatar shows its
/// alias with its destination in parentheses and its state, and on All shows All's own help.
#[gpui::test]
fn an_avatars_hover_help_is_its_name(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    assert_eq!(
        hover_help(cx, "host-rail-host-3").as_deref(),
        Some(
            "Build fleet with a deliberately long caption (deploy@builder-east.example.invalid): Connected"
        )
    );
    assert_eq!(
        hover_help(cx, "host-rail-all").as_deref(),
        Some("All hosts: show sessions from every host")
    );
}

/// A host's name and destination are the host's own text, and a host named to pose as another (a direction override, a
/// zero-width character) must show what it is in the rail too. Specified: for each spoofing string, an aliased host
/// carrying it in its alias is named with the escaped form, and nothing the frame painted holds a presentation-unsafe
/// character.
#[gpui::test]
fn a_hosts_names_are_shown_and_named_escaped(cx: &mut TestAppContext) {
    let spoofs: Vec<Host> = SPOOFING_CORPUS
        .iter()
        .zip(10..)
        .map(|((raw, _), id)| remote(id, "ops@example.invalid").alias(raw))
        .collect();
    let (_fake, cx) = open_main(cx, vec![listing(&[], &[]), hosts(&spoofs)]);
    let nodes = frame(cx);
    for ((_, shown), id) in SPOOFING_CORPUS.iter().zip(10..) {
        assert_eq!(
            label(&nodes, &format!("host-rail-host-{id}")),
            format!("{shown} (ops@example.invalid): Connected")
        );
    }
    let texts = painted(&nodes);
    assert!(
        !texts.iter().any(|text| text
            .chars()
            .any(farhelm_proto::text::is_presentation_unsafe)),
        "nothing painted holds a hidden character; painted: {texts:?}"
    );
}

/// SPEC.md: "a failed first read invents no hosts". The rail then says the host list could not be loaded, and still
/// offers All, which needs no registry. Specified: a hosts collection whose first read failed (unavailable, no hosts)
/// and a snapshot with no hosts collection at all each draw All pressed and, after it, the "?" status named "The host
/// list could not be loaded" with the status role, and no avatar; Add host still follows, since adding a host needs no
/// host list.
#[gpui::test]
fn a_host_list_never_read_shows_the_unavailable_status(cx: &mut TestAppContext) {
    let mut failed = hosts(&[]);
    failed.meta.availability = Availability::Unavailable {
        error: "the registry could not be read".into(),
    };
    let (mut fake, cx) = open_main(cx, vec![listing(&[], &[]), failed]);
    for _ in 0..2 {
        let nodes = frame(cx);
        assert_eq!(
            rail_entries(&nodes),
            ["host-rail-all", "host-rail-unavailable", "host-rail-add"]
        );
        assert!(pressed(&nodes, "host-rail-all"));
        let status = node(&nodes, "host-rail-unavailable").unwrap();
        assert_eq!(status.role, Some(Role::Status));
        assert_eq!(
            status.label.as_deref(),
            Some("The host list could not be loaded")
        );
        fake.connect(vec![listing(&[], &[])]);
        cx.run_until_parked();
    }
}

// ---- Choosing ------------------------------------------------------------------------------------------------------

/// Choosing a host is the helm's query, not a filter the window applies, so what reaches the helm is the behavior:
/// SPEC.md's "Choosing a host sends a server-side query immediately and is not persisted, while All widens it again",
/// and the old app sent nothing when the choice did not change. The query names the host and the open session it
/// leaves open (so the helm keeps carrying it), never a sort, so status, sort and the open session stay as they are.
/// Specified: a click on host 3's avatar sends exactly one `PUT /ui/session-query` with `{"host": 3, "selected": "a"}`
/// and presses that avatar at once, before any reply, with All no longer pressed and the open session unchanged; a
/// second click sends nothing; after the reply, a click on All sends `{"selected": "a"}` and presses All.
#[gpui::test]
fn choosing_a_host_sends_its_query_once_and_all_sends_an_empty_one(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    let open = crate::session_list::tests::selected(cx);

    test_support::click(cx, "host-rail-host-3");
    let (query, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "host": 3, "selected": "a" }));
    let nodes = frame(cx);
    assert!(
        pressed(&nodes, "host-rail-host-3"),
        "pressed before the reply"
    );
    assert!(!pressed(&nodes, "host-rail-all"));
    assert_eq!(crate::session_list::tests::selected(cx), open);

    test_support::click(cx, "host-rail-host-3");
    assert_eq!(queries(&mut fake).len(), 0, "the same choice sends nothing");

    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    test_support::click(cx, "host-rail-all");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "selected": "a" }));
    assert!(pressed(&frame(cx), "host-rail-all"));
}

/// The helm validates a query by serving it, and a refused one leaves its channel on the query before it, so the
/// window must not go on showing a choice the helm is not serving; and SPEC.md wants a refusal reported, not swallowed.
/// The words are the helm's, which may quote a host, so they are host text. Specified: with host 3 chosen and
/// accepted, choosing host 2 and having it refused with words holding a direction override presses host 3 again,
/// keeps the chip on host 3, and paints the words escaped after "Could not show those sessions: "; choosing All
/// then takes the words away.
#[gpui::test]
fn a_refused_query_keeps_the_previous_choice_and_shows_the_helms_words(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    test_support::click(cx, "host-rail-host-3");
    let (accepted, _) = one_query(&mut fake);
    fake.reply(accepted.id, 204, None);
    cx.run_until_parked();

    test_support::click(cx, "host-rail-host-2");
    let (refused, body) = one_query(&mut fake);
    assert_eq!(
        body,
        json!({ "host": 2, "selected": "a" }),
        "the premise: host 2's query"
    );
    fake.reply(refused.id, 400, Some(json!("unknown host \u{202E}2")));
    cx.run_until_parked();
    let nodes = frame(cx);
    assert!(
        pressed(&nodes, "host-rail-host-3"),
        "the accepted choice is back"
    );
    assert!(!pressed(&nodes, "host-rail-host-2"));
    assert!(label(&nodes, "host-filter-chip").contains("Build fleet"));
    assert_eq!(
        label(&nodes, "filter-refusal"),
        "Could not show those sessions: unknown host <U+202E>2"
    );

    test_support::click(cx, "host-rail-all");
    assert!(node(&frame(cx), "filter-refusal").is_none());
}

// ---- The chip ------------------------------------------------------------------------------------------------------

/// SPEC.md: "A chosen host appears beside the status segments as a removable chip with its mark and short name,
/// matching the rail", with the name and destination as its tooltip, and removing it chooses All "exactly as the rail
/// does". Specified: with no host chosen there is no chip; once host 3 is chosen, the chip is named "Filtered to host
/// <alias>; remove filter", its hover help is the host's identity, and it paints the short name and ✕; a click on it
/// sends `{"selected": "a"}`, presses All and takes the chip away.
#[gpui::test]
fn the_chip_names_the_chosen_host_and_removes_the_choice(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    assert!(
        node(&frame(cx), "host-filter-chip").is_none(),
        "no chip under All"
    );

    test_support::click(cx, "host-rail-host-3");
    let (query, _) = one_query(&mut fake);
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-filter-chip"),
        "Filtered to host Build fleet with a deliberately long caption; remove filter"
    );
    let texts = painted(&nodes);
    assert!(texts.iter().any(|text| text == "✕"), "{texts:?}");
    assert_eq!(
        hover_help(cx, "host-filter-chip").as_deref(),
        Some("Build fleet with a deliberately long caption (deploy@builder-east.example.invalid)")
    );

    test_support::click(cx, "host-filter-chip");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "selected": "a" }));
    let nodes = frame(cx);
    assert!(pressed(&nodes, "host-rail-all"));
    assert!(node(&nodes, "host-filter-chip").is_none());
}

/// Removing the filter takes away the control the user just used (the chip goes with the filter), and SPEC.md sends
/// focus to the rail's All then ("After a clear button or the chip removes its focused control, focus returns to the
/// rail's All button"), where the coordinator's ordinary rule would send it home to the terminal. Specified: a click on
/// the chip leaves focus on All, whatever a click focuses on this platform; and by keyboard alone, from All, Tab twice
/// and Space choose host 3, Tab eight times more (host 2, Add host, the heading's order title, Trash, Templates and New
/// buttons, the status segments' one stop) reaches the chip, and Enter on it removes the filter and leaves focus on
/// All. Each removal sends `{"selected": "a"}`.
#[gpui::test]
fn clearing_the_filter_by_keyboard_or_click_focuses_all(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    test_support::click(cx, "host-rail-host-3");
    let (query, _) = one_query(&mut fake);
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    test_support::click(cx, "host-filter-chip");
    let (cleared, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "selected": "a" }));
    fake.reply(cleared.id, 204, None);
    cx.run_until_parked();
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-all"),
        "after a click"
    );

    test_support::keys(cx, "tab tab space");
    let (query, body) = one_query(&mut fake);
    assert_eq!(
        body,
        json!({ "host": 3, "selected": "a" }),
        "the premise: host 3 chosen by keyboard"
    );
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    test_support::keys(cx, "tab tab tab tab tab tab tab tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-filter-chip"),
        "the premise: the chip has focus"
    );
    test_support::keys(cx, "enter");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "selected": "a" }));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-all"),
        "after Enter"
    );
}

// ---- The empty state -----------------------------------------------------------------------------------------------

/// An empty filtered list must say which host it is about, and while a new choice is loading the sentence describes
/// the result still on screen, not the one asked for (SPEC.md, and the old `filters.spec.ts` case "an empty message
/// keeps its answered host while another host read is held"); the chip, by contrast, follows the choice at once.
/// "Show all hosts" clears the choice like the chip. Specified: host 3 chosen and answered with an empty listing (0 of
/// 2) shows the sentence "No sessions on <alias>." and the "Show all hosts" link; choosing host 2 with its reply held
/// keeps that sentence and moves the chip to host 2; once host 2's empty listing and reply arrive the sentence names
/// host 2; "Show all hosts" sends `{"selected": "a"}` and leaves focus on All, and the full listing with its reply draws
/// rows and no sentence.
#[gpui::test]
fn the_empty_state_names_the_answered_host_and_show_all_hosts_clears_it(cx: &mut TestAppContext) {
    let rows = [session("a"), session("b")];
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);

    test_support::click(cx, "host-rail-host-3");
    let (query, _) = one_query(&mut fake);
    // The helm carries the open a outside the empty list, as for any query naming it.
    fake.push(change(carrying(filtered(&[], 2, 0), "a"), &[], &["b"]));
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "filter-empty"),
        "No sessions on Build fleet with a deliberately long caption."
    );
    assert_eq!(label(&nodes, "filter-show-all-hosts"), "Show all hosts");

    test_support::click(cx, "host-rail-host-2");
    let (held, _) = one_query(&mut fake);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "filter-empty"),
        "No sessions on Build fleet with a deliberately long caption.",
        "the sentence still describes the listing on screen"
    );
    assert!(
        label(&nodes, "host-filter-chip").contains("render-west"),
        "the chip follows the choice"
    );

    fake.push(change(filtered(&[], 2, 0), &[], &[]));
    fake.reply(held.id, 204, None);
    cx.run_until_parked();
    assert_eq!(
        label(&frame(cx), "filter-empty"),
        "No sessions on render-west."
    );

    test_support::click(cx, "filter-show-all-hosts");
    let (all, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "selected": "a" }));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-all"),
        "focus moves to All in place of the link"
    );
    fake.push(change(listing(&["a", "b"], &rows).meta, &rows, &[]));
    fake.reply(all.id, 204, None);
    cx.run_until_parked();
    let nodes = frame(cx);
    assert!(node(&nodes, "filter-empty").is_none());
    assert!(node(&nodes, "session-row-a").is_some(), "the rows are back");
}

// ---- A removed host, a reset, the keyboard -------------------------------------------------------------------------

/// SPEC.md: "a removed selected host remains explicitly selected and unavailable until another host or All is
/// chosen", which the old app drew as a pressed, disabled "?" in the rail and a "no longer registered" chip, so the
/// user sees why the list is narrowed and how to undo it. Specified: with host 3 chosen and answered, a registry change
/// without it draws `host-rail-removed-3` pressed, not focusable and with no click handler, named "Host 3 (no longer
/// registered)", and the chip named "Filtered to host Host 3 (no longer registered); remove filter"; a click on the
/// placeholder sends nothing; the chip's click sends `{"selected": "a"}` and takes the placeholder away.
#[gpui::test]
fn a_removed_chosen_host_keeps_a_pressed_inert_placeholder(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    test_support::click(cx, "host-rail-host-3");
    let (query, _) = one_query(&mut fake);
    fake.reply(query.id, 204, None);
    cx.run_until_parked();

    let mut left = fleet_hosts();
    left.retain(|host| host.id != 3);
    let order: Vec<String> = left.iter().map(|host| HostView::key(host.id)).collect();
    let order: Vec<&str> = order.iter().map(String::as_str).collect();
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&order),
        upsert: Vec::new(),
        remove: vec![HostView::key(3)],
    });
    cx.run_until_parked();
    let nodes = frame(cx);
    let placeholder = node(&nodes, "host-rail-removed-3").expect("the placeholder is drawn");
    assert_eq!(
        placeholder.label.as_deref(),
        Some("Host 3 (no longer registered)")
    );
    assert_eq!(placeholder.toggled, Some(Toggled::True));
    assert!(
        !placeholder.flags.focusable && !placeholder.flags.click,
        "inert"
    );
    assert_eq!(
        label(&nodes, "host-filter-chip"),
        "Filtered to host Host 3 (no longer registered); remove filter"
    );
    test_support::click(cx, "host-rail-removed-3");
    assert_eq!(queries(&mut fake).len(), 0);

    test_support::click(cx, "host-filter-chip");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "selected": "a" }));
    assert!(node(&frame(cx), "host-rail-removed-3").is_none());
}

/// Whether the window sent a session query since the last look. Every command is taken, none answered: for a test
/// whose steps are pushes the window must not answer with a query of its own.
fn sent_a_query(fake: &mut FakeHelm) -> bool {
    fake.commands()
        .iter()
        .any(|command| command.path == SESSION_QUERY_PATH)
}

/// The old `filters.spec.ts` case "registry changes retain the selected host ID and rail entry". The choice is a host
/// id, never a name or a place in the rail, since the registry renames, reorders, drops and restores hosts under the
/// user: a choice that followed a position would narrow the list to another host, and one that followed a name would
/// lose the host the moment it was renamed. Each change is a push the helm makes on its own, so none of them is a
/// reason to send a query, and none may take focus. Specified, under the Linux click rule, over the local host and
/// remotes 2 and 3 with 2 chosen by a click (answered), which focuses it: a push renaming 2 "renamed host" while it
/// turns unreachable keeps it pressed and focused, named "renamed host (dev@build-box.invalid): Unreachable,
/// retrying"; a push reversing the order draws 3, 2, 1 with 2 still pressed and focused; with focus moved to All by
/// the keyboard, a push without 2 draws its pressed placeholder, and a push with 2 back as "restored host" draws its
/// live avatar pressed, focusable and clickable, named for its new name, the placeholder gone and focus still on All;
/// none of these pushes sends a query.
#[gpui::test]
fn a_registry_change_keeps_the_chosen_host_by_id_through_rename_reorder_and_return(
    cx: &mut TestAppContext,
) {
    const DESTINATION: &str = "dev@build-box.invalid";
    let local = status_host(1, "local", connected());
    let three = status_host(3, "ssh", connected());
    let (mut fake, cx) = status_rail(
        cx,
        &[
            local.clone(),
            status_host(2, "ssh", connected()),
            three.clone(),
        ],
        &[],
    );
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(true, cx));
    test_support::click(cx, "host-rail-host-2");
    let (query, _) = one_query(&mut fake);
    fake.reply(query.id, 204, None);
    test_support::settle(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-host-2"),
        "the premise: the click focused 2"
    );

    let renamed = aliased(
        status_host(2, "ssh", unreachable()),
        "renamed host",
        DESTINATION,
    );
    push_hosts(
        &mut fake,
        &[local.clone(), renamed.clone(), three.clone()],
        cx,
    );
    let nodes = frame(cx);
    assert!(pressed(&nodes, "host-rail-host-2"), "rename: still chosen");
    assert_eq!(
        label(&nodes, "host-rail-host-2"),
        "renamed host (dev@build-box.invalid): Unreachable, retrying"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-2"));

    push_hosts(&mut fake, &[three.clone(), renamed, local.clone()], cx);
    let nodes = frame(cx);
    assert_eq!(
        rail_entries(&nodes),
        [
            "host-rail-all",
            "host-rail-host-3",
            "host-rail-host-2",
            "host-rail-host-1",
            "host-rail-add"
        ]
    );
    assert!(pressed(&nodes, "host-rail-host-2"), "reorder: still chosen");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-2"));

    test_support::keys(cx, "shift-tab shift-tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-all"),
        "the premise: All has focus"
    );
    let left = status_hosts(&[three.clone(), local.clone()]);
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: left.meta,
        upsert: Vec::new(),
        remove: vec![HostView::key(2)],
    });
    test_support::settle(cx);
    assert!(
        pressed(&frame(cx), "host-rail-removed-2"),
        "removal: the placeholder is chosen"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-all"));

    let restored = aliased(
        status_host(2, "ssh", connected()),
        "restored host",
        DESTINATION,
    );
    push_hosts(&mut fake, &[three, restored, local], cx);
    let nodes = frame(cx);
    assert!(node(&nodes, "host-rail-removed-2").is_none());
    assert!(pressed(&nodes, "host-rail-host-2"), "return: chosen again");
    assert_eq!(
        label(&nodes, "host-rail-host-2"),
        "restored host (dev@build-box.invalid): Connected"
    );
    let avatar = node(&nodes, "host-rail-host-2").expect("2's avatar is drawn");
    assert!(avatar.flags.focusable && avatar.flags.click, "live again");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-all"));
    assert!(!sent_a_query(&mut fake), "no push sent a query");
}

/// The helm's channel forgets its query on a reset (a new channel starts unfiltered), so a window that kept its choice
/// would press a host while the helm serves every session (SPEC_impl.md, "One failure path: the reset"). The open
/// session survives the reset, so the one query the new channel gets only names it. Specified: with host 3 chosen and
/// answered, a broken channel and a new snapshot press All, draw no chip, and send exactly one query, `{"selected":
/// "a"}`, which chooses no host.
#[gpui::test]
fn a_reset_forgets_the_choice(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    test_support::click(cx, "host-rail-host-3");
    let (query, _) = one_query(&mut fake);
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    assert!(
        node(&frame(cx), "host-filter-chip").is_some(),
        "the premise: a chip"
    );

    fake.break_channel();
    cx.run_until_parked();
    fake.connect(snapshot());
    cx.run_until_parked();
    let nodes = frame(cx);
    assert!(pressed(&nodes, "host-rail-all"));
    assert!(!pressed(&nodes, "host-rail-host-3"));
    assert!(node(&nodes, "host-filter-chip").is_none());
    assert_eq!(
        sent_queries(&mut fake),
        [json!({ "selected": "a" })],
        "a reset chooses no host"
    );
}

/// SPEC.md, "Reaching everything by keyboard", and the old `filters.spec.ts` case "the rail keeps keyboard focus":
/// every rail entry is a tab stop in order, and Space and Enter choose, each key press once. Specified (Linux's rule,
/// where Tab reaches every control and a click focuses the control clicked): over an empty fleet, a click on All, the
/// choice already made, focuses it and sends nothing; Tab then focuses the local host's avatar, Shift+Tab All again
/// and Tab the avatar again, and Space sends exactly one query, `{"host": 1}`, and presses it, with focus staying on
/// it; Tab on to the next avatar and Enter send exactly one query, `{"host": 3}`. The walk starts from a click because
/// over an empty fleet focus rests on New, not on nothing (SPEC.md, "Where focus rests").
///
/// NOTE: A second run of the same choice sends nothing (a repeated choice is not sent), so "exactly one" here pins the
/// keys' wiring rather than catching a key press that ran the handler twice; the button's own tests cover that.
#[gpui::test]
fn tab_from_all_reaches_the_first_avatar_and_space_chooses_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, vec![listing(&[], &[]), hosts(&fleet_hosts())]);
    answer_everything(&mut fake);
    // The walk starts from a click, so it needs the rule under which a click focuses the control clicked.
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(true, cx));
    test_support::click(cx, "host-rail-all");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-all"));
    assert_eq!(
        queries(&mut fake).len(),
        0,
        "the click on the chosen All sent nothing"
    );
    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-1"));
    // The click could have focused a control Tab skips; Shift+Tab back shows All is a tab stop.
    test_support::keys(cx, "shift-tab");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-all"));
    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-1"));
    test_support::keys(cx, "space");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "host": 1 }));
    assert!(pressed(&frame(cx), "host-rail-host-1"));
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-1"));
    test_support::keys(cx, "tab");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-3"));
    test_support::keys(cx, "enter");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "host": 3 }));
}

// ---- The foot: the notices bell, Hosts and the Settings gear --------------------------------------------------------

/// The foot's controls' element ids (the old `.hosts-panel-toggle` and `.app-settings-toggle`), outside the
/// `host-rail-` ids so the entry lists above do not count them.
const HOSTS: &str = "hosts-panel-toggle";
const GEAR: &str = "app-settings-toggle";

/// The Help button's element id (the old `.app-help-toggle`), the foot's last control, below the gear; the help menu's
/// own tests cover the button and its menu.
const HELP: &str = "app-help-toggle";

/// The notices bell's element id (the old `.rail-notices-toggle`), the foot's first control, above Hosts.
const BELL: &str = crate::rail_notices::BELL_ID;

/// The id of the switch Settings focuses when it opens (its Sessions section's first switch, Skip session
/// confirmations), as the Settings dialog's own tests name it.
const FIRST_SWITCH: &str = "settings-skip-session-confirmations";

/// Whether the Hosts panel is drawn in place of the list's column: the panel's node is there and the list's is not.
/// Fails on any other combination, which would mean both or neither show.
pub(crate) fn panel_shown(cx: &mut VisualTestContext) -> bool {
    let nodes = frame(cx);
    let panel = node(&nodes, "hosts-panel").is_some();
    let list = node(&nodes, "session-list").is_some();
    assert_ne!(
        panel, list,
        "exactly one of the panel and the list is drawn"
    );
    panel
}

/// SPEC.md puts Hosts above Settings at the foot of the host rail ("The **Hosts** control above Settings and Help",
/// "The gear at the foot of the host rail opens a settings dialog"), and a control that drifted up with the entries or
/// lost its name would leave a keyboard or screen reader user without the way into host management or Settings. The
/// old foot's geometry: 40-pixel controls stacked with no gap in a 56-pixel rail, 7.5 pixels in from the left, the last
/// 8 pixels above the bottom. Specified: in a 1200 by 700 window, Hosts is a button named "Hosts", not pressed, whose
/// hover help is "Hosts: show host management", directly above the gear, a button named "Settings" whose hover help is
/// the old app's words without the updater clause; both are 40-pixel squares 7.5 pixels from the window's left edge,
/// the gear's bottom 88 pixels above the window's, leaving the 40-pixel Help button, the 40-pixel Expand control and the
/// foot's 8-pixel padding below it (the help menu's tests pin Help, and `expansion/tests.rs` the Expand control); the
/// notices bell, the same square at the same inset, sits directly above Hosts at the foot's top (SPEC.md: "at the top
/// of the navigation rail's footer, above Hosts"; the rail notices' own tests cover the bell and its panel); the rail's
/// scrolling part ends at the foot's top, the old foot's 208 pixels above the bottom, so every foot control sits wholly
/// below every entry.
#[gpui::test]
fn the_foot_holds_hosts_above_the_settings_gear_with_their_names_and_help(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    cx.simulate_resize(size(px(1200.), px(700.)));
    test_support::settle(cx);
    let nodes = frame(cx);
    assert_eq!(label(&nodes, HOSTS), "Hosts");
    assert!(!pressed(&nodes, HOSTS), "Sessions shows to begin with");
    assert_eq!(label(&nodes, GEAR), "Settings");
    for id in [HOSTS, GEAR] {
        let control = node(&nodes, id).unwrap();
        assert_eq!(control.role, Some(Role::Button), "{id}");
        let at = control.bounds;
        assert_eq!((at.size.width, at.size.height), (px(40.), px(40.)), "{id}");
        assert_eq!(at.left(), px(7.5), "{id}");
    }
    let (hosts, gear) = (
        node(&nodes, HOSTS).unwrap().bounds,
        node(&nodes, GEAR).unwrap().bounds,
    );
    assert_eq!(
        hosts.bottom(),
        gear.top(),
        "Hosts sits directly above the gear"
    );
    assert_eq!(gear.bottom(), px(700. - 88.));
    let bell = node(&nodes, BELL)
        .expect("the notices bell is drawn")
        .bounds;
    assert_eq!(
        (bell.size.width, bell.size.height, bell.left()),
        (px(40.), px(40.), px(7.5)),
        "the notices bell is the foot's first control"
    );
    assert_eq!(
        bell.bottom(),
        hosts.top(),
        "the bell sits directly above Hosts"
    );
    assert_eq!(
        test_support::bounds(cx, "host-rail").bottom(),
        bell.top(),
        "the entries scroll above the foot"
    );
    assert_eq!(hosts.top(), px(700. - 168.));
    assert_eq!(bell.top(), px(700. - 208.));
    assert_eq!(
        hover_help(cx, HOSTS).as_deref(),
        Some("Hosts: show host management")
    );
    assert_eq!(
        hover_help(cx, GEAR).as_deref(),
        Some("Settings: choices that apply to every host on this helm")
    );
}

/// SPEC.md: "The gear always opens sessions and focuses its first switch" and "Escape and either close control return
/// focus to the gear". The Settings dialog's own tests open it from a stand-in button; this pins the real gear's wiring
/// (its kept focus handle passed as the opener, its mark taken in the handler), under macOS's click rule, where the
/// click leaves the gear unfocused and only the explicit hand-over brings focus back to it, and from the keyboard
/// through the button's Enter. Specified: with focus away from the gear and a click that focuses no control, a click on
/// the gear opens Settings with focus on its first switch, and Escape closes it with focus on the gear; Enter on the
/// focused gear then opens it again the same way, and Escape again leaves focus on the gear.
#[gpui::test]
fn the_gear_opens_settings_at_its_first_switch_and_escape_returns_to_the_gear(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    assert_ne!(
        focused_id(cx).as_deref(),
        Some(GEAR),
        "the premise: focus is away from the gear"
    );

    test_support::click(cx, GEAR);
    assert!(any_open(cx), "the click opened Settings");
    assert_eq!(focused_id(cx).as_deref(), Some(FIRST_SWITCH));
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed Settings");
    assert_eq!(focused_id(cx).as_deref(), Some(GEAR), "after the click");

    test_support::keys(cx, "enter");
    assert!(any_open(cx), "Enter on the gear opened Settings");
    assert_eq!(focused_id(cx).as_deref(), Some(FIRST_SWITCH));
    test_support::keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some(GEAR), "after Enter");
}

/// SPEC.md: the Hosts control "swaps session content for complete host management", "keeps focus and exposes its
/// pressed ... state", and opening or closing it keeps the query and the details choice; the old app focused it
/// explicitly, so it keeps focus on macOS too, where a click focuses no button. Run under that rule for that reason.
/// While the panel shows, the rail's choice is still in force, so All still reports itself pressed (in the quieter
/// look, which the `buttons` golden pins). Specified: with a session open, a click on Hosts draws the panel in place of
/// the list's column, presses Hosts, leaves focus on it, sends nothing, and leaves All pressed; with Details turned on
/// in the panel, Enter on Hosts brings the list back with the same session open, unpresses Hosts and keeps focus on
/// it; a click on Hosts again shows the panel with Details still on.
#[gpui::test]
fn hosts_swaps_the_list_for_the_panel_and_back_keeping_focus(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    assert!(!panel_shown(cx), "the premise: Sessions shows");
    let open = crate::session_list::tests::selected(cx);
    assert!(open.is_some(), "the premise: a session is open");

    test_support::click(cx, HOSTS);
    assert!(panel_shown(cx), "the click showed the panel");
    let nodes = frame(cx);
    assert!(pressed(&nodes, HOSTS));
    assert!(
        pressed(&nodes, "host-rail-all"),
        "the rail's choice is still in force"
    );
    assert_eq!(focused_id(cx).as_deref(), Some(HOSTS), "after the click");
    assert_eq!(queries(&mut fake).len(), 0, "opening Hosts sends no query");

    test_support::click(cx, "host-details-toggle");
    assert_eq!(
        node(&frame(cx), "host-details-toggle").and_then(|node| node.toggled),
        Some(Toggled::True),
        "the premise: Details is on"
    );
    // Under this click rule the switch's click took no focus, so Hosts still holds it for the Enter below.
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(HOSTS),
        "the premise: Hosts has focus"
    );
    test_support::keys(cx, "enter");
    assert!(!panel_shown(cx), "Enter brought Sessions back");
    assert!(!pressed(&frame(cx), HOSTS));
    assert_eq!(
        crate::session_list::tests::selected(cx),
        open,
        "the open session stayed"
    );
    assert_eq!(focused_id(cx).as_deref(), Some(HOSTS), "after Enter");

    test_support::click(cx, HOSTS);
    assert!(panel_shown(cx));
    assert_eq!(
        node(&frame(cx), "host-details-toggle").and_then(|node| node.toggled),
        Some(Toggled::True),
        "the panel kept its Details choice"
    );
}

/// SPEC.md: "All or a host avatar returns to Sessions", and the old app closed the panel on every choice, also one
/// equal to the current choice, which sends no query. Specified: with the panel shown, a click on All (already chosen)
/// brings the list back and sends nothing; with the panel shown again, a click on host 3's avatar brings the list back
/// and sends exactly one query, `{"host": 3, "selected": "a"}`, with Hosts no longer pressed.
#[gpui::test]
fn choosing_all_or_a_host_returns_from_the_panel_to_sessions(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    test_support::click(cx, HOSTS);
    assert!(panel_shown(cx), "the premise: the panel shows");
    test_support::click(cx, "host-rail-all");
    assert!(!panel_shown(cx), "All returned to Sessions");
    assert_eq!(queries(&mut fake).len(), 0, "All was the choice already");

    test_support::click(cx, HOSTS);
    assert!(panel_shown(cx), "the premise: the panel shows again");
    test_support::click(cx, "host-rail-host-3");
    assert!(!panel_shown(cx), "an avatar returned to Sessions");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "host": 3, "selected": "a" }));
    assert!(!pressed(&frame(cx), HOSTS));
}

/// The old app drew its foot after the sidebar's content so keyboard navigation reached host choices, and the list
/// between them, before the foot; in GPUI, Tab follows the order controls are drawn in, which is why the list draws the
/// foot after its own column. A foot drawn inside the rail would put Hosts and the gear between the last avatar and the
/// list. Specified (Linux's rule, where Tab reaches every control): over an empty fleet with host 3 chosen by a click,
/// which leaves focus on its avatar and draws the heading's order title, Trash, Templates and New buttons, the status
/// segments and the chip as the list's controls, Tab goes to host 2's avatar, Add host, the order title, Trash,
/// Templates, New, the segments' chosen All (their one stop), the chip, the notices bell, Hosts, the gear, Help, then
/// the Expand control; Shift+Tab from it goes back to Help.
#[gpui::test]
fn tab_reaches_the_foot_after_the_rail_and_the_lists_controls(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, vec![listing(&[], &[]), hosts(&fleet_hosts())]);
    answer_everything(&mut fake);
    test_support::click(cx, "host-rail-host-3");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-host-3"),
        "the premise: the click focused the avatar"
    );
    assert!(
        node(&frame(cx), "host-filter-chip").is_some(),
        "the premise: the chip is drawn"
    );
    for expected in [
        "host-rail-host-2",
        ADD_HOST,
        "session-order",
        crate::trash::BUTTON_ID,
        "templates-button",
        crate::session_list::NEW_ID,
        "session-status-All",
        "host-filter-chip",
        BELL,
        HOSTS,
        GEAR,
        HELP,
        super::TOGGLE_ID,
    ] {
        test_support::keys(cx, "tab");
        assert_eq!(focused_id(cx).as_deref(), Some(expected));
    }
    test_support::keys(cx, "shift-tab");
    assert_eq!(focused_id(cx).as_deref(), Some(HELP));
}

/// The old spec "rail scrolling leaves settings and help reachable": a long registry makes the rail scroll, and the
/// foot must stay where it is whatever the rail's scroll position, with no entry scrolling under it. Specified: in a
/// 1200 by 500 window over 30 hosts, the last avatar starts below the rail's visible part; after a wheel over the rail
/// scrolls it to its end, the last avatar lies wholly inside the rail's visible part, above Hosts, and neither foot
/// control has moved, the gear's bottom still 88 pixels above the window's and Help's 48 pixels above it.
#[gpui::test]
fn a_long_host_list_scrolls_above_the_foot_which_stays_put(cx: &mut TestAppContext) {
    let many: Vec<Host> = (10..40)
        .map(|id| remote(id, &format!("ops@host-{id}.example.invalid")))
        .collect();
    let (_fake, cx) = open_main(cx, vec![listing(&[], &[]), hosts(&many)]);
    cx.simulate_resize(size(px(1200.), px(500.)));
    test_support::settle(cx);
    let rail = test_support::bounds(cx, "host-rail");
    let foot = (
        test_support::bounds(cx, HOSTS),
        test_support::bounds(cx, GEAR),
    );
    assert!(
        test_support::bounds(cx, "host-rail-host-39").top() > rail.bottom(),
        "the premise: the last host is below the rail's fold"
    );

    let at = rail.center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.), px(-5000.))),
        ..Default::default()
    });
    test_support::settle(cx);
    let last = test_support::bounds(cx, "host-rail-host-39");
    assert!(
        last.top() >= rail.top() && last.bottom() <= rail.bottom(),
        "the last host {last:?} shows inside the rail {rail:?}"
    );
    assert!(
        last.bottom() <= foot.0.top(),
        "and above Hosts {:?}",
        foot.0
    );
    assert_eq!(
        (
            test_support::bounds(cx, HOSTS),
            test_support::bounds(cx, GEAR)
        ),
        foot,
        "the foot did not move"
    );
    assert_eq!(foot.1.bottom(), px(500. - 88.));
    assert_eq!(
        test_support::bounds(cx, HELP).bottom(),
        px(500. - 48.),
        "Help stays at the foot's bottom"
    );
}

// ---- Add host, and the keyboard below the rail's fold ---------------------------------------------------------------

/// Add host's element id (the old `.host-rail-add`), one of the `host-rail-` ids, so [`rail_entries`] counts it among
/// the rail's entries.
const ADD_HOST: &str = "host-rail-add";

/// Whether the Add host dialog is open: the dialog its own tests reach through [`layers::last_opened`] is alive, and
/// its frame (drawn under the dialog's id, `add-host`) is in a fresh frame.
fn add_host_dialog_shown(cx: &mut VisualTestContext) -> bool {
    let alive =
        cx.update(|_, cx| layers::last_opened::<crate::hosts::add::AddHostDialog>(cx).is_some());
    alive && node(&frame(cx), "add-host").is_some()
}

/// SPEC.md's rail ends with **Add host**, after every registered host, and the button is the one way the app offers to
/// add a host from the sidebar, so a screen reader user needs its name and a sighted one its help, both the old app's
/// words. Specified: over the usual fleet, Add host is the rail's last entry, drawn below the last avatar; it is an
/// enabled, focusable button named "Add host" that reports no pressed state (it is not a choice), a 40-pixel rail
/// square in the rail's column; and its hover help is "Add host: connect a machine or create a Tensorlake sandbox".
#[gpui::test]
fn add_host_is_the_rails_last_entry_with_its_name_help_and_role(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    let nodes = frame(cx);
    assert_eq!(
        rail_entries(&nodes).last().map(String::as_str),
        Some(ADD_HOST)
    );
    let add = node(&nodes, ADD_HOST).expect("Add host is drawn");
    assert_eq!(add.role, Some(Role::Button));
    assert_eq!(add.label.as_deref(), Some("Add host"));
    assert_eq!(add.toggled, None, "not a choice");
    assert!(add.flags.focusable && add.flags.click, "enabled");
    assert_eq!(
        (add.bounds.size.width, add.bounds.size.height),
        (px(40.), px(40.))
    );
    assert_eq!(add.bounds.left(), px(7.), "in the rail's column");
    let last_avatar = node(&nodes, "host-rail-host-2").unwrap().bounds;
    assert!(
        add.bounds.top() > last_avatar.bottom(),
        "below the last avatar: {:?} after {last_avatar:?}",
        add.bounds
    );
    assert_eq!(
        hover_help(cx, ADD_HOST).as_deref(),
        Some("Add host: connect a machine or create a Tensorlake sandbox")
    );
}

/// The Add host dialog's own tests open it from a stand-in button; this pins the real rail button's wiring: its kept
/// focus handle passed as the opener and its mark taken in the handler. SPEC.md: "Closing something the user opened
/// returns focus to where it was before it opened, on every platform", which under macOS's click rule (a click focuses
/// no button) only the explicit hand-over to the opener achieves, so the test runs under that rule. Specified: with
/// focus away from Add host, a click on it opens the Add host dialog and sends nothing (no probe before Add), and
/// Escape closes it with focus on Add host; Enter on the focused Add host opens it again, and Escape again leaves focus
/// on Add host.
#[gpui::test]
fn add_host_opens_the_dialog_and_escape_returns_focus_to_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    assert_ne!(
        focused_id(cx).as_deref(),
        Some(ADD_HOST),
        "the premise: focus is away from Add host"
    );

    test_support::click(cx, ADD_HOST);
    assert!(
        add_host_dialog_shown(cx),
        "the click opened the Add host dialog"
    );
    assert_eq!(fake.commands(), [], "opening it sent nothing");
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed the dialog");
    assert_eq!(focused_id(cx).as_deref(), Some(ADD_HOST), "after the click");

    test_support::keys(cx, "enter");
    assert!(
        add_host_dialog_shown(cx),
        "Enter on Add host opened the dialog"
    );
    test_support::keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some(ADD_HOST), "after Enter");
}

/// SPEC.md, "Reaching everything by keyboard", means little if Tab reaches a rail entry the user cannot see, and a long
/// registry pushes the last hosts and Add host below the rail's fold (the rail holds about seven hosts at the minimum
/// window height). The old page scrolled a focused entry into view; here the rail is a focus coordinator scroll region.
/// Specified (Linux's rule, where Tab reaches every control): in a 1200 by 500 window over an empty fleet and 30 hosts,
/// where the last avatar and Add host start below the rail's visible part, a click focuses All (over an empty fleet
/// focus otherwise rests on New), 30 Tabs then focus the last avatar, which then lies wholly inside the rail's visible
/// part, and one more focuses Add host, which then does too; the rail's visible box itself stays where it was.
#[gpui::test]
fn tab_scrolls_the_last_avatar_and_add_host_into_view_below_the_fold(cx: &mut TestAppContext) {
    let many: Vec<Host> = (10..40)
        .map(|id| remote(id, &format!("ops@host-{id}.example.invalid")))
        .collect();
    let (mut fake, cx) = open_main(cx, vec![listing(&[], &[]), hosts(&many)]);
    answer_everything(&mut fake);
    cx.simulate_resize(size(px(1200.), px(500.)));
    test_support::settle(cx);
    let rail = test_support::bounds(cx, "host-rail");
    for below in ["host-rail-host-39", ADD_HOST] {
        assert!(
            test_support::bounds(cx, below).top() >= rail.bottom(),
            "the premise: {below} starts below the rail's fold"
        );
    }
    // Inside the rail's visible part, whole.
    let shown = |cx: &mut VisualTestContext, id: &'static str| {
        let at = test_support::bounds(cx, id);
        at.top() >= rail.top() && at.bottom() <= rail.bottom()
    };

    // The walk starts from a click, so it needs the rule under which a click focuses the control clicked.
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(true, cx));
    test_support::click(cx, "host-rail-all");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-all"));
    // One press at a time, each followed by its frame, as a user tabs.
    for _ in 10..40 {
        test_support::keys(cx, "tab");
        test_support::settle(cx);
    }
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-host-39"),
        "the premise: Tab reached the last avatar"
    );
    assert!(
        shown(cx, "host-rail-host-39"),
        "the last avatar {:?} shows inside the rail {rail:?}",
        test_support::bounds(cx, "host-rail-host-39")
    );

    test_support::keys(cx, "tab");
    test_support::settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some(ADD_HOST));
    assert!(
        shown(cx, ADD_HOST),
        "Add host {:?} shows inside the rail {rail:?}",
        test_support::bounds(cx, ADD_HOST)
    );
    assert_eq!(
        test_support::bounds(cx, "host-rail"),
        rail,
        "the rail itself did not move"
    );
}

// ---- Names as pure functions ---------------------------------------------------------------------------------------

/// The identity rule the avatar and the chip share (the old `rail_identity`): only the local host under the registry's
/// own name reads "Local (this machine)"; an alias stays the user's word; a remote host's destination is added only
/// when it differs from its name. Specified over the local host, an aliased local host, an aliased remote, an unaliased
/// remote, and a remote that happens to be named like the local sentinel.
#[test]
fn identity_names_local_and_adds_a_destination_only_when_it_differs() {
    let name = |host: Host| identity(&host.view()).to_string();
    assert_eq!(name(local()), "Local (this machine)");
    assert_eq!(name(local().alias("desk")), "desk");
    assert_eq!(
        name(remote(2, "ops@build.example").alias("Build farm")),
        "Build farm (ops@build.example)"
    );
    assert_eq!(name(remote(2, "ops@build.example")), "ops@build.example");
    let mut posing = remote(2, "ops@build.example");
    posing.name = LOCAL_NAME.to_owned();
    assert_eq!(name(posing), "this machine (ops@build.example)");
}

// ---- Layout, with the real text system -----------------------------------------------------------------------------

/// The old sidebar's 396 pixels held the 56-pixel rail beside the session list, so the port's list was 56 pixels too
/// wide until the rail came; and a caption is cut at the avatar's 40 pixels rather than widening the rail (the old
/// `host-short-names.spec.ts` case "short host captions ... truncate inside the collapsed rail"). Only the real text
/// system measures a caption, so this lays the main view out with the bundled fonts in a headless app, with no
/// renderer, which layout does not need. Specified: the rail is 56 pixels wide from the sidebar's left edge; the list's
/// column starts where the rail ends and ends at the sidebar's 1-pixel edge (395); the local caption paints "Local"
/// whole; the long alias's caption is 40 pixels wide and paints a cut name ending in "…" that fits in it.
#[test]
fn the_rail_takes_its_56_pixels_of_the_sidebar_and_cuts_long_captions() {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    fake.connect(snapshot());
    cx.run_until_parked();
    let window = cx
        .open_window(size(px(1000.), px(600.)), |window, cx| {
            let main = cx.new(|cx| MainView::new(window, cx));
            cx.new(|cx| Root::new(main, window, cx))
        })
        .unwrap();
    // Two frames: the list asks for a second one to give its rows their height estimates.
    let nodes = cx
        .update_window(window.into(), |_, window, cx| {
            window.set_driver_enabled(true);
            window.draw(cx).clear(cx);
            window.draw(cx).clear(cx);
            window.driver_nodes().to_vec()
        })
        .unwrap();
    let bounds = |id: &str| match node(&nodes, id) {
        Some(node) => node.bounds,
        None => panic!("{id} was not drawn"),
    };
    let rail = bounds("host-rail");
    assert_eq!((rail.left(), rail.size.width), (px(0.), px(56.)));
    let list = bounds("session-list");
    assert_eq!((list.left(), list.right()), (px(56.), px(395.)));

    assert_eq!(
        texts_within(&nodes, "host-rail-caption-1")
            .first()
            .map(|text| text.text.as_ref()),
        Some("Local")
    );
    let long = node(&nodes, "host-rail-caption-3").unwrap();
    assert_eq!(long.bounds.size.width, px(40.));
    let long_caption = texts_within(&nodes, "host-rail-caption-3");
    let cut = long_caption.first().expect("the long caption painted text");
    assert!(cut.text.ends_with('…'), "cut short: {:?}", cut.text);
    assert!(
        cut.bounds.size.width <= px(40.),
        "fits the caption: {:?}",
        cut.bounds
    );
}

// ---- Host status ---------------------------------------------------------------------------------------------------

/// A connected remote on build `0.15.0`, older than the helm's, as the helm marks it: an optional update. The host
/// status golden uses it too.
pub(crate) fn old_build() -> HostStateView {
    HostStateView::Connected {
        identity: Some("id-old".into()),
        build_version: "0.15.0".into(),
        old_version: true,
        newer_version: false,
        refresh: RefreshView::Ok { sessions: 1 },
    }
}

/// A connected host on a build newer than the helm's.
fn newer_build() -> HostStateView {
    HostStateView::Connected {
        identity: Some("id-new".into()),
        build_version: "9.0.0".into(),
        old_version: false,
        newer_version: true,
        refresh: RefreshView::Ok { sessions: 0 },
    }
}

/// A remote the helm cannot reach and keeps re-probing. The host status golden uses it too.
pub(crate) fn unreachable() -> HostStateView {
    HostStateView::Unreachable {
        cause: "transport-failure".into(),
        last_error: "timed out".into(),
    }
}

/// The main view over `listed` (the registry, in that order) with `runs` retained and no sessions, every command sent
/// at startup answered (the connected hosts' trash reads), so that each test sees only what its own steps change.
fn status_rail<'a>(
    cx: &'a mut TestAppContext,
    listed: &[HostView],
    runs: &[ProvisioningView],
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut fake, cx) = open_main(
        cx,
        vec![
            listing(&[], &[]),
            status_hosts(listed),
            status_progress(runs),
        ],
    );
    answer_everything(&mut fake);
    test_support::settle(cx);
    (fake, cx)
}

/// Push `listed` as the whole registry, replacing every host, and settle.
fn push_hosts(fake: &mut FakeHelm, listed: &[HostView], cx: &mut VisualTestContext) {
    let state = status_hosts(listed);
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// Whether the element recorded under the debug `selector` (a rail mark, see `status::avatar_marks`) was drawn.
fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// Why this matters: SPEC.md, "Session list": "Per-host connection state is always visible on each rail avatar, with
/// its name and phase available on hover and to screen readers", and "an unreachable host's identity mark is dimmed".
/// The dot is a picture; the phase's words in the avatar's name are what a screen reader hears. Specified: over a
/// connected local host, a connecting remote and an unreachable one, each avatar draws its connection dot and no other
/// mark, and is named and helped by its identity, ": " and its phase words ("Connected", "Connecting", "Unreachable,
/// retrying"); the dimming rule holds the unreachable, suspended and changing phases and no other.
#[gpui::test]
fn each_avatar_draws_its_dot_and_names_its_phase(cx: &mut TestAppContext) {
    let listed = [
        status_host(1, "local", connected()),
        status_host(2, "ssh", connecting(2, None)),
        status_host(3, "ssh", unreachable()),
    ];
    let (_fake, cx) = status_rail(cx, &listed, &[]);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-rail-host-1"),
        "Local (this machine): Connected"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-2"),
        "dev@build-box.invalid: Connecting"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-3"),
        "dev@build-box.invalid: Unreachable, retrying"
    );
    for (dot, others) in [
        (
            "host-rail-dot-1",
            [
                "host-rail-update-1",
                "host-rail-ring-1",
                "host-rail-warning-1",
            ],
        ),
        (
            "host-rail-dot-2",
            [
                "host-rail-update-2",
                "host-rail-ring-2",
                "host-rail-warning-2",
            ],
        ),
        (
            "host-rail-dot-3",
            [
                "host-rail-update-3",
                "host-rail-ring-3",
                "host-rail-warning-3",
            ],
        ),
    ] {
        assert!(drawn(cx, dot), "{dot} is drawn");
        for other in others {
            assert!(!drawn(cx, other), "{other} is not drawn");
        }
    }
    assert_eq!(
        hover_help(cx, "host-rail-host-3").as_deref(),
        Some("dev@build-box.invalid: Unreachable, retrying")
    );

    let suspended = HostStateView::Suspended {
        observed_at: 1,
        suspended_by_farhelm_at: None,
        resume_failed: None,
    };
    let changing = HostStateView::TensorlakeChanging {
        starting: false,
        direction: farhelm_proto::tensorlake::LifecycleDirection::Resuming,
        requested_at: 1,
    };
    for (state, dims) in [
        (unreachable(), true),
        (suspended, true),
        (changing, true),
        (connected(), false),
        (connecting(1, None), false),
        (old_build(), false),
    ] {
        assert_eq!(
            status::dimmed(&status_host(2, "ssh", state.clone())),
            dims,
            "{state:?}"
        );
    }
}

/// Why this matters: SPEC.md, "Session list": "an available update adds a badge", and "The Hosts control at the foot of
/// the rail carries an `↑` whenever any rail avatar has an available-update badge ... its accessible name and hover
/// help give the number of hosts". The badge follows `hosts::status::update_badge`, so the rail, Hosts' count and the
/// panel's inline Update agree; the helm's own machine and a host running a newer Farhelm never get one. Specified:
/// over the local host on an older build, a remote on an older build, a remote refused for a lower protocol and a
/// remote on a newer build, the two behind remotes draw the update badge and say "; update available" after their
/// phase ("Old version", "Needs update"), the local host says only "Old version", the newer remote says "Too new" and
/// the helm's remedy and has no badge; Hosts is named "Hosts: 2 hosts have updates", helped "Hosts: 2 hosts have
/// updates; show host management", and draws its `↑`.
#[gpui::test]
fn behind_remote_hosts_show_the_update_badge_and_hosts_counts_them(cx: &mut TestAppContext) {
    let listed = [
        status_host(1, "local", old_build()),
        status_host(2, "ssh", old_build()),
        status_host(3, "ssh", skew(28, 29)),
        status_host(4, "ssh", newer_build()),
    ];
    let (_fake, cx) = status_rail(cx, &listed, &[]);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-rail-host-1"),
        "Local (this machine): Old version"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-2"),
        "dev@build-box.invalid: Old version; update available"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-3"),
        "dev@build-box.invalid: Needs update; update available"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-4"),
        format!(
            "dev@build-box.invalid: Too new; This host runs Farhelm 9.0.0, which is newer than this helm's {} (both \
             speak protocol {}); update the helm",
            farhelm_proto::BUILD_VERSION,
            farhelm_proto::PROTOCOL_VERSION,
        )
    );
    for (badge, shown) in [
        ("host-rail-update-1", false),
        ("host-rail-update-2", true),
        ("host-rail-update-3", true),
        ("host-rail-update-4", false),
    ] {
        assert_eq!(drawn(cx, badge), shown, "{badge}");
    }
    assert_eq!(
        label(&nodes, "hosts-panel-toggle"),
        "Hosts: 2 hosts have updates"
    );
    assert!(drawn(cx, "hosts-panel-toggle-update"));
    assert_eq!(
        hover_help(cx, "hosts-panel-toggle").as_deref(),
        Some("Hosts: 2 hosts have updates; show host management")
    );
}

/// Why this matters: the Hosts control's `↑` and count follow the badges as hosts change, and with none it is the
/// plain "Hosts" again (the old `toggle_label`), not an empty count. Specified: with one remote on an older build Hosts
/// is named "Hosts: 1 host has an update" and draws its `↑`; once the helm pushes that host as current, Hosts is named
/// "Hosts", helped "Hosts: show host management", and draws no `↑`, and the avatar no badge.
#[gpui::test]
fn hosts_names_one_update_and_drops_its_arrow_when_none_is_left(cx: &mut TestAppContext) {
    let (mut fake, cx) = status_rail(cx, &[status_host(2, "ssh", old_build())], &[]);
    assert_eq!(
        label(&frame(cx), "hosts-panel-toggle"),
        "Hosts: 1 host has an update"
    );
    assert!(drawn(cx, "hosts-panel-toggle-update"));

    push_hosts(&mut fake, &[status_host(2, "ssh", connected())], cx);
    assert_eq!(label(&frame(cx), "hosts-panel-toggle"), "Hosts");
    assert!(!drawn(cx, "hosts-panel-toggle-update"));
    assert!(!drawn(cx, "host-rail-update-2"));
    assert_eq!(
        hover_help(cx, "hosts-panel-toggle").as_deref(),
        Some("Hosts: show host management")
    );
}

/// Why this matters: SPEC.md, "Session list": "setup/update/uninstall adds a ring", and the old rail named the work in
/// place of "update available", since a host being updated is not still waiting for one. Specified: with an Update run
/// going on on a remote on an older build, a setup run on a current remote and an uninstall run on a third, each
/// avatar draws the ring, the first draws no update badge, and they are named "...: Old version; updating", "...:
/// Connected; setting up" and "...: Connected; uninstalling"; Hosts counts no update.
#[gpui::test]
fn running_work_rings_the_avatar_and_names_the_work(cx: &mut TestAppContext) {
    use farhelm_proto::provisioning::ProvisioningOperation::{Add, Uninstall, Update};
    use farhelm_proto::provisioning::RunStatus::Running;
    let listed = [
        status_host(2, "ssh", old_build()),
        status_host(3, "ssh", connected()),
        status_host(4, "ssh", connected()),
    ];
    let runs = [
        status_run(2, Update, Running),
        status_run(3, Add, Running),
        status_run(4, Uninstall, Running),
    ];
    let (_fake, cx) = status_rail(cx, &listed, &runs);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-rail-host-2"),
        "dev@build-box.invalid: Old version; updating"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-3"),
        "dev@build-box.invalid: Connected; setting up"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-4"),
        "dev@build-box.invalid: Connected; uninstalling"
    );
    for ring in ["host-rail-ring-2", "host-rail-ring-3", "host-rail-ring-4"] {
        assert!(drawn(cx, ring), "{ring} is drawn");
    }
    assert!(!drawn(cx, "host-rail-update-2"));
    assert_eq!(label(&nodes, "hosts-panel-toggle"), "Hosts");
}

/// Whether `nodes` draw the element `id` inside the element `ancestor`, by the frame's own parent links.
fn inside(nodes: &[DriverNode], id: &str, ancestor: &str) -> bool {
    let Some(mut at) = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some(id))
    else {
        return false;
    };
    while let Some(parent) = nodes[at].parent {
        if nodes[parent].element_id.as_deref() == Some(ancestor) {
            return true;
        }
        at = parent;
    }
    false
}

/// Push `runs` as changed host progress, each under its host's id, and settle.
fn push_runs(fake: &mut FakeHelm, runs: &[ProvisioningView], cx: &mut VisualTestContext) {
    let state = status_progress(runs);
    fake.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// The rail half of the old `provisioning.spec.ts` case "accepted ADD registers before execution and releases the page
/// lock while running". The helm registers an added host before its setup runs, so the rail must take the new avatar
/// and its ring from the pushes alone, and drop the ring when the run completes while Hosts is closed (SPEC.md:
/// "setup/update/uninstall adds a ring"; the panel follows the progress whether or not it shows). Those pushes redraw
/// the rail under any menu open on it, and must leave another host's menu where it is, open, with the keyboard in it.
/// The static ring is `running_work_rings_the_avatar_and_names_the_work`'s; the Hosts panel's own row is the
/// settings-and-hosts area's. Specified, with Sessions shown over the local host and a connected remote 2: a pushed
/// registration of 3 with its Add run running draws 3's avatar with its ring, named "...: Connected; setting up"; a
/// right press on 2's avatar opens 2's menu with focus on one of its items; the run's completion pushed then takes 3's
/// ring away and names it "...: Connected", while 2's menu stays open under the same name with focus where it was, and
/// Hosts never opened.
///
/// The old case opened the menu with Shift+F10 on an avatar it had focused without choosing its host. A right press
/// opens the same menu without choosing either, where focusing the avatar here would take a click that chooses the
/// host and narrows the list; the gestures themselves are `avatar_menu_tests.rs`'s, Shift+F10 included.
#[gpui::test]
fn a_registered_hosts_ring_goes_with_its_run_and_leaves_another_avatars_menu_open(
    cx: &mut TestAppContext,
) {
    use farhelm_proto::provisioning::ProvisioningOperation::Add;
    use farhelm_proto::provisioning::RunStatus::{Completed, Running};
    let local = status_host(1, "local", connected());
    // Named apart from 3, which takes the fixtures' destination, so the menu's name tells their menus apart.
    let two = aliased(
        status_host(2, "ssh", connected()),
        "ops box",
        "ops@box.invalid",
    );
    let (mut fake, cx) = status_rail(cx, &[local.clone(), two.clone()], &[]);
    assert!(!panel_shown(cx), "the premise: Sessions shows");

    push_hosts(
        &mut fake,
        &[local, two, status_host(3, "ssh", connected())],
        cx,
    );
    push_runs(&mut fake, &[status_run(3, Add, Running)], cx);
    assert!(drawn(cx, "host-rail-ring-3"), "the new avatar is ringed");
    assert_eq!(
        label(&frame(cx), "host-rail-host-3"),
        "dev@build-box.invalid: Connected; setting up"
    );

    let avatar = test_support::center(cx, "host-rail-host-2");
    cx.simulate_mouse_down(avatar, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(avatar, MouseButton::Right, Modifiers::none());
    test_support::settle(cx);
    let menu = label(&frame(cx), "host-menu-items");
    assert!(menu.contains("ops box"), "the premise: 2's menu: {menu}");
    let focused = focused_id(cx);
    assert!(
        focused
            .as_deref()
            .is_some_and(|id| inside(&frame(cx), id, "host-menu-items")),
        "the premise: focus is on one of the menu's items: {focused:?}"
    );

    push_runs(&mut fake, &[status_run(3, Add, Completed)], cx);
    let nodes = frame(cx);
    assert!(!drawn(cx, "host-rail-ring-3"), "the ring went with the run");
    assert_eq!(
        label(&nodes, "host-rail-host-3"),
        "dev@build-box.invalid: Connected"
    );
    assert_eq!(label(&nodes, "host-menu-items"), menu, "2's menu is open");
    assert_eq!(focused_id(cx), focused, "focus stayed in the menu");
    assert!(!panel_shown(cx), "Hosts never opened");
}

/// Why this matters: SPEC.md, "Session list": "A setup or Update warning has its own mark and hover explanation,
/// alongside the connection dot", and opening Hosts acknowledges it. The rule of when a warning shows is
/// `hosts::status`'s and tested there; this is the rail drawing it. Specified: a remote whose Update run failed while
/// Hosts was closed draws the warning mark beside its dot and is named "...: Connected; setup or update needs
/// attention; open Hosts for details"; after Hosts is opened and closed again, the mark is gone and the name is
/// "...: Connected".
#[gpui::test]
fn a_failed_update_marks_the_avatar_until_hosts_opens(cx: &mut TestAppContext) {
    use farhelm_proto::provisioning::ProvisioningOperation::Update;
    use farhelm_proto::provisioning::RunStatus::Failed;
    let (_fake, cx) = status_rail(
        cx,
        &[status_host(2, "ssh", connected())],
        &[status_run(2, Update, Failed)],
    );
    assert!(drawn(cx, "host-rail-warning-2"));
    assert!(drawn(cx, "host-rail-dot-2"));
    assert_eq!(
        label(&frame(cx), "host-rail-host-2"),
        "dev@build-box.invalid: Connected; setup or update needs attention; open Hosts for details"
    );

    test_support::click(cx, "hosts-panel-toggle");
    test_support::click(cx, "hosts-panel-toggle");
    assert!(!panel_shown(cx), "the premise: Sessions shows again");
    assert!(!drawn(cx, "host-rail-warning-2"));
    assert_eq!(
        label(&frame(cx), "host-rail-host-2"),
        "dev@build-box.invalid: Connected"
    );
}

/// Why this matters: SPEC.md, "Session list": "A failed registry refresh retains the avatars and names their state as
/// last-read" (the old app's tooltip suffix, and the `filters.spec.ts` case whose assertion this ports), so an avatar
/// never presents an old phase as current. Specified: once the helm reports the host list unavailable while keeping
/// the hosts last read, the avatar is still drawn and is named "...: Connected; last state read, host list refresh
/// failed"; once the list is available again, the suffix goes.
#[gpui::test]
fn a_failed_registry_refresh_names_the_avatars_state_as_last_read(cx: &mut TestAppContext) {
    let listed = [status_host(2, "ssh", connected())];
    let (mut fake, cx) = status_rail(cx, &listed, &[]);
    let mut failed = status_hosts(&listed);
    failed.meta.availability = Availability::Unavailable {
        error: "the registry could not be read".into(),
    };
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: failed.meta,
        upsert: Vec::new(),
        remove: Vec::new(),
    });
    test_support::settle(cx);
    assert_eq!(
        label(&frame(cx), "host-rail-host-2"),
        "dev@build-box.invalid: Connected; last state read, host list refresh failed"
    );

    push_hosts(&mut fake, &listed, cx);
    assert_eq!(
        label(&frame(cx), "host-rail-host-2"),
        "dev@build-box.invalid: Connected"
    );
}

/// Why this matters: the old tooltip put each explanation in a fixed order after the phase, and a reader scanning the
/// name relies on the phase coming first. Specified as a pure function: for a remote on an older build with an update
/// available, after a failed refresh and with a warning, the words are "Old version; update available; last state
/// read, host list refresh failed; setup or update needs attention; open Hosts for details".
#[test]
fn state_words_keep_the_old_tooltips_order() {
    use crate::hosts::status::{HostStatus, Urgency};
    let host = status_host(2, "ssh", old_build());
    let words = status::state_words(
        &host,
        &HostStatus {
            update: Some(Urgency::Optional),
            live: None,
            warned: true,
        },
        true,
    );
    assert_eq!(
        words,
        "Old version; update available; last state read, host list refresh failed; setup or update needs \
         attention; open Hosts for details"
    );
}

/// Why this matters: a warning is raised outside the store (a refused Update's completion records it in the hosts
/// status module's own state), so nothing the list reads while drawing changes, and only the list's subscription to
/// that state redraws the rail; without it the mark would wait for some unrelated redraw. Specified: with the window
/// settled and no frame asked for, recording a refused Update for host 2 through `hosts::status::failed` and letting
/// the app run until idle draws host 2's warning mark.
#[gpui::test]
fn a_warning_raised_outside_the_store_redraws_the_rail(cx: &mut TestAppContext) {
    let (_fake, cx) = status_rail(cx, &[status_host(2, "ssh", connected())], &[]);
    assert!(
        !drawn(cx, "host-rail-warning-2"),
        "the premise: no warning yet"
    );
    cx.update(|_, cx| crate::hosts::status::failed(2, crate::hosts::status::Action::Update, cx));
    test_support::settle(cx);
    assert!(drawn(cx, "host-rail-warning-2"));
}

/// Why this matters: the ring has three looks (SPEC.md: "setup/update/uninstall adds a ring"; the old sheet drew it in
/// the accent, in the inverse ink on the pressed avatar, and in the accent again on the pressed avatar while Hosts
/// shows), and the Hosts control's `↑` is red only when some update is required ("It is red when any update is
/// required, amber otherwise"). Inks are not in the driver's record, so these rules are tested as the functions that
/// pick them. Specified, over both palettes: an avatar not pressed rings in the accent, a pressed one in the inverse
/// ink, and a pressed one while Hosts shows in the accent; Hosts' `↑` is amber for optional updates only and red as
/// soon as one is required.
#[test]
fn the_rings_and_the_hosts_arrows_inks_follow_the_old_sheet() {
    use crate::theme::{DARK, LIGHT};
    for palette in [&DARK, &LIGHT] {
        assert_eq!(status::ring_ink(false, false, palette), palette.accent);
        assert_eq!(status::ring_ink(false, true, palette), palette.accent);
        assert_eq!(status::ring_ink(true, false, palette), palette.on_fill);
        assert_eq!(status::ring_ink(true, true, palette), palette.accent);

        let mut optional = status::Updates::default();
        optional.add(Some(crate::hosts::status::Urgency::Optional));
        optional.add(Some(crate::hosts::status::Urgency::Optional));
        assert_eq!(status::hosts_badge_ink(optional, palette), palette.warn);
        let mut required = optional;
        required.add(Some(crate::hosts::status::Urgency::Required));
        assert_eq!(
            status::hosts_badge_ink(required, palette),
            palette.danger_strong
        );
        assert_eq!(required.count, 3);
    }
}

/// A Tensorlake sandbox host `id`, connected, whose forwards failed when `tunnels_failed` and whose credentials were
/// not delivered when `credentials_failed`.
fn troubled_sandbox(id: i64, tunnels_failed: bool, credentials_failed: bool) -> HostView {
    use farhelm_proto::tensorlake::{TunnelState, TunnelView};
    let mut view = status_host(id, "tensorlake", connected());
    view.tensorlake = Some(farhelm_proto::ui_channel::TensorlakeHostView {
        sandbox: "farhelm-host-k7q2".into(),
        recipe: "rust-dev".into(),
        size: None,
        tunnels: Some(TunnelView {
            tunnels: Vec::new(),
            state: if tunnels_failed {
                TunnelState::Failed {
                    reason: "listen port taken".into(),
                }
            } else {
                TunnelState::Up
            },
        }),
        credential_warning: credentials_failed.then(|| "the vault refused".to_owned()),
    });
    view
}

/// Why this matters: the old rail added a machine's failing credentials or forwards to the avatar's name, right after
/// the phase's words and before the too-new explanation, since a screen reader hears only the name, and the amber dot
/// alone says nothing. Specified as a pure function: a connected sandbox whose forwards failed reads "Connected;
/// tunnels failed, retrying"; one whose credentials were not delivered reads "Connected; credentials not delivered";
/// both read "Connected; credentials not delivered; tunnels failed, retrying"; a sandbox with neither reads
/// "Connected".
#[test]
fn failing_credentials_and_forwards_join_the_avatars_words_after_its_phase() {
    use crate::hosts::status::HostStatus;
    let quiet = HostStatus {
        update: None,
        live: None,
        warned: false,
    };
    for (tunnels, credentials, words) in [
        (true, false, "Connected; tunnels failed, retrying"),
        (false, true, "Connected; credentials not delivered"),
        (
            true,
            true,
            "Connected; credentials not delivered; tunnels failed, retrying",
        ),
        (false, false, "Connected"),
    ] {
        assert_eq!(
            status::state_words(&troubled_sandbox(4, tunnels, credentials), &quiet, false),
            words
        );
    }
}

/// Why this matters: the old spec checked the rail's state class turn amber for failed forwards ("tunnel failures make
/// a connected host amber"); the dot is the only thing that shows it at a glance, so the ink actually painted is what
/// is tested, not the function that picks it. Specified: the dot of a connected sandbox whose forwards failed paints
/// the palette's amber (`warn`) and not its green (`ok`); with its forwards up it paints green.
#[gpui::test]
fn failed_forwards_paint_the_rail_dot_amber(cx: &mut TestAppContext) {
    let listed = [troubled_sandbox(4, true, false), {
        let mut fine = troubled_sandbox(5, false, false);
        fine.name = "fine".into();
        fine
    }];
    let (_fake, cx) = status_rail(cx, &listed, &[]);
    let palette = cx.update(|_, cx| crate::theme::palette(cx));
    let inks = |cx: &mut VisualTestContext, selector: &'static str| {
        let dot = cx.debug_bounds(selector).expect("the dot is drawn");
        let (scale, quads) = cx.update(|window, _| (window.scale_factor(), window.painted_quads()));
        let dot = dot.scale(scale);
        quads
            .into_iter()
            .filter(|quad| {
                quad.bounds.origin.x >= dot.origin.x
                    && quad.bounds.origin.y >= dot.origin.y
                    && quad.bounds.bottom_right().x <= dot.bottom_right().x
                    && quad.bounds.bottom_right().y <= dot.bottom_right().y
            })
            .map(|quad| quad.background)
            .collect::<Vec<_>>()
    };
    let (amber, green) = (
        gpui::Background::from(crate::theme::color(palette.warn)),
        gpui::Background::from(crate::theme::color(palette.ok)),
    );
    let failed = inks(cx, "host-rail-dot-4");
    assert!(
        failed.contains(&amber) && !failed.contains(&green),
        "the failed forwards' dot paints amber, not green"
    );
    let fine = inks(cx, "host-rail-dot-5");
    assert!(
        fine.contains(&green) && !fine.contains(&amber),
        "the working forwards' dot paints green"
    );
}
