//! Tests of the host rail: its entries and their names, the session query a choice sends and how the helm's answer
//! settles it, the chip, the empty state of a host with no sessions, the placeholder of a removed host, the status of a
//! host list never read, a reset, the keyboard, and the rail's width in the sidebar.
//!
//! Like the list's other tests, these run the real main view over the helm client's fake, which hands the test every
//! command the window sent, and read controls from GPUI's driver record of the frame by their ids (`host-rail-all`,
//! `host-rail-host-<id>`, `host-filter-chip`). The rules that are pure functions (the choice's bookkeeping, a host's
//! identity, its tint) are tested as such. The one test whose outcome is layout (the rail's width, the list beside it
//! and a caption cut short) uses the real text system, since the test context's fake one gives every character the
//! same width.

use std::sync::Arc;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostView,
    KeyedEntry, ListCounts, SESSION_QUERY_PATH,
};
use gpui::{
    AppContext as _, DriverNode, DriverText, HeadlessAppContext, Modifiers, Role, TestAppContext,
    Toggled, VisualTestContext, px, size,
};
use gpui_component::Root;
use serde_json::{Value, json};

use super::{Choice, LOCAL_NAME, empty_host, identity, tint};
use crate::host_text::HostText;
use crate::host_text::tests::SPOOFING_CORPUS;
use crate::main_view::MainView;
use crate::session_list::tests::{change, listing, open_main, painted, preferences, session};
use crate::test_support::{self, focused_id, frame, meta};
use crate::{assets, bootstrap, layers, theme};

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
fn fleet_hosts() -> Vec<Host> {
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
fn snapshot() -> Vec<CollectionState> {
    vec![
        listing(&["a", "b"], &[session("a"), session("b")]),
        preferences(&["a"], None),
        hosts(&fleet_hosts()),
    ]
}

/// The sessions collection's metadata for a host query that matched `matching` of `total` sessions, complete.
fn filtered(order: &[&str], total: u64, matching: u64) -> CollectionMeta {
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
fn answer_everything(fake: &mut FakeHelm) {
    for command in fake.commands() {
        fake.reply(command.id, 200, None);
    }
}

/// Every command the window sent since the last look, each checked to be a session query, as (its command, its body).
fn queries(fake: &mut FakeHelm) -> Vec<(FakeCommand, Value)> {
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

/// The one query the window sent since the last look, with its body; fails on none or several.
fn one_query(fake: &mut FakeHelm) -> (FakeCommand, Value) {
    let mut sent = queries(fake);
    assert_eq!(sent.len(), 1, "one query: {sent:?}");
    sent.remove(0)
}

/// The node drawn under `id` in `nodes`.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
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
fn texts_within(nodes: &[DriverNode], id: &str) -> Vec<DriverText> {
    within(nodes, id)
        .into_iter()
        .flat_map(|node| node.texts.iter().cloned())
        .collect()
}

/// The accessible name of the control drawn under `id`; fails if nothing was drawn under it.
fn label(nodes: &[DriverNode], id: &str) -> String {
    match node(nodes, id).and_then(|node| node.label.as_deref()) {
        Some(label) => label.to_owned(),
        None => panic!("{id} was not drawn with a name"),
    }
}

/// Whether the rail entry drawn under `id` is pressed, as assistive technology hears it.
fn pressed(nodes: &[DriverNode], id: &str) -> bool {
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
fn hover_help(cx: &mut VisualTestContext, selector: &'static str) -> Option<String> {
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
/// registry order and nothing for the container; All is named "All hosts" and pressed, the avatars are not pressed and
/// are named "Local (this machine)", the alias with its destination in parentheses, and the bare destination (which
/// is also its name, so no suffix); each avatar's caption is a node with no role (hidden from assistive technology),
/// and the local caption paints "Local".
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
            "host-rail-host-2"
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
/// list could not be loaded" with the status role, and no avatar.
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
            ["host-rail-all", "host-rail-unavailable"]
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
/// and the old app sent nothing when the choice did not change. The query names only the host, so status, sort and the
/// open session stay as they are. Specified: a click on host 3's avatar sends exactly one `PUT /ui/session-query` with
/// `{"host": 3}` and presses that avatar at once, before any reply, with All no longer pressed and the open session
/// unchanged; a second click sends nothing; after the reply, a click on All sends `{}` and presses All.
#[gpui::test]
fn choosing_a_host_sends_its_query_once_and_all_sends_an_empty_one(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    let open = crate::session_list::tests::selected(cx);

    test_support::click(cx, "host-rail-host-3");
    let (query, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "host": 3 }));
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
    assert_eq!(body, json!({}));
    assert!(pressed(&frame(cx), "host-rail-all"));
}

/// The helm validates a query by serving it, and a refused one leaves its channel on the query before it, so the
/// window must not go on showing a choice the helm is not serving; and SPEC.md wants a refusal reported, not swallowed.
/// The words are the helm's, which may quote a host, so they are host text. Specified: with host 3 chosen and
/// accepted, choosing host 2 and having it refused with words holding a direction override presses host 3 again,
/// keeps the chip on host 3, and paints the words escaped after "Could not show that host's sessions: "; choosing All
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
    assert_eq!(body, json!({ "host": 2 }), "the premise: host 2's query");
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
        label(&nodes, "host-filter-refusal"),
        "Could not show that host's sessions: unknown host <U+202E>2"
    );

    test_support::click(cx, "host-rail-all");
    assert!(node(&frame(cx), "host-filter-refusal").is_none());
}

/// Replies come in the order queries were sent, and the helm serves the last query it accepted; a refusal of a query
/// the user already replaced must not undo the newer choice. The choice's bookkeeping, as a pure function. Specified:
/// choosing the current choice sends nothing; a refusal of a superseded query changes nothing; an accepted query
/// becomes the answered one; a refusal of the latest puts the choice back to the answered one with the words.
#[test]
fn a_refusal_undoes_only_the_latest_choice() {
    let mut choice = Choice::default();
    assert_eq!(choice.choose(None), None, "All is the choice to begin with");
    let first = choice.choose(Some(3)).unwrap();
    let second = choice.choose(Some(2)).unwrap();
    choice.settle(first, Some(3), Err(HostText::from_host("no")));
    assert_eq!((choice.chosen(), choice.answered()), (Some(2), None));
    assert!(
        choice.refusal().is_none(),
        "a superseded refusal is not shown"
    );
    choice.settle(second, Some(2), Ok(()));
    assert_eq!((choice.chosen(), choice.answered()), (Some(2), Some(2)));
    let third = choice.choose(Some(3)).unwrap();
    choice.settle(third, Some(3), Err(HostText::from_host("no")));
    assert_eq!((choice.chosen(), choice.answered()), (Some(2), Some(2)));
    assert_eq!(choice.refusal(), Some(&HostText::from_host("no")));
    assert!(choice.choose(Some(3)).is_some());
    assert!(choice.refusal().is_none(), "a new choice drops the words");
}

// ---- The chip ------------------------------------------------------------------------------------------------------

/// SPEC.md: "A chosen host appears beside the status segments as a removable chip with its mark and short name,
/// matching the rail", with the name and destination as its tooltip, and removing it chooses All "exactly as the rail
/// does". Specified: with no host chosen there is no chip; once host 3 is chosen, the chip is named "Filtered to host
/// <alias>; remove filter", its hover help is the host's identity, and it paints the short name and ✕; a click on it
/// sends `{}`, presses All and takes the chip away.
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
    assert_eq!(body, json!({}));
    let nodes = frame(cx);
    assert!(pressed(&nodes, "host-rail-all"));
    assert!(node(&nodes, "host-filter-chip").is_none());
}

