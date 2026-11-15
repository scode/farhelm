//! Tests of the list's query: the status segments and the query they send with the rail's host, the empty answer's
//! sentence and links, the heading and segments before the first listing, the selection left alone by a filter, and
//! the query's bookkeeping, refusal wording, empty-answer rule and switcher-pick rule as pure functions.
//!
//! Like the rail's tests, these run the real main view over the helm client's fake, which hands the test every command
//! the window sent, and read controls from GPUI's driver record of the frame by their ids: the segments are
//! `session-status-<label>`, the sentence `filter-empty`, its links `filter-show-all-hosts` and
//! `filter-show-all-statuses`. They share the rail's fixtures (`host_rail/tests.rs`): sessions a and b on host 2, a
//! open, over the local host, the long-named host 3 and host 2. The radio group's own keys (one tab stop, the arrows
//! moving and choosing with wrap, Space) are pinned by `components/tests.rs`
//! (`the_arrows_move_a_radio_groups_choice_and_focus_wrapping_past_unavailable_options`,
//! `tab_enters_a_radio_group_at_its_choice_and_leaves_it_in_one_press`,
//! `space_chooses_a_focused_radio_and_enter_goes_on_to_the_owner`); here only the old status spec's one arrow case
//! is checked, for the query it sends.

use farhelm_helm_client::CommandError;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{Availability, CollectionMeta, ListCounts, Reply, SessionRow};
use gpui::{Role, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::{CUT_SENTENCE, Choice, Empty, Query, Status, empty, pick_hidden, refusal_sentence};
use crate::host_text::HostText;
use crate::operations;
use crate::session_list::host_rail::tests::{
    answer_everything, filtered, fleet_hosts, hosts, hover_help, label, node, one_query, pressed,
    queries, snapshot,
};
use crate::session_list::tests::{
    change, collections, listing, open_main, painted, preferences, selected, session,
};
use crate::test_support::{click, focused_id, frame, keys};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The long-named host 3's sentence form: the short name the rail's fixture gives it.
const HOST_3: &str = "Build fleet with a deliberately long caption";

/// Accept every query the window sent since the last look, after pushing the empty answer the helm would serve for the
/// last of them (0 of 2 sessions matched, complete); `gone` are the rows the answer takes away.
fn answer_empty(fake: &mut FakeHelm, cx: &mut VisualTestContext, gone: &[&str]) {
    let sent = queries(fake);
    fake.push(change(filtered(&[], 2, 0), &[], gone));
    for (command, _) in sent {
        fake.reply(command.id, 204, None);
    }
    cx.run_until_parked();
}

/// Take the one query the window sent since the last look, check that its body is `body`, and accept it.
fn accept_one(fake: &mut FakeHelm, cx: &mut VisualTestContext, body: Value) {
    let (query, sent) = one_query(fake);
    assert_eq!(sent, body);
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
}

/// The ids of the empty sentence's links the frame drew, left to right.
fn links(cx: &mut VisualTestContext) -> Vec<&'static str> {
    let nodes = frame(cx);
    let mut drawn: Vec<(f32, &'static str)> = ["filter-show-all-hosts", "filter-show-all-statuses"]
        .into_iter()
        .filter_map(|id| node(&nodes, id).map(|node| (f32::from(node.bounds.left()), id)))
        .collect();
    drawn.sort_by(|a, b| a.0.total_cmp(&b.0));
    drawn.into_iter().map(|(_, id)| id).collect()
}

/// A row as the switcher's read lists it: session `key` on `host`, in the status named by `state`.
fn row(key: &'static str, host: i64, state: &str) -> SessionRow {
    session(key)
        .host(host)
        .status(json!({ "state": state }))
        .row()
}

// ---- The segments --------------------------------------------------------------------------------------------------

/// The segments are a native radio group in the old app (`role="radiogroup"`, "Session status"), and their words and
/// hover help are how a user learns that Idle leaves out ended sessions while All keeps them. Specified: the group is
/// drawn as a radio group named "Session status" with All checked; its four options, in order, are radio buttons named
/// All, Running, Waiting and Idle, each with the old hover help; with host 3 chosen, the chip is drawn after the
/// segments in their row (to their right on the same line, or below them).
#[gpui::test]
fn the_segments_are_a_radio_group_with_the_old_words_and_the_chip_after_them(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    let nodes = frame(cx);
    let group = node(&nodes, "session-status").expect("the segments are drawn");
    assert_eq!(group.role, Some(Role::RadioGroup));
    assert_eq!(group.label.as_deref(), Some("Session status"));
    assert!(pressed(&nodes, "session-status-All"));
    let mut lefts = Vec::new();
    for (id, name, help) in [
        (
            "session-status-All",
            "All",
            "Show every session, including ended sessions",
        ),
        (
            "session-status-Running",
            "Running",
            "Show sessions whose agents are working",
        ),
        (
            "session-status-Waiting",
            "Waiting",
            "Show sessions that need your attention",
        ),
        (
            "session-status-Idle",
            "Idle",
            "Show live sessions whose agents are at rest",
        ),
    ] {
        let nodes = frame(cx);
        let option = node(&nodes, id).unwrap();
        assert_eq!(option.role, Some(Role::RadioButton), "{id}");
        assert_eq!(option.label.as_deref(), Some(name));
        lefts.push(option.bounds.left());
        assert_eq!(hover_help(cx, id).as_deref(), Some(help));
    }
    assert!(
        lefts.windows(2).all(|pair| pair[0] < pair[1]),
        "in order: {lefts:?}"
    );

    click(cx, "host-rail-host-3");
    let nodes = frame(cx);
    let (segments, chip) = (
        node(&nodes, "session-status").unwrap().bounds,
        node(&nodes, "host-filter-chip").unwrap().bounds,
    );
    assert!(
        chip.left() >= segments.right() || chip.top() >= segments.bottom(),
        "the chip {chip:?} comes after the segments {segments:?}"
    );
}

/// The helm replaces its whole query on every send (SPEC_impl.md, "The UI channel"), so a status sent without the host
/// would silently widen the list to every host, and the other way round; and SPEC.md wants the query sent at once and
/// nothing sent for the choice already in force. Specified: a click on Running sends `{"status": "running"}` and checks
/// it before any reply; a second click sends nothing; host 3 then sends `{"host": 3, "status": "running"}`; Waiting and
/// Idle each send host 3 with their word; Left on the focused Idle chooses Waiting and sends it (the old status spec's
/// arrow case); All sends `{"host": 3}`; with Waiting chosen again, the chip sends `{"status": "waiting"}`. The open
/// session stays open throughout.
#[gpui::test]
fn every_query_carries_both_the_host_and_the_status(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    let open = selected(cx);
    assert!(open.is_some(), "the premise: a session is open");

    click(cx, "session-status-Running");
    let (query, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "status": "running" }));
    let nodes = frame(cx);
    assert!(
        pressed(&nodes, "session-status-Running"),
        "checked before the reply"
    );
    assert!(!pressed(&nodes, "session-status-All"));
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    click(cx, "session-status-Running");
    assert_eq!(queries(&mut fake).len(), 0, "the same choice sends nothing");

    click(cx, "host-rail-host-3");
    accept_one(&mut fake, cx, json!({ "host": 3, "status": "running" }));
    click(cx, "session-status-Waiting");
    accept_one(&mut fake, cx, json!({ "host": 3, "status": "waiting" }));
    click(cx, "session-status-Idle");
    accept_one(&mut fake, cx, json!({ "host": 3, "status": "idle" }));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-status-Idle"),
        "the premise: the click focused Idle"
    );
    keys(cx, "left");
    accept_one(&mut fake, cx, json!({ "host": 3, "status": "waiting" }));
    click(cx, "session-status-All");
    accept_one(&mut fake, cx, json!({ "host": 3 }));
    click(cx, "session-status-Waiting");
    accept_one(&mut fake, cx, json!({ "host": 3, "status": "waiting" }));
    click(cx, "host-filter-chip");
    accept_one(&mut fake, cx, json!({ "status": "waiting" }));
    assert!(pressed(&frame(cx), "session-status-Waiting"));
    assert_eq!(selected(cx), open);
}

