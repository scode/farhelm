//! Tests of an agent's host add through the ordinary Add host dialog, in process against the helm client's fake: the
//! open from a card, Add answering Allow before anything is probed, Cancel and Escape answering Deny, the setup
//! confirmation that always shows, and the dialog following its request.
//!
//! They matter because the dialog is the only place an agent's host add is approved: opening it must approve nothing,
//! Allow must carry what the user left in the field rather than what the agent asked for, nothing may contact the
//! destination before the helm accepted the Allow, and a Cancel must tell the agent no. Each would otherwise let an
//! agent add a host, or dial one, without the user's answer. The last section checks that what opens in the dialog's
//! place belongs to the request's card, as the dialog does, so that an unrelated dialog's close does not take it along.
//!
//! The dialog is opened the way a card's action will open it: from a control of the test's own root view, with a mark
//! taken in the same update, calling [`open_for_agent`] with an [`AgentRequest`] built from the pushed request. The
//! tests reach the dialog's fields through the Add host tests' shared helpers. Typed text avoids `-` and spaces, which
//! GPUI's simulated input reads as keystroke syntax rather than characters.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::approvals::{ApprovalAction, ApprovalAnswer, ApprovalSession, PendingApproval};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY,
};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement as _, Render,
    Styled as _, TestAppContext, VisualTestContext, Window, div, px, size,
};
use serde_json::{Value, json};

use super::answers::{self, ARM_DELAY, Arming, Sent};
use super::host_add::{AgentRequest, NOT_CONFIRMED};
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::host_text::tests::painted_words;
use crate::hosts::add::tests::{alias, alias_focused, destination, error_line, type_text};
use crate::hosts::add::{NotOpened, open_for_agent};
use crate::hosts::setup;
use crate::hosts::tests::{connected, host, hosts, start_work};
use crate::layers::LayerId;
use crate::notices::{self, Notice};
use crate::operations::{OpKey, Work};
use crate::test_support::{
    self, any_open, click, focused, focused_id, frame, handle, is_open, keys, label, meta,
    one_command, settle, started,
};
use crate::{layers, root};

// ---- Fixtures -------------------------------------------------------------------------------------------------------

/// The request's id.
const ID: &str = "add-1";

/// An agent's request from host "build" to add `ops@new.example` under the alias `newbox`.
fn request() -> PendingApproval {
    PendingApproval {
        id: ID.to_owned(),
        host_id: 2,
        host_name: "build".to_owned(),
        session: ApprovalSession {
            id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:asker".to_owned(),
            title: Some("asker title".to_owned()),
            host_name: Some("build".to_owned()),
        },
        action: ApprovalAction::HostAdd {
            destination: "ops@new.example".to_owned(),
            alias: Some("newbox".to_owned()),
        },
        own_host_session_action: false,
        expires_at_ms: 1,
    }
}

/// `approval` as the approvals collection keys it.
fn entry(approval: &PendingApproval) -> KeyedEntry {
    KeyedEntry {
        key: approval.id.clone(),
        value: serde_json::to_value(approval).expect("an approval encodes"),
    }
}

/// The approvals collection holding `approval`.
fn waiting(approval: &PendingApproval) -> CollectionState {
    CollectionState {
        collection: Collection::Approvals,
        meta: meta(&[ID]),
        entries: vec![entry(approval)],
    }
}

/// The preference row with "Set up new hosts without asking" on.
fn never_ask() -> CollectionState {
    CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: json!({ "skip_host_setup_confirmation": true }),
        }],
    }
}

/// The root view: a control standing in for the card's Add host action, which gets focus back when the dialog closes.
struct Card {
    root: root::Attachment,
    opener: FocusHandle,
}

impl Render for Card {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = div().size_full().p(px(20.)).child(
            Button::new(
                "card-open-add",
                "Open add dialog",
                HoverHelp::tooltip("Open add dialog: review the host in the Add host dialog"),
                Role::Button,
            )
            .focus(&self.opener),
        );
        root::wrap(content, &self.root, window, cx)
    }
}

/// What a test drives: the waiting request, the fake helm, the window and the card's control, and the answer bar's
/// pause the cards share.
struct Fixture<'a> {
    approval: PendingApproval,
    helm: FakeHelm,
    cx: &'a mut VisualTestContext,
    opener: FocusHandle,
    arming: Entity<Arming>,
}

