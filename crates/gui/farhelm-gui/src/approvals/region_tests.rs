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
//!
//! Several tests drive the region as a user does, where the old app's layout spec did: with the keyboard, the wheel and
//! window resizes, with a dialog open, and in the real main window.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::approvals::{
    ApprovalAction, ApprovalLaunchKind, ApprovalSession, HostFieldChange, LaunchVerb,
    PendingApproval, QuickLaunchCard, TemplateKeyCard, TemplateShortcutCard,
};
use farhelm_proto::launcher::TemplateFields;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use farhelm_proto::{
    CommandLaunch, DeleteGuard, LaunchHarness, LaunchPermission, LaunchSelection, SessionLaunch,
};
use gpui::{
    Bounds, Context, DriverNode, FocusHandle, IntoElement, Modifiers, ParentElement as _, Pixels,
    Render, ScrollDelta, ScrollEvent, Styled as _, TestAppContext, VisualTestContext, Window, div,
    point, px, size,
};
use serde_json::json;

use super::answers::ARM_DELAY;
use super::card::BOX_MIN_HEIGHT;
use super::card_tests::texts_within;
use super::model::OWN_HOST_HELP;
use super::region::{self, Place, place};
use crate::host_text::tests::paints_isolated;
use crate::layers::{self, Anchor};
use crate::notices::{self, Notice};
use crate::root;
use crate::test_support::{
    self, any_open, button, center, click, focused, focused_id, frame, keys, label, meta,
    one_command, press, release, settle, started,
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
/// Used by another module's tests (`templates::save_tests`), so it is `pub(crate)`.
pub(crate) fn stop(id: &str, host: &str) -> PendingApproval {
    PendingApproval {
        id: id.to_owned(),
        host_id: 1,
        host_name: host.to_owned(),
        session: ApprovalSession {
            id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:asker".to_owned(),
            title: None,
            host_name: None,
            launch_kind: None,
        },
        action: ApprovalAction::Stop {
            target: ApprovalSession {
                id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:target".to_owned(),
                title: None,
                host_name: None,
                launch_kind: Some(ApprovalLaunchKind::Shell),
            },
        },
        own_host_session_action: false,
        expires_at_ms: 1,
    }
}

/// The approvals collection holding `approvals`, oldest first.
/// Used by another module's tests (`templates::save_tests`), so it is `pub(crate)`.
pub(crate) fn waiting(approvals: &[PendingApproval]) -> CollectionState {
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
/// Used by another module's tests (`templates::save_tests`), so it is `pub(crate)`.
pub(crate) fn push(helm: &mut FakeHelm, approvals: &[PendingApproval], gone: &[&str]) {
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
        Some("Stop a session's shell and close its terminal tabs, from host other")
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
        launch_kind: None,
    }
}

/// A command launch whose command line is long enough to need its box's whole width and more.
fn long_command() -> SessionLaunch {
    SessionLaunch::Command(CommandLaunch {
        custom_agent: None,
        command: format!("run-agent --flag {} --then resume", "segment/".repeat(30)),
        yolo: false,
        agent: None,
        resume: Some("run-agent --resume last".to_owned()),
    })
}

/// An agent launch whose agent-supplied wrapper words, command and resume command each run past their box's cap at
/// the usual card width, so a wrapper launch with a long prompt has four long boxes competing for space: the old
/// two-step resize's boxes, built from the old `layoutRequests()`'s wrapper launch.
fn wrapped_agent_launch() -> SessionLaunch {
    let wrapper = vec![
        "env".to_owned(),
        format!("WRAPPER={}", "wrapper".repeat(90)),
    ];
    let command = format!("agent {}", "--review-long-command-argument ".repeat(60));
    let words = |tail: &[&str]| -> Vec<String> {
        wrapper
            .iter()
            .cloned()
            .chain(tail.iter().map(|word| (*word).to_owned()))
            .collect()
    };
    SessionLaunch::Agent {
        custom_reporting: None,
        selection: LaunchSelection {
            custom_agent: None,
            harness: LaunchHarness::Claude,
            model: None,
            effort: None,
            permissions: Some(LaunchPermission::Guarded),
            permission_flags: None,
            wrapper_prefix: Some(wrapper.clone()),
            container: None,
            workspace_trust: None,
        },
        start: words(&["claude", "{farhelm_args}", command.as_str()]),
        resume: Some(words(&[
            "claude",
            "{farhelm_args}",
            "--resume",
            "{conversation}",
            command.as_str(),
        ])),
    }
}

/// One request of every kind the helm sends, each with the longest values its card shows (a deep folder, a long
/// command, long wrapper words, a prompt of many lines, a long URL), so a card that lets any of them push its header or
/// answers out of the region is caught. The ids name the kinds.
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
            launch: wrapped_agent_launch(),
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

/// The region over `queue` with the plain late-answer notice posted, in a window `width` by `height`, settled: the
/// long-queue test's start in each of its windows. Returns the window's bounds beside the usual parts.
fn queued_with_the_late_notice<'a>(
    cx: &'a mut TestAppContext,
    queue: &[PendingApproval],
    width: f32,
    height: f32,
) -> (FakeHelm, &'a mut VisualTestContext, Bounds<Pixels>) {
    let (helm, cx, _terminal) = open(cx, queue);
    cx.update(|_, cx| notices::post(Notice::LateAnswer { always: false }, cx));
    let window = Bounds::new(point(px(0.), px(0.)), size(px(width), px(height)));
    cx.simulate_resize(window.size);
    settle(cx);
    (helm, cx, window)
}

