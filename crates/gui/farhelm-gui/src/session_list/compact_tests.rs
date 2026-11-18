//! Tests of compact rows: what the helm-wide compact preference hides, the marks a compact row draws in place of its
//! ended line (an ended status's shape in the status column, a stale row's clock after the title), the words and hover
//! help those marks and the row keep, and the rows following a pushed change of the preference.
//!
//! Like the list's other tests, these run the real main view over the helm client's fake and read the frame from GPUI's
//! driver record. A compact row's marks are found by their ids (`session-row-<key>-ended` for the ended mark,
//! `session-row-<key>-cause` for the clock), and which picture a mark drew by its debug selector, the mark's id and the
//! icon's path (`session-row-<key>-ended:icons/ended-stopped.svg`), since the record says nothing of pictures. The
//! inks, which the record does not carry either, are pure functions tested as such; the `session-list-compact` goldens
//! pin that the rows draw with them. The order menu's Compact item, which writes the preference, is tested with the
//! rest of the menu (`order/tests.rs`).

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY, Preferences,
};
use gpui::{Role, TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::host_rail::tests::{hosts, local, remote};
use super::row::ITEM_ID_PREFIX;
use super::row_tests::{help_after_crossing, help_at, node, texts_within};
use super::status::{EndedGlyph, stale_mark_color};
use super::tests::{
    Fixture, TEST_NOW, listing, open_main, preferences_state, selected, sent, session,
};
use crate::test_support::{self, frame, meta};
use crate::theme;

// ---- Fixtures -------------------------------------------------------------------------------------------------------

/// The preference row with `compact` (`None` for unset), the visit history opening `open`, and the Welcome card seen,
/// as a helm would push it.
fn compact_preferences(compact: Option<bool>, open: &str) -> Preferences {
    Preferences {
        compact,
        visit_history: vec![open.to_owned()],
        welcome_seen: Some(true),
        ..Preferences::default()
    }
}

/// The registry: the local host, and host 3 (`ops@host3.example.invalid`, whose short name is `host3`), which the
/// helm cannot reach, so a stale row on it reads `Waiting for supervisor`.
fn registry() -> CollectionState {
    let mut registry = hosts(&[local(), remote(3, "ops@host3.example.invalid")]);
    registry.entries[1].value["state"] = json!({
        "phase": "unreachable-reprobing",
        "cause": "transport-failure",
        "last_error": "timed out",
    });
    registry
}

/// The real main view over `rows`, in that order, with the compact preference `compact` and the first row open.
fn open_rows<'a>(
    cx: &'a mut TestAppContext,
    rows: &[Fixture],
    compact: Option<bool>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    open_main(
        cx,
        vec![
            listing(&order, rows),
            preferences_state(compact_preferences(compact, order[0])),
            registry(),
        ],
    )
}

/// A push of the preference row with `compact`, as the helm sends it when the preference changes elsewhere.
fn push_compact(
    fake: &mut FakeHelm,
    compact: Option<bool>,
    open: &str,
    cx: &mut VisualTestContext,
) {
    fake.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        upsert: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(compact_preferences(compact, open)).unwrap(),
        }],
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// A session active a minute ago and seen then, so its age reads `1m` and opening it sends no seen mark.
fn quiet(key: &'static str) -> Fixture {
    session(key)
        .activity(TEST_NOW - 60)
        .seen(Some(TEST_NOW - 60))
}

/// An ended status in the helm's wire form.
fn exited(code: Value) -> Value {
    json!({"state": "exited", "exit_code": code})
}

/// The strings painted under `key`'s row, top to bottom and left to right within a line.
fn row_texts(cx: &mut VisualTestContext, key: &str) -> Vec<String> {
    texts_within(&frame(cx), &format!("{ITEM_ID_PREFIX}{key}"))
        .into_iter()
        .map(|text| text.text.to_string())
        .collect()
}

/// The height `key`'s row was last drawn at.
fn row_height(cx: &mut VisualTestContext, key: &str) -> f32 {
    let nodes = frame(cx);
    match node(&nodes, &format!("{ITEM_ID_PREFIX}{key}")) {
        Some(row) => f32::from(row.bounds.size.height),
        None => panic!("{key}'s row was not drawn"),
    }
}

/// The words of the row's own hover help, its summary card, which are also its description: reported once the pointer
/// rests on the row's title, clear of every mark.
fn row_help(cx: &mut VisualTestContext, key: &str) -> Option<String> {
    let row = format!("session-row-{key}");
    let title = texts_within(&frame(cx), &row)
        .into_iter()
        .find(|text| text.text.as_ref() == key)
        .unwrap_or_else(|| panic!("{key}'s title was not painted"));
    help_at(cx, &row, title.bounds.center())
}

// ---- What compact hides ---------------------------------------------------------------------------------------------