/// The app over the fake helm with [`request`] waiting and `more` beside it, an armed answer bar, and nothing open.
fn fixture(cx: &mut TestAppContext, more: Vec<CollectionState>) -> Fixture<'_> {
    fixture_with(cx, request(), more)
}

/// [`fixture`], with `approval` as the waiting request.
fn fixture_with(
    cx: &mut TestAppContext,
    approval: PendingApproval,
    more: Vec<CollectionState>,
) -> Fixture<'_> {
    let (mut helm, _connection) = started(cx);
    let mut snapshot = vec![hosts(&[host(1, "local", connected())]), waiting(&approval)];
    snapshot.extend(more);
    helm.connect(snapshot);
    cx.run_until_parked();
    let arming = cx.new(|_| Arming::default());
    arming.update(cx, |arming, cx| {
        arming.observe(vec![ID.to_owned()], Some(ID.to_owned()), cx)
    });
    cx.executor().advance_clock(ARM_DELAY);
    cx.run_until_parked();
    let opener = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let held = opener.clone();
    let (_, window) = test_support::open_view(cx, move |window, cx| Card {
        root: root::attach(window, cx),
        opener: held,
    });
    window.simulate_resize(size(px(1600.), px(1000.)));
    settle(window);
    Fixture {
        approval,
        helm,
        cx: window,
        opener,
        arming,
    }
}

impl Fixture<'_> {
    /// The card's action: open the dialog for the pushed request, as a press would, with a mark taken now.
    fn open(&mut self) -> Result<(), NotOpened> {
        let opener = self.opener.clone();
        let arming = self.arming.clone();
        let approval = self.approval.clone();
        let opened = self.cx.update(|window, cx| {
            let mark = Mark::take(cx);
            let request = AgentRequest::of(&approval, arming).expect("a host add");
            open_for_agent(request, &opener, &mark, window, cx).map(|_| ())
        });
        settle(self.cx);
        opened
    }

    /// The one command sent since the last call, which must be an answer to the request: its id and body.
    fn one_answer(&mut self) -> (u64, Value) {
        one_command(
            &mut self.helm,
            Method::Post,
            &format!("/api/approvals/{ID}"),
        )
    }

    /// The helm pushes the request again unchanged, as any refresh of the waiting list does.
    fn request_refreshed(&mut self) {
        self.helm.push(CollectionChanges {
            collection: Collection::Approvals,
            meta: meta(&[ID]),
            upsert: vec![entry(&self.approval)],
            remove: vec![],
        });
    }

    /// The helm stops holding the request, as it pushes before it replies to the answer that ended it.
    fn request_ends(&mut self) {
        self.helm.push(CollectionChanges {
            collection: Collection::Approvals,
            meta: meta(&[]),
            upsert: vec![],
            remove: vec![ID.into()],
        });
    }

    /// Whether the store still lists the request as waiting.
    fn still_waiting(&mut self) -> bool {
        self.cx.update(|_, cx| {
            crate::session::current_store(cx).is_some_and(|store| {
                let collections = store.read(cx).collections();
                collections
                    .ordered::<PendingApproval>()
                    .iter()
                    .any(|approval| approval.id == ID)
            })
        })
    }

    /// Accept the dialog's Allow (the request gone first, as the helm pushes), and return the probe that follows.
    fn allow_accepted(&mut self) -> u64 {
        click(self.cx, "add-host-submit");
        let (id, _) = self.one_answer();
        self.request_ends();
        self.helm.reply(id, 204, None);
        settle(self.cx);
        one_command(&mut self.helm, Method::Post, "/api/hosts/probe").0
    }
}

