//! Tests of the approval region: where it places itself, which request opens, what it does as requests come and go,
//! and that it answers, over an open dialog too, without taking focus from what the user is doing.
//!
//! They matter because the region is the only way the user sees an agent's request: a card placed off the pane, a
//! request that never opens, a card that takes focus from a terminal (so Enter typed there answers it), or a card that
//! cannot be answered while a dialog is open would each leave an agent waiting nine minutes or answer something the
//! user did not mean to. Placement is a pure function, tested on numbers; the rest runs in a window over the helm
//! client's fake, with the region installed as the main view installs it and the shared harness's focus and overlay
//! checks after every step. Layout here runs on this test context's one-width text; the region's look in the real
//! fonts is the running app's `app-approval-card` screenshot scene.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::approvals::{
    ApprovalAction, ApprovalSession, HostFieldChange, LaunchVerb, PendingApproval, QuickLaunchCard,
    TemplateKeyCard, TemplateShortcutCard,
};
use farhelm_proto::launcher::TemplateFields;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use farhelm_proto::{CommandLaunch, DeleteGuard, LaunchHarness, SessionLaunch};
use gpui::{
    Bounds, Context, FocusHandle, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, VisualTestContext, Window, div, point, px, size,
};
use serde_json::json;

use super::answers::ARM_DELAY;
use super::card::BOX_MIN_HEIGHT;
use super::region::{self, Place, place};
use crate::layers::Anchor;
use crate::notices::{self, Notice};
use crate::root;
use crate::test_support::{
    self, any_open, button, click, focused, frame, keys, label, meta, one_command, settle, started,
};

// ---- Placement -------------------------------------------------------------------------------------------------------

/// A pane at (300, 0), 1000 by 800.
fn pane() -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(300.), px(0.)), size(px(1000.), px(800.)))
}

/// The old layout script's placement: the card's width capped at 660 and centered in the pane, 12 pixels below the
/// terminal area's top (the pane's top with no session), at most three quarters of the pane and the room left below.
/// Specified: in a 1000-pixel pane the region is 660 wide at x 470; below a terminal area at 80 it starts at 92 with a
/// cap of 600 (three quarters of 800); with no session it starts at 12; in a 500-pixel pane it is 476 wide.
#[test]
fn the_region_is_centered_below_the_terminal_area_and_capped() {
    assert_eq!(
        place(pane(), Some(px(80.)), px(800.), px(200.)),
        Place {
            left: px(470.),
            width: px(660.),
            top: px(92.),
            cap: px(600.),
            scrolls: false
        }
    );
    assert_eq!(place(pane(), None, px(800.), px(200.)).top, px(12.));
    let narrow = Bounds::new(point(px(300.), px(0.)), size(px(500.), px(800.)));
    let placed = place(narrow, None, px(800.), px(200.));
    assert_eq!((placed.width, placed.left), (px(476.), px(312.)));
}

/// SPEC.md: the card's header and answer bar stay visible within the cap; if session chrome leaves less than they
/// need, the region uses the whole viewport and may cover that chrome; below that minimum the region scrolls as a
/// whole. Specified: below a terminal area at 700 in an 800-pixel pane the cap is 76; fixed parts of 200 then take the
/// whole 800-pixel window from its top, unscrolled; fixed parts of 900 scroll it.
#[test]
fn a_short_pane_gives_the_region_the_window_and_a_tiny_one_scrolls_it() {
    assert_eq!(
        place(pane(), Some(px(700.)), px(800.), px(50.)).cap,
        px(76.)
    );
    let tall = place(pane(), Some(px(700.)), px(800.), px(200.));
    assert_eq!(
        (tall.top, tall.cap, tall.scrolls),
        (px(0.), px(800.), false)
    );
    let tiny = place(pane(), Some(px(700.)), px(800.), px(900.));
    assert!(
        tiny.scrolls,
        "even the window is shorter than the fixed parts"
    );
}

// ---- The window --------------------------------------------------------------------------------------------------------