/// SPEC.md: the other waiting requests are listed below the card in a list of their own, which scrolls by itself, and
/// the late-answer notice sits above the card; neither may push the card's answers out of reach, and in a window too
/// short even for those the region scrolls as a whole. The old app checked a long queue below a tall card ("waiting
/// requests scroll independently below a tall card and fixed late notice") and the same in a short, narrow window ("a
/// narrow short queue and late notice keep every answer reachable"), and in both scrolled the list to its end for real
/// and chose its last request, which must be choosable, not merely drawn. Each window gets an app of its own, since the
/// press changes which card is open. Both start with the notice posted, a long-prompt card expanded and eleven more
/// requests waiting.
///
/// Specified, in a 960 by 900 window: the region lies inside the window, below its top; the notice is above the card,
/// and the notice, Allow, Deny and the waiting list are inside the region, the list below the card with its last header
/// past the list's bottom; a wheel over the list scrolls the last header inside it while the region, the notice, the
/// card, its heading and its Allow and Deny stay where they were, inside the region; a press on the last header opens
/// its card, and the notice still shows.
///
/// Specified, in a 320 by 430 window, where the region's parts do not fit even the window: the region lies inside the
/// window and its content runs past its bottom; after a wheel over it, every answer (Allow, Always allow and Deny) is
/// inside the region and the window while the last header still lies past the list's bottom; after a wheel over the
/// list, the last header is inside the list, the region and the window; and a press on it opens its card, the notice
/// still showing.
#[gpui::test]
fn a_long_queue_scrolls_in_its_own_list_below_the_card_and_the_notice(
    wide: &mut TestAppContext,
    narrow: &mut TestAppContext,
) {
    let tall = every_kind()
        .into_iter()
        .find(|approval| approval.id == "launch-with-prompt")
        .expect("the fixtures hold a launch with a prompt");
    let mut queue = vec![tall.clone()];
    queue.extend((1..=11).map(|n| stop(&format!("waiting-{n}"), "build-box")));
    let id = |part: &str| format!("approval-{}-{part}", tall.id);
    let last = "approval-waiting-11-waiting";
    let chosen_with_the_notice = |cx: &mut VisualTestContext, at: &str| {
        click(cx, last);
        assert_eq!(
            expanded(cx).as_deref(),
            Some("waiting-11"),
            "{at}: the last header opened its card"
        );
        assert_eq!(
            label(cx, "approval-late"),
            Some(Notice::LateAnswer { always: false }.text().to_string()),
            "{at}: the notice still shows"
        );
    };

    let (_helm, cx, window) = queued_with_the_late_notice(wide, &queue, 960., 900.);
    let before = frame(cx);
    let region = drawn_at(&before, "approval-region");
    let notice = drawn_at(&before, "approval-late");
    let card = drawn_at(&before, &id("card"));
    assert!(
        inside(window, region),
        "wide: the region is inside the window"
    );
    assert!(
        region.top() > px(0.),
        "wide: premise: the region is placed below the top, not the whole-window fallback"
    );
    assert!(
        inside(region, notice),
        "wide: the notice is inside the region"
    );
    assert!(
        notice.top() < card.top(),
        "wide: the notice is above the card"
    );
    for word in ["allow", "deny"] {
        let answer = drawn_at(&before, &id(&format!("answer-{word}")));
        assert!(inside(region, answer), "wide: {word} is inside the region");
    }
    let list = drawn_at(&before, "approval-waiting-list");
    assert!(
        inside(region, list),
        "wide: the waiting list is inside the region"
    );
    assert!(
        list.top() > card.top(),
        "wide: the waiting list is below the card"
    );
    assert!(
        drawn_at(&before, last).top() >= list.bottom(),
        "wide: the last header is past the list's bottom, for the list's own scroll to reach"
    );
    wheel(cx, list, px(5000.));
    let after = frame(cx);
    assert!(
        inside(
            drawn_at(&after, "approval-waiting-list"),
            drawn_at(&after, last)
        ),
        "wide: the wheel scrolled the list to its last header"
    );
    let region = drawn_at(&after, "approval-region");
    let held = [
        "approval-region".to_owned(),
        "approval-late".to_owned(),
        id("card"),
        id("heading"),
        id("answer-allow"),
        id("answer-deny"),
    ];
    for part in &held {
        let (was, is) = (drawn_at(&before, part), drawn_at(&after, part));
        assert!(
            same_place(was, is),
            "wide: {part} stayed put: {was:?} then {is:?}"
        );
        assert!(inside(region, is), "wide: {part} is inside the region");
    }
    chosen_with_the_notice(cx, "wide");

    // The region scrolls as a whole here, and the list keeps its own scroll inside it, as the old narrow case had it.
    let (_helm, cx, window) = queued_with_the_late_notice(narrow, &queue, 320., 430.);
    let nodes = frame(cx);
    let region = drawn_at(&nodes, "approval-region");
    assert!(
        inside(window, region),
        "narrow: the region is inside the window"
    );
    assert!(
        overflow_below(&nodes, "approval-region") > px(1.),
        "narrow: premise: the region's parts run past it, so it scrolls as a whole"
    );
    wheel(cx, region, px(5000.));
    let nodes = frame(cx);
    let region = drawn_at(&nodes, "approval-region");
    for word in ["allow", "always-allow", "deny"] {
        let answer = drawn_at(&nodes, &id(&format!("answer-{word}")));
        assert!(
            inside(region, answer) && inside(window, answer),
            "narrow: after the region's own scroll, {word} is inside the region and the window: {answer:?}"
        );
    }
    let list = drawn_at(&nodes, "approval-waiting-list");
    assert!(
        drawn_at(&nodes, last).top() >= list.bottom(),
        "narrow: premise: the region's scroll left the last header past the list's bottom, for the list's own scroll"
    );
    wheel(cx, list, px(5000.));
    let nodes = frame(cx);
    let header = drawn_at(&nodes, last);
    for (what, outer) in [
        ("list", drawn_at(&nodes, "approval-waiting-list")),
        ("region", drawn_at(&nodes, "approval-region")),
        ("window", window),
    ] {
        assert!(
            inside(outer, header),
            "narrow: the last header is inside the {what}: {header:?} in {outer:?}"
        );
    }
    chosen_with_the_notice(cx, "narrow");
}