/// SPEC.md, "Session list": "The second line is hidden when the helm-wide compact preference is on", and compact mode is
/// the escape hatch for a fleet where the repeated host and directory say little, so it has to buy a shorter row:
/// a compact ended row that kept its ended line would not. Specified: over a live row `a` and a stale row `b` on an
/// unreachable host that exited with code 17, with compact unset `a` paints its title, age and last line and `b` its
/// ended words and cause on a line of their own and its last line; once the helm pushes compact on, each paints only its
/// title and age, and each row is shorter than it was.
#[gpui::test]
fn compact_hides_the_last_line_and_the_ended_line_and_shortens_the_row(cx: &mut TestAppContext) {
    let rows = [
        quiet("a"),
        quiet("b").host(3).stale().status(exited(json!(17))),
    ];
    let (mut fake, cx) = open_rows(cx, &rows, None);
    assert_eq!(row_texts(cx, "a"), ["a", "1m", "buildbox", ":", "~/src/a"]);
    assert_eq!(
        row_texts(cx, "b"),
        [
            "b",
            "1m",
            "Exited (code 17)",
            "Waiting for supervisor",
            "host3",
            ":",
            "~/src/b"
        ]
    );
    let tall = (row_height(cx, "a"), row_height(cx, "b"));

    push_compact(&mut fake, Some(true), "a", cx);
    assert_eq!(row_texts(cx, "a"), ["a", "1m"]);
    assert_eq!(row_texts(cx, "b"), ["b", "1m"]);
    assert!(
        row_height(cx, "a") < tall.0 && row_height(cx, "b") < tall.1,
        "compact rows are shorter: {:?} then {:?}",
        tall,
        (row_height(cx, "a"), row_height(cx, "b"))
    );
}

/// The preference is shared by every client (SPEC.md: "shared at the next preference seed across clients"), and a
/// change made elsewhere reaches the open window as it happens, so the rows must follow a push both ways without the
/// window writing anything back. Specified: with compact on at the first snapshot, a row paints only its title and age;
/// a push setting it off brings its last line back; a push leaving it unset keeps it off (unset is off); a push setting
/// it on hides the line again; and none of the pushes sends anything.
#[gpui::test]
fn a_pushed_change_of_the_preference_redraws_the_rows(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_rows(cx, &[quiet("a")], Some(true));
    assert_eq!(row_texts(cx, "a"), ["a", "1m"]);
    let _ = sent(&mut fake);

    push_compact(&mut fake, Some(false), "a", cx);
    assert_eq!(row_texts(cx, "a"), ["a", "1m", "buildbox", ":", "~/src/a"]);
    push_compact(&mut fake, None, "a", cx);
    assert_eq!(row_texts(cx, "a"), ["a", "1m", "buildbox", ":", "~/src/a"]);
    push_compact(&mut fake, Some(true), "a", cx);
    assert_eq!(row_texts(cx, "a"), ["a", "1m"]);
    assert_eq!(sent(&mut fake), [], "a pushed preference sends nothing");
}

// ---- The ended marks ------------------------------------------------------------------------------------------------