/// Removing the filter takes away the control the user just used (the chip goes with the filter), and SPEC.md sends
/// focus to the rail's All then ("After a clear button or the chip removes its focused control, focus returns to the
/// rail's All button"), where the coordinator's ordinary rule would send it home to the terminal. Specified: a click
/// on the chip leaves focus on All, whatever a click focuses on this platform; and by keyboard alone, from All, Tab
/// twice and Space choose host 3, Tab twice more reaches the chip, and Enter on it removes the filter and leaves focus
/// on All. Each removal sends `{}`.
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
    assert_eq!(body, json!({}));
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
        json!({ "host": 3 }),
        "the premise: host 3 chosen by keyboard"
    );
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    test_support::keys(cx, "tab tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-filter-chip"),
        "the premise: the chip has focus"
    );
    test_support::keys(cx, "enter");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({}));
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
/// host 2; "Show all hosts" sends `{}` and leaves focus on All, and the full listing with its reply draws rows and no
/// sentence.
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
        label(&nodes, "host-filter-empty"),
        "No sessions on Build fleet with a deliberately long caption."
    );
    assert_eq!(label(&nodes, "host-filter-show-all"), "Show all hosts");

    test_support::click(cx, "host-rail-host-2");
    let (held, _) = one_query(&mut fake);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-filter-empty"),
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
        label(&frame(cx), "host-filter-empty"),
        "No sessions on render-west."
    );

    test_support::click(cx, "host-filter-show-all");
    let (all, body) = one_query(&mut fake);
    assert_eq!(body, json!({}));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-all"),
        "focus moves to All in place of the link"
    );
    fake.push(change(listing(&["a", "b"], &rows).meta, &rows, &[]));
    fake.reply(all.id, 204, None);
    cx.run_until_parked();
    let nodes = frame(cx);
    assert!(node(&nodes, "host-filter-empty").is_none());
    assert!(node(&nodes, "session-row-a").is_some(), "the rows are back");
}