// ---- Driving the region as a user does ------------------------------------------------------------------------------

/// Whether `inner` lies inside `outer`, with a pixel of rounding allowed on every side.
fn inside(outer: Bounds<Pixels>, inner: Bounds<Pixels>) -> bool {
    inner.left() >= outer.left() - px(1.)
        && inner.top() >= outer.top() - px(1.)
        && inner.right() <= outer.right() + px(1.)
        && inner.bottom() <= outer.bottom() + px(1.)
}

/// Whether `a` and `b` start at the same point, to half a pixel: an element that did not move.
fn same_place(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    (a.left() - b.left()).abs() < px(0.5) && (a.top() - b.top()).abs() < px(0.5)
}

/// The bounds of the element drawn as `id` in `nodes`, which the test needs drawn.
fn drawn_at(nodes: &[DriverNode], id: &str) -> Bounds<Pixels> {
    bounds_of(nodes, id).unwrap_or_else(|| panic!("{id} is drawn"))
}

/// How far the content of the element drawn as `id` runs past its bottom: the lowest bottom among its own children less
/// its own bottom, the driver record's stand-in for the old `scrollHeight - clientHeight`. Positive when the content
/// overflows. Only direct children count, so a box that clips its own long text inside the element does not read as
/// the element overflowing.
fn overflow_below(nodes: &[DriverNode], id: &str) -> Pixels {
    let Some(at) = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some(id))
    else {
        panic!("{id} is drawn");
    };
    let lowest = nodes
        .iter()
        .filter(|node| node.parent == Some(at))
        .map(|node| node.bounds.bottom())
        .fold(nodes[at].bounds.top(), |lowest, bottom| lowest.max(bottom));
    lowest - nodes[at].bounds.bottom()
}