/// SPEC.md, "Status": "Compact mode uses distinct stopped, exited, interrupted, and error icons; those details remain in
/// accessible text and tooltips without expanding the row." A user's Stop is an exit with an annotation, so the stopped
/// mark is told apart by the annotation, either of the two the supervisor writes (the old
/// `compact_rows_draw_both_stop_annotations_with_the_stopped_icon`). Specified, in compact mode: an exit with no code
/// annotated `stopped by user` draws the stopped picture, named `Exited (code unknown) — stopped by user`; an exit with
/// code 0 annotated `stopped before the agent finished starting` draws it too, named with that annotation; an exit with
/// code 17 draws the exited picture, named `Exited (code 17)`; an interruption the interrupted one, named
/// `Interrupted`; a launch error the error one, named `Error — cannot launch`; each is an image with no seen toggle in
/// its column; a running row draws its seen toggle and no ended mark. The stop's and the error's hover help are their
/// names word for word.
#[gpui::test]
fn each_ended_status_draws_its_own_mark_named_and_helped_by_its_full_wording(
    cx: &mut TestAppContext,
) {
    let rows = [
        quiet("stop")
            .status(exited(Value::Null))
            .annotation("stopped by user"),
        quiet("early")
            .status(exited(json!(0)))
            .annotation("stopped before the agent finished starting"),
        quiet("exit").status(exited(json!(17))),
        quiet("intr").status(json!({"state": "interrupted"})),
        quiet("err").status(json!({"state": "error", "detail": "cannot launch"})),
        quiet("live"),
    ];
    let (_fake, cx) = open_rows(cx, &rows, Some(true));
    let nodes = frame(cx);
    for (key, name) in [
        ("stop", "Exited (code unknown) — stopped by user"),
        (
            "early",
            "Exited (code 0) — stopped before the agent finished starting",
        ),
        ("exit", "Exited (code 17)"),
        ("intr", "Interrupted"),
        ("err", "Error — cannot launch"),
    ] {
        let mark = node(&nodes, &format!("session-row-{key}-ended"))
            .unwrap_or_else(|| panic!("{key} draws an ended mark"));
        assert_eq!(mark.label.as_deref(), Some(name), "{key}");
        assert_eq!(mark.role, Some(Role::Image), "{key}");
        assert!(
            node(&nodes, &format!("session-seen-{key}")).is_none(),
            "{key} has no seen toggle"
        );
    }
    for (selector, drawn) in [
        ("session-row-stop-ended:icons/ended-stopped.svg", true),
        ("session-row-early-ended:icons/ended-stopped.svg", true),
        ("session-row-exit-ended:icons/ended-exited.svg", true),
        ("session-row-intr-ended:icons/ended-interrupted.svg", true),
        ("session-row-err-ended:icons/ended-error.svg", true),
    ] {
        assert_eq!(cx.debug_bounds(selector).is_some(), drawn, "{selector}");
    }
    assert!(node(&nodes, "session-row-live-ended").is_none());
    assert!(node(&nodes, "session-seen-live").is_some());

    assert_eq!(
        help_after_crossing(cx, "session-row-stop", "session-row-stop-ended").as_deref(),
        Some("Exited (code unknown) — stopped by user")
    );
    assert_eq!(
        help_after_crossing(cx, "session-row-err", "session-row-err-ended").as_deref(),
        Some("Error — cannot launch")
    );
}

/// SPEC.md, "Status": "Ended icons do not have the live dot's mark-read action." The ended mark sits where the live dot
/// sits, so a dot's toggle carried over to it would mark the session read or unread behind the user's back. Specified,
/// in compact mode, over an exited row already seen (so opening it is due no seen mark): its ended mark takes no click
/// of its own, and a click on it opens the session, as a click anywhere else on the row does, and sends no seen request.
#[gpui::test]
fn a_click_on_an_ended_mark_opens_the_session_and_marks_nothing(cx: &mut TestAppContext) {
    let rows = [quiet("a"), quiet("exit").status(exited(json!(17)))];
    let (mut fake, cx) = open_rows(cx, &rows, Some(true));
    let mark = node(&frame(cx), "session-row-exit-ended")
        .cloned()
        .expect("the premise: the ended mark is drawn");
    assert!(!mark.flags.click, "the mark takes no click of its own");
    let _ = sent(&mut fake);

    test_support::click(cx, "session-row-exit-ended");
    assert_eq!(selected(cx).as_deref(), Some("exit"));
    let seen: Vec<_> = sent(&mut fake)
        .into_iter()
        .filter(|(path, _)| path.ends_with("/seen"))
        .collect();
    assert_eq!(seen, [], "no seen request");
}

// ---- The stale clock and the row's own words ------------------------------------------------------------------------

/// SPEC.md, "Session list": sessions on an unreachable host carry a neutral cause, and "Compact rows carry the same
/// cause in accessible text and hover help". Specified, in compact mode, over a running row on an unreachable host:
/// a status mark after the title named `Waiting for supervisor` draws the stale clock, its hover help `Waiting for
/// supervisor: this is the session's last-known state`; its grey dot is still drawn; and a row on a connected host draws
/// no clock.
#[gpui::test]
fn a_compact_stale_row_draws_a_clock_named_and_helped_by_its_cause(cx: &mut TestAppContext) {
    let rows = [quiet("a"), quiet("s").host(3).stale()];
    let (_fake, cx) = open_rows(cx, &rows, Some(true));
    let nodes = frame(cx);
    let clock = node(&nodes, "session-row-s-cause").expect("the stale row draws its clock");
    assert_eq!(clock.label.as_deref(), Some("Waiting for supervisor"));
    assert_eq!(clock.role, Some(Role::Status));
    assert!(
        cx.debug_bounds("session-row-s-cause:icons/stale.svg")
            .is_some()
    );
    assert!(node(&nodes, "session-seen-s").is_some(), "its dot stays");
    assert!(node(&nodes, "session-row-a-cause").is_none());
    assert_eq!(
        help_after_crossing(cx, "session-row-s", "session-row-s-cause").as_deref(),
        Some("Waiting for supervisor: this is the session's last-known state")
    );
}