/// The empty state's evidence rule as a pure function: only an available, complete listing that matched nothing for
/// an answered host may be named; a cut listing keeps the count line's warning instead, and with no host answered
/// there is nothing to name. Specified over the four conditions, each one alone failing it.
#[test]
fn only_a_complete_empty_answer_for_a_host_is_named() {
    let complete = filtered(&[], 5, 0);
    assert_eq!(empty_host(&complete, 0, Some(3)), Some(3));
    assert_eq!(empty_host(&complete, 0, None), None, "no host answered");
    assert_eq!(empty_host(&complete, 1, Some(3)), None, "rows on screen");
    let mut cut = filtered(&[], 5, 0);
    cut.counts = Some(ListCounts {
        total: 5,
        matching: Some(0),
        truncated: true,
    });
    assert_eq!(empty_host(&cut, 0, Some(3)), None, "a cut listing");
    let mut failed = filtered(&[], 5, 0);
    failed.availability = Availability::Unavailable {
        error: "down".into(),
    };
    assert_eq!(empty_host(&failed, 0, Some(3)), None, "a failed read");
}

// ---- A removed host, a reset, the keyboard -------------------------------------------------------------------------

/// SPEC.md: "a removed selected host remains explicitly selected and unavailable until another host or All is
/// chosen", which the old app drew as a pressed, disabled "?" in the rail and a "no longer registered" chip, so the
/// user sees why the list is narrowed and how to undo it. Specified: with host 3 chosen and answered, a registry change
/// without it draws `host-rail-removed-3` pressed, not focusable and with no click handler, named "Host 3 (no longer
/// registered)", and the chip named "Filtered to host Host 3 (no longer registered); remove filter"; a click on the
/// placeholder sends nothing; the chip's click sends `{}` and takes the placeholder away.
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
    assert_eq!(body, json!({}));
    assert!(node(&frame(cx), "host-rail-removed-3").is_none());
}

/// The helm's channel forgets its query on a reset (a new channel starts unfiltered), so a window that kept its choice
/// would press a host while the helm serves every session (SPEC_impl.md, "One failure path: the reset"). Specified:
/// with host 3 chosen and answered, a broken channel and a new snapshot press All, draw no chip, and send no query.
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
    assert!(
        fake.commands()
            .iter()
            .all(|command| command.path != SESSION_QUERY_PATH),
        "a reset sends no query"
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

// ---- Names and marks as pure functions -----------------------------------------------------------------------------

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

/// A host's color names a palette token by its place in the color list (SPEC.md: host colors identify hosts), and the
/// default color is the ordinary foreground. A tint written per color must stay in step with that list. Specified: for
/// every color but the default, the tint is the palette's host entry at its place in `HostColor::ALL` less one, in both
/// appearances.
#[test]
fn each_host_color_tints_with_its_palette_entry() {
    for palette in [&theme::DARK, &theme::LIGHT] {
        assert_eq!(tint(HostColor::Default, palette), palette.fg_0);
        for (place, color) in HostColor::ALL.iter().enumerate().skip(1) {
            assert_eq!(tint(*color, palette), palette.host[place - 1], "{color:?}");
        }
    }
}

/// Every shape a user can pick draws its own bundled drawing, so no two hosts' shapes collapse into one and none is
/// missing. Specified: the sixteen `HostIcon`s map to sixteen distinct bundled SVGs.
#[test]
fn every_host_shape_has_its_own_bundled_drawing() {
    let paths: std::collections::HashSet<&str> = HostIcon::ALL
        .iter()
        .map(|icon| super::glyph(*icon).path())
        .collect();
    assert_eq!(paths.len(), HostIcon::ALL.len());
    for path in paths {
        assert!(
            gpui::AssetSource::load(&assets::Assets, path)
                .unwrap()
                .is_some(),
            "{path} is bundled"
        );
    }
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