// ---- The empty answer ----------------------------------------------------------------------------------------------

/// SPEC.md: "An empty filtered list names its filters", in the old sentences, and offers a link for each filter it
/// names, Show all hosts before Show all statuses; this is the old app's full table of complete empty answers. The
/// empty fleet's own "No sessions" (All and All) is `an_empty_fleet_says_so_in_the_list_and_the_pane`'s. Specified, for
/// each status and host chosen in turn and answered with an empty, complete listing: the sentence's accessible text and
/// the links drawn, left to right; and the links' hover help.
#[gpui::test]
fn every_empty_answer_names_its_filters_with_a_link_per_filter(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    let both = vec!["filter-show-all-hosts", "filter-show-all-statuses"];
    let cases: [(&str, &str, String, Vec<&str>); 7] = [
        (
            "host-rail-all",
            "session-status-Running",
            "No running sessions.".into(),
            vec!["filter-show-all-statuses"],
        ),
        (
            "host-rail-all",
            "session-status-Waiting",
            "No waiting sessions.".into(),
            vec!["filter-show-all-statuses"],
        ),
        (
            "host-rail-all",
            "session-status-Idle",
            "No idle sessions.".into(),
            vec!["filter-show-all-statuses"],
        ),
        (
            "host-rail-host-3",
            "session-status-All",
            format!("No sessions on {HOST_3}."),
            vec!["filter-show-all-hosts"],
        ),
        (
            "host-rail-host-3",
            "session-status-Running",
            format!("No running sessions on {HOST_3}."),
            both.clone(),
        ),
        (
            "host-rail-host-3",
            "session-status-Waiting",
            format!("No waiting sessions on {HOST_3}."),
            both.clone(),
        ),
        (
            "host-rail-host-3",
            "session-status-Idle",
            format!("No idle sessions on {HOST_3}."),
            both,
        ),
    ];
    let mut gone: &[&str] = &["a", "b"];
    for (host, status, sentence, drawn) in cases {
        click(cx, host);
        click(cx, status);
        answer_empty(&mut fake, cx, gone);
        gone = &[];
        assert_eq!(
            label(&frame(cx), "filter-empty"),
            sentence,
            "{host} {status}"
        );
        assert_eq!(links(cx), drawn, "{host} {status}");
    }
    assert_eq!(
        hover_help(cx, "filter-show-all-hosts").as_deref(),
        Some("Show all hosts: remove the host filter")
    );
    assert_eq!(
        hover_help(cx, "filter-show-all-statuses").as_deref(),
        Some("Show all statuses: remove the status filter")
    );
}

