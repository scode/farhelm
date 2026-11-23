//! Tests of Tunnel and open on a link card, in process against the helm client's fake: the answer sent at once when
//! the shared tunnel consent is recorded, the terminal's tunnel warning asked first when it is not, that warning
//! opening over an unrelated dialog and outliving it, and the terminal's tunnel list re-read once the helm took the
//! answer.
//!
//! They matter because Tunnel and open widens what a remote page can reach on this Mac: the helm refuses the answer
//! without the user's consent, so sending it before the warning was accepted would only fail, and sending it after the
//! user cancelled the warning, or after the request stopped waiting, would answer something the user did not approve.
//! And a tunnel the terminal does not know about would not show in its pill, where the user closes it.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_input_mapping::Platform;
use farhelm_proto::approvals::{ApprovalAction, ApprovalSession, PendingApproval};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY,
};
use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, div, px, size,
};
use serde_json::json;
use std::time::Duration;

use farhelm_helm_client::ApiPath;

use super::answers::{ARM_DELAY, Arming};
use super::model::TunnelOffer;
use super::tunnel::{Pressed, offer, tunnel_and_open};
use crate::focus::Mark;
use crate::host_text::HostText;
use crate::terminal::tunnels::{self, Tunnels};
use crate::test_support::{
    self, any_open, click, focused, is_open, meta, one_command, settle, started,
};
use crate::{layers, root};

// ---- Fixtures -------------------------------------------------------------------------------------------------------

/// The request's id.
const ID: &str = "link-1";

/// The asking session's reference.
const SESSION: &str = "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:remote";

/// A loopback link from a remote session, which the helm marks eligible for Tunnel and open.
fn link() -> PendingApproval {
    PendingApproval {
        id: ID.to_owned(),
        host_id: 2,
        host_name: "build".to_owned(),
        session: ApprovalSession {
            id: SESSION.to_owned(),
            title: Some("asker".to_owned()),
            host_name: Some("build".to_owned()),
            launch_kind: None,
        },
        action: ApprovalAction::OpenUrl {
            url: "http://127.0.0.1:8080/".to_owned(),
            local_network: true,
            preapproved: false,
            tunnel_available: true,
        },
        own_host_session_action: false,
        expires_at_ms: 1,
    }
}

/// The approvals collection holding the link request.
fn waiting() -> CollectionState {
    CollectionState {
        collection: Collection::Approvals,
        meta: meta(&[ID]),
        entries: vec![KeyedEntry {
            key: ID.to_owned(),
            value: serde_json::to_value(link()).expect("an approval encodes"),
        }],
    }
}

/// The preference row with the shared tunnel consent `consent`.
fn consent(consent: bool) -> CollectionState {
    CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: json!({ "tunnel_consent": consent }),
        }],
    }
}

/// An empty root view, as the window the card sits in.
struct Empty {
    root: root::Attachment,
}

impl Render for Empty {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        root::wrap(div().size_full(), &self.root, window, cx)
    }
}

/// The app over the fake helm with the link waiting and the consent as `recorded` says, an armed answer bar, a window,
/// and the asking session's tunnel state as a terminal view of it holds it.
fn fixture(
    cx: &mut TestAppContext,
    recorded: bool,
) -> (
    FakeHelm,
    &mut VisualTestContext,
    Entity<Arming>,
    Entity<Tunnels>,
) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![waiting(), consent(recorded)]);
    cx.run_until_parked();
    let arming = cx.new(|_| Arming::default());
    arming.update(cx, |arming, cx| {
        arming.observe(vec![ID.to_owned()], Some(ID.to_owned()), cx)
    });
    cx.executor().advance_clock(ARM_DELAY);
    cx.run_until_parked();
    assert!(
        arming.read_with(cx, |arming, _| arming.armed()),
        "premise: the answer bar is armed"
    );
    assert_eq!(
        cx.update(|cx| crate::terminal::tunnels::consented(cx)),
        recorded,
        "premise: the pushed consent is as asked"
    );
    let tunnels = cx.new(|cx| Tunnels::new(SESSION.to_owned(), cx));
    let (_, window) = test_support::open_view(cx, |window, cx| Empty {
        root: root::attach(window, cx),
    });
    window.simulate_resize(size(px(1200.), px(900.)));
    settle(window);
    (helm, window, arming, tunnels)
}