/// Turn the wheel `by` pixels toward the end of whatever scrolls under the middle of `over`, the pointer resting there
/// first, as a wheel or a trackpad does, and settle. Every scrolling element under the pointer takes the turn.
fn wheel(cx: &mut VisualTestContext, over: Bounds<Pixels>, by: Pixels) {
    let at = over.center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.), -by)),
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    settle(cx);
}

/// Whether the element drawn as `id` has a click listener, which an answer button draws only while it answers: a busy
/// one, paused or with an answer out, draws none ([`crate::components::Button::busy`]).
fn answers_presses(cx: &mut VisualTestContext, id: &str) -> bool {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} is drawn"))
        .flags
        .click
}

/// SPEC.md: the other waiting requests are headers that open their card, every control is reachable from the
/// keyboard, and the answers pause whenever the expanded request changes. The old app chose a header from the keyboard
/// ("one request stays selected across refreshes and rapid switches rearm the buttons": the second header focused, then
/// Enter), and the choice must pause the answers as a click on a header does, or an Enter or a press already aimed at
/// the card shown before could answer the one that replaced it. Specified: with first, second and third waiting and
/// first's answers armed, Tab reaches second's header; Enter there opens second's card; a press on its Deny right away
/// sends nothing; once the pause has run out, the same press sends second's Deny.
#[gpui::test]
fn a_header_chosen_from_the_keyboard_opens_its_card_and_pauses_the_answers(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _terminal) = open(
        cx,
        &[
            stop("first", "box"),
            stop("second", "laptop"),
            stop("third", "box"),
        ],
    );
    assert_eq!(
        expanded(cx).as_deref(),
        Some("first"),
        "premise: the oldest is open"
    );
    assert!(
        answers_presses(cx, "approval-first-answer-deny"),
        "premise: the answers are armed before the choice"
    );
    // Bounded: the stand-in terminal, the card's Copy buttons and its answers come before the headers in Tab's order.
    for _ in 0..20 {
        if focused_id(cx).as_deref() == Some("approval-second-waiting") {
            break;
        }
        keys(cx, "tab");
    }
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("approval-second-waiting"),
        "premise: Tab reaches the waiting header"
    );
    keys(cx, "enter");
    settle(cx);
    assert_eq!(
        expanded(cx).as_deref(),
        Some("second"),
        "Enter on the header opened its card"
    );
    click(cx, "approval-second-answer-deny");
    assert!(
        helm.commands().is_empty(),
        "the choice paused the answers: nothing sent"
    );
    cx.executor().advance_clock(ARM_DELAY);
    settle(cx);
    click(cx, "approval-second-answer-deny");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/approvals/second");
    assert_eq!(body, json!({ "answer": "deny" }));
}

