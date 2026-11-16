//! Tests of the Hosts attention list: which hosts get a row and in what order, each row's words and action, the
//! header's actions and what they send, Always update's failure line, dismissal, Details opening Hosts at the host,
//! the menus below the list, and the list's bounded scroll.
//!
//! The classification and its words are pure functions and tested as such, over hosts built with the Hosts panel's
//! fixtures and the [`HostStatus`] the rail reads. The rest runs the real main view over the helm client's fake, as
//! the list's other tests do, and reads the frame from GPUI's driver record by element id: the list is
//! `host-attention-list`, a row `host-attention-row-<id>`, its open control `host-attention-open-<id>`, its buttons
//! `host-attention-details-<id>` and `host-attention-update-<id>`, its bar `host-attention-progress-<id>`. When a host
//! has an update, is busy or is warned is `hosts::status`'s rule and tested there; these tests check that the list
//! draws and acts on what that module reads.

use std::collections::HashSet;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::provisioning::ProvisioningOperation::Add;
use farhelm_proto::provisioning::{ProvisioningView, RunStatus, StepStatus};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, HostStateView, HostView, Preferences,
};
use gpui::{Modifiers, TestAppContext, VisualTestContext, px, size};
use serde_json::{Value, json};

use super::{Dismissal, Kind, REQUIRED, Shown, classify, problem_words};
use crate::host_text::HostText;
use crate::hosts::progress::tests::step;
use crate::hosts::status::{Activity, HostStatus, Live, Steps, Urgency};
use crate::hosts::tests::{aliased, connected, host, hosts, progress, run, skew};
use crate::session_list::host_rail::tests::{
    answer_everything, hover_help, label, node, old_build, pressed, texts_within, unreachable,
};
use crate::session_list::marking::{MENU_ID, MENU_TOGGLE_ID};
use crate::session_list::tests::{Fixture, listing, open_main, preferences_state, session};
use crate::test_support::{self, any_open, focused_id, frame, meta};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// A remote host `id` aliased `alias` at `ops@<alias>.example.invalid`, in `state`: its identity is "<alias>
/// (ops@<alias>.example.invalid)" and its short name the alias.
fn remote(id: i64, alias: &str, state: HostStateView) -> HostView {
    aliased(
        host(id, "ssh", state),
        alias,
        &format!("ops@{alias}.example.invalid"),
    )
}

/// What the rail reads about a host with nothing going on: no update, no work, no warning.
fn idle() -> HostStatus {
    HostStatus {
        update: None,
        live: None,
        warned: false,
    }
}

/// [`idle`] with an update of `urgency`.
fn behind(urgency: Urgency) -> HostStatus {
    HostStatus {
        update: Some(urgency),
        ..idle()
    }
}

/// A host's setup run with three steps, the first `done` of them completed and the next one running.
fn setup_run(id: i64, done: usize) -> ProvisioningView {
    let mut view = run(id, Add, RunStatus::Running);
    view.steps = ["Create directories", "Upload the farhelm binary", "Connect"]
        .into_iter()
        .enumerate()
        .map(|(n, name)| {
            let status = match n.cmp(&done) {
                std::cmp::Ordering::Less => StepStatus::Completed,
                std::cmp::Ordering::Equal => StepStatus::Running,
                std::cmp::Ordering::Greater => StepStatus::Pending,
            };
            step(name, status)
        })
        .collect();
    view
}

/// The main view over the fake helm with no Welcome card to open over it, `listed` as the registry, `runs` retained,
/// the sessions `rows` (the first one open) and "Update remote hosts automatically" `auto`, in a window of the
/// reference screens' narrow size. Every command sent at startup is answered, so each test sees only its own steps'.
fn open<'a>(
    cx: &'a mut TestAppContext,
    listed: &[HostView],
    runs: &[ProvisioningView],
    rows: &[Fixture],
    auto: bool,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    let (mut fake, cx) = open_main(
        cx,
        vec![
            listing(&order, rows),
            preferences_state(Preferences {
                welcome_seen: Some(true),
                auto_update_remote_hosts: Some(auto),
                visit_history: order.iter().take(1).map(|key| (*key).to_owned()).collect(),
                ..Preferences::default()
            }),
            hosts(listed),
            progress(runs),
        ],
    );
    cx.simulate_resize(size(px(900.), px(700.)));
    answer_everything(&mut fake);
    test_support::settle(cx);
    (fake, cx)
}

/// Push `listed` as the whole registry, replacing every host the snapshot named, and settle.
fn push_hosts(fake: &mut FakeHelm, listed: &[HostView], gone: &[i64], cx: &mut VisualTestContext) {
    let state = hosts(listed);
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: state.meta,
        upsert: state.entries,
        remove: gone.iter().map(|id| HostView::key(*id)).collect(),
    });
    test_support::settle(cx);
}

/// Push `view` as its host's retained run, and settle.
fn push_run(fake: &mut FakeHelm, view: ProvisioningView, cx: &mut VisualTestContext) {
    let key = HostView::key(view.host_id);
    fake.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&[key.as_str()]),
        upsert: progress(&[view]).entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// Push "Update remote hosts automatically" as `on`, and settle.
fn push_auto(fake: &mut FakeHelm, on: bool, cx: &mut VisualTestContext) {
    let state = preferences_state(Preferences {
        welcome_seen: Some(true),
        auto_update_remote_hosts: Some(on),
        ..Preferences::default()
    });
    fake.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    test_support::settle(cx);
}