/// A waiting stop request `id` from host `host`.
fn stop(id: &str, host: &str) -> PendingApproval {
    PendingApproval {
        id: id.to_owned(),
        host_id: 1,
        host_name: host.to_owned(),
        session: ApprovalSession {
            id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:asker".to_owned(),
            title: None,
            host_name: None,
        },
        action: ApprovalAction::Stop {
            target: ApprovalSession {
                id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:target".to_owned(),
                title: None,
                host_name: None,
            },
        },
        own_host_session_action: false,
        expires_at_ms: 1,
    }
}

/// The approvals collection holding `approvals`, oldest first.
fn waiting(approvals: &[PendingApproval]) -> CollectionState {
    let ids: Vec<&str> = approvals
        .iter()
        .map(|approval| approval.id.as_str())
        .collect();
    CollectionState {
        collection: Collection::Approvals,
        meta: meta(&ids),
        entries: approvals
            .iter()
            .map(|approval| KeyedEntry {
                key: approval.id.clone(),
                value: serde_json::to_value(approval).expect("an approval encodes"),
            })
            .collect(),
    }
}

/// The pushed list becomes `approvals`, oldest first.
fn push(helm: &mut FakeHelm, approvals: &[PendingApproval], gone: &[&str]) {
    let ids: Vec<&str> = approvals
        .iter()
        .map(|approval| approval.id.as_str())
        .collect();
    helm.push(CollectionChanges {
        collection: Collection::Approvals,
        meta: meta(&ids),
        upsert: waiting(approvals).entries,
        remove: gone.iter().map(|id| (*id).to_owned()).collect(),
    });
}

/// The main view's shape for the region: a pane the region centers in, holding a control standing in for a focused
/// terminal.
struct Shell {
    root: root::Attachment,
    terminal: FocusHandle,
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = Anchor::for_element("test-pane-anchor", window, cx);
        region::set_pane(pane.clone(), cx);
        let content = div()
            .size_full()
            .relative()
            .child(button("terminal", &self.terminal))
            .child(pane.track());
        root::wrap(content, &self.root, window, cx)
    }
}

/// The app over the fake helm with `approvals` waiting, a window with the region installed as the main view installs
/// it, focus on the stand-in terminal, and the answer bar's pause run out.
fn open<'a>(
    cx: &'a mut TestAppContext,
    approvals: &[PendingApproval],
) -> (FakeHelm, &'a mut VisualTestContext, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![waiting(approvals)]);
    cx.run_until_parked();
    let terminal = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let held = terminal.clone();
    let (_, window) = test_support::open_view(cx, move |window, cx| {
        let root = root::attach(window, cx);
        region::install(window, cx);
        Shell {
            root,
            terminal: held,
        }
    });
    window.simulate_resize(size(px(1200.), px(900.)));
    window.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        crate::focus::focus_control(&terminal, &mark, window, cx);
    });
    settle(window);
    assert!(
        focused(window, &terminal),
        "premise: the stand-in terminal holds focus"
    );
    window.executor().advance_clock(ARM_DELAY);
    settle(window);
    (helm, window, terminal)
}

/// Whether the region draws anything.
fn drawn(cx: &mut VisualTestContext) -> bool {
    frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some("approval-region"))
}

/// The id of the request whose card is expanded.
fn expanded(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx).into_iter().find_map(|node| {
        node.element_id
            .as_deref()?
            .strip_prefix("approval-")?
            .strip_suffix("-card")
            .map(str::to_owned)
    })
}

// ---- What shows ------------------------------------------------------------------------------------------------------

/// An empty region covers nothing: with no request waiting and no notice, it draws nothing at all (the slot's
/// contract). Specified: nothing waiting, no region; one request, the region with its card.
#[gpui::test]
fn the_region_draws_only_while_a_request_waits(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[]);
    assert!(!drawn(cx), "nothing waits: nothing drawn");
    push(&mut helm, &[stop("a", "box")], &[]);
    settle(cx);
    assert!(drawn(cx), "a request waits: the region shows it");
    assert_eq!(expanded(cx).as_deref(), Some("a"));
}

