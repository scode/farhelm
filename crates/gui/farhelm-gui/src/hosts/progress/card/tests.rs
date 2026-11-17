//! Tests of the host progress card ([`super`]): which hosts get one, what it shows and follows as the helm pushes, the
//! elapsed time it shares with the Hosts row, its hold on the ticker, its fit in a short window (both as drawn, with
//! the current step kept whole as the run moves it, and as the pure share it rests on), and its showing as the hover of
//! the Hosts row's running status, beside the sidebar.
//!
//! The card is drawn alone in a test root view beside a Hosts panel that is never drawn: the panel is there because its
//! store subscription is what records a run's first sight, whether or not Hosts shows, as in the app. The card is read
//! from GPUI's driver record by element id (`host-card-title-<id>`, `host-card-count-<id>`, `host-card-elapsed-<id>`,
//! `host-card-step-<id>-<n>`, `host-card-connection-<id>`, `host-card-run-message-<id>`, `host-card-notice-<id>`).
//!
//! The drawn fit's tests read the frame after the one a change was drawn in, since the card fits from the heights it
//! was last drawn at, and they run on the test context's fake text system. What they assert holds for any font: a diagnosis
//! many times the window's height is previewed and one sentence is not, and the current step is whole either way. The
//! exact cases, a text that fits to the pixel and the share between two texts, are pinned on [`fit`] and [`shares`].

use std::time::Duration;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::provisioning::{ProvisioningOperation, ProvisioningView, RunStatus, StepStatus};
use farhelm_proto::ui_channel::{Collection, CollectionChanges, HostView};
use gpui::{
    AppContext as _, Context, DriverNode, Entity, IntoElement, Modifiers, ParentElement as _,
    Render, Styled as _, TestAppContext, VisualTestContext, Window, div, point, px, size,
};

use super::{FULL_DETAILS, Fit, ProgressCard, fit, progress_card, shares};
use crate::hosts::HostsPanel;
use crate::hosts::progress::tests::step;
use crate::hosts::tests::{aliased, connected, connecting, host, hosts, progress, run};
use crate::layers::{self, TooltipSide};
use crate::test_support::{self, frame, meta, started};
use crate::{render_count, root};

// ---- Fixtures and harness ------------------------------------------------------------------------------------------

/// The failed connection's words in the fixtures, with a right-to-left override a host could send.
const ERROR: &str = "ssh: connect to host build-box port 22: Connection refused\u{202e}";

/// `build-box` (host 2, aliased) on connection attempt 2 after [`ERROR`].
fn build_box() -> HostView {
    aliased(
        host(2, "ssh", connecting(2, Some(ERROR))),
        "build-box",
        "dev@build-box.invalid",
    )
}

/// Host 2's setup in `status` with four steps: completed (with an explanation, and a message only the panel shows),
/// degraded (with its reason), running (with an explanation) and pending.
fn setup(status: RunStatus) -> ProvisioningView {
    let mut view = run(2, ProvisioningOperation::Add, status);
    let mut done = step("Create directories", StepStatus::Completed);
    done.explanation = Some("Prepared once a day, or after Farhelm updates.".into());
    done.message = Some("Created 3 directories".into());
    let mut warned = step("Upload the farhelm binary", StepStatus::Degraded);
    warned.message = Some("The upload was retried once".into());
    let mut current = step("Install the farhelm binary", StepStatus::Running);
    current.explanation = Some("Replaces the binary in ~/.local/bin.".into());
    view.steps = vec![
        done,
        warned,
        current,
        step("Connect to the supervisor", StepStatus::Pending),
    ];
    view
}

/// A root view holding a Hosts panel it never draws, and drawing the card it is given at [`super::WIDTH`], at the top
/// left.
struct Harness {
    _panel: Entity<HostsPanel>,
    card: Option<Entity<ProgressCard>>,
    root: root::Attachment,
}

