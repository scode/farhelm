//! Tests of a session row's hover help beside the sidebar: the summary card on the row's open control (what it says for
//! each kind of session, in either density) and every row mark asking for the beside placement, with its ring.
//!
//! What the card shows is pinned through [`Summary::of`], a pure function, for a running, an idle, an exited and a
//! managed-checkout session; that the row shows it, beside the sidebar and with those words painted, is pinned through
//! the real main view over the helm client's fake, as the list's other tests run it, reading the frame from GPUI's
//! driver record and the showing tooltip from the layer manager ([`layers::shown_tooltip`]: its side and the ring it
//! painted). Ages and stamps are counted from GPUI's test clock, which starts at [`TEST_NOW`].

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::Preferences;
use gpui::{Hsla, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};
use serde_json::{Value, json};

use super::host_rail::tests::{hosts, local, remote};
use super::row_tests::{node, texts_within};
use super::summary::{ACTIONS_HINT, CLICK_HINT, CardStatus, Fact, SUMMARY_ID_PREFIX, Summary};
use super::tests::{
    Fixture, TEST_NOW, collections, listing, open_main, preferences_state, session,
};
use crate::host_text::tests::painted_words;
use crate::layers::{self, ShownTooltip, TooltipSide};
use crate::session_list::status::{Ended, Live};
use crate::test_support::{self, frame};
use crate::theme::{self, metrics};

// ---- Fixtures -------------------------------------------------------------------------------------------------------

/// An hour before [`TEST_NOW`], when every fixture here was created: `2025-07-01 22:59:58 UTC`.
const CREATED: i64 = TEST_NOW - 3_600;

/// Two minutes before [`TEST_NOW`], when every fixture here was last active: `2025-07-01 23:57:58 UTC`.
const ACTIVE: i64 = TEST_NOW - 120;

/// A session on the local host, created at [`CREATED`], last active at [`ACTIVE`] and seen then, in an ordinary folder.
fn local_session(key: &'static str) -> Fixture {
    session(key)
        .title("Fix the widget")
        .host(1)
        .host_name("this machine")
        .created(CREATED)
        .activity(ACTIVE)
        .seen(Some(ACTIVE))
        .unmanaged()
}

/// An agent launch of `harness` with `permissions` (`null` for none) and these start words, as the helm stores one.
fn agent(harness: &str, permissions: Value) -> Value {
    json!({
        "kind": "agent",
        "selection": {"harness": harness, "permissions": permissions},
        "start": [harness, "{farhelm_args}"],
        "resume": null,
    })
}

/// `row`'s card as [`Summary::of`] decides it over the local host and host 3, at [`TEST_NOW`].
fn summary_of(row: &Fixture) -> Summary {
    let collections = collections(vec![
        listing(&[row.key], std::slice::from_ref(row)),
        hosts(&[local(), remote(3, "ops@host3.example.invalid")]),
    ]);
    Summary::of(row.key, &row.row(), &collections, Some(TEST_NOW))
}

/// A card's facts as a reader sees them: each label, and its value's words with host values escaped.
fn facts(summary: &Summary) -> Vec<(&'static str, String)> {
    summary
        .facts
        .iter()
        .map(|fact| {
            (
                fact.label(),
                fact.value("fact".into()).accessible_name().to_string(),
            )
        })
        .collect()
}

// ---- What the card says ---------------------------------------------------------------------------------------------

