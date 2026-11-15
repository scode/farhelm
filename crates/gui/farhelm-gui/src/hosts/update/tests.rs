//! Tests of updating a host over the helm client's fake: the menu's Update and what it sends under which key, the
//! inline status while it runs and the row staying folded, where each kind of refusal shows (the refusal dialog, under
//! the row), the launch and busy refusals before anything is sent, the inline `↑ Update`, Update all, and the rerun of a
//! failed Update; then the urgency and hover rules without a window.
//!
//! Every test starts from the Hosts panel's harness (`hosts/tests.rs`) in a column of the sidebar's width and a window as
//! large as the reference screens', and every step ends in one of the shared harness's step helpers, so the focus and
//! layer invariants are checked after each.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::provisioning::{ProvisioningOperation, ProvisioningView, RunStatus, StepStatus};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HelmMarker, HostStateView, HostView,
    KeyedEntry, RefreshView,
};
use gpui::{TestAppContext, VisualTestContext, px, size};
use serde_json::{Value, json};

use super::{Urgency, hover, urgency};
use crate::host_text::tests::SPOOFING_CORPUS;
use crate::hosts::LAUNCH_IN_FLIGHT;
use crate::hosts::tests::{
    aliased, connected_with, host, hosts, open_panel_in_column, progress, reference_hosts, run,
    skew,
};
use crate::operations::{Command, OpKey, Work};
use crate::session;
use crate::test_support::{
    any_open, click, focused_id, frame, keys, label, meta, one_command, settle,
};

// ---- Fixtures and steps ----------------------------------------------------------------------------------------------

/// A connected host on build `build`, older than the helm's.
fn old(build: &str) -> HostStateView {
    HostStateView::Connected {
        identity: Some("id-old".into()),
        build_version: build.into(),
        old_version: true,
        newer_version: false,
        refresh: RefreshView::Ok { sessions: 1 },
    }
}

/// A connected host on a build newer than the helm's.
fn newer() -> HostStateView {
    HostStateView::Connected {
        identity: Some("id-new".into()),
        build_version: "9.0.0".into(),
        old_version: false,
        newer_version: true,
        refresh: RefreshView::Ok { sessions: 0 },
    }
}

/// The panel over `snapshot`, in a column and a window as large as the reference screens'.
fn panel(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (helm, _, cx) = open_panel_in_column(cx, snapshot);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    settle(cx);
    (helm, cx)
}

/// The helm's plan reply, offering the one-use plan `u-1`.
fn plan_reply() -> Value {
    json!({
        "probe_id": "u-1",
        "plan": { "the helm's own": "plan" },
        "confirmation": "Farhelm will update dev@build-box:\n- Upload the farhelm binary\n",
        "steps": { "lead": "Farhelm will update dev@build-box:", "steps": [], "kept": null },
    })
}

/// The helm's words refusing an Update of a host on a newer build.
const NEWER: &str = "the host runs farhelm 9.0.0, which is newer than this helm's 0.16.0; Update never installs an \
                     older version over a newer one, so update this helm instead";

/// The helm's refusal of a plan it no longer holds.
const USED: &str = "the provisioning plan is unknown or has already been used; probe again";

/// Choose Update from host `id`'s menu and check that it sent the plan request; that request's id.
fn choose_update(helm: &mut FakeHelm, id: i64, cx: &mut VisualTestContext) -> u64 {
    click_id(cx, format!("host-menu-toggle-{id}"));
    click(cx, "host-menu-update");
    let (sent, body) = one_command(helm, Method::Post, &format!("/api/hosts/{id}/update"));
    assert_eq!(body, Value::Null, "the plan request has no body");
    sent
}

/// Click the element `id` names, for ids built at run time.
fn click_id(cx: &mut VisualTestContext, id: String) {
    click(cx, Box::leak(id.into_boxed_str()));
}

/// Push `run` as its host's retained run.
fn push_run(helm: &mut FakeHelm, run: &ProvisioningView) {
    let key = HostView::key(run.host_id);
    helm.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&[key.as_str()]),
        upsert: vec![KeyedEntry {
            key,
            value: serde_json::to_value(run).unwrap(),
        }],
        remove: Vec::new(),
    });
}