/// Each link clears only its own filter (the old status spec: Show all statuses keeps the host, Show all hosts keeps
/// the status), and both take away the control the user just used, so focus moves to the rail's All (SPEC.md); the
/// sentence keeps describing the answered query until the helm answers the new one. Specified: with host 3 and Waiting
/// answered empty, Show all statuses sends `{"host": 3}`, leaves focus on All, checks All at once and keeps "No waiting
/// sessions on <host 3>." until its empty answer, which reads "No sessions on <host 3>."; with Waiting chosen again and
/// answered empty, Enter on Show all hosts (reached by Tab from Waiting, past the chip) sends `{"status": "waiting"}`,
/// leaves focus on All, takes the chip away, and its empty answer reads "No waiting sessions."
#[gpui::test]
fn each_link_clears_its_own_filter_and_focuses_all(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    click(cx, "host-rail-host-3");
    click(cx, "session-status-Waiting");
    answer_empty(&mut fake, cx, &["a", "b"]);
    assert_eq!(
        label(&frame(cx), "filter-empty"),
        format!("No waiting sessions on {HOST_3}."),
        "the premise: host 3 and Waiting answered empty"
    );

    click(cx, "filter-show-all-statuses");
    let (held, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "host": 3 }));
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-all"));
    let nodes = frame(cx);
    assert!(
        pressed(&nodes, "session-status-All"),
        "the choice moved at once"
    );
    assert_eq!(
        label(&nodes, "filter-empty"),
        format!("No waiting sessions on {HOST_3}."),
        "the sentence names the answered query until the reply"
    );
    fake.push(change(filtered(&[], 2, 0), &[], &[]));
    fake.reply(held.id, 204, None);
    cx.run_until_parked();
    assert_eq!(
        label(&frame(cx), "filter-empty"),
        format!("No sessions on {HOST_3}.")
    );

    click(cx, "session-status-Waiting");
    answer_empty(&mut fake, cx, &[]);
    // From the clicked Waiting, past the chip, to the first link (Linux's rule, where Tab reaches every control).
    keys(cx, "tab tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("filter-show-all-hosts"),
        "the premise: Show all hosts has focus"
    );
    keys(cx, "enter");
    let (cleared, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "status": "waiting" }));
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-all"));
    assert!(node(&frame(cx), "host-filter-chip").is_none());
    fake.push(change(filtered(&[], 2, 0), &[], &[]));
    fake.reply(cleared.id, 204, None);
    cx.run_until_parked();
    assert_eq!(label(&frame(cx), "filter-empty"), "No waiting sessions.");
}

