//! Tests of uninstalling a host over the helm client's fake: the menu's Uninstall and what it sends under which key,
//! the confirmation and its plan, the run in the row, where each refusal shows (the refusal dialog, under the row), the
//! removal notice and what withdraws it, and the launch refusal before anything is sent.
//!
//! Every test starts from the Hosts panel's harness (`hosts/tests.rs`) in a column of the sidebar's width and a window as
//! large as the reference screens', and every step ends in one of the shared harness's step helpers, so the focus and
//! layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::provisioning::{
    PlanStep, PlanStepPath, PlanSteps, ProvisioningOperation, ProvisioningView, RunStatus,
    StepStatus,
};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HelmMarker, HostView, KeyedEntry,
};
use gpui::{TestAppContext, VisualTestContext, px, size};
use serde_json::{Value, json};

use super::{MOVED_ON, Plan};
use crate::hosts::LAUNCH_IN_FLIGHT;
use crate::hosts::progress::tests::step;
use crate::hosts::tests::{
    hosts, open_panel_in_column, progress, reference_hosts, run, start_work,
};
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{any_open, click, focused_id, keys, label, meta, one_command, settle};

// ---- Fixtures and steps ----------------------------------------------------------------------------------------------

/// The data directory the reference plan keeps.
const KEPT: &str = "/home/dev/.local/state/farhelm";

/// The helm's refusal of an uninstall while a session still runs on the host, in its own words (the reference screen's).
const SESSIONS: &str = "uninstall is refused while sessions on this host have not ended or still have terminal tabs \
                        open: \"migrate auth tokens\". Stop those sessions and close their tabs, then choose \
                        uninstall again";

/// The helm's refusal of a plan it no longer holds.
const USED: &str = "the provisioning plan is unknown or has already been used; probe again";

/// The reference screens' `build-box` uninstall plan, as the helm words one for a connected Linux host: its lead, the
/// six steps a full uninstall plans, and what it keeps.
pub(crate) fn reference_steps() -> PlanSteps {
    let step = |id: &str, name: &str, paths: &[&str]| PlanStep {
        id: id.into(),
        name: name.into(),
        paths: paths
            .iter()
            .map(|short| PlanStepPath {
                full: short.replacen('~', "/home/dev", 1),
                short: (*short).into(),
            })
            .collect(),
        explanation: None,
    };
    PlanSteps {
        lead: "Farhelm will uninstall from dev@build-box (ubuntu, x86_64):".into(),
        steps: vec![
            step("disable-supervisor", "Disable the supervisor", &[]),
            step(
                "remove-unit",
                "Remove the systemd user unit",
                &["~/.config/systemd/user/farhelm-supervisor.service"],
            ),
            step("reload-units", "Reload systemd user units", &[]),
            step(
                "stop-supervisor",
                "Stop the supervisor and its terminal hosts",
                &[],
            ),
            step(
                "remove-directory",
                "Remove the install directory",
                &["~/.local/lib/farhelm"],
            ),
            step("forget-host", "Remove the host from the list", &[]),
        ],
        kept: Some(format!(
            "Kept: the host's Farhelm data in {KEPT}. Deleting that directory by hand removes the data too."
        )),
    }
}

/// The reference plan for `view`, frozen as the confirmation holds it: the screenshot golden's.
pub(super) fn reference_plan(view: HostView) -> Plan {
    Plan {
        view,
        probe_id: "un-1".into(),
        confirmation: "Farhelm will uninstall from dev@build-box (ubuntu, x86_64):\n".into(),
        steps: reference_steps(),
        kept: KEPT.into(),
    }
}

/// The helm's plan reply, offering the one-use plan `un-1`.
fn plan_reply() -> Value {
    json!({
        "probe_id": "un-1",
        "plan": { "operation": "uninstall", "state_dir": KEPT },
        "confirmation": "Farhelm will uninstall from dev@build-box (ubuntu, x86_64):\n",
        "steps": reference_steps(),
    })
}