/// The hosts whose rows the frame drew, top to bottom, by id.
fn row_ids(cx: &mut VisualTestContext) -> Vec<i64> {
    let mut rows: Vec<(f32, i64)> = frame(cx)
        .iter()
        .filter_map(|node| {
            let id = node
                .element_id
                .as_deref()?
                .strip_prefix("host-attention-open-")?;
            Some((f32::from(node.bounds.origin.y), id.parse().ok()?))
        })
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, id)| id).collect()
}

/// Whether the last frame drew something under the debug selector or component id `selector`.
fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// Where the element drawn under `menu` sits relative to the one drawn under `opener`: what stays the same when a menu
/// reopens beside its moved opener.
fn offset(
    cx: &mut VisualTestContext,
    menu: &'static str,
    opener: &'static str,
) -> gpui::Point<gpui::Pixels> {
    test_support::bounds(cx, menu).origin - test_support::bounds(cx, opener).origin
}

/// Whether two offsets agree to within the pixel a menu's placement rounds to.
fn same_place(a: gpui::Point<gpui::Pixels>, b: gpui::Point<gpui::Pixels>) -> bool {
    f32::from(a.x - b.x).abs() <= 1. && f32::from(a.y - b.y).abs() <= 1.
}

/// The commands sent since the last call, as (method, path, body), leaving out the checkout trash reads the main view
/// sends for each host that connects (a pushed host does). They stay unanswered, so the work they carry stays in
/// flight.
fn sent(fake: &mut FakeHelm) -> Vec<(Method, String, Value)> {
    fake.commands()
        .into_iter()
        .filter(|command| command.path != "/api/checkout-trash/list")
        .map(|command| {
            (
                command.method,
                command.path,
                command.body.unwrap_or_default(),
            )
        })
        .collect()
}

/// An Update's plan request for host `id`, as `sent` reads it.
fn plan_request(id: i64) -> (Method, String, Value) {
    (Method::Post, format!("/api/hosts/{id}/update"), Value::Null)
}

/// The preference write that turns automatic updates on, as `sent` reads it.
fn auto_on_write() -> (Method, String, Value) {
    (
        Method::Put,
        "/api/preferences".to_owned(),
        json!({ "auto_update_remote_hosts": true }),
    )
}

// ---- Classification, as pure functions -------------------------------------------------------------------------------

/// Why this matters: a host has one reason to be in the list, and SPEC.md fixes which wins ("Live setup, update or
/// uninstall progress takes precedence over a problem, which takes precedence over an available update") and the order
/// ("Problems appear first, then working hosts, then updates, in rail order within each group"), so a host whose Update
/// loses its connection stays one working row instead of jumping groups. Specified: over a registry listing an optional
/// update, a working host that is also unreachable and warned, a problem host with an update badge, a host with
/// nothing to say, a required update and a second problem, the rows are the two problems, then the working host, then
/// the two updates, each group in registry order; the working row is "Setting up" with its steps, the problem with an
/// update is a problem, and the idle host has no row.
#[test]
fn rows_group_problems_then_work_then_updates_in_registry_order() {
    let steps = Steps {
        done: 1,
        total: 3,
        current: Some(HostText::from_host("Upload")),
    };
    let listed = [
        (remote(2, "older", old_build()), behind(Urgency::Optional)),
        (
            remote(3, "busy", unreachable()),
            HostStatus {
                live: Some(Live {
                    activity: Activity::Setup,
                    steps: Some(steps.clone()),
                }),
                warned: true,
                ..idle()
            },
        ),
        (
            remote(4, "warned", old_build()),
            HostStatus {
                warned: true,
                ..behind(Urgency::Optional)
            },
        ),
        (remote(5, "fine", connected()), idle()),
        (remote(6, "skewed", skew(28, 29)), behind(Urgency::Required)),
        (remote(7, "down", unreachable()), idle()),
    ];
    let rows = classify(listed.iter().map(|(host, status)| (host, status)), false);
    let shape: Vec<(i64, Kind, &str)> = rows
        .iter()
        .map(|row| (row.host.id, row.kind, row.words.as_str()))
        .collect();
    assert_eq!(
        shape,
        [
            (4, Kind::Problem, "Setup or update needs attention"),
            (7, Kind::Problem, "Unreachable, retrying"),
            (3, Kind::Working, "Setting up"),
            (2, Kind::Update { required: false }, "Update available"),
            (6, Kind::Update { required: true }, "Update required"),
        ]
    );
    assert_eq!(rows[2].steps, Some(steps));
}