/// The card is where a row's abbreviated facts are recorded in full (SPEC.md: "an abbreviation is never the only place
/// a value is recorded"), so a missing or reworded fact loses information the row itself does not show. Specified, for
/// a running session on the local host launched as the helm fixture's Codex agent (YOLO, model gpt-5, high effort):
/// title `Fix the widget`, status `Running` with its mark, agent line `Codex · YOLO · Model: gpt-5 · Effort: High`, and
/// the facts Host `Local (this machine)` (with the host's mark), Directory `/home/user/src/a` unabbreviated, Started
/// `2025-07-01 22:59:58 UTC` and Last active `2m · 2025-07-01 23:57:58 UTC`; no Checkout for an ordinary folder.
#[test]
fn a_running_sessions_card_names_its_agent_host_folder_and_times() {
    let summary = summary_of(&local_session("a"));
    assert_eq!(summary.title.raw(), "Fix the widget");
    assert_eq!(summary.status, Some(CardStatus::Live(Live::Running)));
    assert_eq!(summary.status.map(CardStatus::word), Some("Running"));
    assert_eq!(
        summary.agent.accessible_name(),
        "Codex · YOLO · Model: gpt-5 · Effort: High"
    );
    assert_eq!(
        facts(&summary),
        [
            ("Host", "Local (this machine)".to_owned()),
            ("Directory", "/home/user/src/a".to_owned()),
            ("Started", "2025-07-01 22:59:58 UTC".to_owned()),
            ("Last active", "2m · 2025-07-01 23:57:58 UTC".to_owned()),
        ]
    );
    assert!(
        matches!(summary.facts[0], Fact::Host { mark: Some(_), .. }),
        "the local host is placed, so its mark is drawn"
    );
}

/// An idle session with output nobody looked at is the one live status whose card word is longer than one word, and a
/// guarded launch names its mode the way the launcher does. Specified: an idle Claude session launched guarded on host
/// 3, unseen since before its last activity, reads status `Idle — new output`, agent line `Claude · Guarded`, and Host
/// `ops@host3.example.invalid`, the helm's full name for it (the row says `host3`).
#[test]
fn an_idle_sessions_card_names_its_unseen_output_and_its_guarded_launch() {
    let row = local_session("b")
        .status(json!({"state": "idle"}))
        .seen(Some(ACTIVE - 60))
        .host(3)
        .host_name("ops@host3.example.invalid")
        .launch(agent("claude", json!("guarded")));
    let summary = summary_of(&row);
    assert_eq!(summary.status, Some(CardStatus::Live(Live::IdleUnseen)));
    assert_eq!(
        summary.status.map(CardStatus::word),
        Some("Idle — new output")
    );
    assert_eq!(summary.agent.accessible_name(), "Claude · Guarded");
    assert_eq!(
        facts(&summary)[0],
        ("Host", "ops@host3.example.invalid".to_owned())
    );
}

/// An ended session's card keeps its status to the short word, so a long failure message cannot crowd out the title
/// (the old card's choice; the row keeps the whole wording), and a command launch is named by the program it runs.
/// Specified: a session that exited with code 17 and a stop annotation reads `Exited`; a command launch of
/// `/usr/local/bin/claude --resume x` declaring Claude reads `claude`, and with the status line seen in a mode without
/// checks `claude, YOLO observed`.
#[test]
fn an_exited_sessions_card_says_exited_and_names_its_commands_program() {
    let launch = json!({
        "kind": "command",
        "command": "claude --resume x",
        "yolo": false,
        "agent": "claude",
        "resume": null,
    });
    let row = local_session("c")
        .status(json!({"state": "exited", "exit_code": 17}))
        .annotation("stopped by user")
        .launch(launch)
        .invocation("/usr/local/bin/claude --resume x");
    let summary = summary_of(&row);
    assert_eq!(summary.status, Some(CardStatus::Ended(Ended::Exited)));
    assert_eq!(summary.status.map(CardStatus::word), Some("Exited"));
    assert_eq!(summary.agent.accessible_name(), "claude");
    let observed = summary_of(&row.screen_yolo());
    assert_eq!(observed.agent.accessible_name(), "claude, YOLO observed");
}