/// Host 2's Update run in `status`, its three steps one done, one running and one pending.
fn update_run(status: RunStatus) -> ProvisioningView {
    let mut update = run(2, ProvisioningOperation::Update, status);
    update.steps = vec![
        crate::hosts::progress::tests::step("Upload the farhelm binary", StepStatus::Completed),
        crate::hosts::progress::tests::step("Install the farhelm binary", StepStatus::Running),
        crate::hosts::progress::tests::step("Connect to the supervisor", StepStatus::Pending),
    ];
    update
}

/// The work in flight on `key`.
fn work(key: &OpKey, cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| session::current_operations(cx)?.read(cx).in_flight(key))
}

/// Start `command` under `key` in the operation store, as another feature would, and let it wait for its reply.
fn hold(key: OpKey, command: Command, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("a UI session");
        operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, |_, _| {}).unwrap();
        });
    });
    settle(cx);
}

// ---- The menu's Update -----------------------------------------------------------------------------------------------

/// SPEC.md, "Topology": "the user's Update click ... is the authorization: no plan is shown for confirmation", and the
/// row "stays folded and shows `Updating…` until a progress snapshot is available, then shows the current step,
/// completed-step count, and client-measured elapsed time inline"; failure "still expands the row". This is the
/// Update's whole path through the one-use plan the helm keeps. Specified: `build-box`'s menu starts on Update ("Install
/// the newer Farhelm version"); choosing it closes the menu and sends `POST /api/hosts/2/update` without a body under
/// the host's key, the row reading "Updating…"; the plan's reply sends the same route with `{"probe_id": "u-1"}`, still
/// "Updating…"; with the run pushed and the request accepted, the status counts "Updating: 1 of 3 steps done" with the
/// running step, no question or dialog opened, focus is on the row's toggle, and the row shows no steps (folded) until
/// Details is turned on ("global details alone control disclosure while update progress stays inline"); once the run
/// fails, the row opens on "Update provisioning Failed" with Details off.
#[gpui::test]
fn the_menus_update_plans_then_confirms_under_the_hosts_key(cx: &mut TestAppContext) {
    // The host progress collection is in the snapshot, empty, as the helm sends it: a change to a collection the
    // snapshot did not hold is not applied.
    let (mut helm, panel, cx) =
        open_panel_in_column(cx, vec![hosts(&reference_hosts()), progress(&[])]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    settle(cx);
    click(cx, "host-menu-toggle-2");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-update"));
    assert_eq!(label(cx, "host-menu-update").as_deref(), Some("Update"));
    click(cx, "host-menu-update");
    assert!(!any_open(cx), "the menu closed");
    let (plan, body) = one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    assert_eq!(body, Value::Null);
    let key = OpKey::host("2");
    assert_eq!(work(&key, cx), Some(Work::UpdateHost));
    assert_eq!(label(cx, "host-status-2").as_deref(), Some("Updating…"));

    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    let (confirm, body) = one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    assert_eq!(body, json!({ "probe_id": "u-1" }));
    assert_eq!(work(&key, cx), Some(Work::UpdateHost));
    assert_eq!(label(cx, "host-status-2").as_deref(), Some("Updating…"));

    push_run(&mut helm, &update_run(RunStatus::Running));
    helm.reply(
        confirm,
        202,
        Some(json!({ "host_id": 2, "run_id": "run-2" })),
    );
    settle(cx);
    assert_eq!(work(&key, cx), None);
    assert_eq!(
        label(cx, "host-status-2").as_deref(),
        Some("Updating: 1 of 3 steps done, now Install the farhelm binary")
    );
    assert!(!any_open(cx), "nothing asks or opens");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
    assert_eq!(label(cx, "host-run-title-2"), None, "the row stays folded");
    cx.update(|_, cx| panel.update(cx, |panel, cx| panel.show_details(true, cx)));
    settle(cx);
    assert_eq!(
        label(cx, "host-run-title-2").as_deref(),
        Some("Update provisioning Running")
    );
    cx.update(|_, cx| panel.update(cx, |panel, cx| panel.show_details(false, cx)));

    push_run(&mut helm, &update_run(RunStatus::Failed));
    settle(cx);
    assert_eq!(
        label(cx, "host-run-title-2").as_deref(),
        Some("Update provisioning Failed"),
        "a failure opens the row"
    );
}

/// SPEC.md, "Topology": Update refuses a host on a newer build "with both versions named", a newer database schema and
/// a supervisor below the upgrade floor with its manual remedy; the old app showed each such refusal in its host refusal
/// dialog. The helm words each, so the dialog shows its words whatever they are. Specified: a 409 to the plan request
/// opens "Update host · build-box" with "Farhelm refused this action", the full destination `dev@build-box` and the
/// helm's words, focus on Close; no confirming request goes out; the row, whose last Update completed, neither shows
/// the refusal nor opens on that run, as in the old app, which kept a refusal it showed in its dialog alone; Close closes
/// it with focus back on the row's toggle. Opened again, Enter with focus on the dialog itself (its container) closes it
/// too, Close being its Enter action as in the old dialog.
#[gpui::test]
fn a_refused_plan_opens_the_refusal_dialog_naming_the_host(cx: &mut TestAppContext) {
    let completed = update_run(RunStatus::Completed);
    let (mut helm, cx) = panel(cx, vec![hosts(&reference_hosts()), progress(&[completed])]);
    assert_eq!(
        label(cx, "host-run-title-2"),
        None,
        "the premise: the row is folded"
    );
    let plan = choose_update(&mut helm, 2, cx);
    helm.reply(plan, 409, Some(json!(NEWER)));
    settle(cx);
    assert_eq!(
        label(cx, "host-refusal").as_deref(),
        Some("Update host · build-box")
    );
    assert_eq!(label(cx, "host-refusal-reason").as_deref(), Some(NEWER));
    let painted: String = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(painted.contains("Farhelm refused this action"));
    assert!(painted.contains("dev@build-box"));
    assert_eq!(focused_id(cx).as_deref(), Some("host-refusal-done"));
    assert_eq!(label(cx, "host-error-2"), None, "not under the row too");
    assert_eq!(label(cx, "host-run-title-2"), None, "the row stays folded");
    assert_eq!(helm.commands(), [], "no confirming request");

    click(cx, "host-refusal-done");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));

    let plan = choose_update(&mut helm, 2, cx);
    helm.reply(plan, 409, Some(json!(NEWER)));
    settle(cx);
    assert!(
        label(cx, "host-refusal").is_some(),
        "the premise: it opened again"
    );
    cx.update(|window, cx| {
        let container =
            crate::layers::top_container(window, cx).expect("the refusal is the top layer");
        let mark = crate::focus::Mark::take(cx);
        crate::focus::focus_in_layer(&container, &mark, window, cx);
    });
    settle(cx);
    let focused = focused_id(cx);
    assert!(
        focused
            .as_deref()
            .is_some_and(|id| id.starts_with("layer-")),
        "the premise: focus is on the dialog itself, not on a control: {focused:?}"
    );
    keys(cx, "enter");
    assert!(!any_open(cx), "Enter on the dialog pressed Close");
}