/// Why this matters: a problem row's words are all the list says about what is wrong, and SPEC.md names the problems
/// and the hosts that need no row ("Connecting, a connected current host, a usable connected host running a newer
/// build"); a lower protocol whose Update is on offer is an update row, not a problem, while one without an Update is a
/// problem, and a higher protocol is always one. Specified: the words for each phase that waits for someone, both skews,
/// the warning alone and after a phase's words, and no words for connecting, connected, newer and an offered required
/// update.
#[test]
fn each_problem_has_its_words_and_healthy_hosts_have_none() {
    let words = |state: HostStateView, status: HostStatus| {
        problem_words(&remote(2, "host", state), &status)
    };
    let cases = [
        (unreachable(), "Unreachable, retrying"),
        (
            HostStateView::IdentityMismatch {
                recorded: "a".into(),
                reported: "b".into(),
            },
            "Identity mismatch",
        ),
        (
            HostStateView::IdentityUnverified {
                recorded: "a".into(),
            },
            "Identity unverified",
        ),
        (
            HostStateView::Duplicate {
                twin: 3,
                identity: "a".into(),
            },
            "Duplicate",
        ),
        (
            HostStateView::Retired {
                reason: "gone".into(),
            },
            "Retired",
        ),
        (skew(30, 29), "Too new; update the helm"),
        (skew(28, 29), "Needs update"),
    ];
    for (state, expected) in cases {
        assert_eq!(words(state, idle()).as_deref(), Some(expected));
    }
    assert_eq!(words(skew(28, 29), behind(Urgency::Required)), None);
    let warned = HostStatus {
        warned: true,
        ..idle()
    };
    assert_eq!(
        words(connected(), warned.clone()).as_deref(),
        Some("Setup or update needs attention")
    );
    assert_eq!(
        words(unreachable(), warned).as_deref(),
        Some("Unreachable, retrying; Setup or update needs attention")
    );
    let newer = HostStateView::Connected {
        identity: None,
        build_version: "9.0.0".into(),
        old_version: false,
        newer_version: true,
        refresh: farhelm_proto::ui_channel::RefreshView::Ok { sessions: 0 },
    };
    for healthy in [
        crate::hosts::tests::connecting(1, None),
        connected(),
        newer,
        old_build(),
    ] {
        assert_eq!(words(healthy, idle()), None);
    }
}

/// Why this matters: SPEC.md: "A close control dismisses only the optional Update rows; required updates, problems and
/// working hosts cannot be dismissed", and the reminders stay hidden "until a host outside the dismissed update set
/// gains an update". Specified: dismissed, an optional update has no row while a required one, a problem with an update
/// and a working host keep theirs; a dismissal of hosts 2 and 3 ends when host 4 gains an update but not when the set
/// shrinks or host 3's update returns.
#[test]
fn dismissal_hides_only_optional_updates_until_a_new_host_has_one() {
    let listed = [
        (remote(2, "older", old_build()), behind(Urgency::Optional)),
        (remote(3, "skewed", skew(28, 29)), behind(Urgency::Required)),
        (
            remote(4, "warned", old_build()),
            HostStatus {
                warned: true,
                ..behind(Urgency::Optional)
            },
        ),
    ];
    let rows = classify(listed.iter().map(|(host, status)| (host, status)), true);
    let ids: Vec<i64> = rows.iter().map(|row| row.host.id).collect();
    assert_eq!(ids, [4, 3]);

    let dismissal = Dismissal::new(HashSet::from([2, 3]));
    assert!(!dismissal.ended_by(&HashSet::from([2, 3])));
    assert!(!dismissal.ended_by(&HashSet::from([3])));
    assert!(!dismissal.ended_by(&HashSet::new()));
    assert!(dismissal.ended_by(&HashSet::from([3, 4])));
}

/// Why this matters: the menus below the list close when the list moves them (SPEC.md: changes in its rows, and
/// "Button and error visibility changes retire session menus anchored below the attention list ... ordinary progress
/// ticks still do not"), so what counts as a change decides whether a progress tick closes a menu the user is in.
/// Specified: a working row's steps, step count and current step leave the key unchanged; its verb, a row's kind or
/// words, Always update showing and the failure line showing each change it; buttons waiting for a launch do not.
#[test]
fn the_layout_key_follows_copy_and_buttons_but_not_progress() {
    let listed = [
        (
            remote(2, "busy", connected()),
            HostStatus {
                live: Some(Live {
                    activity: Activity::Update,
                    steps: Some(Steps {
                        done: 0,
                        total: 3,
                        current: Some(HostText::from_host("Create directories")),
                    }),
                }),
                ..idle()
            },
        ),
        (remote(3, "older", old_build()), behind(Urgency::Optional)),
    ];
    let shown = Shown {
        rows: classify(listed.iter().map(|(host, status)| (host, status)), false),
        auto_updates_on: false,
        failed: false,
        launch_pending: false,
    };
    let key = shown.layout_key();

    let mut ticked = shown.clone();
    ticked.rows[0].steps = Some(Steps {
        done: 2,
        total: 3,
        current: Some(HostText::from_host("Connect")),
    });
    assert_eq!(ticked.layout_key(), key, "a progress tick moves nothing");

    let mut changed = shown.clone();
    changed.rows[0].words = "Uninstalling".into();
    assert_ne!(changed.layout_key(), key, "the verb is copy");
    let mut required = shown.clone();
    required.rows[1].kind = Kind::Update { required: true };
    assert_ne!(required.layout_key(), key, "a required update adds a line");
    let waiting = Shown {
        launch_pending: true,
        ..shown.clone()
    };
    assert_eq!(
        waiting.layout_key(),
        key,
        "waiting buttons keep their place"
    );
    let on = Shown {
        auto_updates_on: true,
        ..shown.clone()
    };
    assert_ne!(on.layout_key(), key, "Always update goes");
    let failed = Shown {
        failed: true,
        ..shown
    };
    assert_ne!(failed.layout_key(), key, "the failure line shows");
}

// ---- The list in the window -------------------------------------------------------------------------------------------

