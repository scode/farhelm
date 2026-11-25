//! Tests of a sandbox's and a container's own host actions over the helm client's fake: Delete sandbox and Delete
//! container (which always ask), Retry on a failed deletion (which deletes again), a sandbox's Re-run of its failed
//! setup (the creation question, or none under Tensorlake's own skip), the deletion and rerun actions each refusing
//! while a session launch is in flight, and Resume (which never asks) and Suspend (which asks unless every session
//! there is known idle), which a launch elsewhere does not hold.
//!
//! Every test starts from the Hosts panel's harness (`hosts/tests.rs`) over the reference hosts plus a Tensorlake
//! sandbox `gpubox` (4) and a container `crate-1` (5), in a column of the sidebar's width and a window as large as the
//! reference screens'. No test reaches a real provider: the fake helm answers every request.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus};
use farhelm_proto::recipes::RecipeSize;
use farhelm_proto::ui_channel::{
    Collection, CollectionState, HostStateView, HostView, KeyedEntry, PREFERENCES_KEY, RefreshView,
    SessionList, TensorlakeHostView,
};
use gpui::{TestAppContext, VisualTestContext, px, size};
use serde_json::{Value, json};

use crate::hosts::LAUNCH_IN_FLIGHT;
use crate::hosts::tests::{
    connected, connecting, host, hosts, open_panel_in_column, progress, reference_hosts, run,
    start_work,
};
use crate::launcher::launch_tests::launch_to;
use crate::operations::{OpKey, Work};
use crate::session;
use crate::session_list::environment_tests::{bash_session, idle, listing};
use crate::session_list::host_rail::tests::hover_help;
use crate::session_list::tests::{Fixture, session};
use crate::test_support::{
    any_open, click, focused_id, frame, keys, label, meta, one_command, settle,
};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The Tensorlake sandbox `gpubox` (host 4) in `state`, created from `rust-dev` at 2 CPUs and 4 GB.
fn sandbox(state: HostStateView) -> HostView {
    let mut view = host(4, "tensorlake", state);
    view.name = "gpubox".into();
    view.alias = Some("gpubox".into());
    view.destination = Some("farhelm-host-k7q2@farhelm-host-k7q2.sandbox.invalid".into());
    view.tensorlake = Some(TensorlakeHostView {
        sandbox: "farhelm-host-k7q2".into(),
        recipe: "rust-dev".into(),
        size: Some(RecipeSize {
            cpus: 2,
            memory_gb: 4,
            disk_gb: 20,
        }),
        tunnels: None,
        credential_warning: None,
    });
    view
}

/// The container `crate-1` (host 5), connected.
fn container() -> HostView {
    let mut view = host(5, "podman", connected());
    view.name = "crate-1".into();
    view
}

/// The preference row with both "without asking" preferences set to `on`: removing hosts and setting them up.
fn skip_everything(on: bool) -> CollectionState {
    CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: json!({
                "skip_host_remove_confirmation": on,
                "skip_host_setup_confirmation": on,
            }),
        }],
    }
}

/// Whether the store holds both "without asking" preferences on, as [`skip_everything`] sets them: the premise of the
/// tests that show a question is asked anyway.
fn skips_both(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| {
        let preferences = session::current_store(cx).and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<farhelm_proto::ui_channel::Preferences>(PREFERENCES_KEY)
                .cloned()
        });
        preferences.is_some_and(|preferences| {
            preferences.skip_host_remove_confirmation == Some(true)
                && preferences.skip_host_setup_confirmation == Some(true)
        })
    })
}

/// The panel over the reference hosts, `gpubox` in `state`, `crate-1`, and `more`.
fn panel(
    cx: &mut TestAppContext,
    state: HostStateView,
    more: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let mut listed = reference_hosts();
    listed.push(sandbox(state));
    listed.push(container());
    let mut snapshot = vec![hosts(&listed)];
    snapshot.extend(more);
    let (helm, _, cx) = open_panel_in_column(cx, snapshot);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    settle(cx);
    (helm, cx)
}

/// Push `gpubox` in `state` as the helm does when its phase changes.
fn push_sandbox(helm: &mut FakeHelm, state: HostStateView) {
    let view = sandbox(state);
    helm.push(farhelm_proto::ui_channel::CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1", "2", "3", "4", "5"]),
        upsert: vec![KeyedEntry {
            key: HostView::key(view.id),
            value: serde_json::to_value(&view).expect("a host view encodes"),
        }],
        remove: Vec::new(),
    });
}

/// The labels of the open question's answers, in their drawn order.
fn answers(cx: &mut VisualTestContext) -> Vec<String> {
    frame(cx)
        .into_iter()
        .filter(|node| {
            node.element_id.as_deref().is_some_and(|id| {
                matches!(
                    id,
                    "prompt-cancel" | "prompt-alternative-0" | "prompt-confirm"
                )
            })
        })
        .filter_map(|node| node.label.map(|label| label.to_string()))
        .collect()
}