/// Press Tunnel and open on the link card, with a mark taken now; the refusal of the consent save, if one is heard.
fn press(
    cx: &mut VisualTestContext,
    arming: &Entity<Arming>,
) -> (Pressed, std::rc::Rc<std::cell::RefCell<Option<HostText>>>) {
    let refused = std::rc::Rc::new(std::cell::RefCell::new(None));
    let heard = refused.clone();
    let arming = arming.clone();
    let pressed = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        tunnel_and_open(&link(), &arming, &mark, window, cx, move |reason, _| {
            *heard.borrow_mut() = Some(reason);
        })
    });
    settle(cx);
    (pressed.expect("the press started"), refused)
}

/// The path of the answer route for the link request.
fn answer_path() -> String {
    format!("/api/approvals/{ID}")
}

// ---- Tests ----------------------------------------------------------------------------------------------------------

/// SPEC.md offers Tunnel and open "in the Mac app" only. Specified: the Mac offers it, Linux does not.
#[test]
fn tunnel_and_open_is_offered_in_the_mac_app_only() {
    assert_eq!(offer(Platform::Mac), TunnelOffer::Offered);
    assert_eq!(offer(Platform::Linux), TunnelOffer::NotOffered);
}

/// With the shared consent recorded, Tunnel and open answers at once, and once the helm took it the terminal's views
/// of the asking session (and only of it) read their tunnels again, at once and a second later, since the helm opens
/// the tunnel only after it answered. A refused answer opened nothing, so nothing is read. Specified: the press sends
/// `{"answer": "tunnel_and_open"}`; after its 204 the asking session's listing is read and another session's is not;
/// a second later it is read once more; a 409 to a second press reads nothing.
#[gpui::test]
fn with_consent_the_answer_goes_at_once_and_the_tunnels_are_read_again(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, true);
    let _other = cx.update(|_, cx| cx.new(|cx| Tunnels::new("another-session".to_owned(), cx)));
    let (pressed, _) = press(cx, &arming);
    assert_eq!(pressed, Pressed::Sent);
    let (id, body) = one_command(&mut helm, Method::Post, &answer_path());
    assert_eq!(body, json!({ "answer": "tunnel_and_open" }));
    helm.reply(id, 204, None);
    settle(cx);
    let listing = ApiPath::api(&["sessions", SESSION, "tunnels"]).expect("a valid path");
    let (read, _) = one_command(&mut helm, Method::Get, listing.as_str());
    helm.reply(
        read,
        200,
        Some(json!({ "available": true, "host_name": "build", "tunnels": [] })),
    );
    settle(cx);
    cx.executor().advance_clock(Duration::from_secs(1));
    settle(cx);
    let (again, _) = one_command(&mut helm, Method::Get, listing.as_str());
    helm.reply(
        again,
        200,
        Some(json!({ "available": true, "host_name": "build", "tunnels": [] })),
    );
    settle(cx);

    let (pressed, _) = press(cx, &arming);
    assert_eq!(pressed, Pressed::Sent, "premise: a second press is sent");
    let (id, _) = one_command(&mut helm, Method::Post, &answer_path());
    helm.reply(id, 409, Some(json!("the tunnel could not be opened")));
    settle(cx);
    cx.executor().advance_clock(Duration::from_secs(1));
    settle(cx);
    assert!(helm.commands().is_empty(), "a refused answer reads nothing");
}

/// Without consent, the helm would refuse the answer, so the terminal's tunnel warning is asked first, and the answer
/// goes out only on its Allow, after the consent was saved. Specified: the press asks the warning, its box drawn above
/// the approval cards, and sends nothing; Allow saves `{"tunnel_consent": true}`; once the save is taken, the answer
/// goes out.
#[gpui::test]
fn without_consent_the_warning_is_asked_and_its_allow_sends_the_answer(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, false);
    let (pressed, _) = press(cx, &arming);
    assert_eq!(pressed, Pressed::Asking);
    assert!(any_open(cx), "the tunnel warning is up");
    let raised = cx.update(|window, cx| layers::top_is_above_approval_region(window, cx));
    assert!(raised, "and its box is drawn above the approval cards");
    assert!(
        helm.commands().is_empty(),
        "nothing is sent before the warning is answered"
    );
    click(cx, "prompt-confirm");
    let (save, body) = one_command(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(body, json!({ "tunnel_consent": true }));
    helm.reply(save, 204, None);
    settle(cx);
    let (_, body) = one_command(&mut helm, Method::Post, &answer_path());
    assert_eq!(body, json!({ "answer": "tunnel_and_open" }));
}

