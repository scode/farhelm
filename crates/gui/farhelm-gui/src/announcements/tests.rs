//! Tests of the Announcements dialog opened as the `?` menu's history ([`super::open_history`]): what each announcement
//! shows, the order, that nothing is read as markup, the link, that reopening writes nothing, and closing. Then the
//! automatic batch ([`super::open_pending`]) as the window-start owner opens it: its order, what its dismissal records,
//! and when it opens nothing. Those run in the Welcome card's test window (`welcome::tests::open_start`), since the
//! batch is that owner's to open; the card's tests cover the batch waiting behind the card.
//!
//! As SSH help's tests do, the dialog is opened from a stand-in for the `?` button: one button in a root view of the
//! test's own, whose click takes a mark and calls [`super::open_history`] with its own focus handle, which is where
//! closing must return focus. The queue comes from the fake helm's snapshot, as the helm pushes it. Every step ends in
//! one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::announcements::{Announcement, AnnouncementLink, AnnouncementQueue};
use farhelm_proto::ui_channel::{ANNOUNCEMENTS_KEY, Collection, CollectionState, KeyedEntry};
use gpui::{
    Context, DriverNode, FocusHandle, IntoElement, Modifiers, ParentElement as _, Render,
    Styled as _, TestAppContext, VisualTestContext, Window, div, point, px, size,
};

use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::layers;
use crate::root;
use crate::test_support::{
    self, any_open, click, focused, focused_id, frame, keys, one_command, settle, started,
};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The root view the tests open the history from: a control standing in for whatever held focus before, and the `?`
/// button (standing in for its menu's Announcements item too) with a focus handle the test holds.
struct HelpBar {
    root: root::Attachment,
    toggle: FocusHandle,
}

impl Render for HelpBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = self.toggle.clone();
        let content = div()
            .size_full()
            .p(px(20.))
            .child(Button::new(
                "elsewhere",
                "Elsewhere",
                HoverHelp::tooltip("A control away from the ? button"),
                Role::Button,
            ))
            .child(
                Button::new(
                    "help-toggle",
                    "Help",
                    HoverHelp::tooltip("Help: announcements among other things"),
                    Role::Button,
                )
                .focus(&self.toggle)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    super::open_history(&toggle, &mark, window, cx);
                }),
            );
        root::wrap(content, &self.root, window, cx)
    }
}

/// An announcement with `id` and `title`, the given plain-text `body`, and no detail or link.
pub(crate) fn announcement(id: &str, title: &str, body: &str) -> Announcement {
    Announcement {
        id: id.to_owned(),
        title: title.to_owned(),
        body: body.to_owned(),
        detail: None,
        link: None,
    }
}

/// The announcements collection as the helm pushes it: one entry under its one key.
pub(crate) fn queue(pending: Vec<Announcement>, history: Vec<Announcement>) -> CollectionState {
    CollectionState {
        collection: Collection::Announcements,
        meta: test_support::meta(&[ANNOUNCEMENTS_KEY]),
        entries: vec![KeyedEntry {
            key: ANNOUNCEMENTS_KEY.into(),
            value: serde_json::to_value(AnnouncementQueue { pending, history }).unwrap(),
        }],
    }
}

/// The app over the fake helm with `snapshot`, in a window big enough for the dialog, with the `?` button's focus
/// handle. Nothing is open yet, and clicks focus no control (macOS's rule), so closing must name the button to put focus
/// back on it.
fn open_help_bar(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    helm.connect(snapshot);
    cx.run_until_parked();
    let toggle = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let held = toggle.clone();
    let (_, window) = test_support::open_view(cx, move |window, cx| HelpBar {
        root: root::attach(window, cx),
        toggle: held,
    });
    window.simulate_resize(size(px(1600.), px(1200.)));
    window.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    settle(window);
    (helm, window, toggle)
}