/// SPEC.md: the oldest waiting request opens first; a selected header stays selected until that request is answered or
/// expires, then the oldest remaining one opens; the others are listed below as headers. Specified: with a and b
/// waiting, a opens and b is a header named for its action and host; choosing b opens it; a newer request c arriving
/// leaves b open; b ending opens a, the oldest.
#[gpui::test]
fn the_oldest_opens_and_a_chosen_header_stays_until_its_request_ends(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box"), stop("b", "other")]);
    assert_eq!(expanded(cx).as_deref(), Some("a"));
    assert_eq!(
        label(cx, "approval-b-waiting").as_deref(),
        Some("Stop a session's agent and close its terminal tabs, from host other")
    );
    click(cx, "approval-b-waiting");
    assert_eq!(
        expanded(cx).as_deref(),
        Some("b"),
        "the chosen header opens"
    );
    push(
        &mut helm,
        &[stop("a", "box"), stop("b", "other"), stop("c", "box")],
        &[],
    );
    settle(cx);
    assert_eq!(
        expanded(cx).as_deref(),
        Some("b"),
        "a new arrival leaves the choice"
    );
    push(&mut helm, &[stop("a", "box"), stop("c", "box")], &["b"]);
    settle(cx);
    assert_eq!(
        expanded(cx).as_deref(),
        Some("a"),
        "the chosen request ended: the oldest opens"
    );
}

// ---- Focus and answers -------------------------------------------------------------------------------------------------

/// SPEC.md, "Keyboard focus": a request arriving moves no focus, and Enter typed into a focused terminal keeps going to
/// it, never answering a card. Specified: with focus on the stand-in terminal, a request arrives; focus stays there,
/// and Enter sends nothing.
#[gpui::test]
fn an_arriving_request_takes_no_focus_and_enter_answers_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, terminal) = open(cx, &[]);
    push(&mut helm, &[stop("a", "box")], &[]);
    settle(cx);
    assert!(drawn(cx), "premise: the card is up");
    assert!(focused(cx, &terminal), "focus stayed on the terminal");
    // Past the answer bar's pause, so Enter would answer if focus had gone to a card's button.
    cx.executor().advance_clock(ARM_DELAY);
    settle(cx);
    keys(cx, "enter");
    assert!(helm.commands().is_empty(), "Enter answered nothing");
}

/// SPEC.md: answer buttons pause for 700 ms whenever the waiting list or the expanded request changes. Specified: a
/// press on Deny right after the list changed sends nothing; once 700 ms have passed, it sends the Deny.
#[gpui::test]
fn the_answers_pause_after_the_list_changes(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box")]);
    push(&mut helm, &[stop("a", "box"), stop("b", "box")], &[]);
    settle(cx);
    click(cx, "approval-a-answer-deny");
    assert!(helm.commands().is_empty(), "paused: nothing sent");
    cx.executor().advance_clock(ARM_DELAY);
    settle(cx);
    click(cx, "approval-a-answer-deny");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/approvals/a");
    assert_eq!(body, json!({ "answer": "deny" }));
}

/// SPEC.md: cards remain answerable over open dialogs. Specified: with a dialog open, Deny on the card sends the answer
/// and the dialog stays open.
#[gpui::test]
fn a_card_is_answerable_over_an_open_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box")]);
    let inside = cx.update(|_, cx| cx.focus_handle().tab_stop(true));
    test_support::open_dialog(cx, vec![("dialog-control", inside.clone())], &inside);
    assert!(any_open(cx), "premise: a dialog is open");
    click(cx, "approval-a-answer-deny");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/approvals/a");
    assert_eq!(body, json!({ "answer": "deny" }));
    assert!(any_open(cx), "the dialog stays open");
}

// ---- The late notice ---------------------------------------------------------------------------------------------------