/// Cancelling the warning answers nothing: the request keeps waiting on its card. A refused consent save answers
/// nothing either, and the card hears why. Specified: Cancel sends nothing; after Allow, a 409 to the save reaches the
/// press's refusal callback with the helm's words, and no answer is sent.
#[gpui::test]
fn a_cancelled_warning_or_a_refused_consent_answers_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, false);
    press(cx, &arming);
    click(cx, "prompt-cancel");
    assert!(!any_open(cx), "premise: the warning closed");
    assert!(helm.commands().is_empty(), "Cancel sent nothing");

    let (_, refused) = press(cx, &arming);
    click(cx, "prompt-confirm");
    let (save, _) = one_command(&mut helm, Method::Put, "/api/preferences");
    helm.reply(save, 409, Some(json!("preferences are read-only")));
    settle(cx);
    assert_eq!(
        *refused.borrow(),
        Some(HostText::from_host("preferences are read-only"))
    );
    assert!(helm.commands().is_empty(), "no answer after a refused save");
}

/// The warning goes with its request, as the old app retired a card's warning when its card vanished: an Allow after
/// the request ended would save consent for something no longer asked, and its answer could only be late. Specified:
/// the request ends while the warning is up; the warning closes by itself, and nothing is sent.
#[gpui::test]
fn the_warning_goes_with_its_request(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, false);
    let (pressed, _) = press(cx, &arming);
    assert_eq!(pressed, Pressed::Asking, "premise: the warning is up");
    helm.push(CollectionChanges {
        collection: Collection::Approvals,
        meta: meta(&[]),
        upsert: vec![],
        remove: vec![ID.into()],
    });
    settle(cx);
    assert!(!any_open(cx), "the warning closed with its request");
    assert!(helm.commands().is_empty(), "nothing was saved or answered");
}

/// A press while a tunnel warning is already up must not stack a second, identical warning over it, whoever asked
/// the first: the cards stay usable beside the warning, so a second press is easy to make. The card says why in the
/// old app's words, since a warning another opener asked answers nothing on this card. Specified: a press asks the
/// warning; a second press reports [`Pressed::AnotherTunnelAction`], whose reason is "Another tunnel action is still
/// waiting", opens nothing more (one Cancel leaves no layer open) and sends nothing; with Settings' warning up
/// ([`tunnels::ask_consent`]), a press reports the same, and one Cancel again leaves nothing open.
#[gpui::test]
fn a_press_while_a_tunnel_warning_is_up_asks_nothing_and_says_why(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, false);
    let (pressed, _) = press(cx, &arming);
    assert_eq!(pressed, Pressed::Asking, "premise: the warning is up");
    let (again, _) = press(cx, &arming);
    assert_eq!(again, Pressed::AnotherTunnelAction);
    assert_eq!(
        again.reason(),
        Some("Another tunnel action is still waiting"),
        "the card says why, in the old app's words"
    );
    click(cx, "prompt-cancel");
    assert!(!any_open(cx), "one Cancel closed the only warning");
    assert!(helm.commands().is_empty(), "nothing was sent");

    let settings = cx.update(|window, cx| {
        let mark = Mark::take(cx);
        tunnels::ask_consent(&mark, window, cx, |_, _| {})
    });
    settle(cx);
    assert!(settings.is_some(), "premise: Settings' warning is up");
    let (over, _) = press(cx, &arming);
    assert_eq!(
        over,
        Pressed::AnotherTunnelAction,
        "no warning over Settings' warning"
    );
    click(cx, "prompt-cancel");
    assert!(!any_open(cx), "one Cancel closed the only warning");
    assert!(helm.commands().is_empty(), "nothing was sent");
}