/// The refusal dialog is the host refusal dialog the old app showed for every host action, and its Remove answer, for a
/// refusal the helm marks "remove instead", leads to Remove host's own flow ("uninstall and Update planning refusals
/// open framed dialogs": Remove opened the removal confirmation). The helm sets that marker only on an uninstall plan's
/// refusals, but the dialog reads the marker, not the action, so an Update refusal carrying it shows the same answer.
/// Specified: an unmarked refusal draws Close alone; a refusal marked `RemoveHostInstead` draws Remove after Close, and
/// pressing it closes the refusal and opens "Remove host · build-box", the removal confirmation.
#[gpui::test]
fn a_refusal_marked_remove_instead_offers_remove_host(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, vec![hosts(&reference_hosts())]);
    let plan = choose_update(&mut helm, 2, cx);
    helm.reply(plan, 409, Some(json!(NEWER)));
    settle(cx);
    assert!(
        label(cx, "host-refusal").is_some(),
        "the premise: the refusal is open"
    );
    assert_eq!(
        label(cx, "host-refusal-remove"),
        None,
        "no Remove without the marker"
    );
    keys(cx, "escape");

    let plan = choose_update(&mut helm, 2, cx);
    helm.reply_marked(
        plan,
        409,
        vec![HelmMarker::RemoveHostInstead],
        Some(json!("remove this host instead")),
    );
    settle(cx);
    assert_eq!(label(cx, "host-refusal-remove").as_deref(), Some("Remove"));
    click(cx, "host-refusal-remove");
    assert_eq!(label(cx, "host-refusal"), None, "the refusal closed");
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Remove host · build-box"),
        "Remove host's confirmation opened"
    );
}