/// The panel over the reference hosts and an empty host progress collection (a change to a collection the snapshot did
/// not hold is not applied), in a column and a window as large as the reference screens'.
fn panel(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    panel_over(cx, vec![hosts(&reference_hosts()), progress(&[])])
}

/// [`panel`] over `snapshot`.
fn panel_over(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (helm, _, cx) = open_panel_in_column(cx, snapshot);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    settle(cx);
    (helm, cx)
}

/// Choose Uninstall from `build-box`'s menu and check that it sent the plan request; that request's id.
fn choose_uninstall(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-uninstall");
    let (sent, body) = one_command(helm, Method::Post, "/api/hosts/2/uninstall");
    assert_eq!(body, Value::Null, "the plan request has no body");
    sent
}

/// Choose Uninstall, answer its plan and confirm it: the confirming request's id, with its body checked.
fn confirm_uninstall(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    let plan = choose_uninstall(helm, cx);
    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    click(cx, "prompt-confirm");
    let (confirm, body) = one_command(helm, Method::Post, "/api/hosts/2/uninstall");
    assert_eq!(body, json!({ "probe_id": "un-1" }));
    confirm
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

/// Push the registry without `build-box` and its retained run, as the run's last step leaves it.
fn push_forgotten(helm: &mut FakeHelm) {
    helm.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&[]),
        upsert: Vec::new(),
        remove: vec!["2".into()],
    });
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1", "3"]),
        upsert: Vec::new(),
        remove: vec!["2".into()],
    });
}

/// `build-box`'s uninstall run `run_id` in `status`, its six steps the first done and the second running.
fn uninstall_run(run_id: &str, status: RunStatus) -> ProvisioningView {
    let mut uninstall = run(2, ProvisioningOperation::Uninstall, status);
    uninstall.run_id = Some(run_id.into());
    uninstall.steps = reference_steps()
        .steps
        .iter()
        .enumerate()
        .map(|(n, planned)| {
            let status = match n {
                0 => StepStatus::Completed,
                1 => StepStatus::Running,
                _ => StepStatus::Pending,
            };
            step(&planned.name, status)
        })
        .collect();
    uninstall
}

/// The work in flight on `build-box`'s key.
fn work(cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)?
            .read(cx)
            .in_flight(&OpKey::host("2"))
    })
}

/// The full sentence the removal notice names `build-box`'s removal with: its accessible name and hover.
const NOTICE: &str = "Farhelm was removed from build-box; its data remains in /home/dev/.local/state/farhelm on \
                      that host.";

// ---- The whole path --------------------------------------------------------------------------------------------------

