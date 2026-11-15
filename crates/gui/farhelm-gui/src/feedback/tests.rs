//! Tests of the Send feedback dialog: what it starts from, what Send sends and what becomes of the helm's answer, the
//! contact it remembers or clears afterwards, what it refuses before sending, and the rule that nothing closes it while
//! a send is out.
//!
//! The dialog is opened the way the Help menu opens it, with the `?` button as the opener that closing returns focus
//! to: one button in a root view of the test's own stands in for both the `?` button and its menu's item, and its click
//! takes a mark and calls [`super::open`] with its own focus handle (the Help menu's own tests open it from the real
//! menu). The helm is the helm client's fake, which replies to `POST /api/feedback` as each test says, so nothing
//! reaches a network; the helm's own forwarding is covered by its tests against a stand-in endpoint. The tests reach
//! the open dialog through [`Opened`], which only test builds record. Every step ends in one of the shared harness's
//! step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::feedback::{
    FEEDBACK_MESSAGE_MAX_CHARS, FEEDBACK_OS_MAX_CHARS, FEEDBACK_VERSION_MAX_CHARS,
    fit_machine_field,
};
use farhelm_proto::ui_channel::{CollectionChanges, Preferences};
use gpui::{
    Context, Entity, FocusHandle, Global, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, Toggled, VisualTestContext, WeakEntity, Window, div, px, size,
};
use serde_json::{Value, json};

use super::{FeedbackDialog, THANKS, THANKS_SHOWN, contact_after_success, failure_words, os_name};
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::host_text::HostText;
use crate::layers;
use crate::operations::{OpKey, Refusal, Work};
use crate::root;
use crate::session;
use crate::session_list::tests::preferences_state;
use crate::test_support::{
    self, any_open, click, focused, focused_id, frame, keys, label, one_command, settle, started,
};

/// The dialog [`super::open`] opened last, recorded in test builds only, so a test can read its fields.
pub(super) struct Opened(pub(super) WeakEntity<FeedbackDialog>);

impl Global for Opened {}

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Kit's select-all in a text field on this platform.
const SELECT_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a"
} else {
    "ctrl-a"
};

/// The project's Discord invite, as SPEC.md names it.
const DISCORD: &str = "https://discord.gg/G3bukxTjXR";

/// The helm's own sentence for an endpoint it could not reach, as its feedback route words it.
const UNREACHABLE: &str = "Couldn't send feedback: the feedback service could not be reached.";

/// The root view the tests open the dialog from: the `?` button (standing in for its menu's item too) as a plain button
/// with a focus handle the test holds, after another control standing in for whatever held focus before.
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
                    HoverHelp::tooltip("Help: feedback, docs and SSH help"),
                    Role::Button,
                )
                .focus(&self.toggle)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    super::open(&toggle, &mark, window, cx);
                }),
            );
        root::wrap(content, &self.root, window, cx)
    }
}

/// The app over the fake helm, whose preferences remember `contact`, in a window big enough for the dialog, under
/// macOS's click rule (a click leaves a button unfocused, so focus returning to the `?` button is the dialog's doing),
/// with the `?` button's focus handle. Nothing is open yet.
fn open_bar<'a>(
    cx: &'a mut TestAppContext,
    contact: Option<&str>,
) -> (FakeHelm, &'a mut VisualTestContext, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![remembering(contact)]);
    cx.run_until_parked();
    let toggle = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let held = toggle.clone();
    let (_, window) = test_support::open_view(cx, move |window, cx| HelpBar {
        root: root::attach(window, cx),
        toggle: held,
    });
    window.simulate_resize(size(px(1200.), px(900.)));
    window.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    settle(window);
    (helm, window, toggle)
}

/// The preferences collection remembering `contact` as the feedback contact.
fn remembering(contact: Option<&str>) -> farhelm_proto::ui_channel::CollectionState {
    preferences_state(Preferences {
        feedback_contact: contact.map(str::to_owned),
        ..Preferences::default()
    })
}

/// Open the dialog with a click on the `?` button's stand-in, checking that it opened.
fn open_dialog(cx: &mut VisualTestContext) {
    click(cx, "help-toggle");
    assert!(any_open(cx), "the premise: the dialog opened");
}

