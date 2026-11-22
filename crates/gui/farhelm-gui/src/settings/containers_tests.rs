//! Tests of Settings → Containers ([`super::containers`]) over the helm client's fake: the container hosts the section
//! lists from the pushed hosts and their Podman checks, the checks it asks for, the leftovers it reads, Delete's
//! question and its requests, failed reads, and refresh rounds.
//!
//! The dialog is opened from the gear as in the dialog's own tests (`super::tests`), whose helpers these share. Every
//! step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, ContainerCheckView, HostView, Preferences,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::Section;
use super::recipe_tests::words as painted_words;
use super::tests::{
    available, drawn, heading, open_rail, open_rail_with, open_rail_with_hosts, open_settings,
    painted, push_hosts, reach, section_button, sent, sent_reads,
};
use crate::hosts::tests::{aliased, connected, host, hosts};
use crate::test_support::{click, focused_id, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The leftovers read Containers sends as it is shown and on each Refresh.
const LEFTOVERS: &str = "/api/container-orphans";

/// A leftover as `GET /api/container-orphans` answers it: named `name` on container host 7, with host row `host` if any.
fn leftover(name: &str, host: Option<i64>) -> Value {
    json!({ "id": "a".repeat(64), "name": name, "place": { "kind": "podman_host", "host": 7 }, "role": "session",
            "host": host })
}

/// Container host `id` named `name` (its alias) as the hosts collection carries it: an ssh host whose check is running
/// or not (`refreshing`), with the latest finished check's `result`: `None` when no check of its current address has
/// finished, else when it ended and why it failed (`None`: it passed).
fn container_host(
    id: i64,
    name: &str,
    refreshing: bool,
    result: Option<(u64, Option<&str>)>,
) -> HostView {
    let mut view = aliased(
        host(id, "ssh", connected()),
        name,
        &format!("dev@{name}.invalid"),
    );
    view.container_check = Some(ContainerCheckView {
        checked_at: result.map(|(at, _)| at),
        refreshing,
        usable: matches!(result, Some((_, None))),
        reason: result.and_then(|(_, reason)| reason.map(str::to_owned)),
    });
    view
}

/// `commands` as (method, path).
fn paths(commands: &[FakeCommand]) -> Vec<(Method, String)> {
    commands
        .iter()
        .map(|command| (command.method, command.path.clone()))
        .collect()
}

/// The commands sent since the last call, as (method, path).
fn sent_paths(helm: &mut FakeHelm) -> Vec<(Method, String)> {
    paths(&sent(helm))
}

/// A check of each of `hosts`, in order, as (method, path).
fn checks_of(hosts: &[i64]) -> Vec<(Method, String)> {
    hosts
        .iter()
        .map(|host| (Method::Post, format!("/api/hosts/{host}/podman-probe")))
        .collect()
}

/// What one round sends, split: the leftovers read's id, and the checks of the container hosts it asked for.
struct Round {
    leftovers: u64,
    checks: Vec<FakeCommand>,
}

/// The commands a round just sent ([`Round`]), after checking they are the leftovers read and then a check of each of
/// `hosts`, in order, and nothing else.
fn round(helm: &mut FakeHelm, hosts: &[i64]) -> Round {
    let commands = sent(helm);
    let mut expected = vec![(Method::Get, LEFTOVERS.to_string())];
    expected.extend(checks_of(hosts));
    assert_eq!(
        paths(&commands),
        expected,
        "the leftovers read, then a check of each container host"
    );
    Round {
        leftovers: commands[0].id,
        checks: commands[1..].to_vec(),
    }
}

/// Open Settings and show Containers, returning what its first round sent ([`round`]), a check of each of `hosts`.
fn show_containers(helm: &mut FakeHelm, cx: &mut VisualTestContext, hosts: &[i64]) -> Round {
    open_settings(cx);
    assert_eq!(sent(helm), [], "the premise: Sessions reads nothing");
    click(cx, section_button(Section::Containers));
    assert_eq!(heading(cx).as_deref(), Some("Containers"));
    round(helm, hosts)
}

/// Answer `checks` with 202 and no body, as the helm answers a check it started in the background.
fn answer_checks(helm: &mut FakeHelm, checks: &[FakeCommand], cx: &mut VisualTestContext) {
    for check in checks {
        helm.reply(check.id, 202, None);
    }
    settle(cx);
}

/// Show Containers checking `hosts`, with the leftovers read answered by `leftovers` and the checks answered (202),
/// after which nothing is out.
fn show_containers_with(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    hosts: &[i64],
    leftovers: Value,
) {
    let first = show_containers(helm, cx, hosts);
    helm.reply(first.leftovers, 200, Some(leftovers));
    answer_checks(helm, &first.checks, cx);
    assert_eq!(sent(helm), [], "nothing follows the checks by itself");
}

// ---- The container hosts -------------------------------------------------------------------------------------------

/// SPEC.md, "Container sessions": "Settings → Containers shows probe status per host"; the old section listed every
/// host that can hold containers with Ready or the check's reason and "· Refreshing…", and asked for a fresh check of
/// each when it opened. The hosts and their checks are pushed, so the rows draw at once; the leftovers are not, so the
/// section reads them once per opening. Specified: with build-box passed, gpu-1 failed and checking, and a Tensorlake
/// host (which carries no check), showing Containers sends the leftovers read and then a check of build-box and of
/// gpu-1, in that order and nothing else; the rows read "build-box Ready" and "gpu-1 Podman is not installed on this
/// host · Refreshing…" before anything answers, the Tensorlake host is not listed, and the leftovers read as being
/// read; once answered with none, they read "None found"; the checks' answers send nothing; showing another section and
/// Containers again sends nothing.
#[gpui::test]
fn containers_lists_the_pushed_container_hosts_reads_the_leftovers_once_and_checks_each_host(
    cx: &mut TestAppContext,
) {
    let mut sandbox = host(9, "tensorlake", connected());
    sandbox.name = "sandbox-host".into();
    let not_installed = Some((100, Some("Podman is not installed on this host")));
    let (mut helm, cx, _gear) = open_rail_with_hosts(
        cx,
        &[
            container_host(7, "build-box", false, Some((100, None))),
            container_host(8, "gpu-1", true, not_installed),
            sandbox,
        ],
    );
    let first = show_containers(&mut helm, cx, &[7, 8]);
    let shown = painted_words(cx);
    assert!(shown.contains("build-box Ready"), "{shown}");
    assert!(
        shown.contains("gpu-1 Podman is not installed on this host · Refreshing…"),
        "{shown}"
    );
    assert!(
        !shown.contains("sandbox-host"),
        "a sandbox is not a container host: {shown}"
    );
    assert!(shown.contains("Reading container leftovers…"), "{shown}");

    helm.reply(first.leftovers, 200, Some(json!([])));
    settle(cx);
    assert!(painted(cx).contains("None found"));
    answer_checks(&mut helm, &first.checks, cx);
    assert_eq!(sent(&mut helm), [], "the checks' answers send nothing");

    click(cx, section_button(Section::Hosts));
    click(cx, section_button(Section::Containers));
    assert_eq!(sent(&mut helm), [], "read and checked once per opening");
}

/// The hosts collection pushes a host's check when it starts and when it ends, whoever started it, so the row must
/// follow the push alone, with nothing read for it, and "· Refreshing…" must follow the check itself. Specified: with
/// build-box never checked and gpu-1 passed, the rows read "this host needs a Podman check" and Ready; build-box pushed
/// running adds "· Refreshing…"; pushed with a failed result, it reads that result's reason and drops the mark; gpu-1
/// pushed failed, then passed, reads its reason, then Ready; and nothing is sent throughout.
#[gpui::test]
fn a_pushed_result_changes_the_row_with_no_request_sent(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail_with_hosts(
        cx,
        &[
            container_host(7, "build-box", false, None),
            container_host(8, "gpu-1", false, Some((100, None))),
        ],
    );
    show_containers_with(&mut helm, cx, &[7, 8], json!([]));
    let shown = painted_words(cx);
    assert!(
        shown.contains("build-box this host needs a Podman check gpu-1 Ready"),
        "{shown}"
    );
    assert!(
        !shown.contains("Refreshing…"),
        "premise: no check is running"
    );

    let gpu_ready = container_host(8, "gpu-1", false, Some((100, None)));
    push_hosts(
        &mut helm,
        &[
            container_host(7, "build-box", true, None),
            gpu_ready.clone(),
        ],
        cx,
    );
    let shown = painted_words(cx);
    assert!(
        shown.contains("build-box this host needs a Podman check · Refreshing…"),
        "{shown}"
    );
    let not_installed = Some((200, Some("Podman is not installed on this host")));
    let build_failed = container_host(7, "build-box", false, not_installed);
    push_hosts(&mut helm, &[build_failed.clone(), gpu_ready], cx);
    let shown = painted_words(cx);
    assert!(
        shown.contains("build-box Podman is not installed on this host"),
        "{shown}"
    );
    assert!(!shown.contains("Refreshing…"), "the push ended the mark");

    let cgroup = Some((300, Some("the systemd cgroup manager is not in use")));
    push_hosts(
        &mut helm,
        &[
            build_failed.clone(),
            container_host(8, "gpu-1", false, cgroup),
        ],
        cx,
    );
    assert!(painted_words(cx).contains("gpu-1 the systemd cgroup manager is not in use"));
    push_hosts(
        &mut helm,
        &[
            build_failed,
            container_host(8, "gpu-1", false, Some((400, None))),
        ],
        cx,
    );
    assert!(painted_words(cx).contains("gpu-1 Ready"));
    assert_eq!(sent(&mut helm), [], "a pushed result sends nothing");
}

/// The helm checks a host on its own only when a container launch or reconnect needs it; the old section's re-read on
/// every fleet notice is what checked a host added, or given a new address, while it was open. So the section must ask
/// for a check of such a host as the push shows it, and only once, since a check whose result never arrives would
/// otherwise be asked for on every push. Specified: with build-box listed and checked, gpu-1 pushed in with no result
/// shows its row and sends one `POST /api/hosts/8/podman-probe`; the same push again sends nothing; build-box renamed
/// to big-box at the same address shows the new name and sends nothing; build-box moved to a new address while a check
/// still runs (refreshing, no result) sends nothing, since that check's result is for the old address; once it ends
/// with no result, one check of host 7 goes out; moved back to its first address with no result, host 7 is checked
/// again; gpu-1 pushed out takes its row away and sends nothing.
#[gpui::test]
fn a_host_added_or_moved_while_shown_is_checked_once(cx: &mut TestAppContext) {
    let ready = Some((100, None));
    let build_box = container_host(7, "build-box", false, ready);
    let (mut helm, cx, _gear) = open_rail_with_hosts(cx, std::slice::from_ref(&build_box));
    show_containers_with(&mut helm, cx, &[7], json!([]));

    let gpu = container_host(8, "gpu-1", false, None);
    push_hosts(&mut helm, &[build_box.clone(), gpu.clone()], cx);
    assert!(painted_words(cx).contains("build-box Ready gpu-1 this host needs a Podman check"));
    assert_eq!(
        sent_paths(&mut helm),
        checks_of(&[8]),
        "the new host is checked"
    );
    push_hosts(&mut helm, &[build_box.clone(), gpu.clone()], cx);
    assert_eq!(sent(&mut helm), [], "and only once");

    let mut renamed = build_box.clone();
    renamed.name = "big-box".into();
    renamed.alias = Some("big-box".into());
    push_hosts(&mut helm, &[renamed.clone(), gpu.clone()], cx);
    let shown = painted_words(cx);
    assert!(shown.contains("big-box Ready"), "{shown}");
    assert!(!shown.contains("build-box"), "{shown}");
    assert_eq!(sent(&mut helm), [], "a rename keeps the address");

    let at = |destination: &str, refreshing: bool| {
        let mut host = container_host(7, "build-box", refreshing, None);
        host.name = "big-box".into();
        host.alias = Some("big-box".into());
        host.destination = Some(destination.into());
        host
    };
    push_hosts(&mut helm, &[at("dev@moved.invalid", true), gpu.clone()], cx);
    assert_eq!(sent(&mut helm), [], "a check still runs");
    push_hosts(
        &mut helm,
        &[at("dev@moved.invalid", false), gpu.clone()],
        cx,
    );
    assert_eq!(
        sent_paths(&mut helm),
        checks_of(&[7]),
        "a new address is checked afresh"
    );
    let back = at("dev@build-box.invalid", false);
    push_hosts(&mut helm, &[back.clone(), gpu], cx);
    assert_eq!(
        sent_paths(&mut helm),
        checks_of(&[7]),
        "moved back, the first address is checked again"
    );

    let only = hosts(&[back]);
    helm.push(CollectionChanges {
        collection: only.collection,
        meta: only.meta,
        upsert: only.entries,
        remove: vec![HostView::key(8)],
    });
    settle(cx);
    assert!(!painted(cx).contains("gpu-1"));
    assert_eq!(sent(&mut helm), [], "a host going sends nothing");
}

/// The old places read replaced the hosts with the helm's refusal of a check until its next read, which the next fleet
/// notice brought; here that lasts until the push brings a newer result for the refused host, or Refresh. The refusal
/// must stay readable through an unrelated push, and must not hide a newer result for its own host. Specified: with
/// build-box and gpu-1 checked and gpu-1's check refused, the refusal shows where the hosts were; a push of a newer
/// result for build-box leaves it; a push of a newer result for gpu-1 ends it and shows both rows, with nothing sent;
/// gpu-1's check refused again in the next round shows again until Refresh starts another.
#[gpui::test]
fn a_refused_check_shows_until_its_hosts_next_result(cx: &mut TestAppContext) {
    let ready = |id, name| container_host(id, name, false, Some((100, None)));
    let (mut helm, cx, _gear) =
        open_rail_with_hosts(cx, &[ready(7, "build-box"), ready(8, "gpu-1")]);
    let first = show_containers(&mut helm, cx, &[7, 8]);
    helm.reply(first.leftovers, 200, Some(json!([])));
    helm.reply(first.checks[0].id, 202, None);
    helm.reply(
        first.checks[1].id,
        503,
        Some(json!("fixture check refusal")),
    );
    settle(cx);
    assert!(drawn(cx, "containers-hosts-error"));
    assert!(painted(cx).contains("fixture check refusal"));

    let not_installed = Some((200, Some("Podman is not installed on this host")));
    let build_failed = container_host(7, "build-box", false, not_installed);
    push_hosts(&mut helm, &[build_failed.clone(), ready(8, "gpu-1")], cx);
    let shown = painted(cx);
    assert!(
        shown.contains("fixture check refusal"),
        "the refusal stays: {shown}"
    );
    assert!(!shown.contains("Podman is not installed"), "{shown}");

    let gpu_newer = container_host(8, "gpu-1", false, Some((300, None)));
    push_hosts(&mut helm, &[build_failed.clone(), gpu_newer], cx);
    let shown = painted_words(cx);
    assert!(!shown.contains("fixture check refusal"), "{shown}");
    assert!(
        shown.contains("build-box Podman is not installed on this host gpu-1 Ready"),
        "{shown}"
    );
    assert_eq!(sent(&mut helm), [], "a push sends nothing");

    click(cx, "containers-refresh");
    let next = round(&mut helm, &[7, 8]);
    helm.reply(next.leftovers, 200, Some(json!([])));
    helm.reply(next.checks[0].id, 202, None);
    helm.reply(next.checks[1].id, 503, Some(json!("fixture check refusal")));
    settle(cx);
    assert!(painted(cx).contains("fixture check refusal"));
    click(cx, "containers-refresh");
    let last = round(&mut helm, &[7, 8]);
    assert!(
        !painted(cx).contains("fixture check refusal"),
        "Refresh starts over"
    );
    helm.reply(last.leftovers, 200, Some(json!([])));
    answer_checks(&mut helm, &last.checks, cx);
}

/// Refresh starts over, so it must take a shown refusal away at once, whatever pressed it: a click redraws the window
/// by itself, but a key press does not, and the leftovers' answer that would otherwise redraw can wait up to the helm's
/// host bound. Specified: with gpu-1's check refused and shown, Refresh pressed with Space sends a new round, and the
/// frame drawn for that press, read without asking for another, no longer shows the refusal.
#[gpui::test]
fn refresh_from_the_keyboard_takes_a_refusal_away_at_once(cx: &mut TestAppContext) {
    let ready = |id, name| container_host(id, name, false, Some((100, None)));
    let (mut helm, cx, _gear) =
        open_rail_with_hosts(cx, &[ready(7, "build-box"), ready(8, "gpu-1")]);
    let first = show_containers(&mut helm, cx, &[7, 8]);
    helm.reply(first.leftovers, 200, Some(json!([])));
    helm.reply(first.checks[0].id, 202, None);
    helm.reply(
        first.checks[1].id,
        503,
        Some(json!("fixture check refusal")),
    );
    settle(cx);
    assert!(
        painted(cx).contains("fixture check refusal"),
        "premise: the refusal shows"
    );

    reach(cx, "containers-refresh");
    keys(cx, "space");
    let next = round(&mut helm, &[7, 8]);
    // The frame as last drawn: `painted` would ask for a redraw of its own and hide a missing one.
    let drawn: Vec<String> = cx.update(|window, _| {
        window
            .driver_nodes()
            .iter()
            .flat_map(|node| node.texts.iter().map(|line| line.text.to_string()))
            .collect()
    });
    assert!(
        !drawn
            .iter()
            .any(|line| line.contains("fixture check refusal")),
        "{drawn:?}"
    );
    helm.reply(next.leftovers, 200, Some(json!([])));
    answer_checks(&mut helm, &next.checks, cx);
}

/// The usual refusal of a check is of a host removed between the push the section listed it from and the request, so
/// a stale "not found" must not stay in place of the hosts until Refresh. Specified: with build-box and gpu-1 checked
/// and gpu-1's check refused, the refusal shows where the hosts were; a push that removes gpu-1 ends it and shows
/// build-box's row, with nothing sent.
#[gpui::test]
fn a_refused_checks_line_goes_with_its_host(cx: &mut TestAppContext) {
    let build_box = container_host(7, "build-box", false, Some((100, None)));
    let gpu = container_host(8, "gpu-1", false, Some((100, None)));
    let (mut helm, cx, _gear) = open_rail_with_hosts(cx, &[build_box.clone(), gpu]);
    let first = show_containers(&mut helm, cx, &[7, 8]);
    helm.reply(first.leftovers, 200, Some(json!([])));
    helm.reply(first.checks[0].id, 202, None);
    helm.reply(
        first.checks[1].id,
        404,
        Some(json!("fixture host not found")),
    );
    settle(cx);
    assert!(
        painted(cx).contains("fixture host not found"),
        "premise: the refusal shows"
    );

    let only = hosts(std::slice::from_ref(&build_box));
    helm.push(CollectionChanges {
        collection: only.collection,
        meta: only.meta,
        upsert: only.entries,
        remove: vec![HostView::key(8)],
    });
    settle(cx);
    let shown = painted_words(cx);
    assert!(!shown.contains("fixture host not found"), "{shown}");
    assert!(!drawn(cx, "containers-hosts-error"));
    assert!(shown.contains("build-box Ready"), "{shown}");
    assert_eq!(sent(&mut helm), [], "no Refresh, and nothing read");
}

/// The old section kept a failed read an error ("A failed discovery must stay an error; an empty list promises no
/// leftovers"), and a host that did not answer within the helm's bound reaches the GUI as such a failure. The hosts are
/// pushed now, so their failure is the hosts collection's: the helm could not read its registry, and the hosts it still
/// lists may be gone, so none is checked. Specified: with the hosts collection unavailable but still listing build-box,
/// Containers shows the helm's reason where the hosts would be and asks for no check; a refused leftovers read shows
/// its reason instead of "None found"; once the hosts list is pushed available, the round checks build-box, though it
/// has a result, as a round checks every host, and a second push sends nothing.
#[gpui::test]
fn failed_container_reads_show_their_reasons_and_never_none_found(cx: &mut TestAppContext) {
    let build_box = container_host(7, "build-box", false, Some((100, None)));
    let mut unreadable = hosts(std::slice::from_ref(&build_box));
    unreadable.meta.availability = Availability::Unavailable {
        error: "fixture registry refusal".into(),
    };
    assert_eq!(unreadable.collection, Collection::Hosts);
    let (mut helm, cx, _gear) = open_rail_with(cx, &Preferences::default(), vec![unreadable]);
    let first = show_containers(&mut helm, cx, &[]);
    helm.reply(
        first.leftovers,
        504,
        Some(json!("the host did not answer within 20 seconds")),
    );
    settle(cx);
    let shown = painted(cx);
    assert!(shown.contains("fixture registry refusal"), "{shown}");
    assert!(
        shown.contains("the host did not answer within 20 seconds"),
        "{shown}"
    );
    assert!(!shown.contains("None found"), "{shown}");
    assert!(drawn(cx, "containers-hosts-error") && drawn(cx, "containers-leftovers-error"));

    push_hosts(&mut helm, std::slice::from_ref(&build_box), cx);
    assert!(painted_words(cx).contains("build-box Ready"));
    assert_eq!(
        sent_paths(&mut helm),
        checks_of(&[7]),
        "the round's checks, now"
    );
    push_hosts(&mut helm, &[build_box], cx);
    assert_eq!(sent(&mut helm), []);
}

/// An unreachable place can fail a whole refresh. Keep the last known containers visible beside that error, then use
/// the next successful listing as the new answer, even if it is empty.
#[gpui::test]
fn a_failed_refresh_keeps_the_previous_leftovers_until_a_read_succeeds(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_containers_with(&mut helm, cx, &[], json!([leftover("build-check", None)]));
    assert!(drawn(cx, "containers-leftover-0-delete"));

    click(cx, "containers-refresh");
    let failed = round(&mut helm, &[]);
    helm.reply(
        failed.leftovers,
        502,
        Some(json!("container host unreachable")),
    );
    settle(cx);
    assert!(drawn(cx, "containers-leftover-0-delete"));
    assert!(drawn(cx, "containers-leftovers-error"));
    assert!(painted(cx).contains("container host unreachable"));

    click(cx, "containers-refresh");
    let recovered = round(&mut helm, &[]);
    helm.reply(recovered.leftovers, 200, Some(json!([])));
    settle(cx);
    assert!(drawn(cx, "containers-leftovers-none"));
    assert!(!drawn(cx, "containers-leftovers-error"));
    assert!(!drawn(cx, "containers-leftover-0-delete"));
}

// ---- Delete --------------------------------------------------------------------------------------------------------

/// SPEC.md: a container's removal destroys every session, process, file and unpushed change in it, so Delete must
/// always ask, Cancel must send nothing, and the confirmed request must name only the row that was asked about (its
/// place and canonical id; the helm rechecks ownership). A refusal must stay visible with the helm's words and leave
/// the row, so the user can retry ("retryable failure"). Specified: Delete on a leftover without a host row asks
/// "Delete container?" naming it, with focus on Cancel and nothing sent; Cancel closes it with nothing sent; Delete
/// container sends `DELETE /api/container-orphans` with that place and id, while Refresh is unavailable; the helm's
/// refusal shows under the section and the row stays; a second confirmed Delete that succeeds reads the leftovers
/// again.
#[gpui::test]
fn a_leftover_delete_asks_sends_its_place_and_id_and_shows_a_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_containers_with(
        &mut helm,
        cx,
        &[],
        json!([leftover("Interrupted launch", None)]),
    );
    click(cx, "containers-leftover-0-delete");
    let shown = painted_words(cx);
    assert!(shown.contains("Delete container?"), "{shown}");
    // The name is a host part of its own, so the painted text has a break around it.
    assert!(shown.contains("Delete Interrupted launch"), "{shown}");
    assert!(
        shown.contains(
            "Every session, process, file and unpushed change in it will be permanently removed."
        ),
        "{shown}"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(sent(&mut helm), [], "asking sends nothing");
    click(cx, "prompt-cancel");
    assert!(!drawn(cx, "prompt-confirm"), "Cancel closed the question");
    assert_eq!(sent(&mut helm), [], "Cancel sends nothing");

    click(cx, "containers-leftover-0-delete");
    click(cx, "prompt-confirm");
    let deletes = sent(&mut helm);
    assert_eq!(deletes.len(), 1, "{deletes:?}");
    assert_eq!(
        (deletes[0].method, deletes[0].path.as_str()),
        (Method::Delete, "/api/container-orphans")
    );
    assert_eq!(
        deletes[0].body,
        Some(json!({ "place": { "kind": "podman_host", "host": 7 }, "id": "a".repeat(64) }))
    );
    assert_eq!(
        available(cx, "containers-refresh"),
        Some(false),
        "busy while it is out"
    );
    helm.reply(
        deletes[0].id,
        502,
        Some(json!("container cleanup did not finish: fixture refusal")),
    );
    settle(cx);
    assert!(painted(cx).contains("container cleanup did not finish: fixture refusal"));
    assert!(painted(cx).contains("Interrupted launch"), "the row stays");
    assert_eq!(sent(&mut helm), [], "a refusal reads nothing");

    click(cx, "containers-leftover-0-delete");
    assert!(
        !painted(cx).contains("fixture refusal"),
        "asking again clears the old refusal"
    );
    click(cx, "prompt-confirm");
    let deletes = sent(&mut helm);
    helm.reply(deletes[0].id, 200, Some(json!({})));
    settle(cx);
    let reads = sent(&mut helm);
    assert_eq!(
        paths(&reads),
        [(Method::Get, LEFTOVERS.to_string())],
        "a done Delete reads the leftovers again"
    );
    assert!(drawn(cx, "containers-leftovers-reading"));
    assert!(!drawn(cx, "containers-leftovers-none"));
    assert!(!drawn(cx, "containers-leftover-0-delete"));
    helm.reply(reads[0].id, 502, Some(json!("another host unreachable")));
    settle(cx);
    assert!(drawn(cx, "containers-leftovers-error"));
    assert!(
        !drawn(cx, "containers-leftover-0-delete"),
        "the deleted row stays gone"
    );
}

/// SPEC_impl.md, "Podman places, creation and images": "Row-backed leftovers use the ordinary confirmed whole-host
/// Delete path with its actor hold and provisioning lock"; the container-orphans route refuses them. Specified: a
/// confirmed Delete of a leftover that has host row 12 sends `DELETE /api/hosts/12` with no body.
#[gpui::test]
fn a_leftover_with_a_host_row_is_deleted_through_host_removal(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    show_containers_with(
        &mut helm,
        cx,
        &[],
        json!([leftover("farhelm-sess-recorded", Some(12))]),
    );
    click(cx, "containers-leftover-0-delete");
    click(cx, "prompt-confirm");
    let deletes = sent(&mut helm);
    assert_eq!(deletes.len(), 1, "{deletes:?}");
    assert_eq!(
        (
            deletes[0].method,
            deletes[0].path.as_str(),
            deletes[0].body.clone()
        ),
        (Method::Delete, "/api/hosts/12", None)
    );
}

// ---- Refresh rounds ------------------------------------------------------------------------------------------------

/// A Refresh pressed while the previous round's checks are still out must not let that round's later answers undo it:
/// a stale refusal would replace the rows. The helm answers in send order, so the old round's checks answer after
/// Refresh but before its reads. Specified: with the first round's two checks unanswered, Refresh sends a new round;
/// the old checks then answering (one 202, one refused) show nothing and send nothing; the new round's answers leave
/// the rows showing, and send nothing more.
#[gpui::test]
fn a_refresh_drops_the_older_rounds_check_answers(cx: &mut TestAppContext) {
    let ready = |id, name| container_host(id, name, false, Some((100, None)));
    let (mut helm, cx, _gear) =
        open_rail_with_hosts(cx, &[ready(7, "build-box"), ready(8, "gpu-1")]);
    let first = show_containers(&mut helm, cx, &[7, 8]);
    helm.reply(first.leftovers, 200, Some(json!([])));
    settle(cx);

    click(cx, "containers-refresh");
    let next = round(&mut helm, &[7, 8]);
    helm.reply(first.checks[0].id, 202, None);
    helm.reply(
        first.checks[1].id,
        503,
        Some(json!("fixture stale check refusal")),
    );
    settle(cx);
    assert!(
        !painted(cx).contains("fixture stale check refusal"),
        "the old round's refusal is dropped"
    );
    assert_eq!(
        sent(&mut helm),
        [],
        "the old round sends nothing after its checks"
    );

    helm.reply(next.leftovers, 200, Some(json!([])));
    answer_checks(&mut helm, &next.checks, cx);
    assert!(painted_words(cx).contains("build-box Ready gpu-1 Ready"));
    assert_eq!(sent(&mut helm), [], "nothing more from either round");
}

/// The leftovers read needs its own guard: a read the old round sent before Refresh answers after it, in send order,
/// and taken as the newest it would put stale rows on screen. Specified: with the old round's leftovers read out when
/// Refresh is pressed, its answer (a leftover named stale-leftover) shows nothing, and the section goes on reading
/// until the new round's answer shows "None found".
#[gpui::test]
fn an_older_rounds_leftovers_answering_after_a_refresh_are_dropped(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let first = show_containers(&mut helm, cx, &[]);
    click(cx, "containers-refresh");
    let reads = sent_reads(&mut helm);
    let paths: Vec<&str> = reads.iter().map(|(_, path)| path.as_str()).collect();
    assert_eq!(
        paths,
        [LEFTOVERS],
        "premise: Refresh reads the leftovers again"
    );
    helm.reply(
        first.leftovers,
        200,
        Some(json!([leftover("stale-leftover", None)])),
    );
    settle(cx);
    let shown = painted(cx);
    assert!(
        !shown.contains("stale-leftover"),
        "the old round's leftovers are dropped"
    );
    assert!(shown.contains("Reading container leftovers…"), "{shown}");

    helm.reply(reads[0].0, 200, Some(json!([])));
    settle(cx);
    assert!(painted(cx).contains("None found"));
}