/// What a compact row hides must still reach a screen reader (SPEC.md: the ended details "remain in accessible text and
/// tooltips", the cause too). The row's hover help is its summary card, and these are the card's words: the row's
/// accessible description, and the text the layer manager reports for the showing card. The pointer's half is the
/// card's folder and the ended and stale marks' own help (`summary_tests.rs`,
/// `a_compact_row_shows_the_same_card_with_its_hidden_folder`). Specified, in compact mode: a stale row that exited with
/// code 17 is described `Exited (code 17) · Waiting for supervisor · /home/user/src/b — open this session`; a running
/// row on a connected host `Running · /home/user/src/a — open this session`; a running row with no directory `Running —
/// open this session`; and with compact off, the running row keeps its plain `Running — open this session`.
#[gpui::test]
fn a_compact_rows_help_says_what_its_hidden_lines_would(cx: &mut TestAppContext) {
    let rows = [
        quiet("a"),
        quiet("b").host(3).stale().status(exited(json!(17))),
        quiet("c").cwd(""),
    ];
    let (mut fake, cx) = open_rows(cx, &rows, Some(true));
    assert_eq!(
        row_help(cx, "b").as_deref(),
        Some("Exited (code 17) · Waiting for supervisor · /home/user/src/b — open this session")
    );
    assert_eq!(
        row_help(cx, "a").as_deref(),
        Some("Running · /home/user/src/a — open this session")
    );
    assert_eq!(
        row_help(cx, "c").as_deref(),
        Some("Running — open this session")
    );

    push_compact(&mut fake, Some(false), "a", cx);
    assert_eq!(
        row_help(cx, "a").as_deref(),
        Some("Running — open this session")
    );
}

/// Compact rows keep the waiting dot in its slot and the ended mark
/// unchanged. A stale wait is last-known and instead puts its bell in the
/// dot slot. All three still say what happened when read aloud.
#[gpui::test]
fn compact_mute_keeps_waiting_and_ended_status_visible(cx: &mut TestAppContext) {
    let waiting = quiet("wait")
        .status(json!({"state": "waiting"}))
        .muted(None);
    let stale = quiet("stale")
        .host(3)
        .stale()
        .status(json!({"state": "waiting"}))
        .muted(None);
    let ended = quiet("ended").status(exited(json!(17))).muted(None);
    let (_fake, cx) = open_rows(cx, &[quiet("live"), waiting, stale, ended], Some(true));
    let dot = test_support::bounds(cx, "session-seen-live");
    let waiting_bell = test_support::bounds(cx, "session-mute-bell-wait");
    let stale_bell = test_support::bounds(cx, "session-mute-bell-stale");
    let agent = test_support::bounds(cx, "session-row-wait-agent");
    assert_eq!(
        stale_bell.center().x,
        dot.center().x,
        "the stale wait's bell sits in the dot's column"
    );
    assert!(waiting_bell.left() > dot.right());
    assert!(
        agent.left() - waiting_bell.right() > gpui::px(20.),
        "bell follows the short title"
    );
    assert!(row_help(cx, "wait").unwrap().starts_with("Waiting · Muted"));
    assert!(
        row_help(cx, "stale")
            .unwrap()
            .starts_with("Waiting · Muted")
    );
    assert!(
        row_help(cx, "ended")
            .unwrap()
            .starts_with("Exited (code 17) · Muted")
    );
    assert!(!super::menu_tests::drawn(cx, "session-mute-bell-ended"));
}

/// The marks' inks are what tell an interruption from an error at a glance, and on the selected row's solid fill the
/// ordinary ones lose their contrast (SPEC.md: inverse marks "keep the same shapes instead of their ordinary colors").
/// The frame's record carries no colors, so the rule is pinned here and the `session-list-compact` goldens pin that the
/// rows draw with it. Specified, in both palettes: exited `--fg-1`, interrupted `--warn`, error `--danger-strong`,
/// stopped the row's `--fg-0` (the old sheet gave it no color of its own), and the stale clock `--fg-1`; each
/// `--on-fill` on the selected row.
#[test]
fn the_compact_marks_use_their_inks_and_the_inverse_on_the_selected_row() {
    for palette in [&theme::DARK, &theme::LIGHT] {
        assert_eq!(EndedGlyph::Exited.color(palette, false), palette.fg_1);
        assert_eq!(EndedGlyph::Interrupted.color(palette, false), palette.warn);
        assert_eq!(
            EndedGlyph::Error.color(palette, false),
            palette.danger_strong
        );
        assert_eq!(EndedGlyph::Stopped.color(palette, false), palette.fg_0);
        assert_eq!(stale_mark_color(palette, false), palette.fg_1);
        for glyph in [
            EndedGlyph::Stopped,
            EndedGlyph::Exited,
            EndedGlyph::Interrupted,
            EndedGlyph::Error,
        ] {
            assert_eq!(glyph.color(palette, true), palette.on_fill, "{glyph:?}");
        }
        assert_eq!(stale_mark_color(palette, true), palette.on_fill);
    }
}