/// A managed checkout's session works in a folder Farhelm made, and the card is where the user sees which repository
/// it is and which of its checkouts, with the session's own folder in full beside it. Specified: a session of the helm
/// fixture's checkout of `octo/widgets` at `/home/user/src/widgets`, working in `/home/user/src/widgets/api`, lists
/// Host, Checkout `octo/widgets · widgets`, Directory `/home/user/src/widgets/api`, Started and Last active, in that
/// order.
#[test]
fn a_managed_checkouts_card_names_the_repository_and_the_checkouts_folder() {
    let row = session("d")
        .host(1)
        .host_name("this machine")
        .created(CREATED)
        .activity(ACTIVE)
        .seen(Some(ACTIVE))
        .cwd("/home/user/src/widgets/api");
    assert_eq!(
        facts(&summary_of(&row)),
        [
            ("Host", "Local (this machine)".to_owned()),
            ("Checkout", "octo/widgets · widgets".to_owned()),
            ("Directory", "/home/user/src/widgets/api".to_owned()),
            ("Started", "2025-07-01 22:59:58 UTC".to_owned()),
            ("Last active", "2m · 2025-07-01 23:57:58 UTC".to_owned()),
        ]
    );
}

/// The card says nothing rather than guessing (the old `a legacy session card omits unavailable facts` and `an
/// unplaced host keeps its name without a locality mark`): a session from a helm that sent no host name, on a host no
/// registry read lists, created before stamps existed, invents neither a host nor a 1970 time, and a host name the helm
/// did send is shown without a mark claiming where the host is. Specified, before any registry read and with no usable
/// clock: such a session lists only its Directory, and its stale status is still named; with the helm's name
/// `unplaced-host` it lists Host `unplaced-host` with no mark, then its Directory.
#[test]
fn a_card_leaves_out_what_the_session_does_not_say() {
    let row = local_session("e")
        .host(9)
        .host_name("")
        .created(0)
        .activity(0)
        .stale();
    let collections = collections(vec![listing(&["e"], std::slice::from_ref(&row))]);
    let summary = Summary::of("e", &row.row(), &collections, None);
    assert_eq!(
        facts(&summary),
        [("Directory", "/home/user/src/e".to_owned())]
    );
    assert!(summary.stale);
    assert_eq!(summary.status, Some(CardStatus::Live(Live::Running)));

    let named = row.host_name("unplaced-host");
    let summary = Summary::of("e", &named.row(), &collections, None);
    assert_eq!(
        facts(&summary),
        [
            ("Host", "unplaced-host".to_owned()),
            ("Directory", "/home/user/src/e".to_owned()),
        ]
    );
    assert!(matches!(summary.facts[0], Fact::Host { mark: None, .. }));
}

// ---- The card and the marks beside the sidebar ----------------------------------------------------------------------

/// The real main view over `rows` with the first one open and compact `compact`, the local host and host 3 registered.
fn open_rows<'a>(
    cx: &'a mut TestAppContext,
    rows: &[Fixture],
    compact: bool,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    let preferences = Preferences {
        compact: Some(compact),
        visit_history: vec![order[0].to_owned()],
        welcome_seen: Some(true),
        ..Preferences::default()
    };
    open_main(
        cx,
        vec![
            listing(&order, rows),
            preferences_state(preferences),
            hosts(&[local(), remote(3, "ops@host3.example.invalid")]),
        ],
    )
}

/// The tooltip showing once the pointer has moved from a corner of the control drawn under `row` onto `at` and rested
/// for the hover delay; fails when none shows. A corner of the row is inside its padding, clear of every mark, so a mark
/// reached from there is the latest hover, as a pointer reaches it.
fn shown_at(cx: &mut VisualTestContext, row: &str, at: Point<Pixels>) -> ShownTooltip {
    let corner = match node(&frame(cx), row) {
        Some(row) => row.bounds.origin + point(px(2.), px(2.)),
        None => panic!("{row} was not drawn"),
    };
    for at in [corner, at] {
        cx.simulate_mouse_move(at, None, Modifiers::none());
        test_support::settle(cx);
    }
    test_support::advance(cx, layers::HOVER_DELAY);
    test_support::redraw(cx);
    match cx.update(|window, cx| layers::shown_tooltip(window, cx)) {
        Some(shown) => shown,
        None => panic!("no tooltip shows"),
    }
}