/// Why this matters: the list is how a host's trouble reaches a user who is looking at sessions (SPEC.md: "When hosts
/// need attention, a compact host list sits ... above the session list's title"), and each row says what is wrong and
/// offers what fixes it; the old case "attention groups problems, work and updates". Specified: over the local host, a
/// current remote, a host on a newer build, an optional update, an unreachable host, a setup going on and a required
/// update, the list sits above the order title, says "4 hosts need attention", and draws the unreachable host, the
/// setup, then the two updates, each named "<identity>: <state>...; open Hosts for details"; the problem row offers
/// Details only, the working row no button and a bar, the update rows Update, and the required one says sessions stay
/// unreachable.
#[gpui::test]
fn the_list_draws_one_row_per_host_with_its_words_and_action(cx: &mut TestAppContext) {
    let listed = [
        host(1, "local", connected()),
        remote(2, "fine", connected()),
        remote(3, "older", old_build()),
        remote(4, "down", unreachable()),
        remote(5, "busy", connected()),
        remote(6, "skewed", skew(28, 29)),
        remote(
            7,
            "newer",
            HostStateView::Connected {
                identity: None,
                build_version: "9.0.0".into(),
                old_version: false,
                newer_version: true,
                refresh: farhelm_proto::ui_channel::RefreshView::Ok { sessions: 0 },
            },
        ),
    ];
    let (_fake, cx) = open(cx, &listed, &[setup_run(5, 1)], &[session("a")], false);
    assert_eq!(row_ids(cx), [4, 5, 3, 6]);
    let nodes = frame(cx);
    assert_eq!(
        label(&nodes, "host-attention-heading"),
        "4 hosts need attention"
    );
    assert!(
        test_support::bounds(cx, "host-attention-list").bottom()
            <= test_support::bounds(cx, crate::session_list::order::TITLE_ID).top(),
        "the list is above the heading"
    );
    let name = |alias: &str, state: &str| {
        format!("{alias} (ops@{alias}.example.invalid): {state}; open Hosts for details")
    };
    assert_eq!(
        label(&nodes, "host-attention-open-4"),
        name("down", "Unreachable, retrying")
    );
    assert_eq!(
        label(&nodes, "host-attention-open-5"),
        name("busy", "Setting up · Upload the farhelm binary")
    );
    assert_eq!(
        label(&nodes, "host-attention-open-3"),
        name("older", "Update available")
    );
    assert_eq!(
        label(&nodes, "host-attention-open-6"),
        name("skewed", "Update required")
    );
    let present = |id: &str| node(&nodes, id).is_some();
    assert!(present("host-attention-details-4"));
    assert!(!present("host-attention-update-4"));
    assert!(present("host-attention-progress-5"));
    assert!(!present("host-attention-details-5") && !present("host-attention-update-5"));
    assert!(present("host-attention-update-3") && present("host-attention-update-6"));
    // The words may wrap onto a second line, which the record keeps as a line of its own.
    let required = |id: &str| {
        texts_within(&nodes, id).iter().any(|text| {
            REQUIRED.starts_with(text.text.as_ref()) && text.text.starts_with("Sessions stay")
        })
    };
    assert!(required("host-attention-row-6"));
    assert!(!required("host-attention-row-3"));
}

/// Why this matters: SPEC.md: the list "is absent when nothing needs attention and while Hosts is open", and a failed
/// Always update save "does not keep an otherwise empty attention list visible". Specified: over current hosts, after
/// a failed preference save, no list is drawn; once a host is unreachable the list shows; opening Hosts hides it and
/// closing Hosts shows it again.
#[gpui::test]
fn the_list_is_absent_when_nothing_needs_attention_and_while_hosts_shows(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, &[remote(2, "fine", connected())], &[], &[], false);
    cx.update(|_, cx| crate::hosts::status::retry_auto_update(cx));
    test_support::settle(cx);
    let write = fake.commands();
    assert_eq!(write.len(), 1, "the premise: one preference write");
    fake.reply(write[0].id, 500, Some(json!("disk full")));
    test_support::settle(cx);
    assert!(
        cx.update(|_, cx| crate::hosts::status::auto_update_failed(cx)),
        "the premise: the save failed"
    );
    assert!(!drawn(cx, "host-attention-list"));

    push_hosts(&mut fake, &[remote(2, "fine", unreachable())], &[], cx);
    assert!(drawn(cx, "host-attention-list"));
    test_support::click(cx, "hosts-panel-toggle");
    assert!(drawn(cx, "hosts-panel"), "the premise: Hosts shows");
    assert!(!drawn(cx, "host-attention-list"));
    test_support::click(cx, "hosts-panel-toggle");
    assert!(drawn(cx, "host-attention-list"));
}

