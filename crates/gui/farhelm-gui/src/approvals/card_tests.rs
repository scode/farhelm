//! Tests of the expanded approval card's drawing: what it paints and names, that its answer buttons answer only their
//! own request and only while not busy, that Copy copies the raw reference, that a refused answer's reason is
//! announced, and that a narrow card stacks its labels.
//!
//! These matter because the card is all the user sees of an agent's request: a value painted as the app's words, a
//! button named for the wrong host, a busy button that still answered, or a copy of the escaped spelling instead of
//! the reference would each let an approval cover something other than what was shown. A refusal arrives away from
//! the button the user pressed, so unannounced it goes unnoticed. Pixel layout is pinned by the card's screenshot
//! goldens; these tests read the frame's driver record.

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_proto::approvals::{
    ApprovalAction, ApprovalAnswer, ApprovalSession, LaunchVerb, PendingApproval,
};
use farhelm_proto::{LaunchHarness, LaunchPermission, LaunchSelection, SessionLaunch, SessionRef};
use gpui::{DriverNode, DriverText, IntoElement as _, Role, TestAppContext, VisualTestContext, px};

use super::card::{CardState, NARROW_WIDTH, card};
use super::model::{Card, TunnelOffer};
use crate::host_text::HostText;
use crate::test_support::{self, frame};

// ---- Fixtures -------------------------------------------------------------------------------------------------------

/// A full session reference under a fixed entry, built with the protocol's own formatter.
fn reference(bare: &str) -> String {
    SessionRef::new("7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d", bare)
        .expect("a fixture bare ID within bounds")
        .to_string()
}

/// A Claude create in guarded mode from host `host`, which the helm marks eligible for the narrow grant.
fn launch(host: &str) -> PendingApproval {
    PendingApproval {
        id: "req-1".to_owned(),
        host_id: 3,
        host_name: host.to_owned(),
        session: ApprovalSession {
            id: reference("1234567890abcdefghijkl"),
            title: Some("asker title".to_owned()),
            host_name: Some(host.to_owned()),
        },
        action: ApprovalAction::Launch {
            verb: LaunchVerb::Create,
            host_name: "box".to_owned(),
            cwd: "/work/project".to_owned(),
            repo: None,
            title: Some("child".to_owned()),
            launch: SessionLaunch::Agent {
                selection: LaunchSelection {
                    harness: LaunchHarness::Claude,
                    model: None,
                    effort: None,
                    permissions: Some(LaunchPermission::Guarded),
                    permission_flags: None,
                    wrapper_prefix: None,
                    container: None,
                    workspace_trust: None,
                },
                start: vec!["claude".to_owned()],
                resume: None,
            },
            source: None,
        },
        own_host_session_action: true,
        expires_at_ms: 1,
    }
}

/// What a test's window draws: the card for `approval` at `width`, busy or not, recording every answer pressed.
struct Drawn {
    approval: PendingApproval,
    width: f32,
    busy: bool,
    refusal: Option<HostText>,
    pressed: Rc<RefCell<Vec<ApprovalAnswer>>>,
}

/// Open a window that draws `drawn`'s card, re-reading `drawn` on every frame.
fn open(cx: &mut TestAppContext, drawn: Rc<RefCell<Drawn>>) -> &mut VisualTestContext {
    test_support::open(cx, move |_, cx| {
        let drawn = drawn.borrow();
        let model = Card::of(&drawn.approval, TunnelOffer::NotOffered);
        let pressed = drawn.pressed.clone();
        let state = CardState {
            width: px(drawn.width),
            waiting: 2,
            busy: drawn.busy,
            refusal: drawn.refusal.clone(),
            measure: None,
            body_scroll: None,
        };
        card(
            &model,
            &state,
            Rc::new(move |answer, _, _| pressed.borrow_mut().push(answer)),
            cx,
        )
        .into_any_element()
    })
}

/// The strings painted inside the element with id `id`, in paint order.
fn texts_within(nodes: &[DriverNode], id: &str) -> Vec<DriverText> {
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
        .flat_map(|index| nodes[index].texts.iter().cloned())
        .collect()
}