/// Move focus off the `?` button when it is there: Shift+Tab reaches the control before it. Focus that is already
/// elsewhere (or nowhere, as when the window opens) is left alone.
fn focus_elsewhere(cx: &mut VisualTestContext, toggle: &FocusHandle) {
    if focused(cx, toggle) {
        keys(cx, "shift-tab");
    }
}

/// Every line of text the frame painted, in paint order.
fn texts(nodes: &[DriverNode]) -> Vec<String> {
    nodes
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect()
}

/// Where `text` was first painted in `all`, failing when it was not.
fn at(all: &[String], text: &str) -> usize {
    all.iter()
        .position(|line| line == text)
        .unwrap_or_else(|| panic!("{text:?} was not painted: {all:?}"))
}

// ---- Tests ---------------------------------------------------------------------------------------------------------

/// SPEC.md, "Announcements": blank lines separate paragraphs, and whitespace-only paragraphs are not drawn (the old
/// dialog's `split("\n\n")` with blank parts dropped). Specified: a body with three paragraphs, one of them only
/// spaces, splits into the two with text, unchanged; a single line is one paragraph.
#[test]
fn bodies_split_into_paragraphs_at_blank_lines() {
    assert_eq!(
        super::paragraphs("First line\nstill first.\n\n   \n\nSecond."),
        vec!["First line\nstill first.", "Second."]
    );
    assert_eq!(super::paragraphs("Only one."), vec!["Only one."]);
}

/// SPEC.md, "Announcements": the history opens "the same dialog with all shown announcements, newest shown first, and
/// reopening writes nothing", and "closing this history view returns focus to Help"; the old dialog's look is the
/// title over "Updates and information from Farhelm.", each announcement's title, detail and paragraphs, and Got it.
/// Specified: with three shown announcements whose ids are in neither ascending nor descending order (the helm sends
/// history newest shown first, so only keeping its order passes, not sorting by id either way), the history shows the
/// title, the summary, all three in the helm's order with the first's detail line and both its paragraphs, and starts
/// on Got it; Got it closes it with focus back on the `?` button, and the helm received no command.
#[gpui::test]
fn the_history_shows_shown_announcements_newest_first_and_writes_nothing(cx: &mut TestAppContext) {
    let mut newest = announcement("2", "Newer news", "First paragraph.\n\nSecond paragraph.");
    newest.detail = Some("Since 0.40.0".to_owned());
    let older = announcement("1", "Older news", "Old body.");
    let oldest = announcement("3", "Oldest news", "Oldest body.");
    let (mut helm, cx, toggle) =
        open_help_bar(cx, vec![queue(vec![], vec![newest, older, oldest])]);
    assert!(
        !focused(cx, &toggle),
        "premise: focus is away from the ? button"
    );

    click(cx, "help-toggle");
    assert!(any_open(cx), "the history opened");
    let all = texts(&frame(cx));
    for text in ["Announcements", "Updates and information from Farhelm."] {
        at(&all, text);
    }
    assert!(
        at(&all, "Newer news") < at(&all, "Older news")
            && at(&all, "Older news") < at(&all, "Oldest news"),
        "the helm's order, newest shown first: {all:?}"
    );
    assert!(at(&all, "Newer news") < at(&all, "Since 0.40.0"));
    assert!(at(&all, "First paragraph.") < at(&all, "Second paragraph."));
    at(&all, "Old body.");
    assert_eq!(focused_id(cx).as_deref(), Some("announcements-got-it"));

    click(cx, "announcements-got-it");
    assert!(!any_open(cx), "Got it closed the history");
    assert!(focused(cx, &toggle), "focus is back on the ? button");
    assert!(
        helm.commands().is_empty(),
        "reopening the history writes nothing"
    );
}