/// Every text run of a fresh frame joined, whitespace collapsed.
fn painted(cx: &mut VisualTestContext) -> String {
    frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The work in flight on host `id`'s key.
fn work(cx: &mut VisualTestContext, id: &str) -> Option<Work> {
    cx.update(|_, cx| {
        session::current_operations(cx)?
            .read(cx)
            .in_flight(&OpKey::host(id))
    })
}

/// The helm's Tensorlake creation plan for `gpubox`'s rerun, under the one-use id `probe_id`.
fn plan(probe_id: &str) -> Value {
    json!({
        "result": "tensorlake-provisionable",
        "probe_id": probe_id,
        "sandbox": "farhelm-host-k7q2",
        "destination": "farhelm-host-k7q2",
        "recipe": "rust-dev",
        "resources": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "confirmation": "Create a Tensorlake sandbox\n1. Create the sandbox\n",
        "steps": {
            "lead": "Create a Tensorlake sandbox",
            "kept": null,
            "steps": [{ "id": "create-sandbox", "name": "Create the sandbox", "paths": [], "explanation": null }],
        },
    })
}

/// The Tensorlake account as `GET /api/tensorlake` answers it, with "Create Tensorlake sandboxes without asking" set to
/// `skips`.
fn account(skips: bool) -> Value {
    json!({ "api_key_set": true, "create_without_asking": skips, "helm_tag": "fixture",
            "ssh_key": { "state": "not_created" } })
}

// ---- A machine's forwards ------------------------------------------------------------------------------------------

/// Why this matters: the old spec checked the row's status turn amber for a connected sandbox whose forwards failed
/// ("tunnel failures make a connected host amber"), since agents may work while their inference endpoint is
/// unreachable; the ink actually painted is what is tested, not the function that picks it. Specified: `gpubox`,
/// connected with its forwards failed, paints its row's status dot in the palette's amber (`warn`) and not its green
/// (`ok`), while its words stay "Connected".
#[gpui::test]
fn failed_forwards_paint_the_rows_status_amber(cx: &mut TestAppContext) {
    use farhelm_proto::tensorlake::{TunnelState, TunnelView};
    let mut gpubox = sandbox(connected());
    if let Some(binding) = gpubox.tensorlake.as_mut() {
        binding.tunnels = Some(TunnelView {
            tunnels: Vec::new(),
            state: TunnelState::Failed {
                reason: "listen port taken".into(),
            },
        });
    }
    let mut listed = reference_hosts();
    listed.push(gpubox);
    let (_helm, _, cx) = open_panel_in_column(cx, vec![hosts(&listed)]);
    cx.simulate_resize(size(px(1600.), px(1000.)));
    settle(cx);
    assert_eq!(label(cx, "host-status-4").as_deref(), Some("Connected"));
    let status = frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some("host-status-4"))
        .expect("the row's status is drawn")
        .bounds;
    let palette = cx.update(|_, cx| crate::theme::palette(cx));
    let (scale, quads) = cx.update(|window, _| (window.scale_factor(), window.painted_quads()));
    let status = status.scale(scale);
    let inks: Vec<_> = quads
        .into_iter()
        .filter(|quad| {
            quad.bounds.origin.x >= status.origin.x
                && quad.bounds.origin.y >= status.origin.y
                && quad.bounds.bottom_right().x <= status.bottom_right().x
                && quad.bounds.bottom_right().y <= status.bottom_right().y
        })
        .map(|quad| quad.background)
        .collect();
    assert!(
        inks.contains(&gpui::Background::from(crate::theme::color(palette.warn))),
        "the dot paints amber"
    );
    assert!(
        !inks.contains(&gpui::Background::from(crate::theme::color(palette.ok))),
        "and not green"
    );
}

// ---- A machine's mark ----------------------------------------------------------------------------------------------

/// Why this matters: "Remote: on another machine, reached over SSH" is false for a container on this machine, and the
/// session rows in a sandbox or container already name its mark by the provider's word; a real container run found the
/// Hosts panel calling the same machine "Remote". The help describes the machine, not a session in it, as the session
/// rows' does. Specified: `gpubox`'s mark in its Hosts row is named "Sandbox" with the help "Sandbox: a Tensorlake
/// sandbox, reached over SSH", and `crate-1`'s is named "Container" with "Container: a container on one of your
/// container hosts".
#[gpui::test]
fn a_machines_mark_is_named_by_its_provider(cx: &mut TestAppContext) {
    let (_helm, cx) = panel(cx, connected(), Vec::new());
    assert_eq!(label(cx, "host-mark-4").as_deref(), Some("Sandbox"));
    assert_eq!(
        hover_help(cx, "host-mark-4").as_deref(),
        Some("Sandbox: a Tensorlake sandbox, reached over SSH")
    );
    assert_eq!(label(cx, "host-mark-5").as_deref(), Some("Container"));
    assert_eq!(
        hover_help(cx, "host-mark-5").as_deref(),
        Some("Container: a container on one of your container hosts")
    );
}

// ---- Delete sandbox and Delete container ---------------------------------------------------------------------------

/// Why this matters: deleting a sandbox destroys every session, file and snapshot in it and cannot be undone, so it
/// always asks, whatever "Remove hosts without asking" says (that preference covers forgetting an entry), and the
/// question names the sandbox the user can look up and what goes with it (the old `RemoveCopy::for_host`).
///
/// Specified: with "Remove hosts without asking" on, `gpubox`'s menu offers "Delete sandbox" (described "Terminate this
/// Tensorlake sandbox and delete its snapshots") and no Remove; choosing it asks "Delete sandbox · gpubox", whose body
/// names `farhelm-host-k7q2` and says "Delete the whole sandbox? … This cannot be undone.", with only Cancel and Delete
/// sandbox, and sends nothing; Delete sandbox then sends `DELETE /api/hosts/4` under the host's key.
#[gpui::test]
fn delete_sandbox_always_asks_naming_the_sandbox_and_what_goes_with_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), vec![skip_everything(true)]);
    assert!(
        skips_both(cx),
        "the premise: removing and setting up hosts without asking are on"
    );
    click(cx, "host-menu-toggle-4");
    assert_eq!(
        label(cx, "host-menu-delete").as_deref(),
        Some("Delete sandbox")
    );
    assert!(
        painted(cx).contains("Terminate this Tensorlake sandbox and delete its snapshots"),
        "the item says what it removes"
    );
    assert_eq!(label(cx, "host-menu-remove"), None, "no Remove beside it");
    click(cx, "host-menu-delete");
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Delete sandbox · gpubox"),
        "it asked, though removing hosts without asking is on"
    );
    let text = painted(cx);
    for shown in [
        "farhelm-host-k7q2",
        "Delete the whole sandbox? Every session, running process, file, checkout, unpushed change and snapshot in \
         it will be permanently removed. This cannot be undone.",
    ] {
        assert!(text.contains(shown), "{shown:?} is drawn: {text}");
    }
    assert_eq!(answers(cx), ["Cancel", "Delete sandbox"]);
    assert!(helm.commands().is_empty(), "nothing before an answer");
    click(cx, "prompt-confirm");
    let (_, body) = one_command(&mut helm, Method::Delete, "/api/hosts/4");
    assert_eq!(body, Value::Null);
    assert_eq!(work(cx, "4"), Some(Work::RemoveHost));
}

/// Why this matters: the helm answers a deletion within its reply bound, and one still running then is refused with
/// "the removal carries on"; that is an ordinary refusal of this attempt, shown where any refusal of the question
/// shows, while the pushed hosts list shows how the deletion ends.
///
/// Specified: a 504 to Delete sandbox keeps the question up with "Farhelm refused this action:" and the helm's words.
#[gpui::test]
fn a_deletion_the_helm_answers_at_its_bound_shows_in_the_question(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    click(cx, "host-menu-toggle-4");
    click(cx, "host-menu-delete");
    click(cx, "prompt-confirm");
    let (id, _) = one_command(&mut helm, Method::Delete, "/api/hosts/4");
    let words = "removing the host did not finish within 20 seconds; the removal carries on, and the hosts list will \
                 show the result";
    helm.reply(id, 504, Some(json!(words)));
    settle(cx);
    assert!(any_open(cx), "the question stays up");
    assert_eq!(
        label(cx, "host-remove-refused"),
        Some(format!("Farhelm refused this action: {words}"))
    );
}