/// The painted strings inside `id`, as plain strings.
fn painted(nodes: &[DriverNode], id: &str) -> Vec<String> {
    texts_within(nodes, id)
        .into_iter()
        .map(|text| text.text.to_string())
        .collect()
}

/// The node with id `id`.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> &'a DriverNode {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} was not drawn"))
}

/// The accessible names of the card's answer buttons, in drawing order. Only the buttons themselves: a label naming a
/// host is a sentence inside its button, with an accessible name of its own.
fn answer_names(nodes: &[DriverNode]) -> Vec<String> {
    nodes
        .iter()
        .filter(|node| node.role == Some(Role::Button))
        .filter(|node| {
            node.element_id
                .as_deref()
                .is_some_and(|id| id.starts_with("approval-req-1-answer-"))
        })
        .filter_map(|node| node.label.as_ref().map(ToString::to_string))
        .collect()
}

/// How the host-text element spells `raw`, read from its debug form, after checking the premise every caller relies on:
/// that `raw` holds a character the escape table spells out, so the spelling differs from the raw text.
fn escaped(raw: &str) -> String {
    let debug = format!("{:?}", HostText::from_host(raw));
    let spelled = debug
        .trim_start_matches("HostText(\"")
        .trim_end_matches("\")")
        .to_owned();
    assert!(
        spelled.contains("<U+") && spelled != raw,
        "premise: the escape table spells a control in {raw:?}: {spelled}"
    );
    spelled
}

// ---- What the card paints -------------------------------------------------------------------------------------------

/// SPEC.md: the card shows the "Agent request" badge, the action as its heading, the waiting count, the compact From
/// line, the labelled facts and the answers. Every one of them is painted from the model, in the old app's words.
#[gpui::test]
fn the_card_paints_its_badge_heading_count_from_line_facts_and_answers(cx: &mut TestAppContext) {
    let drawn = Rc::new(RefCell::new(Drawn {
        approval: launch("build"),
        width: 660.,
        busy: false,
        refusal: None,
        pressed: Rc::default(),
    }));
    let cx = open(cx, drawn);
    let nodes = frame(cx);
    let texts = painted(&nodes, "approval-req-1-card");
    for expected in [
        "Agent request",
        "Start a new session",
        "1 of 2 waiting",
        "From",
        "asker title",
        "On host",
        "box",
        "Folder",
        "/work/project",
        "Title",
        "child",
        "Permissions",
        "Guarded",
        "Guarded · Claude auto mode",
        "Agent",
        "Command",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected:?} painted in {texts:?}"
        );
    }
    assert!(
        texts.iter().any(|text| text == "12345678…ijkl"),
        "the From line shows the bare ID shortened: {texts:?}"
    );
    assert_eq!(
        painted(&nodes, "approval-req-1-box-0"),
        ["claude"],
        "the command's box holds the command"
    );
    // The box is a named tab stop the keyboard scrolls (the old labelled box), named for its label.
    let command = node(&nodes, "approval-req-1-box-0");
    assert_eq!(
        (command.role, command.label.as_deref()),
        (Some(Role::Region), Some("Command"))
    );
    assert_eq!(
        texts.iter().filter(|text| *text == "claude").count(),
        2,
        "the Agent fact's value and the command box both show the agent: {texts:?}"
    );
    // The old markup's roles: the heading was an `h2`, and the From line's parts are labelled values.
    assert_eq!(
        node(&nodes, "approval-req-1-heading").role,
        Some(Role::Heading)
    );
    for part in ["host", "title", "id"] {
        assert_eq!(
            node(&nodes, &format!("approval-req-1-from-{part}")).role,
            Some(Role::Label),
            "the From line's {part}"
        );
    }
    assert_eq!(
        answer_names(&nodes),
        [
            "Allow",
            "Always allow on build only",
            "Always allow from build",
            "Deny"
        ]
    );
}