/// SPEC.md: "Got it, the × and Escape all close it", and closing the history returns focus to Help either way.
/// Specified: from focus away from the `?` button, Escape closes the history with focus on the button; with focus moved
/// away again and the history reopened, the header's Close does the same.
#[gpui::test]
fn escape_and_the_header_close_close_the_history(cx: &mut TestAppContext) {
    let (_helm, cx, toggle) = open_help_bar(
        cx,
        vec![queue(vec![], vec![announcement("1", "News", "Body.")])],
    );
    for (close, how) in [
        (None, "Escape"),
        (Some("announcements-close"), "the header's Close"),
    ] {
        focus_elsewhere(cx, &toggle);
        assert!(
            !focused(cx, &toggle),
            "premise: focus is away from the ? button"
        );
        click(cx, "help-toggle");
        assert!(any_open(cx), "premise: the history opened");
        match close {
            None => keys(cx, "escape"),
            Some(id) => click(cx, id),
        }
        assert!(!any_open(cx), "{how} closed it");
        assert!(focused(cx, &toggle), "{how} returned focus to the ? button");
    }
}

/// SPEC.md, "Opening and closing": "If a dialog container itself has focus, Enter presses its enabled main action", and
/// the old dialog's main action was Got it. Specified: after a click on the dialog's empty space puts focus on the
/// dialog itself, Enter closes the history with focus back on the `?` button.
#[gpui::test]
fn enter_on_the_dialog_itself_presses_got_it(cx: &mut TestAppContext) {
    // With a link, so the body has no stop of its own: a click on a linkless body puts focus on its stop instead.
    let mut news = announcement("1", "News", "Body.");
    news.link = Some(AnnouncementLink {
        label: "Release notes".to_owned(),
        url: "https://farhelm.io/docs/release-notes/".to_owned(),
    });
    let (_helm, cx, toggle) = open_help_bar(cx, vec![queue(vec![], vec![news])]);
    click(cx, "help-toggle");
    // Inside the body's padding: empty space.
    let corner = test_support::bounds(cx, "announcements-body").origin;
    cx.simulate_click(
        point(corner.x + px(3.), corner.y + px(3.)),
        Modifiers::none(),
    );
    test_support::check(cx);
    let on_dialog = cx.update(|window, cx| {
        layers::top_container(window, cx).is_some_and(|dialog| dialog.is_focused(window))
    });
    assert!(on_dialog, "premise: the dialog itself has focus");

    keys(cx, "enter");
    assert!(!any_open(cx), "Enter pressed Got it");
    assert!(focused(cx, &toggle), "focus is back on the ? button");
}

/// SPEC.md: an announcement's text "is never interpreted as markup". Its labelled link opens in the browser through
/// the app's one opener (`cx.open_url`), and the old app's end-to-end test (`announcements.spec.ts` at reference commit
/// `24bc0580`) had the dialog stay open meanwhile. Specified: a body holding HTML and Markdown syntax is
/// painted exactly as written; the link is named by its label with the ↗ the old dialog showed, and clicking it opens
/// its address in the system browser while the dialog stays open.
#[gpui::test]
fn text_is_never_markup_and_the_link_opens_in_the_browser(cx: &mut TestAppContext) {
    let mut news = announcement("1", "News", "<b>not bold</b> and **not strong**");
    news.link = Some(AnnouncementLink {
        label: "Release notes".to_owned(),
        url: "https://farhelm.io/docs/release-notes/".to_owned(),
    });
    let (_helm, cx, _toggle) = open_help_bar(cx, vec![queue(vec![], vec![news])]);
    click(cx, "help-toggle");
    at(&texts(&frame(cx)), "<b>not bold</b> and **not strong**");
    assert_eq!(
        test_support::label(cx, "announcement-link-1").as_deref(),
        Some("Release notes ↗")
    );
    click(cx, "announcement-link-1");
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://farhelm.io/docs/release-notes/")
    );
    assert!(any_open(cx), "following the link leaves the dialog open");
}

/// SPEC.md: the Help item "appears after the first successful showing", so before any showing there is no history to
/// open; pending announcements are not history. Specified: with one pending and nothing shown, opening the history
/// opens nothing.
#[gpui::test]
fn with_nothing_shown_the_history_opens_nothing(cx: &mut TestAppContext) {
    let (_helm, cx, _toggle) = open_help_bar(
        cx,
        vec![queue(vec![announcement("1", "Pending", "Body.")], vec![])],
    );
    click(cx, "help-toggle");
    assert!(!any_open(cx), "nothing shown yet, so no history opened");
}

