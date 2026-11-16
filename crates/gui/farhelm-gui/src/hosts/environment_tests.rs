//! Tests of a sandbox's and a container's own host actions over the helm client's fake: Delete sandbox and Delete
//! container (which always ask), Retry on a failed deletion (which deletes again), a sandbox's Re-run of its failed
//! setup (the creation question, or none under Tensorlake's own skip), and each refusing while a session launch is in
//! flight.
//!
//! Every test starts from the Hosts panel's harness (`hosts/tests.rs`) over the reference hosts plus a Tensorlake
//! sandbox `gpubox` (4) and a container `crate-1` (5), in a column of the sidebar's width and a window as large as the
//! reference screens'. No test reaches a real provider: the fake helm answers every request.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus};
use farhelm_proto::recipes::RecipeSize;
use farhelm_proto::ui_channel::{
    Collection, CollectionState, HostStateView, HostView, KeyedEntry, PREFERENCES_KEY,
    TensorlakeHostView,
};
use gpui::{TestAppContext, VisualTestContext, px, size};
use serde_json::{Value, json};

use crate::hosts::LAUNCH_IN_FLIGHT;
use crate::hosts::tests::{
    connected, host, hosts, open_panel_in_column, progress, reference_hosts, run, start_work,
};
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{any_open, click, frame, label, meta, one_command, settle};

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
