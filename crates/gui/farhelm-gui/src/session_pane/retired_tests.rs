//! Tests of a retired containerized session's open view (`retired.rs`): no terminal and no stream, the retired card in
//! the terminal's place as home, the header's one action and its question, a refused Delete, the card giving way to the
//! unavailable card on an unreachable host, and an ordinary container session keeping its terminal and header. The
//! row's half is `session_list/retired_tests.rs`'s.
//!
//! Everything runs in the real main view over the helm client's fake, whose connection hands out fake terminal streams
//! and records them (`test_support::open_main`), and reads what the window sent from the fake and what it drew from
//! GPUI's driver record. The first session of every snapshot is the open one.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::RETIRED_CONTAINER_REASON;
use gpui::TestAppContext;
use serde_json::json;

use super::header::{Action, Split};
use super::retired::CARD_ID;
use super::{details, unavailable};
use crate::session_list::menu_tests::{drawn, label_of, push, snapshot};
use crate::session_list::retired_tests::{container_registry, retired};
use crate::session_list::tests::session;
use crate::terminal::set::STRIP_ID;
use crate::terminal::text_size;
use crate::test_support::{self, MainWindow, focused_id, frame, open_main};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Answer every command the window sent since the last call with success, in order, as the helm answers a preference
/// write or the list's query, so that a Delete sent later can be answered.
fn answer_all(fake: &mut FakeHelm) {
    for command in fake.commands() {
        fake.reply(command.id, 200, None);
    }
}

/// How many terminal streams the pane has opened so far.
fn streams(main: &MainWindow) -> usize {
    main.terminals.borrow().len()
}

/// Whether an element with the id `id` was drawn in a fresh frame.
fn has_id(main: &mut MainWindow, id: &str) -> bool {
    frame(main.cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

// ---- The view ------------------------------------------------------------------------------------------------------

/// SPEC.md: retired containerized sessions "offer no terminal or tab access", and the old view drew a card with the
/// reason where the terminal would be; an attach would meet a terminal nothing serves. SPEC.md, "Where focus rests",
/// rests focus on the card that stands where a terminal would be. Specified: with retired a open at startup, the pane
/// opens no stream and draws no terminal and no tab strip; the card, named by the retired reason, sits between the top
/// row and the details bar and holds focus.
#[gpui::test]
fn a_retired_session_opens_on_its_card_with_no_terminal(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(retired("a"), 0), (session("b"), 0)], false));
    answer_all(&mut main.helm);
    assert_eq!(streams(&main), 0, "no stream");
    assert!(!drawn(main.cx, "terminal-grid"), "no terminal");
    assert!(!drawn(main.cx, STRIP_ID), "no tab strip");
    assert_eq!(
        label_of(main.cx, CARD_ID).as_deref(),
        Some(RETIRED_CONTAINER_REASON)
    );
    let card = test_support::bounds(main.cx, CARD_ID);
    assert!(card.top() >= test_support::bounds(main.cx, "session-header").bottom());
    assert!(card.bottom() <= test_support::bounds(main.cx, details::BAR_ID).top());
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD_ID));
}