/// SPEC.md, "Keyboard focus": a result that arrives after the user moved on opens no dialog and shows where it belongs
/// (the goal's "No queue for results that arrive later"). The layer manager refuses a dialog for a mark the user has
/// moved on from, so this guards the plumbing on Update's side: the refusal dialog opens through the mark taken at the
/// press, never through a fresh one taken when the reply arrives, which would open it over whatever the user turned to.
/// Specified: after Update, a key press, then the plan's refusal: nothing opens, and the refusal stays under
/// `build-box`'s row, which opens for it (on its last, completed Update's steps), the one place left to show it.
#[gpui::test]
fn a_refused_plan_after_the_user_moved_on_opens_nothing(cx: &mut TestAppContext) {
    let completed = update_run(RunStatus::Completed);
    let (mut helm, cx) = panel(cx, vec![hosts(&reference_hosts()), progress(&[completed])]);
    let plan = choose_update(&mut helm, 2, cx);
    keys(cx, "shift-tab");
    helm.reply(plan, 409, Some(json!(NEWER)));
    settle(cx);
    assert!(!any_open(cx), "no refusal dialog");
    assert_eq!(label(cx, "host-error-2").as_deref(), Some(NEWER));
    assert_eq!(
        label(cx, "host-run-title-2").as_deref(),
        Some("Update provisioning Completed"),
        "the row opened for the refusal"
    );
}

/// The old app kept a one-click Update's submission outcomes on the row rather than in a dialog: the user asked for
/// nothing a dialog could answer by then. Specified: a refusal of the confirming request opens nothing and shows under
/// `build-box`'s row.
#[gpui::test]
fn a_refused_confirmation_shows_under_the_row_only(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, vec![hosts(&reference_hosts())]);
    let plan = choose_update(&mut helm, 2, cx);
    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    let (confirm, _) = one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    helm.reply(confirm, 409, Some(json!(USED)));
    settle(cx);
    assert!(!any_open(cx));
    assert_eq!(label(cx, "host-error-2").as_deref(), Some(USED));
}