/// SPEC.md: inside the region's cap the card's body gives way first, its boxes shrinking toward two lines and only then
/// its fact list scrolling, so the header and the answer bar stay. The old app checked it in two steps ("long boxes
/// shrink to their computed minima before the body scrolls"): a first resize takes away half of what the boxes can give
/// up, which a card that scrolled its body at once, its boxes still at their caps, cannot pass; a second exhausts it,
/// and the body then scrolls with the answers pinned. Specified, 1280 pixels wide, over the wrapper launch's four long
/// boxes: at 2400 high each box overflows above its minimum and nothing else gives way; at the height whose cap takes
/// half the boxes' allowance off the card, the boxes shrink by more than a quarter of it, none below its minimum, and
/// the body does not overflow; at 500 high every box is at its minimum and the body overflows, and the wheel scrolls it
/// while Deny stays put. The heading, Allow and Deny stay inside the region throughout.
#[gpui::test]
fn long_boxes_give_way_before_the_body_scrolls_and_then_the_answers_stay(cx: &mut TestAppContext) {
    let wrapper = every_kind()
        .into_iter()
        .find(|approval| approval.id == "launch-with-agent-wrapper")
        .expect("the fixtures hold a wrapper launch");
    let (_helm, cx, _terminal) = open(cx, &[wrapper]);
    let id = |part: &str| format!("approval-launch-with-agent-wrapper-{part}");
    let boxes = |nodes: &[DriverNode]| -> Vec<Bounds<Pixels>> {
        (0..)
            .map_while(|index| bounds_of(nodes, &id(&format!("box-{index}"))))
            .collect()
    };
    let total = |boxes: &[Bounds<Pixels>]| {
        boxes
            .iter()
            .fold(px(0.), |sum, text_box| sum + text_box.size.height)
    };
    let fixed_inside_region = |nodes: &[DriverNode], at: &str| {
        let region = drawn_at(nodes, "approval-region");
        assert!(
            region.top() > px(0.),
            "{at}: the region is placed below the top, not the whole-window fallback"
        );
        for part in ["heading", "answer-allow", "answer-deny"] {
            let drawn = drawn_at(nodes, &id(part));
            assert!(
                inside(region, drawn),
                "{at}: {part} is inside the region: {drawn:?} in {region:?}"
            );
        }
    };

    let window = |height: f32| Bounds::new(point(px(0.), px(0.)), size(px(1280.), px(height)));
    cx.simulate_resize(size(px(1280.), px(2400.)));
    settle(cx);
    let tall = frame(cx);
    let initial = boxes(&tall);
    assert_eq!(
        initial.len(),
        4,
        "premise: the wrapper, the command, the resume command and the prompt compete for space"
    );
    for (index, text_box) in initial.iter().enumerate() {
        assert!(
            overflow_below(&tall, &id(&format!("box-{index}-scroller"))) > px(1.),
            "premise: box {index}'s text overflows it even at its cap"
        );
        assert!(
            text_box.size.height > BOX_MIN_HEIGHT + px(1.),
            "premise: box {index} is above its minimum"
        );
    }
    let cap = place(window(2400.), None, px(2400.), px(0.)).cap;
    assert!(
        drawn_at(&tall, "approval-region").size.height < cap - px(1.),
        "premise: the tall window holds nothing back"
    );
    assert!(
        overflow_below(&tall, &id("body")) <= px(1.),
        "premise: the facts and the capped boxes fit"
    );
    fixed_inside_region(&tall, "the tall window");

    // The first step: the window whose cap takes half the boxes' allowance off the card, found through the placement's
    // own share of the pane, as the old test divided by the measured cap's share.
    let allowance = initial.iter().fold(px(0.), |sum, text_box| {
        sum + text_box.size.height - BOX_MIN_HEIGHT
    });
    let wanted = drawn_at(&tall, &id("card")).size.height - allowance / 2.;
    let share = f32::from(cap) / 2400.;
    let height = (f32::from(wanted) / share).round();
    cx.simulate_resize(size(px(1280.), px(height)));
    settle(cx);
    let middle = frame(cx);
    let shrunk = boxes(&middle);
    assert!(
        total(&shrunk) < total(&initial) - allowance / 4.,
        "the boxes gave up height to the smaller cap: {:?} from {:?}",
        total(&shrunk),
        total(&initial)
    );
    for (index, text_box) in shrunk.iter().enumerate() {
        assert!(
            text_box.size.height >= BOX_MIN_HEIGHT - px(1.),
            "box {index} is {:?} high, below its minimum",
            text_box.size.height
        );
    }
    assert!(
        overflow_below(&middle, &id("body")) <= px(1.),
        "boxes still above their minima keep the fact list from scrolling"
    );
    fixed_inside_region(&middle, "the first step");

    cx.simulate_resize(size(px(1280.), px(500.)));
    settle(cx);
    let short = frame(cx);
    assert!(
        overflow_below(&short, &id("body")) > px(1.),
        "only with every box at its minimum does the body overflow"
    );
    for (index, text_box) in boxes(&short).iter().enumerate() {
        assert!(
            (text_box.size.height - BOX_MIN_HEIGHT).abs() < px(1.),
            "box {index} is {:?} high, not at its minimum",
            text_box.size.height
        );
    }
    fixed_inside_region(&short, "the second step");
    let first = boxes(&short)[0];
    let deny = drawn_at(&short, &id("answer-deny"));
    wheel(cx, drawn_at(&short, &id("body")), px(5000.));
    let scrolled = frame(cx);
    assert!(
        boxes(&scrolled)[0].top() < first.top() - px(1.),
        "the overflowing body really scrolls"
    );
    assert!(
        same_place(deny, drawn_at(&scrolled, &id("answer-deny"))),
        "Deny stayed where it was"
    );
    fixed_inside_region(&scrolled, "scrolled");
}