/// Why this matters: a container's deletion is the same command in the container's own words, which come from one
/// exhaustive per-provider module rather than a comparison of providers here.
///
/// Specified: `crate-1`'s menu offers "Delete container", and its question reads "Delete container · crate-1" with
/// "Delete the whole container? … This cannot be undone." and the answer "Delete container".
#[gpui::test]
fn a_containers_menu_offers_delete_container_in_its_own_words(cx: &mut TestAppContext) {
    let (_helm, cx) = panel(cx, connected(), Vec::new());
    click(cx, "host-menu-toggle-5");
    assert_eq!(
        label(cx, "host-menu-delete").as_deref(),
        Some("Delete container")
    );
    click(cx, "host-menu-delete");
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Delete container · crate-1")
    );
    assert!(painted(cx).contains(
        "Delete the whole container? Every session, running process, file, checkout and unpushed change in it will \
         be permanently removed. This cannot be undone."
    ));
    assert_eq!(answers(cx), ["Cancel", "Delete container"]);
}

/// Why this matters: SPEC.md, "Opening and closing": "A risky action that cannot be undone starts on Cancel", and a
/// machine's deletion destroys everything in it. The old dialog started on its Delete, so a habitual Enter deleted the
/// machine; this pins SPEC's start for every provider, where an SSH host's Remove, which only forgets the entry, keeps
/// focus on Remove (`hosts/remove/tests.rs`, `remove_asks_naming_the_host_its_destination_and_what_removal_keeps`).
///
/// Specified: for `gpubox` (Delete sandbox) and `crate-1` (Delete container) in turn, the menu's Delete opens the
/// question with focus on Cancel, and Enter closes it with nothing sent.
#[gpui::test]
fn a_machines_deletion_starts_on_cancel_and_enter_keeps_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    // The selectors are literal because the test bridge's click takes a `'static` selector.
    for (id, toggle) in [("4", "host-menu-toggle-4"), ("5", "host-menu-toggle-5")] {
        click(cx, toggle);
        click(cx, "host-menu-delete");
        assert!(any_open(cx), "host {id}: the question opened");
        // Confirm is enabled here, since the menu's Delete and the question's confirm read the same offer, so the start
        // on Cancel is the deletion's rule and not the one for a disabled confirm, which would also start there.
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("prompt-cancel"),
            "host {id}: focus starts on Cancel"
        );
        keys(cx, "enter");
        assert!(!any_open(cx), "host {id}: Enter answered Cancel");
        assert!(helm.commands().is_empty(), "host {id}: nothing was sent");
    }
}

/// Why this matters: a deletion is a host registry change, which the migration keeps out of the way of a session
/// launch, with a reason rather than a disabled item.
///
/// Specified: with a launcher `Launch` in flight, Delete sandbox's answer sends nothing and the question says "Delete
/// sandbox gpubox: not started, a session launch is in progress; try again when it finishes".
#[gpui::test]
fn delete_sandbox_refuses_during_a_launch(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    click(cx, "host-menu-toggle-4");
    click(cx, "host-menu-delete");
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    click(cx, "prompt-confirm");
    assert!(helm.commands().is_empty(), "no deletion was sent");
    assert_eq!(
        label(cx, "host-remove-not-started"),
        Some(format!("Delete sandbox gpubox: {LAUNCH_IN_FLIGHT}"))
    );
}

// ---- Retry on a failed deletion ------------------------------------------------------------------------------------

/// Why this matters: a failed deletion may leave a sandbox and its snapshots billing, and its remedy reads "Retry to
/// delete it again", so the menu's Retry must delete again rather than reconnect to a sealed Farhelm; the helm answers
/// it within its reply bound, so a deletion still running then is an ordinary refusal of this attempt.
///
/// Specified: on `gpubox` shown "Deleting failed", the row's details say Farhelm could not finish deleting the sandbox
/// and its snapshots with the provider's reason; Retry is described "Finish the deletion" and sends
/// `POST /api/hosts/4/retry-delete` under the host's key; a 504 opens "Retry host · gpubox" with the helm's words.
#[gpui::test]
fn retry_on_a_failed_deletion_deletes_again_and_a_late_answer_is_a_refusal(
    cx: &mut TestAppContext,
) {
    let failed = HostStateView::Deleting {
        failure: Some("Tensorlake refused the termination".into()),
    };
    let (mut helm, cx) = panel(cx, failed, Vec::new());
    click(cx, "host-details-toggle");
    // The machine's line is the row's first detail, the destination behind the alias next; the deletion's evidence
    // follows them.
    assert_eq!(
        label(cx, "host-detail-4-2").as_deref(),
        Some(
            "Farhelm could not finish deleting this sandbox and its snapshots; some of them may remain and bill. \
             Retry finishes the deletion: Tensorlake refused the termination"
        )
    );
    click(cx, "host-menu-toggle-4");
    assert!(painted(cx).contains("Finish the deletion"));
    click(cx, "host-menu-retry");
    let (id, body) = one_command(&mut helm, Method::Post, "/api/hosts/4/retry-delete");
    assert_eq!(body, Value::Null);
    assert_eq!(work(cx, "4"), Some(Work::RetryHost));
    // The helm's own words for a retry still running at its bound (`retry_delete_carries_on`).
    let words = "retrying the deletion did not finish within 20 seconds; it carries on, and the host's state in the \
                 hosts list will show the result";
    helm.reply(id, 504, Some(json!(words)));
    settle(cx);
    assert_eq!(
        label(cx, "host-refusal-title").as_deref(),
        Some("Retry host · gpubox")
    );
    assert_eq!(label(cx, "host-refusal-reason").as_deref(), Some(words));
}

// ---- A sandbox's Re-run --------------------------------------------------------------------------------------------

/// Open `gpubox`'s menu with its setup failed and choose Re-run; the probe it sends, checked to name the sandbox's
/// host, and its envelope id.
fn rerun_probe(cx: &mut VisualTestContext, helm: &mut FakeHelm) -> u64 {
    click(cx, "host-menu-toggle-4");
    click(cx, "host-menu-rerun");
    let (id, body) = one_command(helm, Method::Post, "/api/hosts/probe");
    assert_eq!(
        body,
        json!({ "target": { "kind": "tensorlake-host", "host": 4 } })
    );
    id
}