impl Render for Harness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        root::wrap(
            div()
                .size_full()
                .child(div().w(super::WIDTH).children(self.card.clone())),
            &self.root,
            window,
            cx,
        )
    }
}

/// The harness over the fake helm, connected with `listed` hosts and `runs`, in an active window, checked once settled.
fn open<'a>(
    cx: &'a mut TestAppContext,
    listed: &[HostView],
    runs: &[ProvisioningView],
) -> (FakeHelm, Entity<Harness>, &'a mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![hosts(listed), progress(runs)]);
    cx.run_until_parked();
    let (harness, window) = test_support::open_view(cx, |window, cx| Harness {
        _panel: cx.new(HostsPanel::new),
        card: None,
        root: root::attach(window, cx),
    });
    (fake, harness, window)
}

/// Make host `host`'s card and draw it, as an owner would when its hover help shows; whether there was one.
fn show(harness: &Entity<Harness>, host: i64, cx: &mut VisualTestContext) -> bool {
    let card = cx.update(|_, cx| progress_card(host, cx));
    let made = card.is_some();
    cx.update(|_, cx| {
        harness.update(cx, |harness, cx| {
            harness.card = card;
            cx.notify();
        })
    });
    test_support::redraw(cx);
    made
}

/// The card the harness draws.
fn card(harness: &Entity<Harness>, cx: &mut VisualTestContext) -> Entity<ProgressCard> {
    cx.update(|_, cx| harness.read(cx).card.clone())
        .expect("the premise: a card is shown")
}

/// The node drawn under `id`, if any.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// The accessible name of the node drawn under `id`, if one was drawn with a name.
fn label(nodes: &[DriverNode], id: &str) -> Option<String> {
    node(nodes, id).and_then(|node| node.label.as_ref().map(ToString::to_string))
}

/// Push `views` as the hosts collection's one change, and settle.
fn push_hosts(fake: &mut FakeHelm, views: &[HostView], cx: &mut VisualTestContext) {
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["2"]),
        upsert: hosts(views).entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// Push `view` as host 2's progress, and settle.
fn push_run(fake: &mut FakeHelm, view: &ProvisioningView, cx: &mut VisualTestContext) {
    fake.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&["2"]),
        upsert: progress(std::slice::from_ref(view)).entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

// ---- Which hosts get a card ----------------------------------------------------------------------------------------

/// Why this matters: the card stands for a run going on (the old app published a card source only for a running run),
/// and an owner that offers it for anything else would show an empty or stale card. Specified: a host with a running
/// setup gets a card; a host whose run failed or completed, and a host with no run, get none.
#[gpui::test]
fn only_a_running_run_has_a_card(cx: &mut TestAppContext) {
    let mut failed = run(3, ProvisioningOperation::Update, RunStatus::Failed);
    failed.steps = vec![step("Install the farhelm binary", StepStatus::Failed)];
    let completed = run(4, ProvisioningOperation::Add, RunStatus::Completed);
    let listed = [
        build_box(),
        host(3, "ssh", connected()),
        host(4, "ssh", connected()),
        host(5, "ssh", connected()),
    ];
    let (_fake, _harness, cx) = open(cx, &listed, &[setup(RunStatus::Running), failed, completed]);
    let offered: Vec<bool> = (2..=5)
        .map(|id| cx.update(|_, cx| progress_card(id, cx).is_some()))
        .collect();
    assert_eq!(offered, [true, false, false, false]);
}