/// The migration's rule (2026-11-14): a host change and a session launch exclude each other, each side refusing its own
/// action with a reason while the other's work is in flight, and a launch there means the launcher's New or Clone or a
/// session's Restart or Replace. Remove host, setup and Re-run say so once on the Hosts panel's line, and Update does
/// too. Specified: with a launch in flight, the menu's Update sends nothing, opens no row, and the panel's line says
/// "Update build-box: not started, …"; Update all sends nothing either and the line says "Update all: not started, …"
/// once, with no row opened; with `build-box`'s Update failed meanwhile, the menu's Re-run sends nothing and the line
/// says "Re-run build-box: not started, …", as a setup's Re-run does; once the launch has settled, the Re-run goes out
/// and the line goes.
#[gpui::test]
fn a_launch_in_flight_refuses_the_update_before_sending(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, vec![hosts(&reference_hosts()), progress(&[])]);
    hold(
        OpKey::new_launch(),
        Command::new(
            Work::Launch,
            Method::Post,
            ApiPath::api(&["sessions"]).unwrap(),
            Some(json!({})),
            "POST /api/sessions",
        ),
        cx,
    );
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "the premise: a launch is in flight");
    let launch = sent[0].id;
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-update");
    assert_eq!(helm.commands(), [], "no Update was sent");
    assert_eq!(
        label(cx, "hosts-update-not-started"),
        Some(format!("Update build-box: {LAUNCH_IN_FLIGHT}"))
    );
    assert_eq!(label(cx, "host-error-2"), None, "nothing under the row");

    click(cx, "hosts-update-all");
    assert_eq!(helm.commands(), [], "Update all sent nothing");
    assert_eq!(
        label(cx, "hosts-update-not-started"),
        Some(format!("Update all: {LAUNCH_IN_FLIGHT}"))
    );
    for id in [2, 3] {
        assert_eq!(label(cx, &format!("host-error-{id}")), None, "host {id}");
    }

    // A failed Update's Re-run names the press the user made, as a setup's Re-run does.
    push_run(&mut helm, &update_run(RunStatus::Failed));
    settle(cx);
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-rerun");
    assert_eq!(helm.commands(), [], "the Re-run sent nothing");
    assert_eq!(
        label(cx, "hosts-update-not-started"),
        Some(format!("Re-run build-box: {LAUNCH_IN_FLIGHT}"))
    );

    helm.reply(launch, 200, Some(json!({})));
    settle(cx);
    assert!(
        !cx.update(|_, cx| crate::hosts::launch_in_flight(cx)),
        "the premise: the launch has settled"
    );
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-rerun");
    one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    assert_eq!(label(cx, "hosts-update-not-started"), None, "the line went");
}

// ---- The inline button -----------------------------------------------------------------------------------------------

/// The listed hosts of the inline button's tests: the local host on an old build (1), `build-box` on an old build (2),
/// `gpu-1` refused at the handshake on an older protocol (3), a current remote (4) and a newer one (5).
fn mixed_hosts() -> Vec<HostView> {
    let mut listed = reference_hosts();
    listed[0].state = old("0.15.0");
    listed[1].state = old("0.15.0");
    listed[2].state = skew(1, 2);
    listed.push(aliased(
        host(4, "ssh", connected_with("0.16.0", "id-4", 0)),
        "current",
        "dev@current",
    ));
    listed.push(aliased(host(5, "ssh", newer()), "newer", "dev@newer"));
    listed
}

/// SPEC.md, "Session list": an SSH host with an older build shows "an outlined `↑ Update` button" beside its words, and
/// "inline progress replaces it"; the old button moved focus to the row's menu toggle, since the progress takes its
/// place. Specified: `build-box` (old build) and `gpu-1` (older protocol) draw "Update build-box" and "Update gpu-1";
/// the local host, a current host and a newer one draw none; pressing `build-box`'s sends its plan request, the button
/// gives way to "Updating…", and focus is on its row's toggle.
#[gpui::test]
fn the_inline_update_shows_on_an_older_remote_host_and_starts_the_update(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, vec![hosts(&mixed_hosts())]);
    assert_eq!(
        label(cx, "host-update-2").as_deref(),
        Some("Update build-box")
    );
    assert_eq!(label(cx, "host-update-3").as_deref(), Some("Update gpu-1"));
    for id in [1, 4, 5] {
        assert_eq!(label(cx, &format!("host-update-{id}")), None, "host {id}");
    }
    click(cx, "host-update-2");
    one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    assert_eq!(label(cx, "host-update-2"), None, "progress replaces it");
    assert_eq!(label(cx, "host-status-2").as_deref(), Some("Updating…"));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
}