/// Host names in answer labels and the From line are host text: escaped where shown and in the button's accessible
/// name, so a host named with direction controls cannot reorder the app's words around it.
#[gpui::test]
fn host_names_on_the_card_are_escaped_host_text(cx: &mut TestAppContext) {
    let drawn = Rc::new(RefCell::new(Drawn {
        approval: launch("ev\u{202e}il"),
        width: 660.,
        busy: false,
        refusal: None,
        pressed: Rc::default(),
    }));
    let cx = open(cx, drawn);
    let nodes = frame(cx);
    let escaped = escaped("ev\u{202e}il");
    assert_eq!(
        answer_names(&nodes),
        [
            "Allow".to_owned(),
            format!("Always allow on {escaped} only"),
            format!("Always allow from {escaped}"),
            "Deny".to_owned()
        ]
    );
    for button in [
        "approval-req-1-answer-always-allow",
        "approval-req-1-answer-always-allow-own-host",
    ] {
        assert!(
            painted(&nodes, button).contains(&escaped),
            "{button} paints the escaped host"
        );
    }
    assert!(
        painted(&nodes, "approval-req-1-from-host").contains(&escaped),
        "the From line paints the escaped host"
    );
}

/// A requester whose title the helm does not know still has its From line read sensibly: the old card's "title unknown"
/// stand-in, named "From session" as the old span was, so a screen reader hears what the words stand in for.
#[gpui::test]
fn an_untitled_requester_reads_as_title_unknown(cx: &mut TestAppContext) {
    let mut approval = launch("build");
    approval.session.title = None;
    let drawn = Rc::new(RefCell::new(Drawn {
        approval,
        width: 660.,
        busy: false,
        refusal: None,
        pressed: Rc::default(),
    }));
    let cx = open(cx, drawn);
    let nodes = frame(cx);
    let title = node(&nodes, "approval-req-1-from-title");
    assert_eq!(title.role, Some(Role::Label));
    assert_eq!(title.label.as_deref(), Some("From session"));
    assert_eq!(
        painted(&nodes, "approval-req-1-from-title"),
        ["title unknown"]
    );
}

/// A host removal's card paints the model's notes, in order, after its facts, and offers only Allow and Deny (SPEC.md:
/// host changes get no "Always allow"). The notes carry what the facts cannot: that removing forgets the host without
/// stopping it, and why a host change asks even where commands run without asking.
#[gpui::test]
fn a_host_removal_paints_its_notes_and_only_allow_and_deny(cx: &mut TestAppContext) {
    let mut approval = launch("build");
    approval.own_host_session_action = false;
    approval.action = ApprovalAction::HostRemove {
        host_name: "build-box".to_owned(),
        destination: "ops@build-box.example".to_owned(),
    };
    let notes = Card::of(&approval, TunnelOffer::NotOffered).notes;
    assert_eq!(notes.len(), 2, "premise: a host removal has two notes");
    let drawn = Rc::new(RefCell::new(Drawn {
        approval,
        width: 660.,
        busy: false,
        refusal: None,
        pressed: Rc::default(),
    }));
    let cx = open(cx, drawn);
    let nodes = frame(cx);
    let texts = painted(&nodes, "approval-req-1-card");
    let at = |note: &str| {
        texts
            .iter()
            .position(|text| text == note)
            .unwrap_or_else(|| panic!("{note:?} painted in {texts:?}"))
    };
    assert!(
        at(notes[0]) < at(notes[1]),
        "the notes keep the model's order"
    );
    assert_eq!(answer_names(&nodes), ["Allow", "Deny"]);
}

// ---- Answers --------------------------------------------------------------------------------------------------------

/// Each answer button answers its own choice, and a busy card (the answer bar paused, or an answer in flight) answers
/// nothing: the press must not reach the request even though the button is still drawn where the user aimed.
#[gpui::test]
fn answer_buttons_answer_their_choice_and_nothing_while_busy(cx: &mut TestAppContext) {
    let pressed = Rc::new(RefCell::new(Vec::new()));
    let drawn = Rc::new(RefCell::new(Drawn {
        approval: launch("build"),
        width: 660.,
        busy: true,
        refusal: None,
        pressed: pressed.clone(),
    }));
    let cx = open(cx, drawn.clone());
    test_support::click(cx, "approval-req-1-answer-deny");
    assert_eq!(*pressed.borrow(), [], "a busy button answers nothing");
    drawn.borrow_mut().busy = false;
    test_support::redraw(cx);
    test_support::click(cx, "approval-req-1-answer-deny");
    test_support::click(cx, "approval-req-1-answer-always-allow-own-host");
    assert_eq!(
        *pressed.borrow(),
        [ApprovalAnswer::Deny, ApprovalAnswer::AlwaysAllowOwnHost]
    );
}