/// The panel with `gpubox`'s setup failed, and `more`.
fn failed_setup(
    cx: &mut TestAppContext,
    more: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let mut more = more;
    more.push(progress(&[run(
        4,
        ProvisioningOperation::Add,
        RunStatus::Failed,
    )]));
    panel(cx, connected(), more)
}

/// Why this matters: running a sandbox's failed setup again creates it again, which bills, so it asks the sandbox's
/// creation question with the size it bills for, and only Tensorlake's own "Create Tensorlake sandboxes without asking"
/// may skip that question: the SSH setup preference never stands in for it. A rerun has no form to go back to, so the
/// question has no Back.
///
/// Specified: with "Set up new hosts without asking" on, Re-run on `gpubox` probes `{"kind": "tensorlake-host", "host":
/// 4}`; its plan reads the Tensorlake account, and with that account's skip off asks "Create Tensorlake sandbox ·
/// gpubox" with "2 CPUs · 4 GB memory · 20 GB disk", the billing callout, and only Cancel and Create sandbox; Create
/// sandbox sends `POST /api/hosts/provision` with the plan's id under the host's key.
#[gpui::test]
fn a_sandboxs_rerun_asks_its_creation_question_without_back(cx: &mut TestAppContext) {
    let (mut helm, cx) = failed_setup(cx, vec![skip_everything(true)]);
    assert!(
        skips_both(cx),
        "the premise: removing and setting up hosts without asking are on"
    );
    let probe = rerun_probe(cx, &mut helm);
    helm.reply(probe, 200, Some(plan("probe-1")));
    settle(cx);
    let (read, _) = one_command(&mut helm, Method::Get, "/api/tensorlake");
    helm.reply(read, 200, Some(account(false)));
    settle(cx);
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Create Tensorlake sandbox · gpubox")
    );
    let text = painted(cx);
    for shown in [
        "2 CPUs · 4 GB memory · 20 GB disk",
        "This sandbox bills while running.",
    ] {
        assert!(text.contains(shown), "{shown:?} is drawn: {text}");
    }
    assert_eq!(answers(cx), ["Cancel", "Create sandbox"]);
    assert!(helm.commands().is_empty(), "nothing before an answer");
    click(cx, "prompt-confirm");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "probe-1" }));
    assert_eq!(work(cx, "4"), Some(Work::Setup));
}

/// Why this matters: the Tensorlake skip is the user's own choice to create sandboxes without a question, and a read
/// of it that fails must not be taken for that choice.
///
/// Specified: with the account's skip on, the rerun's plan is sent as it arrives, with no question; with the account
/// read refused, the question is asked.
#[gpui::test]
fn the_tensorlake_skip_creates_without_asking_and_a_failed_read_asks(cx: &mut TestAppContext) {
    let (mut helm, cx) = failed_setup(cx, Vec::new());
    let probe = rerun_probe(cx, &mut helm);
    helm.reply(probe, 200, Some(plan("probe-1")));
    settle(cx);
    let (read, _) = one_command(&mut helm, Method::Get, "/api/tensorlake");
    helm.reply(read, 200, Some(account(true)));
    settle(cx);
    let (create, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "probe-1" }));
    assert!(!any_open(cx), "no question");
    // The helm refuses that submission, which leaves the failed setup and its Re-run as they were; this time the
    // account cannot be read.
    helm.reply(create, 409, Some(json!("the plan is unknown")));
    settle(cx);
    let probe = rerun_probe(cx, &mut helm);
    helm.reply(probe, 200, Some(plan("probe-2")));
    settle(cx);
    let (read, _) = one_command(&mut helm, Method::Get, "/api/tensorlake");
    helm.reply(read, 500, Some(json!("the settings could not be read")));
    settle(cx);
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Create Tensorlake sandbox · gpubox"),
        "a failed read asks"
    );
    assert!(helm.commands().is_empty(), "nothing was submitted");
}

// ---- Resume --------------------------------------------------------------------------------------------------------

/// `gpubox` suspended outside Farhelm.
fn suspended() -> HostStateView {
    HostStateView::Suspended {
        observed_at: 0,
        suspended_by_farhelm_at: None,
        resume_failed: None,
    }
}

/// `gpubox` resuming, asked at the epoch.
fn resuming() -> HostStateView {
    HostStateView::TensorlakeChanging {
        starting: false,
        direction: farhelm_proto::tensorlake::LifecycleDirection::Resuming,
        requested_at: 0,
    }
}

/// Why this matters: a suspended sandbox is out of reach until it resumes, and the old row put its Resume inline,
/// where the user sees the phase, besides the menu. A Tensorlake Resume routinely outlasts the helm's 20-second reply
/// bound, so the 504 that says it carries on must not read as a failure. The request is the host's, on its key.
///
/// Specified: suspended `gpubox`'s row draws "Resume", named "Resume gpubox"; pressing it sends
/// `POST /api/hosts/4/resume` with no body and no question, as `Resume` work on the host's key, the row's status reads
/// "Resuming…" meanwhile beside its Resume, drawn unavailable, and focus moves to the row's menu toggle; the helm's 504
/// opens nothing and leaves no record under the host's key; pushed resuming, the row's status still reads "Resuming…"
/// and its Resume shows unavailable.
#[gpui::test]
fn the_rows_resume_sends_the_hosts_resume_and_a_late_answer_is_no_failure(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, suspended(), Vec::new());
    assert_eq!(label(cx, "host-resume-4").as_deref(), Some("Resume gpubox"));
    click(cx, "host-resume-4");
    let (id, body) = one_command(&mut helm, Method::Post, "/api/hosts/4/resume");
    assert_eq!(body, Value::Null);
    assert!(!any_open(cx), "Resume never asks");
    assert_eq!(work(cx, "4"), Some(Work::Resume));
    assert_eq!(label(cx, "host-status-4").as_deref(), Some("Resuming…"));
    assert!(
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some("host-resume-4"))
            .is_some_and(|node| !node.flags.click),
        "Resume stays inline, unavailable, while its request is out"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-menu-toggle-4"),
        "the press hands focus to the row's menu toggle"
    );
    helm.reply(
        id,
        504,
        Some(json!(
            "resuming the host did not finish within 20 seconds; it carries on, and the host's state in the hosts \
             list and the session list will show the result"
        )),
    );
    settle(cx);
    assert!(!any_open(cx), "the 504 is no failure");
    assert!(
        cx.update(|_, cx| session::current_operations(cx)
            .is_some_and(|operations| operations.read(cx).failure(&OpKey::host("4")).is_none())),
        "nothing recorded under the host's key"
    );
    push_sandbox(&mut helm, resuming());
    settle(cx);
    assert_eq!(label(cx, "host-status-4").as_deref(), Some("Resuming…"));
    assert!(
        frame(cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some("host-resume-4"))
            .is_some_and(|node| !node.flags.click),
        "Resume shows unavailable while the change runs"
    );
}