/// SPEC.md: "a truncated listing keeps its qualified no-match message": a listing the helm's cap cut was not read to
/// the end, so it cannot claim that no session matches. Specified: Waiting answered with a cut, empty listing (0 of 2
/// matched, truncated) shows the sentence "No matches among the sessions that could be read — the list could not be
/// read to the end" and no link, beside the count line's own warning.
#[gpui::test]
fn a_cut_empty_answer_says_only_what_could_be_read(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot());
    answer_everything(&mut fake);
    click(cx, "session-status-Waiting");
    let (query, _) = one_query(&mut fake);
    let mut cut = filtered(&[], 2, 0);
    cut.counts = Some(ListCounts {
        total: 2,
        matching: Some(0),
        truncated: true,
    });
    fake.push(change(cut, &[], &["a", "b"]));
    fake.reply(query.id, 204, None);
    cx.run_until_parked();
    assert_eq!(
        label(&frame(cx), "filter-empty"),
        "No matches among the sessions that could be read — the list could not be read to the end"
    );
    assert_eq!(links(cx), Vec::<&str>::new());
    let texts = painted(&frame(cx));
    assert!(
        texts.iter().any(|text| text
            == "Showing 0 of 0 matching sessions (2 in all) — could not read the list to the end"),
        "{texts:?}"
    );
}

// ---- Before the first listing, and the selection -------------------------------------------------------------------

/// The old app kept the title and the segments usable while the first read was pending (the old `sidebar.spec.ts` and
/// `filters.spec.ts` cases), so a user can narrow the list before it arrives. Specified: over a snapshot without the
/// sessions collection, the frame paints the title "Recently active" and "Loading sessions…", draws the segments with
/// All checked, and a click on Running sends `{"status": "running"}`.
#[gpui::test]
fn the_heading_and_segments_are_drawn_before_the_first_listing(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, vec![preferences(&[], None), hosts(&fleet_hosts())]);
    answer_everything(&mut fake);
    let nodes = frame(cx);
    let texts = painted(&nodes);
    for line in ["Recently active", "Loading sessions…"] {
        assert!(texts.iter().any(|text| text == line), "{line}: {texts:?}");
    }
    assert!(pressed(&nodes, "session-status-All"));
    click(cx, "session-status-Running");
    let (_, body) = one_query(&mut fake);
    assert_eq!(body, json!({ "status": "running" }));
}

/// SPEC.md: "Hiding the selected session changes neither that selection nor its open terminal" (the old status spec's
/// filtered-out selected session, whose terminal stayed). A filtered listing is no proof that a session is gone.
/// Specified: with a open, Waiting answered with a listing that leaves a out (0 of 2) keeps a selected, sends nothing
/// but the query (no selection is written or cleared), and the pane still draws a's terminal.
///
/// NOTE: The old spec also checked that the pane still named the hidden session. The pane names no session yet,
/// filtered or not (its title and details bar are not ported), so that half is not checked. When they are ported, they
/// must not take the title from the listing, which a filter empties.
#[gpui::test]
fn a_status_that_hides_the_open_session_leaves_it_open(cx: &mut TestAppContext) {
    let rows = [session("a"), session("b")];
    let (mut fake, cx) = open_main(
        cx,
        vec![
            listing(&["a", "b"], &rows),
            preferences(&["a"], None),
            hosts(&fleet_hosts()),
        ],
    );
    answer_everything(&mut fake);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    click(cx, "session-status-Waiting");
    answer_empty(&mut fake, cx, &["a", "b"]);
    assert!(
        node(&frame(cx), "session-row-a").is_none(),
        "the premise: a's row is gone"
    );
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert_eq!(fake.commands(), [], "nothing but the query was sent");
    assert!(
        node(&frame(cx), "terminal-grid").is_some(),
        "a's terminal is still drawn"
    );
}