/// Copy ID puts the complete raw reference on the clipboard, entry prefix and every character included, never the
/// shortened or escaped spelling the line shows.
#[gpui::test]
fn copy_id_copies_the_raw_full_reference(cx: &mut TestAppContext) {
    let mut approval = launch("build");
    approval.session.id = reference("peer\u{200b} id");
    let drawn = Rc::new(RefCell::new(Drawn {
        approval,
        width: 660.,
        busy: false,
        refusal: None,
        pressed: Rc::default(),
    }));
    let cx = open(cx, drawn);
    let nodes = frame(cx);
    assert_eq!(
        node(&nodes, "approval-req-1-from-copy").label.as_deref(),
        Some("Copy from session ID")
    );
    test_support::click(cx, "approval-req-1-from-copy");
    let copied = cx
        .update(|_, cx| cx.read_from_clipboard())
        .and_then(|item| item.text());
    assert_eq!(copied, Some(reference("peer\u{200b} id")));
}

/// An answer the helm refused leaves the request waiting, and the card says why where a screen reader announces it (the
/// old card's `role="alert"`): the refusal arrives after the press, away from the button the user is looking at. The
/// reason is the helm's text, drawn as host text, never as the app's words.
#[gpui::test]
fn a_refused_answer_shows_its_reason_as_an_alert(cx: &mut TestAppContext) {
    let drawn = Rc::new(RefCell::new(Drawn {
        approval: launch("build"),
        width: 660.,
        busy: false,
        refusal: None,
        pressed: Rc::default(),
    }));
    let cx = open(cx, drawn.clone());
    assert!(
        !frame(cx).iter().any(|node| node.role == Some(Role::Alert)),
        "no alert before a refusal"
    );
    drawn.borrow_mut().refusal = Some(HostText::from_host("setting not\u{202e} saved"));
    test_support::redraw(cx);
    let nodes = frame(cx);
    assert_eq!(
        node(&nodes, "approval-req-1-refusal").role,
        Some(Role::Alert)
    );
    assert_eq!(
        painted(&nodes, "approval-req-1-refusal"),
        [escaped("setting not\u{202e} saved")]
    );
}

// ---- Layout ---------------------------------------------------------------------------------------------------------

/// SPEC.md: labels may stack above values in a narrow pane. At the narrow width a fact's label sits above its value;
/// in a wide card it sits to the value's left.
#[gpui::test]
fn a_narrow_card_stacks_labels_above_values(cx: &mut TestAppContext) {
    let drawn = Rc::new(RefCell::new(Drawn {
        approval: launch("build"),
        width: 660.,
        busy: false,
        refusal: None,
        pressed: Rc::default(),
    }));
    let cx = open(cx, drawn.clone());
    let place = |cx: &mut VisualTestContext| {
        let nodes = frame(cx);
        let texts = texts_within(&nodes, "approval-req-1-card");
        let at = |wanted: &str| {
            texts
                .iter()
                .find(|text| text.text.as_ref() == wanted)
                .map(|text| text.bounds.origin)
                .unwrap_or_else(|| panic!("{wanted} painted"))
        };
        (at("Folder"), at("/work/project"))
    };
    let (label, value) = place(cx);
    assert!(
        value.x > label.x && (value.y - label.y).abs() < px(4.),
        "wide: value beside its label"
    );
    drawn.borrow_mut().width = f32::from(NARROW_WIDTH) - 60.;
    test_support::redraw(cx);
    let (label, value) = place(cx);
    assert!(
        value.y > label.y && (value.x - label.x).abs() < px(4.),
        "narrow: value under its label"
    );
}
