//! Tests of the host rail: its entries and their names, the session query a choice sends and how the helm's answer
//! settles it, the chip, the empty sentence of a host with no sessions, the placeholder of a removed host, the status
//! of a host list never read, a reset, the keyboard, Add host and the keyboard's reach below the rail's fold, the
//! foot's Hosts control and Settings gear, and the rail's width in the sidebar. The query's status dimension, and the
//! query's bookkeeping as pure functions, are tested in `filters/tests.rs`, which shares these fixtures.
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
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostView,
    KeyedEntry, ListCounts, SESSION_QUERY_PATH,
};
use gpui::{
    AppContext as _, DriverNode, DriverText, HeadlessAppContext, Modifiers, Role, ScrollDelta,
    ScrollEvent, TestAppContext, Toggled, VisualTestContext, point, px, size,
};
use gpui_component::Root;
use serde_json::{Value, json};

use super::identity;
use crate::host_text::tests::SPOOFING_CORPUS;
use crate::hosts::add::tests::Opened;
use crate::hosts::identity::LOCAL_NAME;
use crate::main_view::MainView;
use crate::session_list::tests::{change, listing, open_main, painted, preferences, session};
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

/// The rail's entries that the frame drew, top to bottom, by id.
fn rail_entries(nodes: &[DriverNode]) -> Vec<String> {
    let mut entries: Vec<_> = nodes
        .iter()
        .filter(|node| node.role.is_some())
        .filter_map(|node| {
            let id = node.element_id.as_deref()?;
            (id.starts_with("host-rail-") && !id.starts_with("host-rail-caption"))
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
/// destination (which is also its name, so no suffix); each avatar's caption is a node with no role (hidden from
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
    assert_eq!(label(&nodes, "host-rail-host-1"), "Local (this machine)");
    assert_eq!(
        label(&nodes, "host-rail-host-3"),
        "Build fleet with a deliberately long caption (deploy@builder-east.example.invalid)"
    );
    assert_eq!(
        label(&nodes, "host-rail-host-2"),
        "ops@render-west.example.invalid"
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

/// The avatar's hover help is the same identity as its name, so a sighted user learns which host a mark stands for
/// (the state the old tooltip also carried joins it with the host status). Specified: resting the pointer on the
/// aliased host's avatar shows its alias with its destination in parentheses, and on All shows All's own help.
#[gpui::test]
fn an_avatars_hover_help_is_its_identity(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot());
    assert_eq!(
        hover_help(cx, "host-rail-host-3").as_deref(),
        Some("Build fleet with a deliberately long caption (deploy@builder-east.example.invalid)")
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
            format!("{shown} (ops@example.invalid)")
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
    fake.push(change(filtered(&[], 2, 0), &[], &["a", "b"]));
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
/// where Tab reaches every control): over an empty fleet, where nothing else holds focus, Tab focuses All, the next Tab
/// the local host's avatar, and Space then sends exactly one query, `{"host": 1}`, and presses it, with focus staying
/// on it; Tab on to the next avatar and Enter send exactly one query, `{"host": 3}`.
///
/// NOTE: A second run of the same choice sends nothing (a repeated choice is not sent), so "exactly one" here pins the
/// keys' wiring rather than catching a key press that ran the handler twice; the button's own tests cover that.
#[gpui::test]
fn tab_from_all_reaches_the_first_avatar_and_space_chooses_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, vec![listing(&[], &[]), hosts(&fleet_hosts())]);
    answer_everything(&mut fake);
    test_support::keys(cx, "tab");
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

// ---- The foot: Hosts and the Settings gear --------------------------------------------------------------------------

/// The foot's controls' element ids (the old `.hosts-panel-toggle` and `.app-settings-toggle`), outside the
/// `host-rail-` ids so the entry lists above do not count them.
const HOSTS: &str = "hosts-panel-toggle";
const GEAR: &str = "app-settings-toggle";

/// The Help button's element id (the old `.app-help-toggle`), the foot's last control, below the gear; the help menu's
/// own tests cover the button and its menu.
const HELP: &str = "app-help-toggle";

/// The id of the switch Settings focuses when it opens (its Sessions section's first switch, Skip session
/// confirmations), as the Settings dialog's own tests name it.
const FIRST_SWITCH: &str = "settings-skip-session-confirmations";

/// Whether the Hosts panel is drawn in place of the list's column: the panel's node is there and the list's is not.
/// Fails on any other combination, which would mean both or neither show.
fn panel_shown(cx: &mut VisualTestContext) -> bool {
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
/// the gear's bottom 48 pixels above the window's, leaving the 40-pixel Help button and the foot's 8-pixel padding
/// below it (the help menu's tests pin that button); the rail's scrolling part ends at the foot's top, 128 pixels
/// above the bottom, so both sit wholly below every entry.
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
    assert_eq!(gear.bottom(), px(700. - 48.));
    assert_eq!(
        test_support::bounds(cx, "host-rail").bottom(),
        hosts.top(),
        "the entries scroll above the foot"
    );
    assert_eq!(hosts.top(), px(700. - 128.));
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
/// Templates, New, the segments' chosen All (their one stop), the chip, Hosts, the gear, then Help; Shift+Tab from Help
/// goes back to the gear.
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
        HOSTS,
        GEAR,
        HELP,
    ] {
        test_support::keys(cx, "tab");
        assert_eq!(focused_id(cx).as_deref(), Some(expected));
    }
    test_support::keys(cx, "shift-tab");
    assert_eq!(focused_id(cx).as_deref(), Some(GEAR));
}

/// The old spec "rail scrolling leaves settings and help reachable": a long registry makes the rail scroll, and the
/// foot must stay where it is whatever the rail's scroll position, with no entry scrolling under it. Specified: in a
/// 1200 by 500 window over 30 hosts, the last avatar starts below the rail's visible part; after a wheel over the rail
/// scrolls it to its end, the last avatar lies wholly inside the rail's visible part, above Hosts, and neither foot
/// control has moved, the gear's bottom still 48 pixels above the window's and Help's 8 pixels above it.
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
    assert_eq!(foot.1.bottom(), px(500. - 48.));
    assert_eq!(
        test_support::bounds(cx, HELP).bottom(),
        px(500. - 8.),
        "Help stays at the foot's bottom"
    );
}

// ---- Add host, and the keyboard below the rail's fold ---------------------------------------------------------------

/// Add host's element id (the old `.host-rail-add`), one of the `host-rail-` ids, so [`rail_entries`] counts it among
/// the rail's entries.
const ADD_HOST: &str = "host-rail-add";

/// Whether the Add host dialog is open: the dialog its own tests reach through [`Opened`] is alive, and its frame
/// (drawn under the dialog's id, `add-host`) is in a fresh frame.
fn add_host_dialog_shown(cx: &mut VisualTestContext) -> bool {
    let alive = cx.update(|_, cx| {
        cx.try_global::<Opened>()
            .is_some_and(|opened| opened.0.upgrade().is_some())
    });
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
/// where the last avatar and Add host start below the rail's visible part, Tab focuses All, 30 more Tabs focus the
/// last avatar, which then lies wholly inside the rail's visible part, and one more focuses Add host, which then does
/// too; the rail's visible box itself stays where it was.
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

    test_support::keys(cx, "tab");
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