/// The old dialog's body was a keyboard stop of its own ("Announcement content"), so a long announcement without a link
/// could be scrolled from the keyboard; without one, nothing in such a body takes focus and the keys cannot reach it.
/// Specified: a history of one linkless announcement draws the body's stop under that name (the next test has the
/// linked case, which draws none).
#[gpui::test]
fn a_linkless_history_has_a_body_stop_named_announcement_content(cx: &mut TestAppContext) {
    let (_helm, window, _toggle) = open_help_bar(
        cx,
        vec![queue(vec![], vec![announcement("1", "News", "Body.")])],
    );
    click(window, "help-toggle");
    settle(window);
    assert_eq!(
        test_support::label(window, "announcements-body-stop").as_deref(),
        Some("Announcement content")
    );
}

/// The other half of the rule above, in its own window: a body holding a link has no stop of its own.
#[gpui::test]
fn a_linked_history_has_no_body_stop(cx: &mut TestAppContext) {
    let mut news = announcement("1", "News", "Body.");
    news.link = Some(AnnouncementLink {
        label: "Release notes".to_owned(),
        url: "https://farhelm.io/docs/release-notes/".to_owned(),
    });
    let (_helm, window, _toggle) = open_help_bar(cx, vec![queue(vec![], vec![news])]);
    click(window, "help-toggle");
    settle(window);
    assert_eq!(
        test_support::label(window, "announcement-link-1").as_deref(),
        Some("Release notes ↗"),
        "the premise: the history opened with its link"
    );
    assert_eq!(test_support::label(window, "announcements-body-stop"), None);
}

// ---- The automatic batch -------------------------------------------------------------------------------------------

/// The pending queue of `ids`, each titled after its id, in the helm's order (oldest first), from a helm that has
/// already shown the Welcome card, so the batch is the window start's first dialog.
fn pending(ids: &[&str]) -> Vec<CollectionState> {
    vec![
        crate::welcome::tests::seen(Some(true)),
        queue(
            ids.iter()
                .map(|id| announcement(id, &format!("Notice {id}"), "Body."))
                .collect(),
            vec![],
        ),
    ]
}

/// SPEC.md: "Once per window start or reset, after the initial preference decision, the client reads pending
/// announcements", the dialog shows "pending entries ... newest first", and "A failed preference read does not prevent
/// this separate check". The helm sends pending entries oldest first, so drawing them as sent would put the oldest
/// news on top. Specified: over a seen card and two pending entries, the batch opens by itself with the newer first
/// and focus on Got it; after a reset whose snapshot holds the queue but no preferences, it opens again.
#[gpui::test]
fn the_pending_batch_opens_at_window_start_newest_first(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, _home) =
        crate::welcome::tests::open_start(cx, Some(pending(&["older", "newer"])));
    assert!(
        crate::welcome::tests::shows(cx, "announcements"),
        "the batch opened by itself"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("announcements-got-it"));
    let all = texts(&frame(cx));
    assert!(
        at(&all, "Notice newer") < at(&all, "Notice older"),
        "newest first: {all:?}"
    );
    let without_preferences = pending(&["older", "newer"]).split_off(1);
    crate::welcome::tests::reset(&mut helm, cx, without_preferences);
    assert!(
        crate::welcome::tests::shows(cx, "announcements"),
        "a failed preference read does not stop the check"
    );
}