// ---- What it shows ---------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md, "Topology": the card shows "the host's mark and name, operation and completed-step count
/// with elapsed time, every step with its status and explanation, the current step highlighted, and any degraded or
/// failed step's reason"; the sidebar is too narrow for any of it, so the card is the one place it shows on hover.
/// Specified: the title is the host's short name (its alias); the count reads "Setting up: 2 of 4 steps done", a
/// warned step counting as done; the time reads "0:00 elapsed"; the list "Setup steps" names every step with its result
/// word; the completed step shows its explanation and not its message, the warned one its reason, the running one its
/// explanation, the pending one nothing.
#[gpui::test]
fn the_card_shows_the_host_its_count_time_and_every_step(cx: &mut TestAppContext) {
    let (_fake, harness, cx) = open(cx, &[build_box()], &[setup(RunStatus::Running)]);
    assert!(show(&harness, 2, cx));
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-card-title-2").as_deref(),
        Some("build-box")
    );
    assert_eq!(
        label(&nodes, "host-card-count-2").as_deref(),
        Some("Setting up: 2 of 4 steps done")
    );
    assert_eq!(
        label(&nodes, "host-card-elapsed-2").as_deref(),
        Some("0:00 elapsed")
    );
    assert_eq!(
        label(&nodes, "host-card-steps-2").as_deref(),
        Some("Setup steps")
    );
    let steps: Vec<Option<String>> = (0..4)
        .map(|n| label(&nodes, &format!("host-card-step-2-{n}")))
        .collect();
    assert_eq!(
        steps,
        [
            Some("Create directories: Completed".to_owned()),
            Some("Upload the farhelm binary: Warning".to_owned()),
            Some("Install the farhelm binary: Running".to_owned()),
            Some("Connect to the supervisor: Pending".to_owned()),
        ]
    );
    let lines = |n: usize| -> Vec<String> {
        (0..3)
            .filter_map(|k| label(&nodes, &format!("host-card-step-2-{n}-line-{k}")))
            .collect()
    };
    assert_eq!(lines(0), ["Prepared once a day, or after Farhelm updates."]);
    assert_eq!(lines(1), ["The upload was retried once"]);
    assert_eq!(lines(2), ["Replaces the binary in ~/.local/bin."]);
    assert_eq!(lines(3), Vec::<String>::new());
}

/// Why this matters: the connection detail belongs to the host, not the run (the old card read it live from the host
/// while the run stayed the same), so a host that reconnects mid-run must lose the section and one that drops again
/// must show its new error, without the card being made again; host text in it stays escaped. Specified: while
/// `build-box` is connecting, the "Connection" section names the attempt and the last error, its override escaped;
/// once the host is pushed connected, the section is gone; pushed connecting again with another error, it is back with
/// that error; a run message pushed with the same run shows after it.
#[gpui::test]
fn the_connection_detail_follows_the_hosts_phase(cx: &mut TestAppContext) {
    let (mut fake, harness, cx) = open(cx, &[build_box()], &[setup(RunStatus::Running)]);
    assert!(show(&harness, 2, cx));
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-card-connection-2").as_deref(),
        Some(
            "Attempt 2: waiting for the next connection attempt…; the last one failed: ssh: connect to host \
             build-box port 22: Connection refused<U+202E>"
        )
    );
    assert!(
        nodes.iter().any(|node| node
            .texts
            .iter()
            .any(|line| line.text.as_ref() == "Connection")),
        "the section is labelled"
    );

    let mut back = build_box();
    back.state = connected();
    push_hosts(&mut fake, &[back], cx);
    assert_eq!(
        label(&frame(cx), "host-card-connection-2"),
        None,
        "connected: no section"
    );

    let mut again = build_box();
    again.state = connecting(3, Some("Host key verification failed"));
    push_hosts(&mut fake, &[again], cx);
    let mut with_message = setup(RunStatus::Running);
    with_message.message = Some("Waiting for the host to answer".into());
    push_run(&mut fake, &with_message, cx);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-card-connection-2").as_deref(),
        Some(
            "Attempt 3: waiting for the next connection attempt…; the last one failed: Host key verification failed"
        )
    );
    assert_eq!(
        label(&nodes, "host-card-run-message-2").as_deref(),
        Some("Waiting for the host to answer")
    );
}