/// [`shown_at`] the middle of the element drawn under `target`, inside `row`.
fn shown_over(cx: &mut VisualTestContext, row: &str, target: &str) -> ShownTooltip {
    let at = match node(&frame(cx), target) {
        Some(target) => target.bounds.center(),
        None => panic!("{target} was not drawn"),
    };
    shown_at(cx, row, at)
}

/// [`shown_at`] the middle of `key`'s title, clear of every mark: the row's own hover, its summary card.
fn card_over_title(cx: &mut VisualTestContext, key: &str, title: &str) -> ShownTooltip {
    let row = format!("session-row-{key}");
    let at = texts_within(&frame(cx), &row)
        .into_iter()
        .find(|text| text.text.as_ref() == title)
        .unwrap_or_else(|| panic!("{key}'s title was not painted"))
        .bounds
        .center();
    shown_at(cx, &row, at)
}

/// The words painted inside `key`'s card, top to bottom, with the isolates around host values taken out.
fn card_words(cx: &mut VisualTestContext, key: &str) -> Vec<String> {
    texts_within(&frame(cx), &format!("{SUMMARY_ID_PREFIX}{key}"))
        .into_iter()
        .map(|text| painted_words(&text.text))
        .collect()
}

/// The theme's accent and on-fill inks as the ring paints them.
fn ring_inks(cx: &mut VisualTestContext) -> (Hsla, Hsla) {
    cx.update(|_, cx| {
        let palette = theme::palette(cx);
        (theme::color(palette.accent), theme::color(palette.on_fill))
    })
}

/// SPEC.md, "Session list": "The same card appears from the title or any unmarked part of the open button", and it sits
/// "just right of the sidebar ... A left-edge notch points back to the target's centre ... and a thin accent ring
/// identifies that target", in the on-fill ink on the selected row's accent fill. Specified, over a running local
/// session `b` beside the open session `a`: resting on b's title shows its card beside the sidebar, reporting the row's
/// words `Running — open this session`, with the accent ring painted at b's open control's own bounds with square
/// corners and the notch drawn; the card paints exactly `Fix the widget`, `Running`, the agent line, Host
/// `Local (this machine)`, Directory, Started, Last active, `Click to open` and `⋯ For actions`, in that order; resting
/// on the open row a's title shows a's card with its ring in the on-fill ink.
#[gpui::test]
fn a_rows_title_shows_its_summary_card_beside_the_sidebar_with_its_ring(cx: &mut TestAppContext) {
    let (_fake, cx) = open_rows(cx, &[local_session("a"), local_session("b")], false);
    let (accent, on_fill) = ring_inks(cx);
    let shown = card_over_title(cx, "b", "Fix the widget");
    assert_eq!(shown.side(), TooltipSide::Beside);
    assert_eq!(shown.text, "Running — open this session");
    let control = node(&frame(cx), "session-row-b")
        .expect("b's open control is drawn")
        .bounds;
    let Some(ring) = shown.ring() else {
        panic!("a ring is painted on b's open control");
    };
    assert_eq!(
        (ring.bounds, ring.color, ring.radius),
        (control, accent, px(0.))
    );
    assert!(
        cx.debug_bounds("tooltip-notch").is_some(),
        "the notch shows"
    );
    assert_eq!(
        card_words(cx, "b"),
        [
            "Fix the widget",
            "Running",
            "Codex · YOLO · Model: gpt-5 · Effort: High",
            "Host",
            "Local (this machine)",
            "Directory",
            "/home/user/src/b",
            "Started",
            "2025-07-01 22:59:58 UTC",
            "Last active",
            "2m · 2025-07-01 23:57:58 UTC",
            CLICK_HINT,
            ACTIONS_HINT,
        ]
    );

    let shown = card_over_title(cx, "a", "Fix the widget");
    assert_eq!(shown.side(), TooltipSide::Beside);
    assert_eq!(shown.ring().map(|ring| ring.color), Some(on_fill));
    assert_eq!(card_words(cx, "a")[0], "Fix the widget");
}