/// SPEC.md, "Topology": the helm "plans the removal over SSH and shows it before touching the host ... Nothing changes
/// until the user confirms. Setup and uninstall planning show their action in the folded row while the helm prepares a
/// dialog ... Confirming closes the dialog, and the running row expands once to show the helm's steps ... Success
/// removes the host from the list ... and the window that confirmed shows a one-line removal notice. Its hover text
/// names the kept data directory, a copy control copies that path, and × dismisses the notice." This is the whole
/// path, the old "uninstall-remote-host" and "uninstall-progress-inline" cases. Specified: `build-box`'s menu offers
/// Uninstall ("Remove Farhelm from this host, keeping its data"); choosing it sends the plan request without a body under
/// the host's key, the row reading "Uninstalling…" with no step list; the plan opens "Uninstall Farhelm · build-box"
/// with its kept note and focus on Uninstall; Uninstall closes it and sends the plan's id; with the run pushed the row
/// reads "Uninstalling: 1 of 6 steps done, now Remove the systemd user unit" and opens on "Uninstall provisioning
/// Running"; once the registry drops the host the panel shows the notice, "Removed from build-box · data kept" on screen
/// and the full sentence as its name, Copy puts the kept path on the clipboard, and × hides it.
#[gpui::test]
fn the_menus_uninstall_plans_asks_runs_and_leaves_a_notice(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    click(cx, "host-menu-toggle-2");
    assert_eq!(
        label(cx, "host-menu-uninstall").as_deref(),
        Some("Uninstall")
    );
    click(cx, "host-menu-uninstall");
    assert!(!any_open(cx), "the menu closed and nothing asks yet");
    let (plan, body) = one_command(&mut helm, Method::Post, "/api/hosts/2/uninstall");
    assert_eq!(body, Value::Null);
    assert_eq!(work(cx), Some(Work::UninstallHost));
    assert_eq!(label(cx, "host-status-2").as_deref(), Some("Uninstalling…"));
    assert_eq!(
        label(cx, "host-run-title-2"),
        None,
        "planning keeps the row folded"
    );

    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Uninstall Farhelm · build-box")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(
        label(cx, "plan-kept-text").as_deref(),
        reference_steps().kept.as_deref()
    );
    click(cx, "prompt-confirm");
    assert!(!any_open(cx), "confirming closed the question");
    let (confirm, body) = one_command(&mut helm, Method::Post, "/api/hosts/2/uninstall");
    assert_eq!(body, json!({ "probe_id": "un-1" }));

    push_run(&mut helm, &uninstall_run("run-u", RunStatus::Running));
    helm.reply(
        confirm,
        202,
        Some(json!({ "host_id": 2, "run_id": "run-u" })),
    );
    settle(cx);
    assert_eq!(work(cx), None);
    assert_eq!(
        label(cx, "host-status-2").as_deref(),
        Some("Uninstalling: 1 of 6 steps done, now Remove the systemd user unit")
    );
    assert_eq!(
        label(cx, "host-run-title-2").as_deref(),
        Some("Uninstall provisioning Running"),
        "the running row opens once"
    );
    assert_eq!(
        label(cx, "uninstall-notice-summary"),
        None,
        "no notice while it runs"
    );

    push_forgotten(&mut helm);
    settle(cx);
    assert_eq!(label(cx, "host-row-2"), None, "the row went with the host");
    assert_eq!(
        label(cx, "uninstall-notice-summary").as_deref(),
        Some(NOTICE)
    );
    click(cx, "uninstall-notice-copy");
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some(KEPT)
    );
    click(cx, "uninstall-notice-dismiss");
    assert_eq!(
        label(cx, "uninstall-notice-summary"),
        None,
        "× dismissed it"
    );
}

/// SPEC.md: "Nothing changes until the user confirms", and closing a question returns focus to where it was. The old
/// case dismissed the plan by Cancel and by the header's ×, sending nothing either way. Specified: Cancel closes the
/// confirmation, sends nothing, owes no notice, and returns focus to `build-box`'s menu toggle; the host is still
/// listed, and the registry dropping it later (a Remove from the command line, say) shows no notice.
#[gpui::test]
fn cancel_sends_nothing_and_owes_no_notice(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let plan = choose_uninstall(&mut helm, cx);
    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    click(cx, "prompt-cancel");
    assert!(!any_open(cx));
    assert_eq!(helm.commands(), []);
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));
    push_forgotten(&mut helm);
    settle(cx);
    assert_eq!(label(cx, "uninstall-notice-summary"), None);
}

// ---- Refusals --------------------------------------------------------------------------------------------------------