/// The dialog opened last, which must still be open.
fn dialog(cx: &mut VisualTestContext) -> Entity<FeedbackDialog> {
    let dialog = cx.update(|_, cx| {
        cx.try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
    });
    dialog.expect("the premise: a dialog is open")
}

/// The message field's text.
fn message(cx: &mut VisualTestContext) -> String {
    let dialog = dialog(cx);
    cx.update(|_, cx| dialog.read(cx).message.text(cx).to_string())
}

/// The contact field's text.
fn contact(cx: &mut VisualTestContext) -> String {
    let dialog = dialog(cx);
    cx.update(|_, cx| dialog.read(cx).contact.text(cx).to_string())
}

/// Whether the message field holds focus.
fn message_focused(cx: &mut VisualTestContext) -> bool {
    let dialog = dialog(cx);
    cx.update(|window, cx| dialog.read(cx).message.focus_handle(cx).is_focused(window))
}

/// Put focus in the contact field with a click, select what it holds, and type `keystrokes` over it.
fn type_contact(cx: &mut VisualTestContext, keystrokes: &str) {
    click(cx, "feedback-contact-field");
    keys(cx, SELECT_ALL);
    keys(cx, keystrokes);
}

/// The Re-use checkbox's checked state as drawn, or `None` while it is not drawn.
fn reuse(cx: &mut VisualTestContext) -> Option<bool> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("feedback-reuse"))
        .map(|node| node.toggled == Some(Toggled::True))
}

/// Whether `words` are drawn anywhere in the window, for text that has no id of its own (the dialog's footer note).
fn shows(cx: &mut VisualTestContext, words: &str) -> bool {
    frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter())
        .any(|text| text.text.as_ref() == words)
}

/// What the dialog shows and sends besides the fields: the version and operating system fitted to their caps, and the
/// desktop surface.
fn fitted() -> (String, String) {
    (
        fit_machine_field(farhelm_proto::BUILD_VERSION, FEEDBACK_VERSION_MAX_CHARS),
        fit_machine_field(os_name(), FEEDBACK_OS_MAX_CHARS),
    )
}

/// The body a send of `message` with `contact` must carry: those, and exactly the values [`fitted`] gives.
fn submission(message: &str, contact: Option<&str>) -> Value {
    let (version, os) = fitted();
    json!({ "message": message, "contact": contact, "version": version, "surface": "desktop", "os": os })
}

/// Press Send and take the one feedback command it sent: its id to answer, and its body.
fn send(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> (u64, Value) {
    click(cx, "feedback-send");
    one_command(helm, Method::Post, "/api/feedback")
}

/// Answer the feedback command `id` with `status` and `body`, and let the dialog settle.
fn answer(helm: &mut FakeHelm, cx: &mut VisualTestContext, id: u64, status: u16, body: Value) {
    let body = (status != 204).then_some(body);
    helm.reply(id, status, body);
    settle(cx);
}

// ---- The rules as plain functions ----------------------------------------------------------------------------------

/// SPEC.md: "A successful send remembers exactly the contact sent when checked; when unchecked, or when the user
/// emptied a prefilled field, it clears the remembered contact." The rule is the one place the three cases meet, so a
/// wrong branch here would quietly keep or lose a contact the user chose about. Specified: a sent contact is kept as
/// sent (spaces included) when Re-use is checked, cleared when it is not, and nothing sent, or a blank one, clears it.
#[test]
fn a_successful_send_remembers_only_an_opted_in_nonblank_contact() {
    let sent = Some("  contact@example.test  ".to_string());
    assert_eq!(contact_after_success(sent.clone(), true), sent);
    assert_eq!(contact_after_success(sent, false), None);
    assert_eq!(contact_after_success(None, true), None);
    assert_eq!(
        contact_after_success(Some(" \u{2003} ".to_string()), true),
        None
    );
}

/// SPEC.md: a failure "says sending failed, in plain words". The helm's feedback route words every refusal of its own
/// that way, but a body it could not read gets axum's text, which is not written for people. Specified: a sentence
/// starting "Couldn't send feedback:" is shown as the helm sent it; anything else becomes the plain sentence naming the
/// status, as the old app's did.
#[test]
fn a_refusal_is_shown_in_plain_words() {
    let refusal = |words: &str, status| Refusal {
        words: HostText::from_host(words),
        status,
        markers: Vec::new(),
    };
    assert_eq!(failure_words(&refusal(UNREACHABLE, 502)).raw(), UNREACHABLE);
    assert_eq!(
        failure_words(&refusal("length limit exceeded", 413)).raw(),
        "Couldn't send feedback: Farhelm could not take the request (HTTP 413)."
    );
}

/// The operation store's two cross-area questions about work (`Work::creates_a_session` and
/// `Work::changes_host_registry`) decide whether a launch or a host registry change waits for other work, and
/// `operations.rs` asks each new kind of work to pin its answers in its own area's tests. A feedback send neither starts
/// a session nor touches a host, so it must hold back neither. Specified: `Work::SendFeedback` answers no to both.
#[test]
fn a_feedback_send_neither_creates_a_session_nor_changes_the_host_registry() {
    assert!(!Work::SendFeedback.creates_a_session());
    assert!(!Work::SendFeedback.changes_host_registry());
}

/// The dialog names the operating system the app was built for rather than anything more specific, and a test build
/// runs on one of the shipped platforms, so its name is never the `unknown` fallback. Specified: the name is "macOS" on
/// a Mac and "Linux" on Linux. (That the dialog shows "Desktop app" is checked where it opens.)
#[test]
fn the_attached_os_names_the_platform() {
    let expected = if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    };
    assert_eq!(os_name(), expected);
}