/// Why this matters: SPEC.md, "Topology": a Suspend shows "Suspending…" "with a spinner and how long ago Tensorlake was
/// asked", and a Resume the same, so a user watching a slow provider can tell a change still waiting from one that
/// stalled; the count must move with nothing pushed, under reduced motion too, where only the panel's one-second clock
/// redraws it.
///
/// Specified: under reduced motion, `gpubox` pushed resuming, asked 12 seconds before the test clock, reads "asked
/// Tensorlake 12 s ago" after its status's "Resuming…", and a second later, with nothing pushed, the panel has redrawn
/// by itself to read "asked Tensorlake 13 s ago"; the status is still named "Resuming…".
#[gpui::test]
fn a_changing_row_counts_the_seconds_since_farhelm_asked(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, suspended(), Vec::new());
    // Under reduced motion no pulse redraws the panel, so only the one-second clock can move the count on.
    cx.update(|_, cx| crate::ticker::set_reduced_motion(true, cx));
    let now = cx.update(|_, cx| crate::ticker::wall_now(cx));
    let now_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    push_sandbox(
        &mut helm,
        HostStateView::TensorlakeChanging {
            starting: false,
            direction: farhelm_proto::tensorlake::LifecycleDirection::Resuming,
            requested_at: now_ms - 12_000,
        },
    );
    settle(cx);
    assert_eq!(label(cx, "host-status-4").as_deref(), Some("Resuming…"));
    assert!(
        painted(cx).contains("Resuming… asked Tensorlake 12 s ago"),
        "{}",
        painted(cx)
    );
    // What the window last drew, not a fresh frame of the test's own: only the panel's clock can have redrawn it.
    cx.update(|_, cx| crate::render_count::reset(cx));
    crate::test_support::advance(cx, std::time::Duration::from_secs(1));
    assert!(
        cx.update(|_, cx| crate::render_count::count("hosts-panel", cx)) > 0,
        "the clock redrew the panel by itself"
    );
    let drawn = cx
        .update(|window, _| window.driver_nodes().to_vec())
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(drawn.contains("asked Tensorlake 13 s ago"), "{drawn}");
}

/// Why this matters: SPEC.md, "Topology": a changing machine shows its words "with a spinner" until the provider
/// confirms, which here is the shared pulse on the row's dot; the ticker's rule is that a view holds the pulse only
/// while it draws a pulsing mark, and never under reduced motion, where the dot stays still.
///
/// Specified: with suspended `gpubox` and nothing else running, half a second delivers no tick; pushed resuming, half
/// a second delivers ticks; pushed connected again, half a second delivers none; under reduced motion, pushed resuming
/// again, half a second delivers none.
#[gpui::test]
fn a_changing_rows_dot_pulses_while_the_change_lasts(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, suspended(), Vec::new());
    let ticks_in_half_a_second = |cx: &mut VisualTestContext| {
        let before = cx.update(|_, cx| crate::ticker::ticks(cx));
        crate::test_support::advance(cx, std::time::Duration::from_millis(500));
        cx.update(|_, cx| crate::ticker::ticks(cx)) - before
    };
    assert_eq!(ticks_in_half_a_second(cx), 0, "the premise: nothing pulses");
    push_sandbox(&mut helm, resuming());
    settle(cx);
    assert!(ticks_in_half_a_second(cx) > 0, "the changing dot pulses");
    push_sandbox(&mut helm, connected());
    settle(cx);
    assert_eq!(ticks_in_half_a_second(cx), 0, "and stops with the change");
    cx.update(|_, cx| crate::ticker::set_reduced_motion(true, cx));
    push_sandbox(&mut helm, resuming());
    settle(cx);
    assert_eq!(ticks_in_half_a_second(cx), 0, "still under reduced motion");
}

/// Why this matters: every control carries its help, and Resume's is the provider's, which says what continues; the
/// old button had none, a recorded deviation.
///
/// Specified: suspended `gpubox`'s inline Resume is helped by "Resume this environment; preserved processes continue".
#[gpui::test]
fn the_inline_resume_carries_the_providers_help(cx: &mut TestAppContext) {
    let (_helm, cx) = panel(cx, suspended(), Vec::new());
    assert_eq!(
        hover_help(cx, "host-resume-4").as_deref(),
        Some("Resume this environment; preserved processes continue")
    );
}

/// Why this matters: the menu offers Resume where the row does, and the helm can refuse a Resume (the provider refused
/// it, the host is busy); the old app showed that refusal in its host refusal dialog.
///
/// Specified: suspended `gpubox`'s menu offers "Resume" after Retry; choosing it sends `POST /api/hosts/4/resume`; the
/// helm's 409 opens "Resume host · gpubox" with its words, and no record stays under the host's key to show again.
#[gpui::test]
fn the_menus_resume_shows_a_refusal_in_the_refusal_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, suspended(), Vec::new());
    click(cx, "host-menu-toggle-4");
    assert_eq!(label(cx, "host-menu-resume").as_deref(), Some("Resume"));
    assert!(
        crate::test_support::bounds(cx, "host-menu-retry").top()
            < crate::test_support::bounds(cx, "host-menu-resume").top(),
        "Resume comes after Retry"
    );
    click(cx, "host-menu-resume");
    let (id, _) = one_command(&mut helm, Method::Post, "/api/hosts/4/resume");
    helm.reply(id, 409, Some(json!("Tensorlake refused the resume")));
    settle(cx);
    assert!(
        cx.update(|_, cx| session::current_operations(cx)
            .is_some_and(|operations| operations.read(cx).failure(&OpKey::host("4")).is_none())),
        "shown in the dialog, so not again under the row"
    );
    assert_eq!(
        label(cx, "host-refusal-title").as_deref(),
        Some("Resume host · gpubox")
    );
    assert_eq!(
        label(cx, "host-refusal-reason").as_deref(),
        Some("Tensorlake refused the resume")
    );
}

/// Why this matters: a connected sandbox has nothing to resume, so its row must not offer it.
///
/// Specified: a connected sandbox's row draws no Resume, and its menu offers none.
#[gpui::test]
fn a_connected_sandbox_offers_no_resume(cx: &mut TestAppContext) {
    let (_helm, cx) = panel(cx, connected(), Vec::new());
    assert_eq!(label(cx, "host-resume-4"), None);
    click(cx, "host-menu-toggle-4");
    assert!(
        label(cx, "host-menu-delete").is_some(),
        "the premise: the menu is open"
    );
    assert_eq!(label(cx, "host-menu-resume"), None);
}