/// The help of the mark drawn under `target` in `row` that the pointer rests on, checked as that mark's own and not the
/// row's summary card, whose help is beside the sidebar with a ring too: it shows beside the sidebar, says `words` (or
/// starts with them, for help longer than a test should spell out), and its ring is painted in `color` with `radius`
/// corners exactly on the mark's own bounds, inside its 1-pixel border when it has one (`bordered`).
#[allow(
    clippy::too_many_arguments,
    reason = "one mark's expected help, ring and border"
)]
fn assert_mark_help(
    cx: &mut VisualTestContext,
    row: &str,
    target: &str,
    words: &str,
    color: Hsla,
    radius: Pixels,
    bordered: bool,
) {
    let shown = shown_over(cx, row, target);
    assert_eq!(shown.side(), TooltipSide::Beside, "{target}");
    assert!(
        shown.text.starts_with(words),
        "{target}: its own help, not the card's: {:?}",
        shown.text
    );
    let ring = shown.ring().unwrap_or_else(|| panic!("{target}: a ring"));
    let mut bounds = node(&frame(cx), target)
        .unwrap_or_else(|| panic!("{target} was not drawn"))
        .bounds;
    if bordered {
        bounds = gpui::Bounds::new(
            bounds.origin + point(px(1.), px(1.)),
            gpui::size(bounds.size.width - px(2.), bounds.size.height - px(2.)),
        );
    }
    assert_eq!(
        (ring.bounds, ring.color, ring.radius),
        (bounds, color, radius),
        "{target}: the ring on the mark itself"
    );
}

/// One notification, unread, as the helm lists it; its row draws a loud bell.
fn one_note() -> Value {
    json!([{ "seq": 5, "at": ACTIVE, "text": "Look at this" }])
}

/// The old app opted every row target into the beside placement one by one, and a mark left at the pointer would put
/// its help over the rows below it, the very thing the placement exists to avoid. A mark's help that went missing would
/// leave the row's card showing there instead, also beside the sidebar with a ring, so each check reads the mark's own
/// words and a ring on the mark's own bounds. Specified, over the open session `a` (pinned, running, one notification)
/// and `b` (the same) and a stale `s` on the unreachable host 3: on b, the status dot (`Running — Mark unread`), the host
/// mark (`Local: …`), the agent track (`Codex — …`), the age (`Last activity …`), the pin (`Pinned`), the bell
/// (`Notifications: 1 unread; …`) and the actions button (`Session actions: …`) each show their help beside the
/// sidebar with an accent ring on the mark's bounds, square except the button's, which follows its corners inside its
/// edge (2 pixels); on s, the stale cause too; and on the open row a, the dot, the age and the bell with the ring in
/// on-fill.
#[gpui::test]
fn every_row_mark_shows_its_help_beside_the_sidebar_with_its_ring(cx: &mut TestAppContext) {
    let unreachable = json!({
        "phase": "unreachable-reprobing",
        "cause": "transport-failure",
        "last_error": "timed out",
    });
    let rows = [
        local_session("a").pinned(1).notifications(one_note(), 0),
        local_session("b").pinned(2).notifications(one_note(), 0),
        local_session("s").host(3).stale(),
    ];
    let order = ["a", "b", "s"];
    let preferences = Preferences {
        visit_history: vec!["a".to_owned()],
        welcome_seen: Some(true),
        ..Preferences::default()
    };
    let mut registry = hosts(&[local(), remote(3, "ops@host3.example.invalid")]);
    registry.entries[1].value["state"] = unreachable;
    let (_fake, cx) = open_main(
        cx,
        vec![
            listing(&order, &rows),
            preferences_state(preferences),
            registry,
        ],
    );
    let (accent, on_fill) = ring_inks(cx);
    let square = px(0.);
    let button_radius = metrics::RADIUS_SM - px(1.);
    let age = "Last activity 2025-07-01 23:57:58 UTC";
    let bell = "Notifications: 1 unread; click to list them";
    for (target, words, radius, bordered) in [
        ("session-seen-b", "Running — Mark unread", square, false),
        (
            "session-row-b-host",
            "Local: on this machine, where the helm runs",
            square,
            false,
        ),
        ("session-row-b-agent", "Codex — ", square, false),
        ("session-row-b-age", age, square, false),
        ("session-row-b-pin", "Pinned", square, false),
        ("session-bell-b", bell, square, false),
        (
            "session-menu-b",
            "Session actions: rename, clone, replace, stop, delete and more",
            button_radius,
            true,
        ),
    ] {
        assert_mark_help(cx, "session-row-b", target, words, accent, radius, bordered);
    }
    assert_mark_help(
        cx,
        "session-row-s",
        "session-row-s-cause",
        "Waiting for supervisor: this is the session's last-known state",
        accent,
        square,
        false,
    );
    for (target, words) in [
        ("session-seen-a", "Running — Mark unread"),
        ("session-row-a-age", age),
        ("session-bell-a", bell),
    ] {
        assert_mark_help(cx, "session-row-a", target, words, on_fill, square, false);
    }
}