// ---- Pure functions ------------------------------------------------------------------------------------------------

/// Replies come in the order queries were sent, and the helm serves the last query it accepted; a refusal of a query
/// the user already replaced must not undo the newer choice, and a refusal puts back both dimensions. Specified:
/// choosing the current query sends nothing; a refusal of a superseded query changes nothing; an accepted query becomes
/// the answered one; a refusal of the latest puts the choice back to the answered one, host and status, with its words;
/// a new choice drops the words.
#[test]
fn a_refusal_undoes_only_the_latest_choice() {
    let host = |host| Query {
        host,
        status: Status::All,
    };
    let mut choice = Choice::default();
    assert_eq!(
        choice.choose(Query::default()),
        None,
        "All is the choice to begin with"
    );
    let first = choice.choose(host(Some(3))).unwrap();
    let second = choice.choose(host(Some(2))).unwrap();
    choice.settle(first, host(Some(3)), Err(HostText::from_host("no")));
    assert_eq!(
        (choice.chosen(), choice.answered()),
        (host(Some(2)), Query::default())
    );
    assert!(
        choice.refusal().is_none(),
        "a superseded refusal is not shown"
    );
    choice.settle(second, host(Some(2)), Ok(()));
    assert_eq!(choice.answered(), host(Some(2)));
    let both = Query {
        host: None,
        status: Status::Idle,
    };
    let third = choice.choose(both).unwrap();
    choice.settle(third, both, Err(HostText::from_host("no")));
    assert_eq!(
        (choice.chosen(), choice.answered()),
        (host(Some(2)), host(Some(2)))
    );
    assert_eq!(choice.refusal(), Some(&HostText::from_host("no")));
    assert!(choice.choose(both).is_some());
    assert!(choice.refusal().is_none(), "a new choice drops the words");
}

/// A refused query must put the answered choice back rather than leave a choice shown that the helm is not serving,
/// with one line under the heading: the lead and the helm's words in the store's shared refusal wording. The lost row
/// pins the premise that the shared send hands a lost command over as an error, which this list then handles like any
/// refusal; it cannot be made in process (the reset that loses it drops the wait first), so both answers are built with
/// the store's own `operations::answer`. Specified, with All answered and Idle on host 2 chosen in turn: a 400 whose
/// words hold a direction override puts All back and reads "Could not show those sessions: unknown status <U+202E>x"; a
/// lost command puts All back too and reads the same lead, followed by the helm client's own words for the loss.
#[test]
fn a_refused_or_lost_query_puts_the_answered_choice_back_with_one_line() {
    let idle_on_2 = Query {
        host: Some(2),
        status: Status::Idle,
    };
    let refused = Reply {
        id: 0,
        status: 400,
        body: Some(json!("unknown status \u{202E}x")),
    };
    // The lost row matches only the lead: the words after it are the helm client's, worded in another crate.
    for (answer, line, whole) in [
        (
            Ok(refused),
            "Could not show those sessions: unknown status <U+202E>x",
            true,
        ),
        (
            Err(CommandError::Reset),
            "Could not show those sessions: ",
            false,
        ),
    ] {
        let mut choice = Choice::default();
        let number = choice.choose(idle_on_2).unwrap();
        let answer = operations::answer(answer, "PUT /ui/session-query");
        choice.settle(number, idle_on_2, answer.map(|_| ()));
        assert_eq!(choice.chosen(), Query::default(), "{line}");
        let words = choice.refusal().expect("the refusal is kept");
        let shown = refusal_sentence(words).accessible_name();
        if whole {
            assert_eq!(shown, line);
        } else {
            assert!(shown.starts_with(line), "{shown}");
        }
    }
}