/// Every line of text the window painted, as words on screen (a sentence's isolates left out), joined with single
/// spaces.
fn painted(cx: &mut VisualTestContext) -> String {
    let lines: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| painted_words(&line.text)))
        .collect();
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The strings painted inside the element with id `id`, as words on screen (a sentence's isolates left out), joined
/// with single spaces: what one part of the dialog shows, apart from the fields, which may hold the same text.
fn painted_in(cx: &mut VisualTestContext, id: &str) -> String {
    let nodes = frame(cx);
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
        .flat_map(|index| {
            nodes[index]
                .texts
                .iter()
                .map(|text| painted_words(&text.text))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A provisionable probe reply with a one-step plan.
fn plan_reply() -> Value {
    json!({
        "result": "provisionable",
        "probe_id": "p-1",
        "plan": {},
        "confirmation": "Farhelm will set up ops@new.example",
        "steps": {
            "lead": "Farhelm will set up ops@new.example:",
            "steps": [{ "id": "upload-farhelm", "name": "Upload the farhelm binary", "paths": [], "explanation": null }],
            "kept": null,
        },
    })
}

// ---- Opening --------------------------------------------------------------------------------------------------------

/// SPEC.md: the card's action opens the ordinary add host dialog with the requested destination and alias, and
/// opening is not approval. Specified: the dialog opens with focus in the destination field, both fields prefilled,
/// who asked and what Add approves painted above them, no host kind choice (the agent asked for an SSH destination,
/// so the dialog must not answer with a sandbox), and nothing sent to the helm.
#[gpui::test]
fn the_cards_action_opens_the_dialog_prefilled_and_answers_nothing(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    assert_eq!(f.open(), Ok(()));
    assert!(any_open(f.cx), "the dialog opened");
    // Kit's text inputs share one focus id, so a text input holding focus that is not the alias is the destination.
    assert_eq!(
        (focused_id(f.cx).as_deref(), alias_focused(f.cx)),
        (Some("input-state"), false),
        "focus starts in the destination field"
    );
    assert_eq!(destination(f.cx), "ops@new.example");
    assert_eq!(alias(f.cx), "newbox");
    let asked = painted_in(f.cx, "add-host-agent-requester");
    for expected in [
        "From host",
        "build",
        "From session",
        "asker title",
        "From session ID",
    ] {
        assert!(asked.contains(expected), "{expected:?} painted in {asked}");
    }
    let approved = painted_in(f.cx, "add-host-agent-approved");
    for expected in [
        "Destination to add",
        "ops@new.example",
        "Alias to add",
        "newbox",
    ] {
        assert!(
            approved.contains(expected),
            "{expected:?} painted in {approved}"
        );
    }
    assert!(
        frame(f.cx)
            .iter()
            .all(|node| node.element_id.as_deref() != Some("add-host-kind")),
        "an agent's add draws no host kind choice"
    );
    assert!(f.helm.commands().is_empty(), "opening sent nothing");
}

/// The lines spelling out what Add approves exist to show what the raw field may hide: a destination with a
/// direction-changing character shows it escaped there. Specified: a request whose destination holds U+202E paints
/// its escape in the approved lines, never the raw character.
#[gpui::test]
fn what_add_approves_is_shown_escaped(cx: &mut TestAppContext) {
    let mut approval = request();
    approval.action = ApprovalAction::HostAdd {
        destination: "ops@ev\u{202e}il".to_owned(),
        alias: None,
    };
    let mut f = fixture_with(cx, approval, Vec::new());
    f.open().expect("opened");
    let approved = painted_in(f.cx, "add-host-agent-approved");
    assert!(approved.contains("<U+202E>"), "escaped: {approved}");
    assert!(!approved.contains('\u{202e}'), "never raw: {approved}");
}

/// The card says why its Add host did not open, in words of its own for each reason, and nothing when there is nothing
/// to say (the user moved on, the bar is paused, the request is gone). Why: the card shows exactly this sentence, and a
/// reason that lost its words or took the other's would leave the user with no idea why the press did nothing.
/// Specified: another dialog and another host change each give their own sentence, naming what to wait for; `Nothing`
/// gives none.
#[test]
fn each_refused_open_says_why_in_its_own_words() {
    let dialog = NotOpened::AnotherDialog
        .reason()
        .expect("another dialog says why");
    let change = NotOpened::HostChangeInFlight
        .reason()
        .expect("a host change says why");
    assert!(dialog.starts_with("Another dialog is open."), "{dialog}");
    assert!(
        change.starts_with("Another host change is still in progress."),
        "{change}"
    );
    assert_eq!(NotOpened::Nothing.reason(), None);
}

/// SPEC.md: "an existing draft or another modal is kept". The old app queued the open behind the open dialog; this port
/// refuses it with a reason (a recorded deviation), and a press during the answer bar's pause opens nothing, as it
/// answers nothing. The old case typed a draft into the ordinary dialog first ("host add preserves a manual draft and
/// defers only an explicit open"), since a refused open that reset the ordinary dialog's fields would leave an empty
/// field that looks kept too. Specified: with the ordinary Add host open and a destination typed into it, the open
/// reports [`NotOpened::AnotherDialog`] and that dialog stays, its typed destination intact; with the bar paused, it
/// reports [`NotOpened::Nothing`] and opens nothing.
#[gpui::test]
fn another_dialog_or_the_pause_keeps_the_dialog_from_opening(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    let opener = f.opener.clone();
    let other = f.cx.update(|window, cx| {
        let mark = Mark::take(cx);
        crate::hosts::add::open(&opener, &mark, window, cx)
    });
    settle(f.cx);
    assert!(other.is_some(), "premise: the ordinary Add host opened");
    type_text(f.cx, "user@manualdraft.invalid");
    assert_eq!(
        destination(f.cx),
        "user@manualdraft.invalid",
        "premise: a draft is typed into the ordinary dialog"
    );
    assert_eq!(f.open(), Err(NotOpened::AnotherDialog));
    assert_eq!(
        destination(f.cx),
        "user@manualdraft.invalid",
        "the ordinary dialog and its draft are kept"
    );
    keys(f.cx, "escape");
    assert!(!any_open(f.cx), "premise: nothing open");

    f.arming.update(f.cx, |arming, cx| {
        arming.observe(
            vec![ID.to_owned(), "other".to_owned()],
            Some(ID.to_owned()),
            cx,
        )
    });
    assert_eq!(f.open(), Err(NotOpened::Nothing));
    assert!(!any_open(f.cx));
    assert!(f.helm.commands().is_empty());
}

/// An answer to the request already in flight (from its card) wins, so the dialog must not open over it, where its Add
/// could only be refused; and a host probe, setup or removal in flight holds the open, so that an ordinary add's
/// refused setup can still reopen its own draft (the old app held the open; this port refuses it with a reason).
/// Specified: with the card's Deny out, the open reports [`NotOpened::Nothing`]; with a host setup out, it reports
/// [`NotOpened::HostChangeInFlight`]; neither opens anything.
#[gpui::test]
fn an_answer_or_a_host_change_in_flight_keeps_the_dialog_from_opening(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    let arming = f.arming.clone();
    let sent =
        f.cx.update(|_, cx| answers::send(ID, ApprovalAnswer::Deny, None, &arming, cx));
    assert_eq!(sent, Ok(Sent::Sent), "premise: the card's Deny is out");
    assert_eq!(f.open(), Err(NotOpened::Nothing));
    assert!(!any_open(f.cx));
    let (id, _) = f.one_answer();
    f.helm.reply(id, 409, Some(json!("refused for a test")));
    settle(f.cx);

    let _setup = start_work(&mut f.helm, f.cx, Work::Setup, OpKey::new_host_add());
    assert_eq!(f.open(), Err(NotOpened::HostChangeInFlight));
    assert!(!any_open(f.cx));
}

// ---- Add ------------------------------------------------------------------------------------------------------------

/// SPEC.md: answer buttons pause for 700 ms whenever the waiting list or the expanded request changes, and the old
/// dialog's Add paused with them, so a press aimed at the previous request cannot answer through the dialog either.
/// Specified: during a pause, Add still reads "Add" but sends nothing; once the pause has run out, Add sends the Allow
/// without anything else redrawing the dialog.
#[gpui::test]
fn add_waits_out_the_answer_pause(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    f.open().expect("opened");
    f.arming.update(f.cx, |arming, cx| {
        arming.observe(
            vec![ID.to_owned(), "other".to_owned()],
            Some(ID.to_owned()),
            cx,
        )
    });
    settle(f.cx);
    assert_eq!(label(f.cx, "add-host-submit").as_deref(), Some("Add"));
    click(f.cx, "add-host-submit");
    assert!(f.helm.commands().is_empty(), "a paused Add sends nothing");
    f.cx.executor().advance_clock(ARM_DELAY);
    settle(f.cx);
    click(f.cx, "add-host-submit");
    let (_, body) = f.one_answer();
    assert_eq!(body["answer"], json!("allow"));
}

/// SPEC.md: Add answers Allow with the destination then in the dialog, and only an accepted Allow lets the ordinary
/// discovery-first add go on, with the edited fields; a refresh of the waiting list keeps the edits. Specified: after
/// editing both fields, the approved lines follow the edit and a refresh leaves both fields as edited; Add sends only
/// the Allow, carrying the edited destination, while Add reads busy; once the helm accepts it (the request gone first,
/// as the helm pushes), the probe goes out for the edited destination and alias.
#[gpui::test]
fn add_answers_allow_with_the_edited_destination_then_probes(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    f.open().expect("opened");
    crate::test_support::select_all_text(f.cx);
    type_text(f.cx, "ops@new.example2");
    keys(f.cx, "tab");
    crate::test_support::select_all_text(f.cx);
    type_text(f.cx, "newbox2");
    assert_eq!(
        (destination(f.cx), alias(f.cx)),
        ("ops@new.example2".to_owned(), "newbox2".to_owned()),
        "premise: both fields edited"
    );
    assert!(
        painted_in(f.cx, "add-host-agent-approved").contains("ops@new.example2"),
        "what Add approves follows the edit"
    );
    f.request_refreshed();
    settle(f.cx);
    assert_eq!(
        (destination(f.cx), alias(f.cx)),
        ("ops@new.example2".to_owned(), "newbox2".to_owned()),
        "a refresh of the waiting list keeps the edits"
    );
    click(f.cx, "add-host-submit");
    let (id, body) = f.one_answer();
    assert_eq!(
        body,
        json!({ "answer": "allow", "host_add": { "destination": "ops@new.example2" } })
    );
    assert!(
        f.helm.commands().is_empty(),
        "no probe before the helm accepts the Allow"
    );
    assert_eq!(
        label(f.cx, "add-host-submit").as_deref(),
        Some("Checking…"),
        "Add is busy while the Allow is out"
    );
    f.request_ends();
    f.helm.reply(id, 204, None);
    settle(f.cx);
    let (_, probe) = one_command(&mut f.helm, Method::Post, "/api/hosts/probe");
    assert_eq!(
        probe,
        json!({
            "target": { "kind": "ssh", "destination": "ops@new.example2" },
            // Keep the approved agent origin through discovery so this
            // request cannot receive a takeover confirmation.
            "new_entry": { "alias": "newbox2", "from_agent": true },
        })
    );
    assert!(any_open(f.cx), "the dialog stays for the probe's outcome");
}

/// SPEC.md: a late or refused answer starts nothing, no discovery probe included, and a refused form stays editable.
/// Only the helm's empty 204 says the waiting add took the destination, so any other success starts nothing either,
/// as the old app refused it. A late Allow closes the dialog at once, where the old form said "no longer waiting" in
/// itself until its request left the list (a recorded deviation), and the cards' late-answer notice says why. The 410
/// arrives while the request is still listed, so the close is the reply's, not the list's. Specified: a 409 refusal of
/// the Allow shows the helm's words and probes nothing, and a second Add sends the Allow again; a 200 with a body says
/// acceptance was not confirmed and probes nothing; a 410 with the request still waiting probes nothing, closes the
/// dialog and posts the plain late-answer notice.
#[gpui::test]
fn a_refused_unconfirmed_or_late_allow_probes_nothing(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    f.open().expect("opened");
    click(f.cx, "add-host-submit");
    let (id, _) = f.one_answer();
    f.helm
        .reply(id, 409, Some(json!("the destination is not allowed")));
    settle(f.cx);
    assert_eq!(
        error_line(f.cx).as_deref(),
        Some("the destination is not allowed")
    );
    assert!(
        f.helm.commands().is_empty(),
        "a refused Allow probes nothing"
    );
    assert!(any_open(f.cx), "the dialog stays");

    click(f.cx, "add-host-submit");
    let (id, _) = f.one_answer();
    f.helm.reply(id, 200, Some(json!({ "unexpected": true })));
    settle(f.cx);
    assert_eq!(error_line(f.cx).as_deref(), Some(NOT_CONFIRMED));
    assert!(
        f.helm.commands().is_empty(),
        "an unconfirmed Allow probes nothing"
    );

    click(f.cx, "add-host-submit");
    let (id, _) = f.one_answer();
    assert!(
        f.still_waiting(),
        "premise: the request is still listed as the 410 arrives"
    );
    f.helm.reply(
        id,
        410,
        Some(json!("this approval request is no longer waiting")),
    );
    settle(f.cx);
    assert!(f.helm.commands().is_empty(), "a late Allow probes nothing");
    assert!(!any_open(f.cx), "a late Allow closes the dialog");
    assert_eq!(
        f.cx.update(|_, cx| notices::posted(cx)),
        [Notice::LateAnswer { always: false }],
        "and the late-answer notice says nothing was added"
    );
}

/// SPEC.md: an agent's add always shows the setup confirmation when installation is needed, whatever the setup
/// preference says. Specified: with "Set up new hosts without asking" on, an accepted Allow's probe answered with a plan
/// opens the confirmation in the dialog's place and sends no provision request.
#[gpui::test]
fn an_agents_add_always_asks_the_setup_confirmation(cx: &mut TestAppContext) {
    let mut f = fixture(cx, vec![never_ask()]);
    f.open().expect("opened");
    let probe = f.allow_accepted();
    f.helm.reply(probe, 200, Some(plan_reply()));
    settle(f.cx);
    let text = painted(f.cx);
    assert!(
        text.contains("Set up host · ops@new.example"),
        "the confirmation asks: {text}"
    );
    assert!(
        f.helm.commands().is_empty(),
        "no provision request without the answer"
    );
}

/// SPEC.md: the confirmation always shows for an agent's add, "including after a refused setup submission is
/// reopened": the reopened dialog's next plan asks too, whatever the setup preference says. Specified: with "Set up new
/// hosts without asking" on, a refused provision request reopens Add host, whose Add probes again, and its plan opens
/// the confirmation rather than going out.
#[gpui::test]
fn a_refused_setup_reopens_still_asking(cx: &mut TestAppContext) {
    let mut f = fixture(cx, vec![never_ask()]);
    f.open().expect("opened");
    let probe = f.allow_accepted();
    f.helm.reply(probe, 200, Some(plan_reply()));
    settle(f.cx);
    click(f.cx, "prompt-confirm");
    let (provision, _) = one_command(&mut f.helm, Method::Post, "/api/hosts/provision");
    f.helm
        .reply(provision, 409, Some(json!("the plan was already used")));
    settle(f.cx);
    assert_eq!(
        destination(f.cx),
        "ops@new.example",
        "premise: Add host reopened with the destination"
    );
    click(f.cx, "add-host-submit");
    let (probe, _) = one_command(&mut f.helm, Method::Post, "/api/hosts/probe");
    f.helm.reply(probe, 200, Some(plan_reply()));
    settle(f.cx);
    let text = painted(f.cx);
    assert!(
        text.contains("Set up host · ops@new.example"),
        "the reopened add's plan asks: {text}"
    );
    assert!(
        f.helm.commands().is_empty(),
        "no provision request without the answer"
    );
}

// ---- Cancel ---------------------------------------------------------------------------------------------------------

/// SPEC.md: Cancel, the header's close and Escape answer Deny through the ordinary approval route; Cancel after
/// acceptance is ordinary add cancellation and sends no second answer. The dialog waits for the helm to take the Deny,
/// so the agent is told before the dialog is gone. Specified: Cancel sends Deny and the dialog stays until its 204,
/// then closes with focus back on the card's control; Escape does the same in a second opening; a refused Deny keeps
/// the dialog with the helm's words; after an accepted Allow, Cancel closes at once and sends nothing.
#[gpui::test]
fn cancel_and_escape_answer_deny_until_the_allow_was_accepted(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    for close in ["add-host-cancel", "escape"] {
        f.open().expect("opened");
        if close == "escape" {
            keys(f.cx, "escape");
        } else {
            click(f.cx, "add-host-cancel");
        }
        let (id, body) = f.one_answer();
        assert_eq!(body, json!({ "answer": "deny" }), "{close}");
        assert!(any_open(f.cx), "{close}: the dialog waits for the Deny");
        f.helm.reply(id, 204, None);
        settle(f.cx);
        assert!(!any_open(f.cx), "{close}: closed once the helm took it");
        assert!(
            focused(f.cx, &f.opener),
            "{close}: focus back on the card's control"
        );
    }

    f.open().expect("opened");
    click(f.cx, "add-host-cancel");
    let (id, _) = f.one_answer();
    f.helm.reply(id, 409, Some(json!("refused for a test")));
    settle(f.cx);
    assert!(any_open(f.cx), "a refused Deny keeps the dialog");
    assert_eq!(error_line(f.cx).as_deref(), Some("refused for a test"));

    let _probe = f.allow_accepted();
    click(f.cx, "add-host-cancel");
    assert!(!any_open(f.cx), "after acceptance Cancel closes at once");
    assert!(f.helm.commands().is_empty(), "and answers nothing");
}

// ---- Following the request ------------------------------------------------------------------------------------------

/// The old dialog closed when its request stopped waiting before the user accepted it (expired, answered from
/// elsewhere, its session gone): an Add then could only be refused. But a request ended by the dialog's own answer
/// must not close it before that answer settles. Specified: the request ending while the dialog's Deny is out keeps
/// the dialog until the Deny's reply; the request ending with no answer out closes it, answering nothing.
#[gpui::test]
fn the_dialog_closes_when_its_request_stops_waiting_unanswered(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    f.open().expect("opened");
    keys(f.cx, "escape");
    let (id, _) = f.one_answer();
    f.request_ends();
    settle(f.cx);
    assert!(any_open(f.cx), "its own Deny out: the dialog stays");
    f.helm.reply(id, 204, None);
    settle(f.cx);
    assert!(!any_open(f.cx), "premise: the Deny's reply closed it");

    f.request_refreshed();
    settle(f.cx);
    f.open().expect("opened");
    f.request_ends();
    settle(f.cx);
    assert!(
        !any_open(f.cx),
        "an unanswered request that ended closed its dialog"
    );
    assert!(
        f.cx.update(|window, cx| !layers::any_open(window, cx)),
        "no layer is left"
    );
    assert!(f.helm.commands().is_empty(), "and nothing was answered");
}

/// An answer from another surface (the request's card) can end the request while the dialog is open: the helm pushes
/// the removal before it replies, so the dialog must wait for that answer to settle and then close, rather than stay
/// over a request that is gone until some unrelated change arrives. Specified: with the dialog open, the card's Deny
/// goes out and the request ends; the dialog stays while the Deny is out and closes once its 204 arrives.
#[gpui::test]
fn the_dialog_closes_once_the_cards_answer_ends_its_request(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    f.open().expect("opened");
    let arming = f.arming.clone();
    let sent =
        f.cx.update(|_, cx| answers::send(ID, ApprovalAnswer::Deny, None, &arming, cx));
    assert_eq!(sent, Ok(Sent::Sent), "premise: the card's Deny is out");
    let (id, _) = f.one_answer();
    f.request_ends();
    settle(f.cx);
    assert!(any_open(f.cx), "an answer is out: the dialog stays");
    f.helm.reply(id, 204, None);
    settle(f.cx);
    assert!(
        !any_open(f.cx),
        "the card's answer settled: the dialog closed"
    );
}

// ---- Layers opened in Add host's place ------------------------------------------------------------------------------

/// The setup offer an add's probe leads to, for an agent's add (`from_agent`) or an ordinary one: [`plan_reply`]'s plan
/// for the requested destination and alias, under the new host add's key, Add host's opener the card's control, and a
/// mark taken now.
fn offer(f: &mut Fixture<'_>, from_agent: bool) -> setup::Offer {
    let steps = serde_json::from_value(plan_reply()["steps"].clone()).expect("the steps decode");
    let opener = f.opener.clone();
    f.cx.update(|_, cx| setup::Offer {
        key: OpKey::new_host_add(),
        probe_id: "p-1".to_owned(),
        confirmation: "Farhelm will set up ops@new.example".to_owned(),
        steps,
        target: setup::Target::New {
            destination: "ops@new.example".to_owned(),
            alias: "newbox".to_owned(),
            opener,
            from_agent,
        },
        mark: Mark::take(cx),
    })
}

/// Open a dialog unrelated to the add, with its one control focused, and no press or key: where focus is on macOS when
/// a layer opens in Add host's place after Add host returned focus to its opener, if that opener was a control of a
/// dialog the card was pressed over (a press there moves no focus).
fn unrelated_dialog(f: &mut Fixture<'_>) -> LayerId {
    let inside = handle(f.cx);
    let (dialog, _, _) =
        test_support::open_dialog(f.cx, vec![("unrelated-control", inside.clone())], &inside);
    assert!(
        focused(f.cx, &inside),
        "premise: the unrelated dialog holds focus"
    );
    dialog
}

/// Close the layer `dialog` as its owner would on a data change, and settle.
fn owner_closes(f: &mut Fixture<'_>, dialog: LayerId) {
    f.cx.update(|window, cx| layers::close(dialog, window, cx));
    settle(f.cx);
}

/// Why this matters: SPEC.md, "Nothing closes a dialog, menu, popup list or confirmation that it did not open". An
/// agent's setup confirmation opens in the place of Add host, which belongs to the request's card and to no dialog, so
/// an unrelated dialog under it must not take it along when that dialog's owner closes it. On macOS the focus rules
/// alone would make that dialog its parent (see [`unrelated_dialog`]). The card's Add host does not open over another
/// dialog today ([`NotOpened::AnotherDialog`]), so the test asks the confirmation directly in that state. An ordinary
/// add's confirmation keeps the focus rules, which also shows that it is the agent's add, not something else, that
/// keeps the confirmation open.
///
/// Specified, under the macOS click rule, with an unrelated dialog holding focus each time: an ordinary add's
/// confirmation opens over the dialog and closes with it; an agent's add's confirmation opens over it, stays open when
/// the dialog's owner closes it, and its Set up still sends the plan.
#[gpui::test]
fn an_agents_setup_confirmation_outlives_an_unrelated_dialog_under_it(cx: &mut TestAppContext) {
    let mut f = fixture(cx, Vec::new());
    f.cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    for from_agent in [false, true] {
        let dialog = unrelated_dialog(&mut f);
        let offer = offer(&mut f, from_agent);
        f.cx.update(|window, cx| setup::confirm(offer, window, cx));
        settle(f.cx);
        assert!(
            painted(f.cx).contains("Set up host · ops@new.example"),
            "premise: the confirmation opened (agent's add: {from_agent})"
        );
        owner_closes(&mut f, dialog);
        assert!(!is_open(f.cx, dialog), "premise: the dialog closed");
        if !from_agent {
            assert!(
                !any_open(f.cx),
                "an ordinary add's confirmation went with the dialog"
            );
            continue;
        }
        assert!(
            painted(f.cx).contains("Set up host · ops@new.example"),
            "an agent's add's confirmation stayed open"
        );
        assert!(f.helm.commands().is_empty(), "nothing was sent");
        click(f.cx, "prompt-confirm");
        let (_, body) = one_command(&mut f.helm, Method::Post, "/api/hosts/provision");
        assert_eq!(body, json!({ "probe_id": "p-1" }), "Set up sent the plan");
    }
}

/// Why this matters: the same rule for Add host reopened after the helm refused an agent's setup. It is the request's
/// Add host again ([`crate::hosts::add::Seed`]'s `from_agent`), opened once the confirmation has closed, so the focus
/// rules alone would make a dialog under the card its parent on macOS, as for the confirmation. A dialog the user opens
/// while the setup is out means they moved on, which reopens nothing, so the test opens the unrelated dialog without
/// a press. An ordinary add's reopened Add host keeps the focus rules, the contrast that shows the agent's add is what
/// keeps the dialog open.
///
/// Specified, under the macOS click rule, for an ordinary add and then an agent's: the setup goes out, an unrelated
/// dialog opens and holds focus, and the helm refuses the setup; Add host reopens over the dialog with the destination
/// and the helm's words. The ordinary add's closes with the dialog. The agent's add's stays open when the dialog's
/// owner closes it, and its Add still probes the destination.
#[gpui::test]
fn an_agents_add_host_reopened_after_a_refused_setup_outlives_an_unrelated_dialog_under_it(
    cx: &mut TestAppContext,
) {
    let mut f = fixture(cx, Vec::new());
    f.cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    for from_agent in [false, true] {
        let offer = offer(&mut f, from_agent);
        f.cx.update(|window, cx| setup::submit(offer, window.window_handle(), cx));
        settle(f.cx);
        let (provision, _) = one_command(&mut f.helm, Method::Post, "/api/hosts/provision");
        let dialog = unrelated_dialog(&mut f);
        f.helm
            .reply(provision, 409, Some(json!("the plan was already used")));
        settle(f.cx);
        assert_eq!(
            (destination(f.cx), error_line(f.cx)),
            (
                "ops@new.example".to_owned(),
                Some("the plan was already used".to_owned())
            ),
            "premise: Add host reopened with the destination and the helm's words (agent's add: {from_agent})"
        );
        assert!(is_open(f.cx, dialog), "premise: over the unrelated dialog");
        owner_closes(&mut f, dialog);
        assert!(!is_open(f.cx, dialog), "premise: the dialog closed");
        if !from_agent {
            assert!(
                !any_open(f.cx),
                "an ordinary add's Add host went with the dialog"
            );
            continue;
        }
        assert!(any_open(f.cx), "an agent's add's Add host stayed open");
        click(f.cx, "add-host-submit");
        one_command(&mut f.helm, Method::Post, "/api/hosts/probe");
    }
}