/// Why this matters: a working row stands for a run the sidebar is too narrow to show (SPEC.md: "followed by the current
/// step's name when known", and the bar "filled as steps complete"), so its step and bar must follow the pushed run.
/// Specified: a setup three steps long, one done, names its running step and fills a third of its bar; pushed on to
/// two done, it names the next step and fills two thirds.
#[gpui::test]
fn a_working_row_names_its_step_and_fills_its_bar(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(
        cx,
        &[remote(5, "busy", connected())],
        &[setup_run(5, 1)],
        &[],
        false,
    );
    let filled = |cx: &mut VisualTestContext| {
        let bar = test_support::bounds(cx, "host-attention-progress-5");
        let fill = test_support::bounds(cx, "host-attention-progress-fill-5");
        f32::from(fill.size.width) / f32::from(bar.size.width)
    };
    assert!(
        (filled(cx) - 1. / 3.).abs() < 0.01,
        "a third: {}",
        filled(cx)
    );
    assert!(
        label(&frame(cx), "host-attention-open-5")
            .contains("Setting up · Upload the farhelm binary")
    );

    push_run(&mut fake, setup_run(5, 2), cx);
    assert!(
        (filled(cx) - 2. / 3.).abs() < 0.01,
        "two thirds: {}",
        filled(cx)
    );
    assert!(label(&frame(cx), "host-attention-open-5").contains("Setting up · Connect"));
}

/// Why this matters: a working row's hover is the host progress card (SPEC.md: "Hovering anywhere on a working row opens
/// the identity button's shared progress card ... Moving within that row keeps the card in place ... Before step data
/// arrives, the whole row shares the button's plain tooltip"), beside the sidebar. The tooltip's text is the same
/// sentence either way, so only the card's own element tells the two apart. Specified: while only the setup's request
/// is out, resting the pointer on the row's open control shows its plain help and no card; once the run is pushed,
/// resting it there again draws the card `host-card-5`, wholly right of the session list's column, and moving onto the
/// row's bar keeps it drawn.
#[gpui::test]
fn a_working_rows_hover_is_the_progress_card_once_its_run_shows(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(
        cx,
        &[remote(5, "busy", connected())],
        &[],
        &[session("a")],
        false,
    );
    crate::hosts::tests::start_work(
        &mut fake,
        cx,
        crate::operations::Work::Setup,
        crate::operations::OpKey::host(&HostView::key(5)),
    );
    assert_eq!(
        row_ids(cx),
        [5],
        "the premise: a working row while the request is out"
    );
    let card = |cx: &mut VisualTestContext| node(&frame(cx), "host-card-5").map(|node| node.bounds);
    assert_eq!(
        hover_help(cx, "host-attention-open-5").as_deref(),
        Some("busy (ops@busy.example.invalid): Setting up; open Hosts for details")
    );
    assert_eq!(card(cx), None, "no card before the run's steps");

    push_run(&mut fake, setup_run(5, 1), cx);
    // Away and back, so the hover begins again under the row's new help.
    let away = test_support::center(cx, crate::session_list::order::TITLE_ID);
    cx.simulate_mouse_move(away, None, Modifiers::none());
    test_support::advance(cx, crate::layers::HOVER_DELAY);
    hover_help(cx, "host-attention-open-5");
    let column = node(&frame(cx), "session-list").map(|node| node.bounds);
    let shown = card(cx).expect("the card shows");
    assert!(
        column.is_some_and(|column| shown.left() >= column.right()),
        "{shown:?} beside {column:?}"
    );
    hover_help(cx, "host-attention-progress-5");
    assert!(card(cx).is_some(), "moving onto the bar keeps the card");
}

/// Why this matters: the list's actions must reach exactly the hosts it shows (SPEC.md: Update all "sends the ordinary
/// authorized Update request for exactly those displayed Update hosts", and "only when more than one displayed row
/// offers Update"), never a problem or busy host; the old cases "one listed update uses its own Update action" and
/// "Update all includes required and optional rows but no other attention". Specified: with one update row and a
/// problem row there is no Update all, and the row's Update sends that host's plan request alone; once an optional and
/// a required update join while that host is still busy with its request, Update all sends one plan request each for
/// the two newcomers and nothing for the busy or the problem host.
#[gpui::test]
fn update_and_update_all_send_exactly_the_displayed_hosts(cx: &mut TestAppContext) {
    let mut listed = vec![
        remote(3, "older", old_build()),
        remote(4, "down", unreachable()),
    ];
    let (mut fake, cx) = open(cx, &listed, &[], &[], true);
    assert!(!drawn(cx, "host-attention-update-all"));
    test_support::click(cx, "host-attention-update-3");
    test_support::settle(cx);
    assert_eq!(sent(&mut fake), [plan_request(3)]);

    listed.push(remote(6, "skewed", skew(28, 29)));
    listed.push(remote(7, "other", old_build()));
    push_hosts(&mut fake, &listed, &[], cx);
    assert_eq!(row_ids(cx), [4, 3, 6, 7], "the premise: host 3 is working");
    test_support::click(cx, "host-attention-update-all");
    test_support::settle(cx);
    assert_eq!(sent(&mut fake), [plan_request(6), plan_request(7)]);
}

/// Why this matters: Always update both updates the hosts shown and turns the preference on (SPEC.md: "It requests the
/// same displayed hosts as Update all, then turns on that preference"), and is offered only while the preference is
/// off and a row offers Update; the old cases "Always update requests exactly N displayed hosts" and "is absent when
/// automatic updates are already enabled". Specified: with the preference off, one update row and a problem row,
/// Always update shows beside no Update all, and its press sends host 3's plan request and the preference write,
/// nothing for the problem host; with a new update row it shows again until the preference reads on, when it goes while
/// the row's Update stays.
#[gpui::test]
fn always_update_asks_for_the_displayed_hosts_and_the_preference(cx: &mut TestAppContext) {
    let mut listed = vec![
        remote(3, "older", old_build()),
        remote(4, "down", unreachable()),
    ];
    let (mut fake, cx) = open(cx, &listed, &[], &[], false);
    assert!(!drawn(cx, "host-attention-update-all"));
    test_support::click(cx, "host-attention-always-update");
    test_support::settle(cx);
    assert_eq!(sent(&mut fake), [plan_request(3), auto_on_write()]);

    listed.push(remote(8, "another", old_build()));
    push_hosts(&mut fake, &listed, &[], cx);
    assert!(
        drawn(cx, "host-attention-always-update"),
        "the premise: offered again"
    );
    push_auto(&mut fake, true, cx);
    assert!(!drawn(cx, "host-attention-always-update"));
    assert!(drawn(cx, "host-attention-update-8"));
}