/// A compact row hides its last line, and SPEC.md points to the card for what it hides ("the directory in the row's
/// hover summary card"; the Compact item says the folder is "shown on hover"); the old app showed the same card in
/// either density. The card's words stay what the compact row's description said, so a screen reader still hears the
/// hidden ended wording and cause. Specified, in compact mode, over a row `x` that exited with code 17 on the
/// unreachable host 3: its title shows the same card a noncompact row shows, beside the sidebar, its Directory
/// `/home/user/src/x` in full and its status `Exited`, reporting `Exited (code 17) · Waiting for supervisor ·
/// /home/user/src/x — open this session`; its ended mark (`Exited (code 17)`) and its stale clock (`Waiting for
/// supervisor: …`) show their own help beside the sidebar too, each ring on the mark itself.
#[gpui::test]
fn a_compact_row_shows_the_same_card_with_its_hidden_folder(cx: &mut TestAppContext) {
    let unreachable = json!({
        "phase": "unreachable-reprobing",
        "cause": "transport-failure",
        "last_error": "timed out",
    });
    let rows = [
        local_session("a"),
        local_session("x")
            .title("Exited one")
            .host(3)
            .host_name("ops@host3.example.invalid")
            .stale()
            .status(json!({"state": "exited", "exit_code": 17})),
    ];
    let preferences = Preferences {
        compact: Some(true),
        visit_history: vec!["a".to_owned()],
        welcome_seen: Some(true),
        ..Preferences::default()
    };
    let mut registry = hosts(&[local(), remote(3, "ops@host3.example.invalid")]);
    registry.entries[1].value["state"] = unreachable;
    let (_fake, cx) = open_main(
        cx,
        vec![
            listing(&["a", "x"], &rows),
            preferences_state(preferences),
            registry,
        ],
    );
    let shown = card_over_title(cx, "x", "Exited one");
    assert_eq!(shown.side(), TooltipSide::Beside);
    assert_eq!(
        shown.text,
        "Exited (code 17) · Waiting for supervisor · /home/user/src/x — open this session"
    );
    let words = card_words(cx, "x");
    assert_eq!(&words[..2], ["Exited one", "Exited"]);
    assert!(
        words
            .windows(2)
            .any(|pair| pair == ["Directory", "/home/user/src/x"]),
        "the hidden folder in full: {words:?}"
    );
    let (accent, _) = ring_inks(cx);
    for (target, words) in [
        ("session-row-x-ended", "Exited (code 17)"),
        (
            "session-row-x-cause",
            "Waiting for supervisor: this is the session's last-known state",
        ),
    ] {
        assert_mark_help(cx, "session-row-x", target, words, accent, px(0.), false);
    }
}