/// SPEC.md: "**Got it**, the × and Escape all close it. Dismissal records only the entries that dialog showed", which
/// the helm's `POST /api/announcements/shown` takes as an explicit list, so a newer entry the dialog did not show is
/// never drained. A reset is not a dismissal: the entries stay pending for the next UI session. Specified: for Got it,
/// the × and Escape in turn, closing the batch sends exactly one `POST /api/announcements/shown` naming the two shown
/// ids, newest first, and returns focus home; a reset (over the fake, which keeps them pending) opens the batch again;
/// a reset while it is open sends nothing.
#[gpui::test]
fn each_dismissal_records_exactly_the_shown_entries_once(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, home) =
        crate::welcome::tests::open_start(cx, Some(pending(&["older", "newer"])));
    for way in ["announcements-got-it", "announcements-close", "escape"] {
        assert!(
            crate::welcome::tests::shows(cx, "announcements"),
            "premise: the batch is open before {way}"
        );
        match way {
            "escape" => keys(cx, "escape"),
            "announcements-got-it" => click(cx, "announcements-got-it"),
            _ => click(cx, "announcements-close"),
        }
        assert!(!any_open(cx), "{way} closed the batch");
        let (id, body) = one_command(&mut helm, Method::Post, "/api/announcements/shown");
        assert_eq!(
            body,
            serde_json::json!({ "ids": ["newer", "older"] }),
            "{way} recorded what it showed"
        );
        helm.reply(id, 204, None);
        settle(cx);
        assert!(focused(cx, &home), "{way} returned focus home");
        crate::welcome::tests::reset(&mut helm, cx, pending(&["older", "newer"]));
    }
    crate::welcome::tests::reset(&mut helm, cx, pending(&["older", "newer"]));
    assert!(helm.commands().is_empty(), "a reset records nothing");
}

/// SPEC.md: "A failed write still closes the dialog and leaves them queued for a later window start or reset." A batch
/// that stayed up behind a write the user cannot fix would trap them. Specified: Got it closes the batch; the helm
/// refuses the record; the batch stays closed, nothing is retried, and the next reset shows it again.
#[gpui::test]
fn a_failed_record_still_closes_the_batch(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, _home) =
        crate::welcome::tests::open_start(cx, Some(pending(&["1"])));
    click(cx, "announcements-got-it");
    let (id, _) = one_command(&mut helm, Method::Post, "/api/announcements/shown");
    helm.reply(id, 500, None);
    settle(cx);
    assert!(!any_open(cx), "the refused record left the batch closed");
    assert!(helm.commands().is_empty(), "nothing retried the record");
    crate::welcome::tests::reset(&mut helm, cx, pending(&["1"]));
    assert!(
        crate::welcome::tests::shows(cx, "announcements"),
        "the next reset shows the batch again"
    );
}

/// SPEC.md: "An empty queue opens nothing; a failed announcement read is logged silently", and "there is no live
/// announcement event reader today": the check runs once per window start or reset, so an entry pushed later waits for
/// the next one. Specified: over a seen card and an empty queue nothing opens; an entry pushed afterwards opens nothing
/// either; a reset whose snapshot holds no queue at all opens nothing; a reset that brings the entry opens the batch;
/// nothing was written before that.
#[gpui::test]
fn an_empty_queue_or_a_later_push_opens_nothing(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::CollectionChanges;
    let (mut helm, cx, _toggle, _home) = crate::welcome::tests::open_start(cx, Some(pending(&[])));
    assert!(!any_open(cx), "an empty queue opens nothing");
    let later = queue(vec![announcement("1", "Later news", "Body.")], vec![]);
    helm.push(CollectionChanges {
        collection: Collection::Announcements,
        meta: later.meta.clone(),
        upsert: later.entries.clone(),
        remove: vec![],
    });
    settle(cx);
    assert!(
        !any_open(cx),
        "an entry pushed after the start's check opens nothing by itself"
    );
    crate::welcome::tests::reset(&mut helm, cx, vec![crate::welcome::tests::seen(Some(true))]);
    assert!(!any_open(cx), "no queue read, no batch");
    crate::welcome::tests::reset(&mut helm, cx, pending(&["1"]));
    assert!(
        crate::welcome::tests::shows(cx, "announcements"),
        "the next start's check finds it"
    );
    assert!(helm.commands().is_empty());
}