// ---- Opening -------------------------------------------------------------------------------------------------------

/// SPEC.md: the dialog offers "a required message, an optional field for how to reach the user, and a plain display of
/// everything else that will be sent", and "Each dialog starts with the current remembered contact and reuse checked".
/// Nothing may leave the machine before Send. Specified: opening with "remembered" stored shows it in the contact
/// field with Re-use checked, focus in the message field, the version, "Desktop app" and the operating system as
/// fitted, and sends nothing; Send with a blank message sends nothing either.
#[gpui::test]
fn it_opens_on_the_remembered_contact_and_sends_nothing_before_send(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle) = open_bar(cx, Some("remembered"));
    open_dialog(cx);
    assert!(message_focused(cx), "focus starts in the message");
    assert_eq!(contact(cx), "remembered");
    assert_eq!(reuse(cx), Some(true), "Re-use shows, checked");
    let (version, os) = fitted();
    assert_eq!(label(cx, "feedback-version"), Some(version));
    assert_eq!(
        label(cx, "feedback-surface").as_deref(),
        Some("Desktop app")
    );
    assert_eq!(label(cx, "feedback-os"), Some(os));
    click(cx, "feedback-send");
    assert_eq!(helm.commands(), [], "nothing was sent");
}

/// SPEC.md: the dialog "starts with the current remembered contact", read from the helm's pushed preferences, and
/// nothing pushed may change what the user is typing ("A refresh never changes what the user is typing"). Specified:
/// with nothing remembered, the contact is empty and Re-use is not drawn; a push remembering "elsewhere" while the
/// dialog is open leaves the field empty; the next dialog starts on "elsewhere".
#[gpui::test]
fn a_pushed_contact_seeds_the_next_dialog_but_never_the_open_one(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle) = open_bar(cx, None);
    open_dialog(cx);
    assert_eq!(contact(cx), "");
    assert_eq!(reuse(cx), None, "Re-use is not drawn for a blank contact");
    let pushed = remembering(Some("elsewhere"));
    helm.push(CollectionChanges {
        collection: pushed.collection,
        meta: pushed.meta,
        upsert: pushed.entries,
        remove: Vec::new(),
    });
    settle(cx);
    assert_eq!(contact(cx), "", "the open dialog's field is untouched");
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed the dialog");
    open_dialog(cx);
    assert_eq!(contact(cx), "elsewhere");
}