/// Why this matters: an Update is refused while a session launch is in flight, and the Hosts panel's line that would
/// say so is not drawn beside this list, so the list's own update actions must say why they wait instead of looking
/// ready and doing nothing (the area owner's decision, in the words the launcher's triggers use for a pending launch).
/// Specified: with a launch's create out, Update all, Always update and a row's Update are still focusable, each is
/// named "<label> unavailable: wait for the pending launch to finish" (a busy button's state reaches assistive
/// technology only through its name) and has "Wait for the pending launch to finish" as its hover help, and pressing
/// each sends nothing; once the create is answered, the row's Update is named and helped by its ordinary words again.
#[gpui::test]
fn update_actions_wait_visibly_while_a_launch_is_in_flight(cx: &mut TestAppContext) {
    let listed = [
        remote(3, "older", old_build()),
        remote(6, "other", old_build()),
        remote(4, "down", unreachable()),
    ];
    let (mut fake, cx) = open(cx, &listed, &[], &[], false);
    let launch = crate::hosts::tests::start_work(
        &mut fake,
        cx,
        crate::operations::Work::Launch,
        crate::operations::OpKey::session("new"),
    );
    for (control, name) in [
        ("host-attention-update-all", "Update all"),
        ("host-attention-always-update", "Always update"),
        ("host-attention-update-3", "Update"),
    ] {
        let focusable = node(&frame(cx), control).is_some_and(|node| node.flags.focusable);
        assert!(focusable, "{control} stays keyboard-reachable");
        assert_eq!(
            label(&frame(cx), control),
            format!("{name} unavailable: wait for the pending launch to finish")
        );
        assert_eq!(
            hover_help(cx, control).as_deref(),
            Some(crate::session_list::menu::LAUNCH_PENDING),
            "{control}'s help"
        );
        test_support::click(cx, control);
        test_support::settle(cx);
    }
    assert_eq!(sent(&mut fake), [], "no press sent anything");

    fake.reply(launch, 200, None);
    test_support::settle(cx);
    assert_eq!(label(&frame(cx), "host-attention-update-3"), "Update");
    assert_ne!(
        hover_help(cx, "host-attention-update-3").as_deref(),
        Some(crate::session_list::menu::LAUNCH_PENDING)
    );
}

/// Why this matters: a failed opt-in must not vanish behind Hosts or a later redraw (SPEC.md: the error "survives opening
/// and closing Hosts while attention remains", "with **Try again** saving only the preference"), and its line appearing
/// moves the sessions under the list, so it closes a session menu opened while the save was out; the old cases
/// "Always update retains a hidden save failure and recovers through Try again" and "save failure retires a session
/// menu measured while the save was pending". Specified: with b's row menu opened after Always update's press, the
/// refused save closes the menu and shows "Could not turn on automatic updates. Later updates will still wait for
/// you." with Try again, through opening and closing Hosts; Try again sends the preference write alone, and once the
/// preference reads on the line is gone.
#[gpui::test]
fn a_failed_always_update_shows_its_line_until_try_again_saves(cx: &mut TestAppContext) {
    let listed = [
        remote(3, "older", old_build()),
        remote(4, "down", unreachable()),
    ];
    let (mut fake, cx) = open(cx, &listed, &[], &[session("a"), session("b")], false);
    test_support::click(cx, "host-attention-always-update");
    test_support::settle(cx);
    let commands = fake.commands();
    assert_eq!(
        commands.len(),
        2,
        "the premise: a plan request and the write"
    );
    let write = commands[1].id;
    test_support::click(cx, "session-menu-b");
    assert!(any_open(cx), "the premise: b's menu is open");

    // Host 3's Update goes on to its confirming request, so its row stays a working one and only the line moves b.
    fake.reply(commands[0].id, 200, Some(json!({ "probe_id": "u-3" })));
    fake.reply(write, 500, Some(json!("disk full")));
    test_support::settle(cx);
    assert!(
        !any_open(cx),
        "the line moved the menu's row, which closed it"
    );
    let failure = node(&frame(cx), "host-attention-save-error").map(|node| node.label.clone());
    assert_eq!(
        failure.flatten().as_deref(),
        Some(crate::hosts::status::AUTO_UPDATE_FAILED)
    );
    test_support::click(cx, "hosts-panel-toggle");
    test_support::click(cx, "hosts-panel-toggle");
    assert!(drawn(cx, "host-attention-save-error"), "it survives Hosts");

    let confirming = sent(&mut fake);
    assert_eq!(
        confirming.len(),
        1,
        "the premise: host 3's confirming request, still out"
    );
    test_support::click(cx, "host-attention-try-again");
    test_support::settle(cx);
    assert_eq!(sent(&mut fake), [auto_on_write()]);
    push_auto(&mut fake, true, cx);
    assert!(!drawn(cx, "host-attention-save-error"));
}