/// The empty answer's evidence rule (the old `no_match_line`): only an available listing with no rows, answering a
/// query that narrows, which matched nothing may be named, with its filters when complete and only as a cut answer
/// when the cap cut it. Specified: a complete empty answer to Waiting on host 3 is that query's; a cut one is `Cut`;
/// none is named under All (the empty fleet's), with rows on screen, with matches counted, or for a failed read.
#[test]
fn only_a_narrowed_available_empty_answer_is_named() {
    let waiting = Query {
        host: Some(3),
        status: Status::Waiting,
    };
    let complete = filtered(&[], 5, 0);
    assert_eq!(empty(&complete, 0, waiting), Some(Empty::Filtered(waiting)));
    assert_eq!(empty(&complete, 0, Query::default()), None, "All and All");
    assert_eq!(empty(&complete, 1, waiting), None, "rows on screen");
    assert_eq!(
        empty(&filtered(&[], 5, 2), 0, waiting),
        None,
        "matches counted"
    );
    let mut cut = filtered(&[], 5, 0);
    cut.counts = Some(ListCounts {
        total: 5,
        matching: Some(0),
        truncated: true,
    });
    assert_eq!(empty(&cut, 0, waiting), Some(Empty::Cut));
    let mut failed: CollectionMeta = filtered(&[], 5, 0);
    failed.availability = Availability::Unavailable {
        error: "down".into(),
    };
    assert_eq!(empty(&failed, 0, waiting), None, "a failed read");
    assert_eq!(
        CUT_SENTENCE,
        "No matches among the sessions that could be read — the list could not be read to the end"
    );
}

/// Whether a switcher pick is hidden decides whether the filters reset under the user (the old
/// `switcher_pick_hidden` and its unit cases): the list's own listing decides when it answers the chosen query, since
/// the switcher's snapshot can be stale; otherwise the picked row's host and status do, and only the three offered live
/// statuses match a status. Specified, with host 2 and Idle chosen and answered over a complete listing of b: b is
/// shown even though the switcher's row calls it running; a, absent, is hidden even though its row is idle on host 2.
/// With the query still in flight, the rows decide: idle on host 2 is shown, running or on host 3 is hidden, and an
/// exited row is hidden under any status. Under All on every host, nothing is hidden; a cut listing without the row
/// falls back to the row.
#[test]
fn a_pick_is_hidden_by_the_answered_listing_or_else_by_its_row() {
    let idle_on_2 = Query {
        host: Some(2),
        status: Status::Idle,
    };
    let mut answered = Choice::default();
    let number = answered.choose(idle_on_2).unwrap();
    answered.settle(number, idle_on_2, Ok(()));
    let mut listing_b = listing(&["b"], &[session("b")]);
    listing_b.meta = filtered(&["b"], 3, 1);
    let current = collections(vec![listing_b.clone()]);
    assert!(!pick_hidden(&answered, &current, &row("b", 2, "running")));
    assert!(pick_hidden(&answered, &current, &row("a", 2, "idle")));

    let mut in_flight = Choice::default();
    in_flight.choose(idle_on_2).unwrap();
    assert!(!pick_hidden(&in_flight, &current, &row("a", 2, "idle")));
    assert!(pick_hidden(&in_flight, &current, &row("a", 2, "running")));
    assert!(pick_hidden(&in_flight, &current, &row("a", 3, "idle")));
    let exited = session("a")
        .host(2)
        .status(json!({ "state": "exited", "exit_code": 0 }))
        .row();
    assert!(pick_hidden(&in_flight, &current, &exited));

    assert!(!pick_hidden(
        &Choice::default(),
        &current,
        &row("a", 3, "exited")
    ));

    let mut cut = listing_b;
    cut.meta.counts = Some(ListCounts {
        total: 3,
        matching: Some(1),
        truncated: true,
    });
    let cut = collections(vec![cut]);
    assert!(
        !pick_hidden(&answered, &cut, &row("a", 2, "idle")),
        "the row decides"
    );
    assert!(pick_hidden(&answered, &cut, &row("a", 2, "waiting")));
}