/// The button's urgency is its color (amber optional, red required) and its hover says why, naming both builds, so a
/// host's own build string must not be able to disguise which side is older. Specified: an older build is optional, an
/// older protocol required, and a current, newer, equal-protocol, newer-protocol or unconnected host has none; the
/// optional hover names the host's build, this build and "update optional", the required one both builds with their
/// protocols and "update required", both ending in "click to update this host to the helm's version"; every spoofing
/// value is escaped in the hover's text.
#[test]
fn the_hover_names_both_builds_the_urgency_and_the_action() {
    assert_eq!(urgency(&old("0.15.0")), Some(Urgency::Optional));
    assert_eq!(urgency(&skew(1, 2)), Some(Urgency::Required));
    for state in [
        connected_with("0.16.0", "id", 0),
        newer(),
        skew(2, 2),
        skew(3, 2),
        crate::hosts::tests::connecting(1, None),
    ] {
        assert_eq!(urgency(&state), None, "{state:?}");
        assert!(hover(&state).is_none());
    }

    let optional = hover(&old("0.15.0")).unwrap().accessible_name();
    assert_eq!(
        optional,
        format!(
            "This host runs Farhelm 0.15.0; this helm runs {}; update optional; click to update this host to the \
             helm's version",
            farhelm_proto::BUILD_VERSION
        )
    );
    let required = hover(&skew(1, 2)).unwrap().accessible_name();
    assert_eq!(
        required,
        "This host runs Farhelm 9.0.0 (protocol 1); this helm runs 0.16.0 (protocol 2); update required; click to \
         update this host to the helm's version"
    );
    for (raw, shown) in SPOOFING_CORPUS {
        let named = hover(&old(raw)).unwrap().accessible_name();
        assert!(named.contains(shown), "{named}");
    }
}

// ---- Update all ------------------------------------------------------------------------------------------------------

/// SPEC.md, "Topology": Update all "makes the same authorized Update request for each remote host whose individual
/// Update action is available at the click. A host already busy with setup or another run is skipped rather than
/// queued ...; the local host is excluded. Each remote host keeps its own validation, progress, and result". Specified:
/// over the local host, `build-box` (old), `gpu-1` with a setup running, a current host with a save in flight and a
/// newer host, Update all sends the plan request for `build-box` alone, which goes on to its confirming request. The
/// press lists every host and decides there which to update, so this is also the press's own check.
#[gpui::test]
fn update_all_updates_every_available_remote_host_and_skips_the_rest(cx: &mut TestAppContext) {
    let mut listed = mixed_hosts();
    listed[2].state = connected_with("0.16.0", "id-3", 0);
    let setup = run(3, ProvisioningOperation::Add, RunStatus::Running);
    let (mut helm, cx) = panel(cx, vec![hosts(&listed), progress(&[setup])]);
    hold(
        OpKey::host("4"),
        Command::new(
            Work::Save,
            Method::Post,
            ApiPath::api(&["hosts", "4", "alias"]).unwrap(),
            Some(json!({ "alias": "x" })),
            "POST /api/hosts/[host]/alias",
        ),
        cx,
    );
    let save = helm.commands()[0].id;
    click(cx, "hosts-update-all");
    let (plan, _) = one_command(&mut helm, Method::Post, "/api/hosts/2/update");

    // The fake answers in the order it was asked: the save first.
    helm.reply(save, 200, None);
    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    one_command(&mut helm, Method::Post, "/api/hosts/2/update");
}

/// Two hosts updated by one Update all go on independently. Specified: over the reference hosts, Update all sends the
/// plan request for `build-box` and `gpu-1` and not the local host; `build-box`'s refusal opens its refusal dialog
/// (and so not under its row), while `gpu-1`'s plan still goes on to its confirming request.
#[gpui::test]
fn update_all_lets_each_host_keep_its_own_result(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, vec![hosts(&reference_hosts())]);
    click(cx, "hosts-update-all");
    let sent = helm.commands();
    let paths: Vec<&str> = sent.iter().map(|command| command.path.as_str()).collect();
    assert_eq!(paths, ["/api/hosts/2/update", "/api/hosts/3/update"]);
    helm.reply(sent[0].id, 409, Some(json!(NEWER)));
    helm.reply(sent[1].id, 200, Some(plan_reply()));
    settle(cx);
    assert_eq!(
        label(cx, "host-refusal").as_deref(),
        Some("Update host · build-box")
    );
    assert_eq!(label(cx, "host-error-2"), None, "shown in the dialog alone");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/hosts/3/update");
    assert_eq!(body, json!({ "probe_id": "u-1" }));
}