/// Why this matters: dismissing optional reminders must neither hide anything else nor last past a new update (SPEC.md:
/// "The dismissal leaves the rail badges and hides those reminders until a host outside the dismissed update set gains
/// an update"; "a reset after a lost connection shows optional reminders again"); the old case "optional dismissal
/// leaves other attention visible and revives only for new updates". Specified: the close control hides the two
/// optional rows, keeps the problem row and the rail's badges, and hands focus to the Hosts control; host 3 turning
/// required shows it alone; host 7 gaining an update shows every update again, which stays so after host 7's update
/// goes; dismissed again, a reset shows them again.
#[gpui::test]
fn dismissal_hides_optional_rows_until_another_host_gains_an_update(cx: &mut TestAppContext) {
    let listed = vec![
        remote(2, "older", old_build()),
        remote(3, "other", old_build()),
        remote(4, "down", unreachable()),
        remote(7, "later", connected()),
    ];
    let (mut fake, cx) = open(cx, &listed, &[], &[], true);
    assert_eq!(row_ids(cx), [4, 2, 3], "the premise");
    test_support::click(cx, "host-attention-dismiss");
    test_support::settle(cx);
    assert_eq!(row_ids(cx), [4]);
    assert!(
        cx.debug_bounds("host-rail-update-2").is_some(),
        "the badge stays"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("hosts-panel-toggle"));

    let mut now = listed.clone();
    now[1].state = skew(28, 29);
    push_hosts(&mut fake, &now, &[], cx);
    assert_eq!(row_ids(cx), [4, 3]);
    now[3].state = old_build();
    push_hosts(&mut fake, &now, &[], cx);
    assert_eq!(row_ids(cx), [4, 2, 3, 7]);
    now[3].state = connected();
    push_hosts(&mut fake, &now, &[], cx);
    assert_eq!(row_ids(cx), [4, 2, 3], "the dismissal is gone for good");

    test_support::click(cx, "host-attention-dismiss");
    test_support::settle(cx);
    assert_eq!(row_ids(cx), [4, 3], "the premise: dismissed again");
    fake.break_channel();
    cx.run_until_parked();
    fake.connect(vec![
        listing(&[], &[]),
        preferences_state(Preferences {
            welcome_seen: Some(true),
            auto_update_remote_hosts: Some(true),
            ..Preferences::default()
        }),
        hosts(&now),
        progress(&[]),
    ]);
    test_support::settle(cx);
    assert_eq!(row_ids(cx), [4, 2, 3], "a reset shows the reminders again");
}