// ---- Time --------------------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md has the Hosts row's status and the attention row's card be "the same card", and the old
/// spec checked that the two surfaces agree; a card counting from when it was made would read 0:00 on every hover while
/// the row reads minutes. And the count must move on by itself while the card shows, in an inactive window (often still
/// on screen) and under reduced motion too, where no pulse redraws the card: the ticker's one-second clock, which the
/// card holds while it shows a run, is then the only thing that redraws it, so the last read is of what the window last
/// drew, not of a frame the test draws. Specified: a run first pushed 30 seconds before its card is made reads "0:30
/// elapsed" on the card's first frame; in an inactive window under reduced motion, one more second on the executor's
/// clock redraws the card by itself, and what the window last drew reads "0:31 elapsed".
#[gpui::test]
fn the_card_counts_from_the_runs_first_sight_and_by_itself(cx: &mut TestAppContext) {
    let (_fake, harness, cx) = open(cx, &[build_box()], &[setup(RunStatus::Running)]);
    test_support::advance(cx, Duration::from_secs(30));
    assert!(show(&harness, 2, cx));
    assert_eq!(
        label(&frame(cx), "host-card-elapsed-2").as_deref(),
        Some("0:30 elapsed")
    );
    cx.update(|_, cx| crate::ticker::set_reduced_motion(true, cx));
    cx.deactivate_window();
    assert!(
        !cx.update(|window, cx| crate::ticker::is_active(window, cx)),
        "the premise: the window counts as inactive"
    );
    // Settle what the deactivation itself redraws, so the count below is the clock's alone.
    test_support::settle(cx);
    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(1));
    assert!(
        cx.update(|_, cx| render_count::count("host-progress-card", cx)) > 0,
        "the clock redrew the card by itself"
    );
    let last = cx.update(|window, _| window.driver_nodes().to_vec());
    assert_eq!(
        label(&last, "host-card-elapsed-2").as_deref(),
        Some("0:31 elapsed")
    );
}

/// Why this matters: the ticker's rules: a view holds the one-second clock only while it shows a count, and the pulse
/// only while it draws a pulsing mark in an active window, or the ticker's loops keep waking for nothing. A card whose
/// run ended under it must let go of both. Specified: in an active window, a card showing a run with a running step
/// holds both; once the run is pushed completed, the card draws nothing and holds neither.
#[gpui::test]
fn the_card_lets_go_of_the_ticker_once_its_run_ends(cx: &mut TestAppContext) {
    let (mut fake, harness, cx) = open(cx, &[build_box()], &[setup(RunStatus::Running)]);
    assert!(show(&harness, 2, cx));
    let held = |cx: &mut VisualTestContext| {
        let card = card(&harness, cx);
        cx.update(|_, cx| {
            let card = card.read(cx);
            (card.clock.is_some(), card.pulse.is_some())
        })
    };
    assert_eq!(
        held(cx),
        (true, true),
        "a running run holds the clock and the pulse"
    );

    let mut done = setup(RunStatus::Completed);
    for step in &mut done.steps {
        step.status = StepStatus::Completed;
    }
    push_run(&mut fake, &done, cx);
    let nodes = frame(cx);
    assert!(
        node(&nodes, "host-card-2").is_none(),
        "the card draws nothing"
    );
    assert_eq!(held(cx), (false, false), "and holds nothing");
}

// ---- The fit as drawn ------------------------------------------------------------------------------------------------

/// A long diagnosis: many times any short window's height at the card's width.
fn long() -> String {
    "SSH failed while reconnecting. ".repeat(18) + "Final diagnosis: permission denied."
}

/// Host 2's running setup with `steps` steps (the second running when there are three) and `message` as its message.
fn diagnosed(steps: usize, message: &str) -> ProvisioningView {
    let mut view = setup(RunStatus::Running);
    view.steps = match steps {
        0 => Vec::new(),
        _ => vec![
            step("Create directories", StepStatus::Completed),
            step("Install the farhelm binary", StepStatus::Running),
            step("Connect to the supervisor", StepStatus::Pending),
        ],
    };
    view.message = Some(message.to_owned());
    view
}