/// SPEC.md, "Topology": uninstall refuses, naming them, while the host has sessions or tabs, a host it cannot check, a
/// foreign supervisor path and a symlinked lib directory, and "when the host list already
/// shows incompatible versions, an unverified or changed identity, or two entries reaching the same machine, uninstall's
/// refusal also explains the remedy and offers Remove". The helm words every refusal and marks the ones that offer
/// Remove; the GUI's part is to show the words in the host refusal dialog and Remove exactly for the marker (the old
/// "uninstall and Update planning refusals open framed dialogs", and the reference screen `host-uninstall-refusal`).
/// Specified: a refused plan opens "Uninstall Farhelm · build-box" with the helm's words and Close focused, no Remove,
/// and nothing under the row; a refusal marked "remove instead" offers Remove, which opens Remove host's confirmation.
#[gpui::test]
fn a_refused_plan_opens_the_refusal_dialog_with_remove_only_when_marked(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let plan = choose_uninstall(&mut helm, cx);
    helm.reply(plan, 409, Some(json!(SESSIONS)));
    settle(cx);
    assert_eq!(
        label(cx, "host-refusal-title").as_deref(),
        Some("Uninstall Farhelm · build-box")
    );
    assert_eq!(label(cx, "host-refusal-reason").as_deref(), Some(SESSIONS));
    assert_eq!(focused_id(cx).as_deref(), Some("host-refusal-done"));
    assert_eq!(
        label(cx, "host-refusal-remove"),
        None,
        "no Remove without the marker"
    );
    assert_eq!(
        label(cx, "host-error-2"),
        None,
        "the dialog showed it, so the row does not"
    );
    keys(cx, "escape");

    let plan = choose_uninstall(&mut helm, cx);
    helm.reply_marked(
        plan,
        409,
        vec![HelmMarker::RemoveHostInstead],
        Some(json!(
            "the host's identity changed; remove it from the list instead"
        )),
    );
    settle(cx);
    assert_eq!(label(cx, "host-refusal-remove").as_deref(), Some("Remove"));
    click(cx, "host-refusal-remove");
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Remove host · build-box")
    );
}

/// SPEC.md, "Keyboard focus": a result that arrives after the user moved on opens no dialog and shows where it belongs
/// (the goal's "No queue for results that arrive later"). Specified: after Uninstall and a key press, a refused plan
/// opens nothing and stays under `build-box`'s row; a plan that arrives after the next press opens no confirmation
/// either, and the row says to choose Uninstall again ([`MOVED_ON`]).
#[gpui::test]
fn results_after_the_user_moved_on_open_nothing_and_show_under_the_row(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let plan = choose_uninstall(&mut helm, cx);
    keys(cx, "shift-tab");
    helm.reply(plan, 409, Some(json!(SESSIONS)));
    settle(cx);
    assert!(!any_open(cx), "no refusal dialog");
    assert_eq!(label(cx, "host-error-2").as_deref(), Some(SESSIONS));

    let plan = choose_uninstall(&mut helm, cx);
    keys(cx, "shift-tab");
    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    assert!(!any_open(cx), "no confirmation");
    assert_eq!(label(cx, "host-error-2").as_deref(), Some(MOVED_ON));
}

/// Why this matters: the old page kept a plain-text renderer for planning replies from older helms that sent the plan's
/// text without its steps (`provisioning.spec.ts`, "unstructured ${operation} plan keeps a named keyboard entry"), and
/// gave that body its own named keyboard stop. This app's helm is the one built with it and always sends the steps, so
/// the confirmation never shows a plan without them; such a reply must open no confirmation built from nothing, and
/// must still say what went wrong where the host's refusals show. Specified: a 200 plan reply with its probe id, plan
/// and text but no steps opens nothing, and `build-box`'s row says the helm's answer to the uninstall request could not
/// be read.
#[gpui::test]
fn a_plan_without_its_steps_opens_nothing_and_says_it_could_not_be_read(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let plan = choose_uninstall(&mut helm, cx);
    helm.reply(
        plan,
        200,
        Some(json!({
            "probe_id": "un-1",
            "plan": { "operation": "uninstall", "state_dir": KEPT },
            "confirmation": "Farhelm will uninstall from dev@build-box (ubuntu, x86_64):\n",
        })),
    );
    settle(cx);
    assert!(!any_open(cx), "no confirmation");
    let line = label(cx, "host-error-2").unwrap_or_default();
    assert!(
        line.starts_with(
            "the helm's answer to POST /api/hosts/[host]/uninstall could not be read: "
        ),
        "{line}"
    );
}