/// SPEC.md: the answer bar wraps in a narrow pane with every answer visible. The old app checked an 80-character host
/// name ending in a direction control in both Always allow buttons of a 320-pixel pane (the long-host half of "a
/// compact card follows the main pane and keeps the answer buttons visible"): a row that wraps whole buttons can still
/// leave one host token wider than the card, which runs the button's words past the card's edge where nothing scrolls
/// them back. The buttons' escaped names are `card_tests`'. Specified: in a 320 by 800 window, a launch from host
/// `remote-`, 80 x's and U+202E, eligible for the narrow grant, shows four answers; each Always allow button paints the
/// whole escaped host, isolated, and lies inside the card and the region with all its words inside it; the card lies
/// inside the region and the region inside the window; and the narrow grant's hover help names the escaped host twice.
#[gpui::test]
fn a_long_host_stays_whole_inside_both_always_allow_buttons_in_a_narrow_pane(
    cx: &mut TestAppContext,
) {
    let raw = format!("remote-{}\u{202e}", "x".repeat(80));
    let shown = farhelm_proto::text::escaped(&raw);
    assert_eq!(
        shown,
        format!("remote-{}<U+202E>", "x".repeat(80)),
        "premise: the host's direction control is spelled out"
    );
    let mut approval = every_kind()
        .into_iter()
        .find(|approval| approval.id == "launch")
        .expect("the fixtures hold a launch");
    approval.host_name = raw;
    assert!(
        approval.own_host_session_action,
        "premise: the launch is eligible for the narrow grant"
    );
    let (_helm, cx, _terminal) = open(cx, &[approval]);
    let window = Bounds::new(point(px(0.), px(0.)), size(px(320.), px(800.)));
    cx.simulate_resize(window.size);
    settle(cx);

    let nodes = frame(cx);
    for word in ["allow", "always-allow-own-host", "always-allow", "deny"] {
        drawn_at(&nodes, &format!("approval-launch-answer-{word}"));
    }
    let region = drawn_at(&nodes, "approval-region");
    let card = drawn_at(&nodes, "approval-launch-card");
    assert!(inside(window, region), "the region is inside the window");
    assert!(inside(region, card), "the card is inside the region");
    for word in ["always-allow-own-host", "always-allow"] {
        let id = format!("approval-launch-answer-{word}");
        let button = drawn_at(&nodes, &id);
        let texts = texts_within(&nodes, &id);
        assert!(
            texts.iter().any(|text| paints_isolated(&text.text, &shown)),
            "{word} paints the whole escaped host, isolated: {texts:?}"
        );
        assert!(inside(card, button), "{word} is inside the card");
        assert!(inside(region, button), "{word} is inside the region");
        for text in &texts {
            assert!(
                text.bounds.left() >= button.left() - px(0.5)
                    && text.bounds.right() <= button.right() + px(0.5),
                "{word}'s words at {:?} stay inside the button {button:?}",
                text.bounds
            );
        }
    }

    let at = center(cx, "approval-launch-answer-always-allow-own-host");
    cx.simulate_mouse_move(at, None, Modifiers::none());
    settle(cx);
    test_support::advance(cx, layers::HOVER_DELAY);
    assert_eq!(
        cx.update(|window, cx| layers::shown_tooltip(window, cx))
            .map(|shown| shown.text.to_string()),
        Some(format!(
            "{}{shown}{}{shown}{}",
            OWN_HOST_HELP[0], OWN_HOST_HELP[1], OWN_HOST_HELP[2]
        )),
        "the narrow grant's help names the escaped host twice"
    );
}

/// SPEC.md: below the minimum its fixed parts need, the region scrolls as a whole, so the request and its answers stay
/// reachable however short the window. The old app checked a 48-pixel window, whose top row and margins once used up
/// the whole region ("a short viewport keeps approval content and answers reachable"); a region with some height could
/// still hold Deny out of reach, so it turned the wheel over the region and pressed Deny. The fallback's arithmetic is
/// the placement tests'. Specified: in a 1200 by 48 window with a stop request waiting, the region has a height and
/// lies inside the window; a wheel over it scrolls it (its heading moves up), and Deny then lies inside the window and
/// the region; a press on Deny sends exactly that answer, and once the helm removes the request and accepts the answer,
/// the region draws nothing.
#[gpui::test]
fn in_a_tiny_window_the_wheel_scrolls_the_region_to_deny_and_deny_answers(cx: &mut TestAppContext) {
    let (mut helm, cx, _terminal) = open(cx, &[stop("a", "box")]);
    let window = Bounds::new(point(px(0.), px(0.)), size(px(1200.), px(48.)));
    cx.simulate_resize(window.size);
    settle(cx);
    let before = frame(cx);
    let region = drawn_at(&before, "approval-region");
    assert!(
        region.size.height > px(0.),
        "a short window does not collapse the region"
    );
    assert!(
        inside(window, region),
        "the region is inside the window: {region:?}"
    );
    let heading = drawn_at(&before, "approval-a-heading");

    wheel(cx, region, px(2000.));
    let after = frame(cx);
    assert!(
        drawn_at(&after, "approval-a-heading").top() < heading.top() - px(1.),
        "the wheel scrolled the region"
    );
    let deny = drawn_at(&after, "approval-a-answer-deny");
    assert!(
        inside(window, deny) && inside(drawn_at(&after, "approval-region"), deny),
        "Deny is in view: {deny:?}"
    );
    click(cx, "approval-a-answer-deny");
    let (id, body) = one_command(&mut helm, Method::Post, "/api/approvals/a");
    assert_eq!(body, json!({ "answer": "deny" }));
    push(&mut helm, &[], &["a"]);
    helm.reply(id, 204, None);
    settle(cx);
    assert!(!drawn(cx), "the answered card went");
}