/// Why this matters: SPEC.md, "Creation": Suspend and Resume are not changes to the host registry, refused only while
/// this window's own launch targets or wakes that same host, and launches on other hosts proceed beside them; the
/// registry exclusion that holds a host's setup or removal during any launch must not hold a Resume.
///
/// Specified: with a launcher `Launch` in flight (one this window started for no host of the panel's), suspended
/// `gpubox`'s inline Resume sends `POST /api/hosts/4/resume`.
#[gpui::test]
fn a_launch_elsewhere_does_not_hold_resume(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, suspended(), Vec::new());
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    assert!(
        cx.update(|_, cx| crate::hosts::launch_in_flight(cx)),
        "the premise: a launch is in flight"
    );
    click(cx, "host-resume-4");
    one_command(&mut helm, Method::Post, "/api/hosts/4/resume");
    assert_eq!(work(cx, "4"), Some(Work::Resume));
}

/// Why this matters: the attention list's working rows and the rail's ring are for setup, Update and uninstall only
/// (SPEC.md, "Session list"), and the old attention list treated an explicit lifecycle request as routine work, so a
/// Resume in flight must neither add a working row, which would also hide a problem behind it, nor ring the rail.
///
/// Specified: in the main window, with suspended `gpubox` in the rail, a Resume from the Hosts panel is in flight on
/// the host's key; with Hosts closed again, the rail draws no ring for `gpubox` and nothing reads "Resuming".
#[gpui::test]
fn a_resume_in_flight_is_no_live_work(cx: &mut TestAppContext) {
    let mut listed = reference_hosts();
    listed.push(sandbox(suspended()));
    let (mut helm, cx) = crate::session_list::tests::open_main(
        cx,
        vec![
            crate::session_list::tests::listing(&["a"], &[session("a")]),
            crate::session_list::tests::preferences(&["a"], None),
            hosts(&listed),
        ],
    );
    crate::session_list::host_rail::tests::answer_everything(&mut helm);
    assert!(
        crate::session_list::menu_tests::drawn(cx, "host-rail-dot-4"),
        "the premise: the rail lists gpubox"
    );
    click(cx, "hosts-panel-toggle");
    click(cx, "host-resume-4");
    // Opening Hosts may write a preference beside the Resume, so only the Resume is looked for.
    let sent = helm.commands();
    assert!(
        sent.iter()
            .any(|command| (command.method, command.path.as_str())
                == (Method::Post, "/api/hosts/4/resume")),
        "the Resume was sent: {sent:?}"
    );
    click(cx, "hosts-panel-toggle");
    assert_eq!(work(cx, "4"), Some(Work::Resume), "the premise: it is out");
    assert!(!crate::session_list::menu_tests::drawn(
        cx,
        "host-rail-ring-4"
    ));
    assert!(
        !painted(cx).contains("Resuming"),
        "no working row: {}",
        painted(cx)
    );
}

// ---- Suspend -------------------------------------------------------------------------------------------------------

/// Open `gpubox`'s menu, choose Suspend and answer its activity read (`GET /api/sessions?host=4`) with `status` and
/// `body`.
fn suspend_read(cx: &mut VisualTestContext, helm: &mut FakeHelm, status: u16, body: Value) {
    click(cx, "host-menu-toggle-4");
    click(cx, "host-menu-suspend");
    let (read, _) = one_command(helm, Method::Get, "/api/sessions?host=4");
    helm.reply(read, status, Some(body));
    settle(cx);
}

/// `rows` as sessions on `gpubox` (host 4), as the helm lists them for `GET /api/sessions?host=4`.
fn on_gpubox<const N: usize>(rows: [Fixture; N]) -> [Fixture; N] {
    rows.map(|row| row.host(4))
}

/// Hold the operation store's launch key for a launcher `Launch`, as a launch in flight does, without a command on the
/// wire: the fake helm answers in order, so a sent launch left unanswered would stop the read's reply behind it.
fn hold_launch(cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("a UI session");
        operations.update(cx, |operations, cx| {
            operations
                .hold(&OpKey::new_launch(), Work::Launch, cx)
                .expect("the launch holds its key");
        });
    });
    settle(cx);
}

/// Why this matters: Suspend freezes every process on the machine, so it may go at once only when Farhelm knows no
/// agent there is working (SPEC.md, "Topology": a connected host after the activity read and a complete listing with
/// conclusive non-working statuses); the old Hosts panel read the host's own listing for that, never the pushed one. A
/// Suspend routinely outlasts the helm's reply bound, so its 504 must not read as a failure, and the request is the
/// host's, on its key.
///
/// Specified: connected `gpubox`'s menu offers "Suspend"; choosing it sends `GET /api/sessions?host=4` and nothing
/// else; a complete listing of an idle and a waiting session asks nothing and sends `POST /api/hosts/4/suspend` with no
/// body, as `Suspend` work on the host's key, the row's status reading "Suspending…"; the helm's 504 opens nothing and
/// leaves no record under the host's key; pushed suspending, the row's status still reads "Suspending…".
#[gpui::test]
fn suspend_goes_at_once_when_every_session_there_is_idle(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    click(cx, "host-menu-toggle-4");
    assert_eq!(label(cx, "host-menu-suspend").as_deref(), Some("Suspend"));
    keys(cx, "escape");
    let waiting = session("b").status(json!({"state": "waiting"}));
    suspend_read(
        cx,
        &mut helm,
        200,
        listing(&on_gpubox([idle("a"), waiting]), false),
    );
    assert!(!any_open(cx), "a known idle host is not asked about");
    let (id, body) = one_command(&mut helm, Method::Post, "/api/hosts/4/suspend");
    assert_eq!(body, Value::Null);
    assert_eq!(work(cx, "4"), Some(Work::Suspend));
    assert_eq!(label(cx, "host-status-4").as_deref(), Some("Suspending…"));
    helm.reply(
        id,
        504,
        Some(json!(
            "suspending the host did not finish within 20 seconds; it carries on, and the host's state in the hosts \
             list and the session list will show the result"
        )),
    );
    settle(cx);
    assert!(!any_open(cx), "the 504 is no failure");
    assert!(
        cx.update(|_, cx| session::current_operations(cx)
            .is_some_and(|operations| operations.read(cx).failure(&OpKey::host("4")).is_none())),
        "nothing recorded under the host's key"
    );
    push_sandbox(
        &mut helm,
        HostStateView::TensorlakeChanging {
            starting: false,
            direction: farhelm_proto::tensorlake::LifecycleDirection::Suspending,
            requested_at: 0,
        },
    );
    settle(cx);
    assert_eq!(label(cx, "host-status-4").as_deref(), Some("Suspending…"));
}