/// The old header drew Delete alone for a retired session (no Stop, Restart, Replace or Clone, and no text size for a
/// terminal it does not have), and its Delete asked first like any session's; the old spec answered with a refusal and
/// found its words on the row or the header. Specified: the header draws Delete and none of the other actions, carets
/// or the text size group; Delete asks and sends nothing before the answer; Confirm delete sends the one unguarded
/// `DELETE /api/sessions/a`; the helm's refusal shows under the top row with its words, Delete comes back, and the card
/// stays.
#[gpui::test]
fn the_header_offers_delete_alone_and_shows_its_refusal(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(retired("a"), 0), (session("b"), 0)], false));
    answer_all(&mut main.helm);
    for action in Action::ALL {
        assert_eq!(
            has_id(&mut main, action.id()),
            action == Action::Delete,
            "{action:?}"
        );
    }
    for split in [Split::Restart, Split::Replace] {
        assert!(!has_id(&mut main, split.caret_id()), "{split:?}'s caret");
    }
    assert!(!has_id(&mut main, text_size::GROUP_ID), "no text size");

    test_support::click(main.cx, Action::Delete.id());
    assert!(drawn(main.cx, "prompt"), "Delete asks first");
    assert!(main.helm.commands().is_empty(), "nothing before the answer");
    test_support::click(main.cx, "prompt-confirm");
    let (id, _) = test_support::one_command(&mut main.helm, Method::Delete, "/api/sessions/a");
    main.helm
        .reply(id, 409, Some(json!("retired fixture retained for retry")));
    test_support::settle(main.cx);
    assert_eq!(
        label_of(main.cx, "session-header-outcome-a").as_deref(),
        Some("retired fixture retained for retry")
    );
    assert!(has_id(&mut main, Action::Delete.id()), "Delete is back");
    assert!(drawn(main.cx, CARD_ID));
}

/// A guard for the window before a selected session's row reaches the store: the pane mounts terminals for a selection
/// the store holds no row for, and once the row arrives saying the session is retired, those terminals must go, or
/// they would attach to a terminal nothing serves. The helm never adds the old container choice to an existing
/// session's launch; a push that changes it stands in here for the row arriving after the set was made, and drives the
/// same drop. Specified: with an ordinary a open and its stream opened, a push that lists a with the retired launch
/// closes that stream, draws the card, moves focus there, and opens no other stream.
#[gpui::test]
fn a_row_that_turns_out_retired_drops_its_terminal(cx: &mut TestAppContext) {
    let mut main = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    main.terminal_live();
    assert_eq!(streams(&main), 1, "the premise");

    push(&mut main.helm, &["a", "b"], &[(retired("a"), 0)], main.cx);
    answer_all(&mut main.helm);
    assert_eq!(streams(&main), 1, "no new stream");
    assert_eq!(
        main.terminals.borrow_mut()[0].phase(),
        farhelm_helm_client::fake::terminal::FakePhase::Closed
    );
    assert!(drawn(main.cx, CARD_ID));
    assert_eq!(focused_id(main.cx).as_deref(), Some(CARD_ID));
}

/// Stale beats retired, as it beats interrupted: a host the helm cannot reach shows the unavailable card in the
/// terminal area alone, so the area never holds two cards. Specified: with retired a open on an unreachable host, the
/// unavailable card is drawn, the retired card is not, and no stream opens.
#[gpui::test]
fn an_unreachable_retired_session_shows_the_unavailable_card_alone(cx: &mut TestAppContext) {
    let mut main = open_main(
        cx,
        snapshot(&[(retired("a").stale(), 0), (session("b"), 0)], false),
    );
    answer_all(&mut main.helm);
    assert!(drawn(main.cx, unavailable::CARD_ID));
    assert!(!drawn(main.cx, CARD_ID));
    assert_eq!(streams(&main), 0);
}

/// The old spec's other half: a session in a new container, on a host its environment owns, attaches its terminal and
/// keeps its header's controls, because retirement is the stored launch's and never the host's. Specified: an
/// ordinary session on a session-owned container host opens one stream, draws the tab strip, all five actions and no
/// retired card. Suspend is not drawn yet: it comes with the environment sessions' rows.
#[gpui::test]
fn an_ordinary_container_session_keeps_its_terminal_and_header(cx: &mut TestAppContext) {
    let mut rows = snapshot(&[(session("a"), 0), (session("b"), 0)], false);
    rows.push(container_registry());
    let mut main = open_main(cx, rows);
    main.terminal_live();
    assert_eq!(streams(&main), 1);
    assert!(drawn(main.cx, STRIP_ID));
    for action in Action::ALL {
        assert!(has_id(&mut main, action.id()), "{action:?}");
    }
    assert!(!drawn(main.cx, CARD_ID));
}