/// The confirmation freezes the host as the registry lists it when the plan arrives, not as it was at the press, since
/// up to the helm's host-wait bound can pass between them; and a plan for a host that left the list meanwhile has
/// nowhere to show. Specified: with `build-box` renamed `builder` while its plan request is out, the confirmation is
/// titled "Uninstall Farhelm · builder"; and when the registry drops the host before the plan arrives, nothing opens
/// and nothing is recorded under its key.
#[gpui::test]
fn the_plan_shows_the_host_as_listed_when_it_arrives(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let plan = choose_uninstall(&mut helm, cx);
    let renamed =
        crate::hosts::tests::aliased(reference_hosts().remove(1), "builder", "dev@build-box");
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1", "2", "3"]),
        upsert: vec![KeyedEntry {
            key: "2".into(),
            value: serde_json::to_value(&renamed).unwrap(),
        }],
        remove: Vec::new(),
    });
    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Uninstall Farhelm · builder")
    );
    keys(cx, "escape");

    let plan = choose_uninstall(&mut helm, cx);
    push_forgotten(&mut helm);
    helm.reply(plan, 200, Some(plan_reply()));
    settle(cx);
    assert!(!any_open(cx), "no confirmation for a host that is gone");
    let recorded = cx.update(|_, cx| {
        session::current_operations(cx)
            .and_then(|operations| operations.read(cx).failure(&OpKey::host("2")).cloned())
    });
    assert!(
        recorded.is_none(),
        "no moved-on note under a row that no longer exists"
    );
}

/// The old "confirmed uninstall refusal reopens its dialog": a one-use plan can be spent or expire after the user read
/// it, and its definite refusal must be seen, while the notice that confirming owed is withdrawn because no run was
/// accepted. Specified: a refused confirmation opens "Uninstall Farhelm · build-box" with the helm's words through the
/// Uninstall answer's mark and shows nothing under the row; the registry dropping the host afterwards shows no notice.
#[gpui::test]
fn a_refused_confirmation_opens_the_refusal_and_withdraws_the_notice(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let confirm = confirm_uninstall(&mut helm, cx);
    helm.reply(confirm, 409, Some(json!(USED)));
    settle(cx);
    assert_eq!(label(cx, "host-refusal-reason").as_deref(), Some(USED));
    assert_eq!(label(cx, "host-error-2"), None);
    keys(cx, "escape");
    push_forgotten(&mut helm);
    settle(cx);
    assert_eq!(
        label(cx, "uninstall-notice-summary"),
        None,
        "nothing was owed any more"
    );
}

/// The old "confirmed uninstall refusal survives a vanished host row": an agent or the command line can remove the
/// host while the confirmation is out, and the helm then refuses it. The refusal still belongs to the host the user
/// confirmed, and since Farhelm was not removed from it, no notice may say it was, although the host did leave the
/// list. Specified: with the confirmation out, the registry drops `build-box` (the notice it owed shows for now); the
/// helm's refusal then opens the refusal dialog naming `build-box` and its destination from the frozen plan, and the
/// notice goes.
#[gpui::test]
fn a_refusal_after_the_host_vanished_still_opens_and_takes_the_notice_back(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = panel(cx);
    let confirm = confirm_uninstall(&mut helm, cx);
    push_forgotten(&mut helm);
    settle(cx);
    assert_eq!(
        label(cx, "host-row-2"),
        None,
        "the premise: the row is gone"
    );
    helm.reply(confirm, 409, Some(json!("host 2 is not registered")));
    settle(cx);
    assert_eq!(
        label(cx, "host-refusal-title").as_deref(),
        Some("Uninstall Farhelm · build-box")
    );
    assert_eq!(
        label(cx, "host-refusal-reason").as_deref(),
        Some("host 2 is not registered")
    );
    keys(cx, "escape");
    assert_eq!(
        label(cx, "uninstall-notice-summary"),
        None,
        "the refused uninstall removed nothing"
    );
}