/// Why this matters: suspending a machine where a session works freezes it mid-task, and an incomplete or failed read,
/// or a host that is not connected, proves nothing about that; the old question asked then, with Cancel focused first
/// so a stray Enter keeps the machine running, and nothing went out before the answer.
///
/// Specified: answered with a running shell session, with a session whose
/// status is unknown, with an idle session read stale, with an idle but truncated listing, with a failed read (503),
/// and, with `gpubox` pushed connecting before the click, with an idle complete listing, Suspend asks "Suspend gpubox?"
/// each time, with the same wording for shell and unknown session types and a statement that Suspend preserves
/// running processes. Cancel holds focus and nothing is sent
/// before the answer, and Cancel (clicked and Escape, in turn) sends nothing. The last question's Suspend sends
/// `POST /api/hosts/4/suspend`.
#[gpui::test]
fn suspend_asks_when_a_session_works_or_its_activity_is_unknown(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    let unknown = session("u").status(json!({"state": "unknown"}));
    let shell = bash_session("b").agent_kind("generic").invocation("bash");
    let busy = listing(&on_gpubox([idle("a"), shell]), false);
    let decoded: SessionList =
        serde_json::from_value(busy.clone()).expect("a complete activity reply");
    assert!(!decoded.truncated);
    let bash = &decoded.sessions[1];
    assert_eq!(bash.host, 4);
    assert!(bash.info.launch.is_shell() && !bash.stale);
    assert_eq!(bash.info.status, farhelm_proto::SessionStatus::Running);
    assert!(cx.update(|_, cx| matches!(
        super::listed(4, cx).map(|host| host.state),
        Some(HostStateView::Connected {
            refresh: RefreshView::Ok { .. },
            ..
        })
    )));
    let cases = [
        (200, busy, "a running shell session"),
        (
            200,
            listing(&on_gpubox([idle("a"), unknown]), false),
            "a session whose status is unknown",
        ),
        (
            200,
            listing(&on_gpubox([idle("a").stale()]), false),
            "an idle session read stale",
        ),
        (
            200,
            listing(&on_gpubox([idle("a")]), true),
            "a listing the cap cut",
        ),
        (503, json!("the helm is busy"), "a failed read"),
        (
            200,
            listing(&on_gpubox([idle("a")]), false),
            "a host already unreachable at the click",
        ),
    ];
    let last = cases.len() - 1;
    for (index, (status, body, why)) in cases.into_iter().enumerate() {
        if index == last {
            push_sandbox(&mut helm, connecting(1, Some("connection refused")));
            settle(cx);
        }
        suspend_read(cx, &mut helm, status, body);
        assert_eq!(
            label(cx, "prompt").as_deref(),
            Some("Suspend gpubox?"),
            "{why}"
        );
        assert_eq!(
            label(cx, "host-suspend-line").as_deref(),
            Some(
                "A session on gpubox is working, or its status could not be checked. Suspend preserves running \
                 processes. They continue when you resume the host."
            ),
            "{why}"
        );
        assert_eq!(answers(cx), ["Cancel", "Suspend"], "{why}");
        assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"), "{why}");
        assert!(
            helm.commands().is_empty(),
            "nothing before the answer ({why})"
        );
        if index == last {
            break;
        }
        if index % 2 == 0 {
            click(cx, "prompt-cancel");
        } else {
            keys(cx, "escape");
        }
        settle(cx);
        assert!(!any_open(cx), "the question closed ({why})");
        assert!(helm.commands().is_empty(), "Cancel sends nothing ({why})");
    }
    click(cx, "prompt-confirm");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/hosts/4/suspend");
    assert_eq!(body, Value::Null);
    assert_eq!(work(cx, "4"), Some(Work::Suspend));
}

/// Why this matters: the helm can refuse a Suspend (the provider refused it, the host is busy); the old app showed
/// that refusal in its host refusal dialog.
///
/// Specified: the helm's 409 to `gpubox`'s Suspend opens "Suspend host · gpubox" with its words.
#[gpui::test]
fn the_menus_suspend_shows_a_refusal_in_the_refusal_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    suspend_read(cx, &mut helm, 200, listing(&on_gpubox([idle("a")]), false));
    let (id, _) = one_command(&mut helm, Method::Post, "/api/hosts/4/suspend");
    helm.reply(id, 409, Some(json!("Tensorlake refused the suspend")));
    settle(cx);
    assert_eq!(
        label(cx, "host-refusal-title").as_deref(),
        Some("Suspend host · gpubox")
    );
    assert_eq!(
        label(cx, "host-refusal-reason").as_deref(),
        Some("Tensorlake refused the suspend")
    );
}

/// Why this matters: SPEC.md asks for "a connected host after the activity read": a host that disconnects while the
/// read is out proves nothing about the agents the read listed, even when it lists none, and a check made only at the
/// click would miss it (the old e2e "an empty cached listing asks when the host disconnects during the read").
///
/// Specified: with connected `gpubox`, Suspend's read is out; `gpubox` is pushed connecting, and its row reads
/// "Connecting"; the read then answers with an empty, complete listing, and Suspend asks "Suspend gpubox?",
/// sending nothing.
#[gpui::test]
fn suspend_asks_when_the_host_disconnects_during_its_read(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    click(cx, "host-menu-toggle-4");
    click(cx, "host-menu-suspend");
    let (read, _) = one_command(&mut helm, Method::Get, "/api/sessions?host=4");
    push_sandbox(&mut helm, connecting(1, Some("connection refused")));
    settle(cx);
    assert_eq!(
        label(cx, "host-status-4").as_deref(),
        Some(crate::hosts::identity::phase::words(&connecting(
            1,
            Some("connection refused")
        ))),
        "the premise: gpubox reads connecting while the read is out"
    );
    helm.reply(read, 200, Some(listing(&[], false)));
    settle(cx);
    assert_eq!(label(cx, "prompt").as_deref(), Some("Suspend gpubox?"));
    assert!(helm.commands().is_empty(), "nothing before the answer");
}