/// SPEC.md: "Under the header summary, the dialog offers "Rather talk in the open? Chat on Discord". Its Chat on
/// Discord link opens the same invite through the same browser opener as the menu item, leaving the dialog and its
/// draft open." Specified: with focus in the message as the dialog opens, after typing a message, a click on Chat on
/// Discord opens the invite, and the dialog is still open with the message as typed.
#[gpui::test]
fn chat_on_discord_opens_the_invite_and_keeps_the_draft(cx: &mut TestAppContext) {
    let (_helm, cx, _toggle) = open_bar(cx, None);
    open_dialog(cx);
    assert!(
        message_focused(cx),
        "the premise: typing goes to the message"
    );
    keys(cx, "k e e p");
    click(cx, "feedback-discord");
    assert_eq!(cx.opened_url().as_deref(), Some(DISCORD));
    assert!(any_open(cx), "the dialog stays open");
    assert_eq!(message(cx), "keep");
}

// ---- Sending -------------------------------------------------------------------------------------------------------

/// SPEC.md: the display "is the whole submission", the helm sends it ("the window never sends it itself"), and "On
/// success the dialog thanks the user and closes"; a successful send with Re-use checked "remembers exactly the
/// contact sent" in a separate write after it. Specified: a two-line message and a typed contact go out as one
/// `POST /api/feedback` whose body is exactly those and the displayed values; Re-use appeared once the contact was not
/// blank; the helm's 204 shows the thanks, moves focus to the footer's Close and writes the contact as sent with
/// `PUT /api/preferences`; after the thanks' time the dialog closes with focus on the `?` button.
#[gpui::test]
fn send_sends_exactly_what_is_shown_then_thanks_and_closes(cx: &mut TestAppContext) {
    let (mut helm, cx, toggle) = open_bar(cx, None);
    open_dialog(cx);
    keys(cx, "o n e enter t w o");
    assert_eq!(reuse(cx), None, "the premise: no contact, no Re-use");
    type_contact(cx, "space m e space");
    assert_eq!(reuse(cx), Some(true), "Re-use appears, checked");
    let (id, body) = send(&mut helm, cx);
    assert_eq!(body, submission("one\ntwo", Some(" me ")));
    answer(&mut helm, cx, id, 204, Value::Null);
    assert_eq!(label(cx, "feedback-thanks").as_deref(), Some(THANKS));
    assert_eq!(focused_id(cx).as_deref(), Some("feedback-cancel"));
    assert_eq!(label(cx, "feedback-cancel").as_deref(), Some("Close"));
    let (_, remembered) = one_command(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(remembered, json!({ "feedback_contact": " me " }));

    test_support::advance(cx, THANKS_SHOWN);
    assert!(!any_open(cx), "the dialog closed itself");
    assert!(focused(cx, &toggle), "focus went back to the ? button");
}

/// SPEC.md: an unchecked Re-use, or a prefilled contact the user emptied, clears the remembered contact after a
/// successful send, and "Every successful send writes the preference, even when it looks unchanged locally" (another
/// client may have changed it meanwhile). The submission itself is unchanged by the choice. Specified, each from a
/// dialog opened on "remembered": unchecked, the body carries "remembered" and the write clears it (`null`); emptied,
/// the body carries no contact and the write clears it; left as it was, the write sends "remembered" again.
#[gpui::test]
fn a_successful_send_clears_or_rewrites_the_remembered_contact(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle) = open_bar(cx, Some("remembered"));
    for (case, sent, written) in [
        ("unchecked", Some("remembered"), Value::Null),
        ("emptied", None, Value::Null),
        ("as it was", Some("remembered"), json!("remembered")),
    ] {
        open_dialog(cx);
        keys(cx, "h i");
        match case {
            "unchecked" => click(cx, "feedback-reuse"),
            "emptied" => {
                type_contact(cx, "backspace");
                assert_eq!(reuse(cx), None, "{case}: Re-use goes with the contact");
            }
            _ => {}
        }
        let (id, body) = send(&mut helm, cx);
        assert_eq!(body, submission("hi", sent), "{case}");
        answer(&mut helm, cx, id, 204, Value::Null);
        let (put, remembered) = one_command(&mut helm, Method::Put, "/api/preferences");
        assert_eq!(remembered, json!({ "feedback_contact": written }), "{case}");
        // Answered before the next case sends, since the fake answers commands in the order they came.
        helm.reply(put, 204, None);
        test_support::advance(cx, THANKS_SHOWN);
        assert!(!any_open(cx), "{case}: the premise: the dialog closed");
    }
}