// ---- A failed run ----------------------------------------------------------------------------------------------------

/// SPEC.md, "Topology": "A run that fails partway keeps the host listed with its steps showing what is left, and
/// choosing uninstall again continues from there." The notice belongs to a run that removed the host, so a failed run
/// withdraws it, and a continuation, confirmed while the row still shows the failed run, owes a new one that the old
/// failure must not withdraw (the old app's `superseded_run`). Specified: a failed run opens the row on "Uninstall
/// provisioning Failed" and keeps Uninstall available in the menu; a continuation confirmed over the failed run, with
/// another change pushed while the store still shows that failure, then its run pushed and accepted, shows the notice
/// once the host is gone.
#[gpui::test]
fn a_failed_run_withdraws_the_notice_and_uninstall_continues_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let confirm = confirm_uninstall(&mut helm, cx);
    push_run(&mut helm, &uninstall_run("run-1", RunStatus::Running));
    helm.reply(
        confirm,
        202,
        Some(json!({ "host_id": 2, "run_id": "run-1" })),
    );
    settle(cx);
    push_run(&mut helm, &uninstall_run("run-1", RunStatus::Failed));
    settle(cx);
    assert_eq!(
        label(cx, "host-run-title-2").as_deref(),
        Some("Uninstall provisioning Failed")
    );

    let confirm = confirm_uninstall(&mut helm, cx);
    // A change the store takes while the continuation is out and `build-box` still retains the failed run-1: the
    // notice must survive the failure it was owed over. Another host's run is such a change, as any push would be.
    let mut other = run(3, ProvisioningOperation::Add, RunStatus::Completed);
    other.run_id = Some("run-3".into());
    helm.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&["2", "3"]),
        upsert: vec![KeyedEntry {
            key: "3".into(),
            value: serde_json::to_value(&other).unwrap(),
        }],
        remove: Vec::new(),
    });
    settle(cx);
    assert_eq!(
        label(cx, "host-run-title-2").as_deref(),
        Some("Uninstall provisioning Failed"),
        "the premise: the store still shows the failed run while the continuation is owed its notice"
    );
    // The helm's order: the run is pushed before the reply that accepts it.
    push_run(&mut helm, &uninstall_run("run-2", RunStatus::Running));
    helm.reply(
        confirm,
        202,
        Some(json!({ "host_id": 2, "run_id": "run-2" })),
    );
    settle(cx);
    assert_eq!(label(cx, "uninstall-notice-summary"), None);
    push_forgotten(&mut helm);
    settle(cx);
    assert_eq!(
        label(cx, "uninstall-notice-summary").as_deref(),
        Some(NOTICE)
    );
}

/// The withdrawal half of the rule above on its own: a host whose uninstall failed and that then leaves the list some
/// other way (a Remove from the command line) did not have Farhelm removed, so no notice may claim it was. Specified:
/// after a failed run, the registry dropping the host shows no notice.
#[gpui::test]
fn a_host_gone_after_a_failed_run_shows_no_notice(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let confirm = confirm_uninstall(&mut helm, cx);
    push_run(&mut helm, &uninstall_run("run-1", RunStatus::Running));
    helm.reply(
        confirm,
        202,
        Some(json!({ "host_id": 2, "run_id": "run-1" })),
    );
    settle(cx);
    push_run(&mut helm, &uninstall_run("run-1", RunStatus::Failed));
    settle(cx);
    push_forgotten(&mut helm);
    settle(cx);
    assert_eq!(label(cx, "uninstall-notice-summary"), None);
}

// ---- A reset ---------------------------------------------------------------------------------------------------------