/// Update all can have several plans refused together, and the old app showed them one dialog at a time, focus moving
/// to the next one as one closed ("queued Update refusals retain modal focus and isolation"). Here nothing queues: each
/// refusal opens its own dialog through the press's mark, the later over the earlier, and closing the top one returns
/// focus to the one below, as closing any layer does. Specified: with both remote plans refused, two refusal dialogs are
/// open; Escape closes one and leaves the other, naming the other host's reason, with focus on its Close; a second
/// Escape closes it.
#[gpui::test]
fn two_refusals_from_update_all_show_one_after_the_other(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, vec![hosts(&reference_hosts())]);
    click(cx, "hosts-update-all");
    let sent = helm.commands();
    assert_eq!(sent.len(), 2, "the premise: both remotes were sent");
    helm.reply(sent[0].id, 409, Some(json!("first update refused")));
    helm.reply(sent[1].id, 409, Some(json!("second update refused")));
    settle(cx);
    let dialogs = |cx: &mut VisualTestContext| {
        frame(cx)
            .into_iter()
            .filter(|node| node.element_id.as_deref() == Some("host-refusal-reason"))
            .filter_map(|node| node.label.map(|label| label.to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(dialogs(cx).len(), 2);
    keys(cx, "escape");
    assert_eq!(dialogs(cx), ["first update refused"]);
    assert_eq!(focused_id(cx).as_deref(), Some("host-refusal-done"));
    keys(cx, "escape");
    assert!(!any_open(cx));
}

// ---- The rerun -------------------------------------------------------------------------------------------------------

/// The old menu offered a failed Update again as Re-run, which sends the same Update; on the helm's own machine, which
/// the panel cannot update, it was greyed. Specified: with `build-box`'s Update failed, its row is open on "Update
/// provisioning Failed" and its menu starts on Re-run ("Try the failed update again"), which sends the plan request;
/// with the local host's Update failed (a run an older helm retained), its Re-run is greyed and sends nothing.
#[gpui::test]
fn re_run_of_a_failed_update_sends_the_update_again(cx: &mut TestAppContext) {
    let failed = [
        run(1, ProvisioningOperation::Update, RunStatus::Failed),
        run(2, ProvisioningOperation::Update, RunStatus::Failed),
    ];
    let (mut helm, cx) = panel(cx, vec![hosts(&reference_hosts()), progress(&failed)]);
    assert_eq!(
        label(cx, "host-run-title-2").as_deref(),
        Some("Update provisioning Failed")
    );
    click(cx, "host-menu-toggle-2");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-rerun"));
    let painted: String = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(painted.contains("Try the failed update again"));
    click(cx, "host-menu-rerun");
    one_command(&mut helm, Method::Post, "/api/hosts/2/update");

    click(cx, "host-menu-toggle-1");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-menu-settings"),
        "the premise: the menu starts on Settings, past its greyed Re-run and Update"
    );
    click(cx, "host-menu-rerun");
    assert_eq!(
        helm.commands(),
        [],
        "the local host's greyed Re-run sent nothing"
    );
}

// ---- How the operation store classifies an Update --------------------------------------------------------------------

/// The launcher refuses a launch while host registry work is in flight, and the host side refuses its changes while a
/// session is being created, each by asking the operation store's classification of the work in flight. An Update was
/// decided to count as a registry change (it replaces the host's installation), and it creates no session; a flipped
/// arm would let a launch start mid-Update or block Remove and setup during one for nothing. This file pins the answers
/// for the kind of work it adds, as `operations_tests.rs` pins the older kinds. Specified: `Work::UpdateHost` changes the
/// host registry and does not create a session.
#[test]
fn an_update_changes_the_host_registry_and_creates_no_session() {
    assert!(Work::UpdateHost.changes_host_registry());
    assert!(!Work::UpdateHost.creates_a_session());
}