/// SPEC.md: the region draws nothing while nothing waits, and it places its card from what the card's fixed parts need
/// as they are drawn. The old app's region went when the last request did, and the next request's region came back
/// "with a fresh measured anchor" (the remount half of "placement follows the empty pane, session mount, and approval
/// remount"). This region is never removed: once it has had nothing to draw, the settled placement of the next card
/// must follow that card's own measurements, not hold it in the whole-window fallback for what earlier, taller
/// occupants needed. (The region also resets its measurements when it empties; that only spares the first frame after
/// the card arrives, which this test cannot observe.) Specified: in a 320 by 500 window, a long-host launch with four
/// more requests waiting needs more than the cap, so the region takes the window from its top; once every one of them
/// is gone the region draws nothing; a stop request arriving alone then puts the region 12 pixels below the pane's top.
#[gpui::test]
fn a_card_after_the_region_emptied_is_placed_by_its_own_needs(cx: &mut TestAppContext) {
    let mut tall = every_kind()
        .into_iter()
        .find(|approval| approval.id == "launch")
        .expect("the fixtures hold a launch");
    tall.host_name = format!("remote-{}\u{202e}", "x".repeat(80));
    let mut first = vec![tall];
    first.extend((0..4).map(|n| stop(&format!("queued-{n}"), "box")));
    let (mut helm, cx, _terminal) = open(cx, &first);
    cx.simulate_resize(size(px(320.), px(500.)));
    settle(cx);
    assert_eq!(
        drawn_at(&frame(cx), "approval-region").top(),
        px(0.),
        "premise: the first occupants need more than the cap, so the region took the whole window"
    );
    let gone: Vec<&str> = first.iter().map(|approval| approval.id.as_str()).collect();
    push(&mut helm, &[], &gone);
    settle(cx);
    assert!(!drawn(cx), "premise: nothing is left to draw");

    push(&mut helm, &[stop("later", "box")], &[]);
    settle(cx);
    assert_eq!(
        drawn_at(&frame(cx), "approval-region").top(),
        px(12.),
        "the new card is placed below the pane's top by what it needs itself"
    );
}

/// SPEC.md: the card is centered at the top of the main area below the session's tabs. The old app moved the card
/// below a newly mounted session's terminal (the session-mount half of "placement follows the empty pane, session
/// mount, and approval remount"); the placement tests give the region an area's top as a number, and only a window
/// shows the session pane handing its terminal area over as the region's anchor. Specified: in the real main window,
/// 1200 by 900, with a session open on a reachable host, its terminal live and a request waiting, the session's tab
/// strip lies above its terminal, and the region starts at least 12 pixels below the strip's bottom and at most 12
/// below the terminal's top: 12 below the terminal area's top, which lies between the two.
#[gpui::test]
fn with_a_session_open_the_region_starts_below_its_tabs(cx: &mut TestAppContext) {
    let mut collections = crate::session_list::menu_tests::snapshot(
        &[(crate::session_list::tests::session("a"), 0)],
        false,
    );
    collections.push(crate::hosts::tests::hosts(&[crate::hosts::tests::host(
        2,
        "ssh",
        crate::hosts::tests::connected(),
    )]));
    collections.push(waiting(&[stop("req", "box")]));
    let mut main = test_support::open_main(cx, collections);
    main.terminal_live();
    let cx = main.cx;
    cx.simulate_resize(size(px(1200.), px(900.)));
    settle(cx);
    let nodes = frame(cx);
    let strip = drawn_at(&nodes, crate::terminal::set::STRIP_ID);
    let grid = drawn_at(&nodes, "terminal-grid");
    let region = drawn_at(&nodes, "approval-region");
    assert!(
        strip.bottom() <= grid.top(),
        "premise: the tabs are drawn above the terminal: {strip:?}, {grid:?}"
    );
    assert!(
        region.top() >= strip.bottom() + px(12.),
        "the region starts 12 pixels clear of the tabs: {region:?} under {strip:?}"
    );
    assert!(
        region.top() <= grid.top() + px(12.5),
        "and no further than 12 below the terminal's top: {region:?}, {grid:?}"
    );
}