/// The old app opened the card's warning over whatever dialog was open, and SPEC.md's "Nothing closes a dialog, menu,
/// popup list or confirmation that it did not open" holds for the dialog's owner: closing that dialog (a bulk question
/// whose targets went away) must not take the warning the user is reading along, as closing a layer closes the layers
/// opened from it. On macOS a press on the card moves no focus, so focus is still in the dialog when the warning opens,
/// and the focus rules alone would make the dialog the warning's parent; the warning's opener says it was opened from
/// the card ([`layers::opened_from_approval_card`]). The press here comes from an update with focus left in the dialog,
/// standing in for that click; that a real click on a card leaves focus there is the layer manager's tests'.
///
/// Specified: with an unrelated dialog holding focus, a press asks the warning over the dialog, which stays open; the
/// dialog's owner closing it leaves the warning open, with nothing sent; the warning's Allow then saves the consent
/// and sends the answer.
#[gpui::test]
fn the_warning_opens_over_a_dialog_and_outlives_its_close(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, false);
    let inside = cx.update(|_, cx| cx.focus_handle().tab_stop(true));
    let (dialog, _, _) =
        test_support::open_dialog(cx, vec![("unrelated-control", inside.clone())], &inside);
    assert!(
        focused(cx, &inside),
        "premise: the unrelated dialog holds focus"
    );
    let (pressed, _) = press(cx, &arming);
    assert_eq!(
        pressed,
        Pressed::Asking,
        "the warning is asked over the dialog"
    );
    assert!(is_open(cx, dialog), "premise: the dialog is still open");

    cx.update(|window, cx| layers::close(dialog, window, cx));
    settle(cx);
    assert!(!is_open(cx, dialog), "premise: the dialog closed");
    assert!(any_open(cx), "the warning stayed open");
    assert!(helm.commands().is_empty(), "nothing was sent");

    click(cx, "prompt-confirm");
    let (save, body) = one_command(&mut helm, Method::Put, "/api/preferences");
    assert_eq!(body, json!({ "tunnel_consent": true }));
    helm.reply(save, 204, None);
    settle(cx);
    let (_, body) = one_command(&mut helm, Method::Post, &answer_path());
    assert_eq!(body, json!({ "answer": "tunnel_and_open" }));
}

/// The warning closes on Allow before its consent save comes back, so a press in between would pass every other
/// check and ask a second warning for a consent already being saved. Specified: after Allow, with the save not yet
/// answered, a press reports [`Pressed::Nothing`] and opens nothing; once the save is taken, the one answer goes out.
#[gpui::test]
fn a_press_while_the_consent_is_being_saved_asks_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, false);
    let (pressed, _) = press(cx, &arming);
    assert_eq!(pressed, Pressed::Asking, "premise: the warning is up");
    click(cx, "prompt-confirm");
    let (save, _) = one_command(&mut helm, Method::Put, "/api/preferences");
    assert!(!any_open(cx), "premise: Allow closed the warning");
    let (again, _) = press(cx, &arming);
    assert_eq!(again, Pressed::Nothing);
    assert!(!any_open(cx), "no second warning");
    helm.reply(save, 204, None);
    settle(cx);
    let (_, body) = one_command(&mut helm, Method::Post, &answer_path());
    assert_eq!(body, json!({ "answer": "tunnel_and_open" }));
}

/// With consent recorded, Tunnel and open opens nothing and answers at once, over a dialog as anywhere: cards stay
/// answerable over dialogs (SPEC.md). Specified: with consent and a dialog open, the press is sent.
#[gpui::test]
fn with_consent_a_press_over_a_dialog_is_sent(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, true);
    let inside = cx.update(|_, cx| cx.focus_handle().tab_stop(true));
    test_support::open_dialog(cx, vec![("dialog-control", inside.clone())], &inside);
    let (pressed, _) = press(cx, &arming);
    assert_eq!(pressed, Pressed::Sent);
    let (_, body) = one_command(&mut helm, Method::Post, &answer_path());
    assert_eq!(body, json!({ "answer": "tunnel_and_open" }));
}

/// The answer bar's pause holds Tunnel and open too: a press aimed at a card that just changed does nothing, not even
/// the warning. Specified: during a pause, the press reports [`Pressed::Nothing`], opens nothing and sends nothing.
#[gpui::test]
fn a_press_during_the_pause_does_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, arming, _tunnels) = fixture(cx, false);
    arming.update(cx, |arming, cx| {
        arming.observe(
            vec![ID.to_owned(), "other".to_owned()],
            Some(ID.to_owned()),
            cx,
        )
    });
    let (pressed, _) = press(cx, &arming);
    assert_eq!(pressed, Pressed::Nothing);
    assert!(!any_open(cx));
    assert!(helm.commands().is_empty());
}