/// Why this matters: Details is how the user gets from a row to the host's diagnosis (SPEC.md: "**Details**, or choosing
/// a row outside its action button, opens Hosts with that host's row in view and its menu button focused, without
/// changing the session filter. This uses the existing acknowledgement of warnings when Hosts opens"); the old cases
/// "attention groups problems, work and updates and opens the exact host" and "many attention rows ... opening the last
/// host must scroll its management row into view". Specified: in a short window over thirty current hosts and, last
/// in the registry, one whose setup failed unseen and one with an update, Details on the failed host shows Hosts,
/// clears its warning, focuses its menu toggle inside the panel's visible part, leaves All pressed and sends nothing;
/// back on Sessions, a press on the update row's open control focuses that host's toggle the same way.
#[gpui::test]
fn details_opens_hosts_at_the_host_with_its_toggle_focused(cx: &mut TestAppContext) {
    let mut listed: Vec<HostView> = (10..40)
        .map(|id| remote(id, &format!("host-{id}"), connected()))
        .collect();
    listed.push(remote(40, "failed", connected()));
    listed.push(remote(41, "older", old_build()));
    let (mut fake, cx) = open(cx, &listed, &[run(40, Add, RunStatus::Failed)], &[], true);
    cx.simulate_resize(size(px(900.), px(500.)));
    test_support::settle(cx);
    let warned =
        |cx: &mut VisualTestContext| cx.update(|_, cx| crate::hosts::status::warned(40, cx));
    assert!(warned(cx), "the premise: host 40's failure is unseen");
    let in_view = |cx: &mut VisualTestContext, toggle: &'static str| {
        let panel = test_support::bounds(cx, "hosts-panel");
        let at = test_support::bounds(cx, toggle);
        at.top() >= panel.top() && at.bottom() <= panel.bottom()
    };

    test_support::click(cx, "host-attention-details-40");
    test_support::settle(cx);
    assert!(drawn(cx, "hosts-panel"));
    assert!(!warned(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-40"));
    assert!(
        in_view(cx, "host-menu-toggle-40"),
        "the row was scrolled into view"
    );
    assert!(
        !in_view(cx, "host-menu-toggle-10"),
        "the rows scrolled to get there"
    );
    assert!(
        pressed(&frame(cx), "host-rail-all"),
        "the filter is unchanged"
    );
    assert_eq!(sent(&mut fake), []);

    test_support::click(cx, "hosts-panel-toggle");
    test_support::click(cx, "host-attention-open-41");
    test_support::settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-41"));
    assert!(in_view(cx, "host-menu-toggle-41"));
}

/// Why this matters: the list sits above the sessions, so a row coming or going moves every session menu's button,
/// while progress repaints in place (SPEC.md: changes in its rows "retire session menus anchored below the attention
/// list ... ordinary progress ticks still do not"; old cases "progress-only publications preserve an open session
/// menu", "attention insertion and removal retire existing compact session menus" and, shared with bulk marking, "host
/// attention movement closes the marked menu and reopening remeasures"). Specified: b's row menu stays open through a
/// progress tick of a working row; a new unreachable host closes it; reopened, it sits beside b's moved button as it
/// did before; the marked menu likewise closes when that host leaves and reopens at its toggle's new place.
#[gpui::test]
fn menus_below_close_when_the_list_moves_and_stay_through_progress(cx: &mut TestAppContext) {
    let listed = [remote(5, "busy", connected())];
    let (mut fake, cx) = open(
        cx,
        &listed,
        &[setup_run(5, 0)],
        &[session("a"), session("b")],
        false,
    );
    // Tall enough that the menu never meets the window's bottom edge, which would hold it there rather than beside b.
    cx.simulate_resize(size(px(900.), px(1100.)));
    test_support::settle(cx);
    test_support::click(cx, "session-menu-b");
    let beside = offset(cx, "session-menu", "session-menu-b");
    push_run(&mut fake, setup_run(5, 1), cx);
    assert!(any_open(cx), "a progress tick keeps the menu");

    let mut grown = listed.to_vec();
    grown.push(remote(9, "down", unreachable()));
    let before = test_support::bounds(cx, "session-menu-b");
    push_hosts(&mut fake, &grown, &[], cx);
    assert!(!any_open(cx), "a new row closes the menu");
    assert_ne!(
        test_support::bounds(cx, "session-menu-b"),
        before,
        "the premise: b moved"
    );
    test_support::click(cx, "session-menu-b");
    let again = offset(cx, "session-menu", "session-menu-b");
    assert!(
        same_place(again, beside),
        "{again:?} vs {beside:?}: menu {:?}, button {:?}",
        test_support::bounds(cx, "session-menu"),
        test_support::bounds(cx, "session-menu-b")
    );
    test_support::keys(cx, "escape");

    let at = test_support::center(cx, "session-row-a");
    cx.simulate_click(
        at,
        Modifiers {
            control: true,
            ..Modifiers::default()
        },
    );
    test_support::settle(cx);
    test_support::click(cx, MENU_TOGGLE_ID);
    assert!(drawn(cx, MENU_ID), "the premise: the marked menu is open");
    let below = offset(cx, MENU_ID, MENU_TOGGLE_ID);
    push_hosts(&mut fake, &listed, &[9], cx);
    assert!(!drawn(cx, MENU_ID), "a row leaving closes the marked menu");
    test_support::click(cx, MENU_TOGGLE_ID);
    assert!(same_place(offset(cx, MENU_ID, MENU_TOGGLE_ID), below));
}

/// Why this matters: many hosts needing attention must not push the sessions off a small window (SPEC.md: "It stays
/// visible while sessions scroll; its own bounded row region scrolls when needed"); the old case "many attention rows
/// scroll above usable sessions in a short narrow sidebar". Specified: in a 420-pixel-high window with twelve update
/// rows under long aliases, the list is at most 280 pixels and 45% of the window high, a session row shows whole below
/// it, and the last row, below the rows' fold at first, is scrolled into the rows' visible part when Tab reaches its
/// open control, with the list itself not moving.
#[gpui::test]
fn many_rows_scroll_inside_a_bounded_list(cx: &mut TestAppContext) {
    let listed: Vec<HostView> = (20..32)
        .map(|id| {
            remote(
                id,
                &format!("host-{id}-{}", "long-alias-".repeat(5)),
                old_build(),
            )
        })
        .collect();
    let (_fake, cx) = open(cx, &listed, &[], &[session("a")], true);
    cx.simulate_resize(size(px(1024.), px(420.)));
    test_support::settle(cx);
    let list = test_support::bounds(cx, "host-attention-list");
    assert!(
        f32::from(list.size.height) <= 280. && f32::from(list.size.height) <= 0.45 * 420.,
        "{list:?}"
    );
    let row = test_support::bounds(cx, "session-row-a");
    assert!(
        row.top() >= list.bottom() && f32::from(row.bottom()) <= 420.,
        "{row:?} under {list:?}"
    );
    let body = test_support::bounds(cx, "host-attention-body");
    assert!(
        test_support::bounds(cx, "host-attention-row-31").top() >= body.bottom(),
        "the premise: the last row starts below the fold"
    );

    // From the rail's All (pressed already, so the click sends nothing), one press at a time, each with its frame, as a
    // user tabs; bounded so a broken Tab order fails rather than hangs.
    test_support::click(cx, "host-rail-all");
    test_support::settle(cx);
    for _ in 0..80 {
        if focused_id(cx).as_deref() == Some("host-attention-open-31") {
            break;
        }
        test_support::keys(cx, "tab");
        test_support::settle(cx);
    }
    assert_eq!(focused_id(cx).as_deref(), Some("host-attention-open-31"));
    let last = test_support::bounds(cx, "host-attention-row-31");
    assert!(
        last.top() >= body.top() && last.bottom() <= body.bottom(),
        "{last:?} in {body:?}"
    );
    assert_eq!(test_support::bounds(cx, "host-attention-list"), list);
}