/// SPEC.md: an answer that arrives after its request stopped waiting says so, in the region, above the card, until
/// dismissed. Specified: with a request waiting, the posted late-answer notice is drawn with its words above that
/// request's card; with nothing waiting it still draws the region; Dismiss withdraws it and the region draws nothing
/// again.
#[gpui::test]
fn the_late_notice_shows_in_the_region_until_dismissed(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box")]);
    cx.update(|_, cx| notices::post(Notice::LateAnswer { always: false }, cx));
    settle(cx);
    assert_eq!(
        label(cx, "approval-late"),
        Some(Notice::LateAnswer { always: false }.text().to_string())
    );
    let top = |cx: &mut VisualTestContext, id: &str| {
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| node.bounds.origin.y)
            .unwrap_or_else(|| panic!("{id} is drawn"))
    };
    assert!(
        top(cx, "approval-late") < top(cx, "approval-a-card"),
        "the notice sits above the card"
    );
    push(&mut helm, &[], &["a"]);
    settle(cx);
    assert!(drawn(cx), "the notice alone still draws the region");
    click(cx, "approval-late-dismiss");
    assert!(!drawn(cx), "dismissed: nothing left to draw");
}

/// SPEC.md: if session chrome leaves less than the card's header and answer bar need, the region uses the whole
/// viewport. What those need is measured as they are drawn, so the first frame of a new card cannot know it: the region
/// must draw again by itself once it does, not wait for something else to redraw the window. Specified: in a window
/// 150 pixels high (a cap of 112, less than a card's header, From line and answer bar), a request arriving puts the region at the window's top (the fallback) once the window settles,
/// with no time passing.
#[gpui::test]
fn a_short_window_gives_the_region_the_whole_window_without_waiting(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[]);
    cx.simulate_resize(size(px(1200.), px(150.)));
    settle(cx);
    push(&mut helm, &[stop("a", "box")], &[]);
    settle(cx);
    let region = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("approval-region"))
        .expect("the region is drawn");
    assert_eq!(
        region.bounds.origin.y,
        px(0.),
        "the region took the whole window"
    );
}

// ---- Every kind at the old pane widths --------------------------------------------------------------------------------

/// A session reference for the layout fixtures.
fn session(name: &str) -> ApprovalSession {
    ApprovalSession {
        id: format!("7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:{name}"),
        title: Some(format!("{name} title")),
        host_name: Some("build-box".to_owned()),
    }
}

/// A command launch whose command line is long enough to need its box's whole width and more.
fn long_command() -> SessionLaunch {
    SessionLaunch::Command(CommandLaunch {
        command: format!("run-agent --flag {} --then resume", "segment/".repeat(30)),
        yolo: false,
        agent: None,
        resume: Some("run-agent --resume last".to_owned()),
    })
}