/// SPEC.md, "Keyboard focus": a request arriving moves no focus and opens nothing, and "an existing draft or another
/// modal is kept". The old app let an agent's host add arrive while the user typed into the ordinary Add host dialog
/// (the arrival half of "host add preserves a manual draft and defers only an explicit open"): the card shows, and the
/// draft is untouched. The explicit open's half is the host add tests'. Specified: with the ordinary Add host open and
/// a destination and an alias typed into it, an agent's host add arriving draws its card, and the dialog stays open
/// with both typed values as they were and focus still in the alias.
#[gpui::test]
fn an_agents_host_add_arriving_leaves_a_typed_add_host_draft_alone(cx: &mut TestAppContext) {
    use crate::hosts::add::tests::{alias, alias_focused, destination, type_text};
    let (mut helm, cx, terminal) = open(cx, &[]);
    let opened = cx.update(|window, cx| {
        let mark = crate::focus::Mark::take(cx);
        crate::hosts::add::open(&terminal, &mark, window, cx)
    });
    settle(cx);
    assert!(opened.is_some(), "premise: the ordinary Add host opened");
    type_text(cx, "user@manualdraft.invalid");
    // The Tailscale host checkbox sits between the destination and the alias.
    keys(cx, "tab tab");
    assert!(alias_focused(cx), "premise: Tab reached the alias");
    type_text(cx, "manualalias");
    let typed = (
        "user@manualdraft.invalid".to_owned(),
        "manualalias".to_owned(),
    );
    assert_eq!(
        (destination(cx), alias(cx)),
        typed,
        "premise: the draft is typed"
    );

    let mut add = stop("add-1", "build");
    add.action = ApprovalAction::HostAdd {
        destination: "ops@new.example".to_owned(),
        alias: Some("newbox".to_owned()),
    };
    push(&mut helm, &[add], &[]);
    settle(cx);
    assert_eq!(
        expanded(cx).as_deref(),
        Some("add-1"),
        "the request's card shows"
    );
    assert!(any_open(cx), "the dialog stays open");
    assert_eq!(
        (destination(cx), alias(cx)),
        typed,
        "the draft is untouched"
    );
    assert!(alias_focused(cx), "the arrival left focus in the alias");
}

/// SPEC.md, "Agent-spawned sessions": "Opening is not approval", and repeated Enter cannot answer a new question.
/// Opening the prefilled Add host dialog is a chance to review the agent's request. Enter on the armed card moves
/// focus to the destination on key-down. Repeats of that press must send neither Allow nor a probe. After release,
/// a fresh Enter in the destination sends exactly one Allow for the displayed destination.
#[gpui::test]
fn a_held_enter_that_opened_add_host_cannot_approve_it(cx: &mut TestAppContext) {
    use crate::hosts::add::tests::{alias, destination, destination_focused};

    let mut add = stop("add-1", "build");
    add.action = ApprovalAction::HostAdd {
        destination: "ops@new.example".to_owned(),
        alias: Some("newbox".to_owned()),
    };
    let (mut helm, cx, _terminal) = open(cx, &[add]);
    let opener = "approval-add-1-answer-allow";
    assert!(answers_presses(cx, opener), "premise: the card is armed");
    for _ in 0..20 {
        if focused_id(cx).as_deref() == Some(opener) {
            break;
        }
        keys(cx, "tab");
    }
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(opener),
        "premise: Tab reached Open add dialog"
    );
    assert!(!any_open(cx), "premise: Add host has yet to open");

    press(cx, "enter", false);
    assert!(any_open(cx), "Enter opened Add host on key-down");
    assert!(
        destination_focused(cx),
        "premise: the destination has focus"
    );
    assert_eq!(destination(cx), "ops@new.example");
    assert_eq!(alias(cx), "newbox");
    assert!(helm.commands().is_empty(), "opening sent nothing");

    press(cx, "enter", true);
    cx.executor().advance_clock(ARM_DELAY);
    // Put Add on screen so the next repeat exercises the held-key guard even when the form can act.
    test_support::show_next_frame(cx);
    press(cx, "enter", true);
    let sent = helm.commands();
    assert!(sent.is_empty(), "held Enter sent requests: {sent:?}");

    release(cx, "enter");
    assert!(helm.commands().is_empty(), "releasing Enter sent nothing");
    assert!(
        destination_focused(cx),
        "premise: the destination still has focus"
    );
    press(cx, "enter", false);
    release(cx, "enter");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/approvals/add-1");
    assert_eq!(
        body,
        json!({ "answer": "allow", "host_add": { "destination": "ops@new.example" } })
    );
}