/// A frame of the card once its fit has caught up: the card fits from the heights it was last drawn at, so the frame
/// after a change (a resize, a pushed message) is drawn from the old heights and the one after it from the new.
fn fitted(cx: &mut VisualTestContext) -> Vec<DriverNode> {
    test_support::redraw(cx);
    frame(cx)
}

/// Whether the node under `id` is drawn whole: nothing of it clipped away, inside the window.
fn whole(nodes: &[DriverNode], id: &str, window_height: f32) -> bool {
    node(nodes, id).is_some_and(|node| {
        node.visible_bounds == node.bounds && f32::from(node.bounds.bottom()) <= window_height
    })
}

/// The preview's cases at `steps` steps (three, the second running, or none yet): at 300 pixels high, with a long
/// connection error and a long run message, the card says "Full details in Hosts", the notice and the current step
/// (when there is one) are whole, and the run message is clipped at its start only, so its end shows; at 1200 pixels
/// high there is no notice and the message is whole; back at 300 pixels with a short error and message there is no
/// notice either.
fn previews_only_what_cannot_fit(steps: usize, cx: &mut TestAppContext) {
    let mut sick = build_box();
    sick.state = connecting(2, Some(&long()));
    let (mut fake, harness, cx) = open(cx, &[sick], &[diagnosed(steps, &long())]);
    cx.simulate_resize(size(px(720.), px(300.)));
    assert!(show(&harness, 2, cx));
    let nodes = fitted(cx);
    assert_eq!(
        label(&nodes, "host-card-notice-2").as_deref(),
        Some(FULL_DETAILS)
    );
    assert!(
        whole(&nodes, "host-card-notice-2", 300.),
        "the notice is readable"
    );
    if steps > 0 {
        assert!(
            whole(&nodes, "host-card-step-2-1", 300.),
            "the current step is readable"
        );
    }
    let message = node(&nodes, "host-card-run-message-2").expect("the message is drawn");
    assert!(
        message.visible_bounds.size.height < message.bounds.size.height,
        "the message is a preview"
    );
    assert_eq!(
        message.visible_bounds.bottom(),
        message.bounds.bottom(),
        "the preview shows the message's end"
    );

    cx.simulate_resize(size(px(720.), px(1200.)));
    let nodes = fitted(cx);
    assert_eq!(label(&nodes, "host-card-notice-2"), None, "tall: no notice");
    assert!(
        whole(&nodes, "host-card-run-message-2", 1200.),
        "tall: the message is whole"
    );

    cx.simulate_resize(size(px(720.), px(300.)));
    let mut short = build_box();
    short.state = connecting(2, Some("Short connection failure."));
    push_hosts(&mut fake, &[short], cx);
    push_run(&mut fake, &diagnosed(steps, "Short run failure."), cx);
    let nodes = fitted(cx);
    assert_eq!(
        label(&nodes, "host-card-notice-2"),
        None,
        "short texts: no notice"
    );
    assert!(
        whole(&nodes, "host-card-run-message-2", 300.),
        "short texts: the message is whole"
    );
}

/// Why this matters: SPEC.md: "If connection detail or a run message cannot fit beside a readable current step, only
/// the oversized text becomes a preview of whole lines from its end and the card says `Full details in Hosts`.
/// Messages that fit stay whole." The old spec ran it in a 300-pixel window and again in a tall one. Specified: the
/// cases of [`previews_only_what_cannot_fit`], with three steps.
#[gpui::test]
fn a_short_window_previews_only_diagnostics_that_cannot_fit(cx: &mut TestAppContext) {
    previews_only_what_cannot_fit(3, cx);
}

/// Why this matters: a run is pushed before its steps arrive, and its diagnosis still needs the same preview (the old
/// spec's no-steps variant); with no current step the list keeps only its padding. Specified: the cases of
/// [`previews_only_what_cannot_fit`], with no steps.
#[gpui::test]
fn a_run_without_steps_yet_previews_its_diagnostics_too(cx: &mut TestAppContext) {
    previews_only_what_cannot_fit(0, cx);
}