/// One request of every kind the helm sends, each with the longest values its card shows (a deep folder, a long
/// command, a prompt of many lines, a long URL), so a card that lets any of them push its header or answers out of the
/// region is caught. The ids name the kinds.
fn every_kind() -> Vec<PendingApproval> {
    let folder = format!("/srv/{}", "nested-folder/".repeat(12));
    let prompt = (1..=40)
        .map(|line| format!("Line {line} of the prompt.\n"))
        .collect::<String>();
    let fields = TemplateFields {
        kind: Some(farhelm_proto::launcher::LauncherKind::Agent),
        agent: Some(LaunchHarness::Codex),
        ..TemplateFields::default()
    };
    let actions = [
        ApprovalAction::Launch {
            verb: LaunchVerb::Spawn,
            host_name: "build-box".into(),
            cwd: folder.clone(),
            repo: Some("example/repository".into()),
            title: Some("child".into()),
            launch: long_command(),
            source: None,
        },
        ApprovalAction::LaunchWithPrompt {
            verb: LaunchVerb::Create,
            host_name: "build-box".into(),
            cwd: folder.clone(),
            repo: None,
            title: None,
            launch: long_command(),
            source: None,
            prompt: prompt.clone(),
        },
        ApprovalAction::LaunchWithAgentWrapper {
            verb: LaunchVerb::Clone,
            host_name: "build-box".into(),
            cwd: folder.clone(),
            repo: None,
            title: None,
            launch: long_command(),
            source: Some(session("source")),
            prompt: Some(prompt.clone()),
        },
        ApprovalAction::LaunchEnvironment {
            verb: LaunchVerb::Create,
            recipe: "dev".into(),
            place: farhelm_proto::Place::TensorlakeAccount,
            place_name: None,
            cwd: folder.clone(),
            repo: None,
            title: None,
            launch: long_command(),
            source: None,
            prompt: Some(prompt.clone()),
        },
        ApprovalAction::Rename {
            target: session("target"),
            title: "a new title".into(),
        },
        ApprovalAction::Stop {
            target: session("target"),
        },
        ApprovalAction::Prompt {
            target: session("target"),
            launch: long_command(),
            screen_yolo: false,
            attached: true,
            prompt: prompt.clone(),
        },
        ApprovalAction::Delete {
            target: session("target"),
            guard: DeleteGuard::Unconditional,
        },
        ApprovalAction::Restart {
            target: session("target"),
            stop_if_running: true,
            launch: Some(long_command()),
        },
        ApprovalAction::TemplateWrite {
            name: "t".into(),
            replaces_existing: true,
            fields: fields.clone(),
            host_name: None,
        },
        ApprovalAction::TemplateWriteWithShortcuts {
            name: "t".into(),
            replaces_existing: false,
            fields: fields.clone(),
            host_name: None,
            shortcuts: TemplateShortcutCard {
                quick_launch: Some(QuickLaunchCard::Set {
                    moved_from: Some("old".into()),
                }),
                key: Some(TemplateKeyCard::Set {
                    key: "1".into(),
                    replaced_key: Some("2".into()),
                    moved_from: Some("old".into()),
                }),
            },
        },
        ApprovalAction::TemplateDelete {
            name: "t".into(),
            fields,
            host_name: None,
        },
        ApprovalAction::HostAdd {
            destination: format!("ops@{}example", "long-host-name.".repeat(8)),
            alias: Some("new".into()),
        },
        ApprovalAction::HostEdit {
            host_name: "build-box".into(),
            destination: Some("ops@build-box.example".into()),
            alias: Some(HostFieldChange {
                old: Some("old".into()),
                new: Some("new".into()),
            }),
            icon: None,
            color: None,
        },
        ApprovalAction::HostRemove {
            host_name: "build-box".into(),
            destination: "ops@build-box.example".into(),
        },
        ApprovalAction::OpenUrl {
            url: format!("https://example.com/{}?end=1", "path-part/".repeat(40)),
            local_network: false,
            preapproved: false,
            tunnel_available: false,
        },
    ];
    // Exhaustive on purpose: a new request kind does not compile until it has a slot here, and the assertion then
    // asks for a fixture of it above.
    let slot = |action: &ApprovalAction| match action {
        ApprovalAction::HostAdd { .. } => 0,
        ApprovalAction::OpenUrl { .. } => 1,
        ApprovalAction::Launch { .. } => 2,
        ApprovalAction::LaunchWithPrompt { .. } => 3,
        ApprovalAction::LaunchWithAgentWrapper { .. } => 4,
        ApprovalAction::LaunchEnvironment { .. } => 5,
        ApprovalAction::Rename { .. } => 6,
        ApprovalAction::Stop { .. } => 7,
        ApprovalAction::Prompt { .. } => 8,
        ApprovalAction::Delete { .. } => 9,
        ApprovalAction::Restart { .. } => 10,
        ApprovalAction::TemplateWrite { .. } => 11,
        ApprovalAction::TemplateWriteWithShortcuts { .. } => 12,
        ApprovalAction::TemplateDelete { .. } => 13,
        ApprovalAction::HostEdit { .. } => 14,
        ApprovalAction::HostRemove { .. } => 15,
    };
    let mut covered = [false; 16];
    for action in &actions {
        covered[slot(action)] = true;
    }
    assert!(
        covered.iter().all(|kind| *kind),
        "a fixture of every request kind"
    );
    actions
        .into_iter()
        .map(|action| {
            let mut approval = stop("", "build-box");
            approval.id = super::model::kind(&action).replace('_', "-");
            approval.own_host_session_action = matches!(action, ApprovalAction::Launch { .. });
            approval.action = action;
            approval
        })
        .collect()
}