/// SPEC.md: "On any failure ... it says sending failed, in plain words, and keeps the typed text so the user can retry
/// or copy it. Nothing is queued or retried later", and "A failed send changes nothing" about the remembered contact.
/// Specified: the helm's refusal shows its sentence followed by the note that the text is still there, the message and
/// the contact keep what was typed, and no preference is written; a refusal in words not written for people (a 413's
/// "length limit exceeded") shows the plain sentence naming its status; the footer says "Nothing was sent" after a
/// failure, and while a retry is out neither that note nor the last failure's line shows, since whether anything was
/// sent is what the user is waiting to learn; a retry sends the same text, and its success thanks the user.
#[gpui::test]
fn a_failed_send_says_so_and_keeps_the_text_for_a_retry(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle) = open_bar(cx, Some("remembered"));
    open_dialog(cx);
    keys(cx, "k e e p");
    let (id, _) = send(&mut helm, cx);
    answer(&mut helm, cx, id, 502, json!(UNREACHABLE));
    assert_eq!(
        label(cx, "feedback-failure").as_deref(),
        Some(
            format!("{UNREACHABLE} Your text is still here, so you can try again or copy it.")
                .as_str()
        )
    );
    assert_eq!(message(cx), "keep");
    assert_eq!(contact(cx), "remembered");
    assert_eq!(
        helm.commands(),
        [],
        "nothing else was sent: no preference write"
    );

    assert!(
        shows(cx, "Nothing was sent"),
        "the footer says nothing was sent"
    );
    let (id, _) = send(&mut helm, cx);
    assert_eq!(
        label(cx, "feedback-failure"),
        None,
        "the retry hides the last failure while it is out"
    );
    assert!(
        !shows(cx, "Nothing was sent"),
        "no footer note while a send is out"
    );
    answer(&mut helm, cx, id, 413, json!("length limit exceeded"));
    assert_eq!(
        label(cx, "feedback-failure").as_deref(),
        Some(
            "Couldn't send feedback: Farhelm could not take the request (HTTP 413). Your text is still here, so you \
             can try again or copy it."
        )
    );

    let (id, body) = send(&mut helm, cx);
    assert_eq!(
        body,
        submission("keep", Some("remembered")),
        "the retry sends the same text"
    );
    answer(&mut helm, cx, id, 204, Value::Null);
    assert_eq!(label(cx, "feedback-thanks").as_deref(), Some(THANKS));
}