/// Why this matters: the read and the question can outlast the phase they were asked in. A question about a host
/// already suspended, being suspended or deleted, or gone would ask about something no longer true, and its answer
/// would only meet a refusal; the other host questions close when their host goes.
///
/// Specified: pushed suspended while its read is out, `gpubox`'s Suspend opens no question and sends nothing; asked
/// while connected, then pushed suspended while the question is up, the question's Suspend sends nothing; asked again
/// while connected, the question closes when the registry drops `gpubox`.
#[gpui::test]
fn the_suspend_question_follows_its_host(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    let running = || listing(&on_gpubox([session("a")]), false);

    click(cx, "host-menu-toggle-4");
    click(cx, "host-menu-suspend");
    let (read, _) = one_command(&mut helm, Method::Get, "/api/sessions?host=4");
    push_sandbox(&mut helm, suspended());
    settle(cx);
    helm.reply(read, 200, Some(running()));
    settle(cx);
    assert!(!any_open(cx), "no question about a suspended host");
    assert!(helm.commands().is_empty(), "nothing was sent");

    push_sandbox(&mut helm, connected());
    settle(cx);
    suspend_read(cx, &mut helm, 200, running());
    assert_eq!(label(cx, "prompt").as_deref(), Some("Suspend gpubox?"));
    push_sandbox(&mut helm, suspended());
    settle(cx);
    click(cx, "prompt-confirm");
    assert!(helm.commands().is_empty(), "no Suspend of a suspended host");

    push_sandbox(&mut helm, connected());
    settle(cx);
    suspend_read(cx, &mut helm, 200, running());
    assert_eq!(label(cx, "prompt").as_deref(), Some("Suspend gpubox?"));
    helm.push(farhelm_proto::ui_channel::CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1", "2", "3", "5"]),
        upsert: Vec::new(),
        remove: vec![HostView::key(4)],
    });
    settle(cx);
    assert!(!any_open(cx), "the question closed with its host");
}

/// Why this matters: a question never opens over what the user turned to while the read was out (SPEC.md, "Keyboard
/// focus"), and dropping the Suspend silently would leave the user wondering what their press did; Uninstall's late
/// plan says so under the row, and Suspend follows it.
///
/// Specified: with `gpubox`'s Suspend read out, a key press moves the user on; the read's answer, a running session,
/// opens nothing and sends nothing, and the row reads "Farhelm checked this host's sessions after you moved on. Choose
/// Suspend again to see the question."
#[gpui::test]
fn a_suspend_read_after_the_user_moved_on_says_so(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    click(cx, "host-menu-toggle-4");
    click(cx, "host-menu-suspend");
    let (read, _) = one_command(&mut helm, Method::Get, "/api/sessions?host=4");
    keys(cx, "shift-tab");
    helm.reply(read, 200, Some(listing(&on_gpubox([session("a")]), false)));
    settle(cx);
    assert!(!any_open(cx), "no question");
    assert!(helm.commands().is_empty(), "nothing was sent");
    assert_eq!(
        label(cx, "host-error-4").as_deref(),
        Some(crate::hosts::lifecycle::MOVED_ON)
    );
}

/// Why this matters: SPEC.md, "Creation": Suspend and Resume are not changes to the host registry, refused only while
/// this window's own launch targets or wakes that same host, and launches on other hosts proceed beside them; the
/// registry exclusion that holds a host's setup or removal during any launch must not hold a Suspend.
///
/// Specified: with a launcher `Launch` holding its key (for no host of the panel's), connected `gpubox`'s Suspend, its
/// read listing an idle session, sends `POST /api/hosts/4/suspend`.
#[gpui::test]
fn a_launch_elsewhere_does_not_hold_suspend(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    hold_launch(cx);
    assert!(
        cx.update(|_, cx| crate::hosts::launch_in_flight(cx)),
        "the premise: a launch is in flight"
    );
    suspend_read(cx, &mut helm, 200, listing(&on_gpubox([idle("a")]), false));
    one_command(&mut helm, Method::Post, "/api/hosts/4/suspend");
    assert_eq!(work(cx, "4"), Some(Work::Suspend));
}

/// A host-level Suspend could freeze the supervisor before it answers a create already sent to that host.
/// The open launch protects its destination while the other host's menu stays actionable.
/// Check that Resume on the destination loses activation and focus before the launch replies.
#[gpui::test]
fn a_pending_launch_holds_its_hosts_lifecycle_controls(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel(cx, connected(), Vec::new());
    let create = launch_to(&mut helm, cx, 4);
    assert_eq!(
        create.body.unwrap()["host"],
        4,
        "the unanswered launch targets gpubox"
    );
    assert!(
        cx.update(|_, cx| { crate::launcher::launch::host_lifecycle_refusal(5, cx).is_none() }),
        "the same launch leaves the other host available"
    );
    click(cx, "host-menu-toggle-5");
    assert!(
        frame(cx).iter().any(|node| {
            node.element_id.as_deref() == Some("host-menu-suspend") && node.flags.click
        }),
        "the other host's menu still offers Suspend"
    );
    keys(cx, "escape");
    click(cx, "host-menu-toggle-4");
    assert_eq!(
        hover_help(cx, "host-menu-suspend").as_deref(),
        Some("A launch on this host is in progress; try again once it finishes")
    );
    click(cx, "host-menu-suspend");
    assert!(
        label(cx, "host-menu-suspend").is_some(),
        "the disabled item left its menu open"
    );
    assert!(
        helm.commands()
            .iter()
            .all(|command| command.path != "/api/sessions?host=4"),
        "Suspend did not start its activity read"
    );
    keys(cx, "escape");
    // The helm pushes a command's changes before its reply. Exercise the interval in which the host row has
    // changed but the launcher still owns the destination.
    push_sandbox(&mut helm, suspended());
    settle(cx);
    assert!(
        frame(cx).iter().any(|node| {
            node.element_id.as_deref() == Some("host-resume-4")
                && !node.flags.click
                && !node.flags.focusable
        }),
        "inline Resume has neither an activation target nor a focus target"
    );
    assert_eq!(
        hover_help(cx, "host-resume-4").as_deref(),
        Some("A launch on this host is in progress; try again once it finishes")
    );
    click(cx, "host-resume-4");
    assert!(
        helm.commands()
            .iter()
            .all(|command| !command.path.ends_with("/resume"))
    );
    helm.reply(
        create.id,
        409,
        Some(json!({"error": "The host was suspended"})),
    );
    settle(cx);
    assert!(
        cx.update(|_, cx| crate::launcher::launch::host_lifecycle_refusal(4, cx).is_none()),
        "a launch refusal releases the host even while the draft has a notice"
    );
    click(cx, "host-resume-4");
    one_command(&mut helm, Method::Post, "/api/hosts/4/resume");
}