/// Host 2's running setup of 24 steps, too many for a card in a window of the app's least size: step `current` running,
/// those before it completed and those after it pending.
fn long_run(current: usize) -> ProvisioningView {
    let mut view = setup(RunStatus::Running);
    view.steps = (0..24)
        .map(|n| {
            let status = match n.cmp(&current) {
                std::cmp::Ordering::Less => StepStatus::Completed,
                std::cmp::Ordering::Equal => StepStatus::Running,
                std::cmp::Ordering::Greater => StepStatus::Pending,
            };
            step(&format!("Step {n}"), status)
        })
        .collect();
    view
}

/// Why this matters: SPEC.md, "Topology": "In a short window the step list shrinks, keeping the current step in view
/// as it changes", so the card must keep each new current step in view as the helm pushes the run, not only the one it
/// first drew. The old card followed its marked step list again after every update (`tooltip.spec.ts`, "a card
/// follows only its marked step list after every clone", at reference commit `24bc0580`, which moved the current step
/// from late in the list to early; its unmarked list, left where it was, was the old tooltip's own markup, with nothing
/// here to port). Specified: in a window of the app's least size, a setup of 24 steps whose list does not fit draws its
/// running step 18 whole; pushed with step 4 running instead, it draws step 4 whole; each time some step is not drawn
/// whole, so the list did shrink.
#[gpui::test]
fn the_current_step_stays_whole_as_the_run_moves_it(cx: &mut TestAppContext) {
    let height = f32::from(crate::bootstrap::MIN_WINDOW_SIZE.height);
    let (mut fake, harness, cx) = open(cx, &[build_box()], &[long_run(18)]);
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    assert!(show(&harness, 2, cx), "the premise: the card shows");
    for current in [18, 4] {
        push_run(&mut fake, &long_run(current), cx);
        let nodes = fitted(cx);
        let drawn_whole = |n: usize| whole(&nodes, &format!("host-card-step-2-{n}"), height);
        assert!(
            (0..24).any(|n| !drawn_whole(n)),
            "step {current}: the premise: the list shrank"
        );
        assert!(
            drawn_whole(current),
            "step {current}, the current one, is drawn whole"
        );
    }
}

// ---- The fit's arithmetic --------------------------------------------------------------------------------------------

/// Why this matters: the old preview's rule, which SPEC.md keeps ("Messages that fit stay whole"): a short message is
/// never cut to spare a long one, and the long ones share what is left. Specified: a 2-line text beside a long one
/// keeps its whole height and the long one takes the rest; two texts both over half share equally; three texts where
/// only the shortest fits a third keep it whole and split the rest; no room gives nothing.
#[test]
fn the_shorter_text_stays_whole_and_the_longer_ones_share_the_rest() {
    assert_eq!(shares(&[36., 400.], 200.), [36., 164.]);
    assert_eq!(shares(&[300., 400.], 200.), [100., 100.]);
    assert_eq!(shares(&[400., 18., 300.], 216.), [99., 18., 99.]);
    assert_eq!(shares(&[36., 400.], -10.), [0., 0.]);
}

/// Why this matters: the old card had to stop a two-line diagnosis that fitted exactly from losing a notice's height of
/// text to rounding ("a one-step host card keeps a fitting multiline diagnosis whole"); and a preview must keep whole
/// lines only. Specified: texts that fit their room exactly, or with less than half a pixel over, stay whole with no
/// notice; nothing to fit needs no notice even with no room; texts that do not fit give the notice its room first, keep
/// the one that fits its share whole, and cut the other down to the whole 18-pixel lines of its share, which can be
/// none.
#[test]
fn a_fitting_text_stays_whole_and_a_preview_keeps_whole_lines() {
    let whole = |n| Fit {
        caps: vec![None; n],
        notice: false,
    };
    assert_eq!(fit(&[36.], 36., 28., 18.), whole(1));
    assert_eq!(fit(&[36., 18.], 53.6, 28., 18.), whole(2));
    assert_eq!(fit(&[], -50., 28., 18.), whole(0));
    assert_eq!(
        fit(&[36., 400.], 230., 28., 18.),
        Fit {
            caps: vec![None, Some(162.)],
            notice: true
        }
    );
    assert_eq!(
        fit(&[100.], 20., 28., 18.),
        Fit {
            caps: vec![Some(0.)],
            notice: true
        }
    );
}