/// The old app's `may_close`, and the stay-open rule of the operation's busy state: closing while the send is out
/// would drop its outcome and lose the text, and a second send would file a second issue. The dialog reads whether a
/// send is out from the operation store, never a copy of its own, so the first part starts feedback work on the
/// store's feedback key from outside the dialog, which only the store can tell the dialog about. A close can
/// reach the dialog before the redraw that disables Cancel (a click queued behind Send), so the rule must hold for the
/// dismissal itself, not only for the disabled controls; the same goes for a second send. Specified: with feedback work
/// started on the store's key by something other than the dialog, Send reads "Sending…" and Escape leaves the dialog
/// open, and once the helm answers that work Escape closes it. Then, in a new dialog, with its own send out, Send reads
/// "Sending…"; Escape, and a dismissal standing in for a Cancel or Close that beat the redraw, leave the dialog open;
/// typed keys do not change the message; a press on the busy Send, and a second send standing in for one that beat
/// the redraw (Send, or Enter on the dialog itself, which runs the same send), send nothing more. Once the helm
/// refuses, the text is still there and Escape closes the dialog with focus on the `?` button.
#[gpui::test]
fn nothing_closes_the_dialog_or_sends_again_while_a_send_is_out(cx: &mut TestAppContext) {
    let (mut helm, cx, toggle) = open_bar(cx, None);
    open_dialog(cx);
    let started = cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("the premise: a live UI session");
        let (version, os) = fitted();
        let outside = farhelm_proto::feedback::FeedbackSubmission {
            message: "from elsewhere".to_owned(),
            contact: None,
            version,
            surface: farhelm_proto::feedback::FeedbackSurface::Desktop,
            os,
        };
        let command = super::command(&outside).expect("the premise: the route builds");
        operations.update(cx, |operations, cx| {
            operations.start(&OpKey::feedback(), command, cx, |_, _| {})
        })
    });
    assert_eq!(started, Ok(()), "the premise: the outside work started");
    let (id, _) = one_command(&mut helm, Method::Post, "/api/feedback");
    test_support::redraw(cx);
    assert_eq!(
        label(cx, "feedback-send").as_deref(),
        Some("Sending…"),
        "the dialog sees the store's work as its send"
    );
    keys(cx, "escape");
    assert!(any_open(cx), "Escape leaves the dialog open");
    answer(&mut helm, cx, id, 204, Value::Null);
    keys(cx, "escape");
    assert!(
        !any_open(cx),
        "once the store's work settled, Escape closes it"
    );

    open_dialog(cx);
    keys(cx, "w a i t");
    let (id, _) = send(&mut helm, cx);
    assert_eq!(label(cx, "feedback-send").as_deref(), Some("Sending…"));

    keys(cx, "escape");
    assert!(any_open(cx), "Escape leaves the dialog open");
    let layer = dialog(cx).read_with(cx, |dialog, _| dialog.layer);
    let layer = layer.expect("the premise: the dialog knows its layer");
    cx.update(|window, cx| layers::dismiss(layer, window, cx));
    test_support::check(cx);
    assert!(any_open(cx), "a dismissal leaves it open");

    click(cx, "feedback-message-field");
    assert!(
        message_focused(cx),
        "the premise: the keys go to the message"
    );
    keys(cx, "x");
    assert_eq!(
        message(cx),
        "wait",
        "the message is read-only while sending"
    );
    click(cx, "feedback-send");
    let opened = dialog(cx);
    cx.update(|window, cx| opened.update(cx, |dialog, cx| dialog.send(window, cx)));
    test_support::check(cx);
    assert_eq!(helm.commands(), [], "nothing more was sent");

    answer(&mut helm, cx, id, 502, json!(UNREACHABLE));
    assert_eq!(message(cx), "wait", "the failed send keeps the text");
    keys(cx, "escape");
    assert!(!any_open(cx), "with no send out, Escape closes it");
    assert!(focused(cx, &toggle), "focus went back to the ? button");
}

/// SPEC_impl.md, "Feedback forwarding": the caps are shared, so the dialog refuses what the helm would refuse before
/// anything is sent, and says why when the user could not otherwise tell. Specified: a whitespace-only message sends
/// nothing and shows no problem; a message one character over the cap shows "The message is longer than 4000
/// characters" and Send sends nothing; at the cap the line goes and Send sends.
#[gpui::test]
fn blank_and_over_long_messages_are_refused_before_sending(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle) = open_bar(cx, None);
    open_dialog(cx);
    keys(cx, "space enter space");
    assert_eq!(
        message(cx),
        " \n ",
        "the premise: the message holds only whitespace"
    );
    click(cx, "feedback-send");
    assert_eq!(helm.commands(), [], "a blank message sends nothing");
    assert_eq!(label(cx, "feedback-problem"), None, "and says nothing");

    let opened = dialog(cx);
    let set = |text: String, cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            let field = opened.read(cx).message.clone();
            field.set_text(text, window, cx);
        });
        test_support::redraw(cx);
    };
    set("x".repeat(FEEDBACK_MESSAGE_MAX_CHARS + 1), cx);
    assert_eq!(
        label(cx, "feedback-problem").as_deref(),
        Some("The message is longer than 4000 characters")
    );
    click(cx, "feedback-send");
    assert_eq!(helm.commands(), [], "an over-long message sends nothing");

    set("x".repeat(FEEDBACK_MESSAGE_MAX_CHARS), cx);
    assert_eq!(label(cx, "feedback-problem"), None);
    let (_, body) = send(&mut helm, cx);
    assert_eq!(
        body["message"],
        json!("x".repeat(FEEDBACK_MESSAGE_MAX_CHARS))
    );
}