/// The bounds of the element with id `id` in the last frame, if it was drawn.
fn bounds_of(nodes: &[gpui::DriverNode], id: &str) -> Option<Bounds<gpui::Pixels>> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.bounds)
}

/// SPEC.md: the card's header and answer bar stay visible within the region's cap; long texts give way first. The old
/// app checked every request kind in panes 960, 650 and 320 pixels wide ("every approval kind keeps its header and
/// answers visible in a <width>px pane"), stepping one region through the kinds so that geometry a previous kind left
/// behind is caught too. Specified: in windows of those widths, 900 and 500 pixels high, each kind's expanded card has
/// its heading and every answer button inside the region, and the region inside the window, with at least two answers;
/// and the card keeps one left edge and width across the kinds. This test context's text gives every character one
/// width, so the real fonts' version of the same layout is the running app's screenshot scene.
#[gpui::test]
fn every_kind_keeps_its_heading_and_answers_inside_the_region_at_the_old_pane_widths(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _terminal) = open(cx, &[]);
    let kinds = every_kind();
    // Whether a box taller than its minimum in a tall window was held at the minimum in a short one: without one, the
    // minimum check below would only ever see boxes at their natural height (a one-line box sits at its minimum in any
    // window). The tall windows come first at each width, so each box's tall height is known when its short one is.
    let mut tall_heights = std::collections::HashMap::new();
    let mut squeezed = false;
    for (width, height) in [
        (960., 900.),
        (650., 900.),
        (320., 900.),
        (960., 500.),
        (320., 500.),
    ] {
        cx.simulate_resize(size(px(width), px(height)));
        let mut left_and_width = None;
        let mut previous: Option<&str> = None;
        for approval in &kinds {
            let gone: Vec<&str> = previous.into_iter().collect();
            push(&mut helm, std::slice::from_ref(approval), &gone);
            settle(cx);
            previous = Some(&approval.id);
            let at = format!("{} in a {width}x{height} window", approval.id);
            let nodes = frame(cx);
            let region =
                bounds_of(&nodes, "approval-region").unwrap_or_else(|| panic!("{at}: drawn"));
            let window = Bounds::new(point(px(0.), px(0.)), size(px(width), px(height)));
            let inside = |outer: Bounds<gpui::Pixels>, inner: Bounds<gpui::Pixels>| {
                inner.origin.x >= outer.origin.x - px(1.)
                    && inner.origin.y >= outer.origin.y - px(1.)
                    && inner.bottom_right().x <= outer.bottom_right().x + px(1.)
                    && inner.bottom_right().y <= outer.bottom_right().y + px(1.)
            };
            assert!(
                inside(window, region),
                "{at}: the region is inside the window"
            );
            let card = bounds_of(&nodes, &format!("approval-{}-card", approval.id))
                .unwrap_or_else(|| panic!("{at}: its card is expanded"));
            let heading = bounds_of(&nodes, &format!("approval-{}-heading", approval.id))
                .unwrap_or_else(|| panic!("{at}: its heading is drawn"));
            assert!(
                inside(region, heading),
                "{at}: the heading is inside the region"
            );
            let answers: Vec<_> = [
                "allow",
                "always-allow",
                "always-allow-own-host",
                "tunnel-and-open",
                "deny",
            ]
            .iter()
            .filter_map(|word| {
                bounds_of(&nodes, &format!("approval-{}-answer-{word}", approval.id))
            })
            .collect();
            assert!(
                answers.len() >= 2,
                "{at}: at least two answers, got {}",
                answers.len()
            );
            for answer in &answers {
                assert!(
                    inside(region, *answer),
                    "{at}: every answer is inside the region"
                );
            }
            // The old "long boxes shrink to their computed minima before the body scrolls": a box gives way, but never
            // below its minimum, about two lines and its padding.
            for index in 0.. {
                let Some(text_box) =
                    bounds_of(&nodes, &format!("approval-{}-box-{index}", approval.id))
                else {
                    break;
                };
                assert!(
                    text_box.size.height >= BOX_MIN_HEIGHT - px(1.),
                    "{at}: box {index} is {:?} high, below its minimum",
                    text_box.size.height
                );
                let key = (approval.id.clone(), index, width as u32);
                if height == 900. {
                    tall_heights.insert(key.clone(), text_box.size.height);
                }
                squeezed |= tall_heights
                    .get(&key)
                    .is_some_and(|tall| *tall > BOX_MIN_HEIGHT + px(1.))
                    && text_box.size.height <= BOX_MIN_HEIGHT + px(1.);
            }
            let placed = (card.origin.x, card.size.width);
            assert_eq!(
                *left_and_width.get_or_insert(placed),
                placed,
                "{at}: one left edge and width"
            );
        }
    }
    assert!(
        squeezed,
        "premise: a short window held a box at its minimum that a tall one drew taller"
    );
}