// ---- The card as the Hosts row's hover ------------------------------------------------------------------------------

/// A root view drawing a Hosts panel in a sidebar column (the sidebar's width less the rail's), with the edge that help
/// placed beside the sidebar sits against, as the sidebar draws them.
struct PanelHarness {
    panel: Entity<HostsPanel>,
    root: root::Attachment,
}

impl Render for PanelHarness {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let column = div()
            .relative()
            .w(crate::theme::metrics::SIDEBAR_WIDTH - px(56.))
            .h_full()
            .child(self.panel.clone())
            .child(layers::beside_edge(window, cx));
        root::wrap(column, &self.root, window, cx)
    }
}

/// The showing tooltip, if any.
fn tooltip(cx: &mut VisualTestContext) -> Option<layers::ShownTooltip> {
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
}

/// Why this matters: SPEC.md, "Topology": "Hovering that status opens the same card as a working host in the attention
/// list", placed "beside the sidebar" with "the shared hover delay"; the sidebar is too narrow for the run, so the card
/// is the one place its steps and diagnosis show on hover. The card's view holds the ticker's clock and pulse, so it
/// must go with the hover, and moving within the status must keep the card rather than make it again.
///
/// Specified: over the Hosts panel with `build-box` setting up, the running status draws no card while the pointer is
/// away; after the hover delay on the status, a tooltip reporting "Progress of the run on build-box" shows beside the sidebar with the card drawn on it (its count and its
/// connection detail); moving within the status keeps that very tooltip, card view included; moving off the status
/// takes the tooltip and the card away.
#[gpui::test]
fn hovering_the_running_status_shows_the_card_beside_the_sidebar(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![
        hosts(&[build_box()]),
        progress(&[setup(RunStatus::Running)]),
    ]);
    cx.run_until_parked();
    let (_harness, cx) = test_support::open_view(cx, |window, cx| PanelHarness {
        panel: cx.new(HostsPanel::new),
        root: root::attach(window, cx),
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    let nodes = frame(cx);
    assert!(
        node(&nodes, "host-status-2").is_some(),
        "the premise: the row shows its running status"
    );
    assert!(
        node(&nodes, "host-card-2").is_none(),
        "no card before the hover"
    );

    let status = test_support::bounds(cx, "host-status-2");
    cx.simulate_mouse_move(status.center(), None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    let nodes = frame(cx);
    let shown = tooltip(cx).expect("the status's help shows");
    assert_eq!(shown.text.as_ref(), "Progress of the run on build-box");
    assert_eq!(shown.side(), TooltipSide::Beside, "beside the sidebar");
    assert_eq!(
        label(&nodes, "host-card-count-2").as_deref(),
        Some("Setting up: 2 of 4 steps done"),
        "the card is drawn on the tooltip"
    );
    assert!(label(&nodes, "host-card-connection-2").is_some());

    let within = point(status.origin.x + px(4.), status.center().y);
    cx.simulate_mouse_move(within, None, Modifiers::none());
    test_support::redraw(cx);
    let still = tooltip(cx).expect("moving within the status keeps the card");
    assert_eq!(still.owner, shown.owner, "the same control's help");

    cx.simulate_mouse_move(point(px(1150.), px(780.)), None, Modifiers::none());
    test_support::settle(cx);
    let nodes = frame(cx);
    assert!(
        tooltip(cx).is_none(),
        "leaving the status takes the help away"
    );
    assert!(
        node(&nodes, "host-card-2").is_none(),
        "and the card with it"
    );
}