/// SPEC_impl.md, "One failure path: the reset": a reset drops everything that came from the helm, and the old app's
/// reload forgot its notices too. The notices live in an app global, which a reset does not clear by itself, so they
/// are tagged with the UI session's operation store; this pins that tag. Specified: a shown notice is gone once the
/// channel breaks and the helm connects again; and a notice owed over a confirmation accepted before a reset is not
/// shown when the new UI session's registry no longer lists the host.
#[gpui::test]
fn a_reset_forgets_a_shown_notice_and_an_owed_one(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let confirm = confirm_uninstall(&mut helm, cx);
    push_run(&mut helm, &uninstall_run("run-1", RunStatus::Running));
    helm.reply(
        confirm,
        202,
        Some(json!({ "host_id": 2, "run_id": "run-1" })),
    );
    settle(cx);
    push_forgotten(&mut helm);
    settle(cx);
    assert_eq!(
        label(cx, "uninstall-notice-summary").as_deref(),
        Some(NOTICE),
        "the premise: the notice shows"
    );
    helm.break_channel();
    settle(cx);
    helm.connect(vec![hosts(&reference_hosts()), progress(&[])]);
    settle(cx);
    assert_eq!(
        label(cx, "uninstall-notice-summary"),
        None,
        "the reset forgot the shown notice"
    );

    let confirm = confirm_uninstall(&mut helm, cx);
    push_run(&mut helm, &uninstall_run("run-2", RunStatus::Running));
    helm.reply(
        confirm,
        202,
        Some(json!({ "host_id": 2, "run_id": "run-2" })),
    );
    settle(cx);
    helm.break_channel();
    settle(cx);
    let mut listed = reference_hosts();
    listed.remove(1);
    helm.connect(vec![hosts(&listed), progress(&[])]);
    settle(cx);
    assert_eq!(
        label(cx, "host-row-2"),
        None,
        "the premise: the new session lists no build-box"
    );
    assert_eq!(
        label(cx, "uninstall-notice-summary"),
        None,
        "the reset forgot the owed notice"
    );
}

// ---- The kind of work ------------------------------------------------------------------------------------------------

/// The operation store's two cross-area sets decide what refuses what: a launch refuses while host registry work is in
/// flight, and host registry work refuses while a launch is. An uninstall forgets its host, so it is registry work, and
/// it starts no session (decided with the migration's rule of 2026-11-14). The rail's live work is a separate question:
/// an uninstall's requests are a plan the user may still cancel and a confirmation answered with the run already pushed,
/// so only its run shows as working (`hosts::status::live`). Specified: [`Work::UninstallHost`] changes the host
/// registry and creates no session, and with only its requests in flight a host shows no live work.
#[test]
fn an_uninstall_is_registry_work_and_only_its_run_is_live_work() {
    assert!(Work::UninstallHost.changes_host_registry());
    assert!(!Work::UninstallHost.creates_a_session());
    assert_eq!(
        crate::hosts::status::live(None, Some(Work::UninstallHost)),
        None
    );
}

// ---- During a launch -------------------------------------------------------------------------------------------------

/// The migration's rule (2026-11-14): a host registry change refuses to start while a session launch is in flight,
/// with a reason, and uninstall forgets the host it removes. Specified: with a launcher New in flight, the menu's
/// Uninstall sends nothing, opens nothing and the Hosts panel's line says "Uninstall build-box: not started, …"; once
/// the launch settles, Uninstall sends its plan request and the line goes.
#[gpui::test]
fn a_launch_in_flight_refuses_uninstall_before_sending(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx);
    let launch = start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-uninstall");
    assert_eq!(helm.commands(), []);
    assert!(!any_open(cx));
    let line = format!("Uninstall build-box: {LAUNCH_IN_FLIGHT}");
    assert_eq!(
        label(cx, "hosts-uninstall-not-started").as_deref(),
        Some(line.as_str())
    );

    helm.reply(launch, 201, Some(json!({})));
    settle(cx);
    choose_uninstall(&mut helm, cx);
    assert_eq!(label(cx, "hosts-uninstall-not-started"), None);
}