/// SPEC.md: the other waiting requests are listed below the card in a list of their own, which scrolls by itself, and
/// the late-answer notice sits above the card; neither may push the card's answers out of reach. The old app checked a
/// long queue below a tall card ("waiting requests scroll independently below a tall card and fixed late notice") and
/// the same in a narrow pane ("a narrow short queue and late notice keep every answer reachable"). Specified: with the
/// notice posted, a long-prompt card expanded and eleven more requests waiting, in a 960 by 900 and a 320 by 800
/// window, the notice is above the card and inside the region, every answer is inside the region, and the waiting list
/// is inside the region while its last header lies below the list's bottom, where only the list's own scroll reaches
/// it. A window too short for the notice, the list and the card's fixed parts together makes the whole region scroll
/// instead, which the old test accepted too; that fallback is the placement tests'. (In this test context's
/// one-width text the 296-pixel notice alone takes 174 pixels, so a 500-pixel window already falls back.)
#[gpui::test]
fn a_long_queue_scrolls_in_its_own_list_below_the_card_and_the_notice(cx: &mut TestAppContext) {
    let tall = every_kind()
        .into_iter()
        .find(|approval| approval.id == "launch-with-prompt")
        .expect("the fixtures hold a launch with a prompt");
    let mut queue = vec![tall.clone()];
    queue.extend((1..=11).map(|n| stop(&format!("waiting-{n}"), "build-box")));
    let (_helm, cx, _terminal) = open(cx, &queue);
    cx.update(|_, cx| notices::post(Notice::LateAnswer { always: false }, cx));
    for (width, height) in [(960., 900.), (320., 800.)] {
        cx.simulate_resize(size(px(width), px(height)));
        settle(cx);
        let at = format!("a {width}x{height} window");
        let nodes = frame(cx);
        let found =
            |id: &str| bounds_of(&nodes, id).unwrap_or_else(|| panic!("{at}: {id} is drawn"));
        let region = found("approval-region");
        let within = |inner: Bounds<gpui::Pixels>| {
            inner.origin.y >= region.origin.y - px(1.)
                && inner.bottom_right().y <= region.bottom_right().y + px(1.)
        };
        let notice = found("approval-late");
        let card = found(&format!("approval-{}-card", tall.id));
        assert!(
            region.origin.y > px(0.),
            "{at}: premise: the region is placed below the top, not the whole-window fallback"
        );
        assert!(within(notice), "{at}: the notice is inside the region");
        assert!(
            notice.origin.y < card.origin.y,
            "{at}: the notice is above the card"
        );
        for word in ["allow", "deny"] {
            let answer = found(&format!("approval-{}-answer-{word}", tall.id));
            assert!(within(answer), "{at}: {word} is inside the region");
        }
        let list = found("approval-waiting-list");
        assert!(within(list), "{at}: the waiting list is inside the region");
        assert!(
            list.origin.y > card.origin.y,
            "{at}: the waiting list is below the card"
        );
        let last = found("approval-waiting-11-waiting");
        assert!(
            last.origin.y >= list.bottom_right().y,
            "{at}: the last header is past the list's bottom, for the list's own scroll to reach"
        );
    }
}
