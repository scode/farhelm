//! Tests of a launch into a new container in the real main view over the helm client's fake, from its press to what
//! the run it is accepted onto ends in. The launcher opens on a draft already in a container of `web` on host1, as
//! the draft's Runs on edit leaves it; the switch that makes that edit is `environment_view_tests`'s.
//!
//! The helm answers an environment create 202 with `{host_id, run_id}` once it admits the launch, and the outcome
//! arrives on that run in the pushed host progress (SPEC_impl.md, "The UI channel"). So every launch here is answered
//! in that order: the run pushed (the channel pushes what a command changed before its reply), the 202, then the run's
//! end, and for a completed run the read of the new host's sessions. The new host is 99, the run `r1`.
//!
//! The fleet is the helm's pinned fixture's first three hosts: host1 (connected, Podman usable), host2 (Podman not
//! installed) and host3 (no Podman check); the catalog holds `web`, on managed Ubuntu, which a container runs.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HostView, KeyedEntry, Preferences,
};
use farhelm_proto::{LaunchTarget, Place, ProviderKind};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{LaunchDraft, RunsOn};
use super::environment_tests::recipe_json;
use super::launch_tests::command_draft;
use super::view::open;
use super::view_tests::{fixture_hosts, name_of, read, shown_refusal};
use crate::focus::Mark;
use crate::local_state;
use crate::main_view::MainView;
use crate::notices::{self, Notice};
use crate::session_list::tests::{listing, preferences_state, session};
use crate::test_support::{self, any_open, keys, meta, settle, started};

/// The new environment's host, which the helm registers as it admits the launch.
const NEW_HOST: i64 = 99;
/// The run the launch's outcome lands on.
const RUN: &str = "r1";

/// The pushed catalog holding `web`.
pub(crate) fn catalog() -> CollectionState {
    CollectionState {
        collection: Collection::Recipes,
        meta: meta(&["web"]),
        entries: vec![KeyedEntry {
            key: "web".into(),
            value: recipe_json("web", json!({ "kind": "tensorlake_ubuntu" })),
        }],
    }
}

/// Host progress with no run, as a helm that has run nothing pushes it.
pub(super) fn no_runs() -> CollectionState {
    CollectionState {
        collection: Collection::HostProgress,
        meta: meta(&[]),
        entries: Vec::new(),
    }
}

/// The launch's run on [`NEW_HOST`] as the helm lists it: `status` ("running", "completed" or "failed"), with one
/// step running while it runs, `message` as its reason, and `unknown` as its marker for an outcome the helm cannot
/// vouch for.
fn run(status: &str, message: Option<&str>, unknown: bool) -> Value {
    let step = if status == "running" {
        "running"
    } else {
        "completed"
    };
    json!({
        "host_id": NEW_HOST,
        "run_id": RUN,
        "operation": "add",
        "status": status,
        "steps": [{
            "step": "prepare",
            "name": "Preparing web on host1…",
            "paths": [],
            "explanation": null,
            "status": step,
            "message": null,
        }],
        "message": message,
        "outcome_unknown": unknown,
    })
}

/// Push host progress as holding `run` alone, under its host.
fn push_run(helm: &mut FakeHelm, cx: &mut VisualTestContext, run: Value) {
    let key = HostView::key(NEW_HOST);
    helm.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&[key.as_str()]),
        upsert: vec![KeyedEntry { key, value: run }],
        remove: Vec::new(),
    });
    settle(cx);
}

/// Push host progress with the launch's run gone.
fn drop_run(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    helm.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&[]),
        upsert: Vec::new(),
        remove: vec![HostView::key(NEW_HOST)],
    });
    settle(cx);
}

/// The real main view over the fake helm with the fixture's three hosts, session `a`, `preferences`, the catalog and
/// no runs, and the launcher opened on `draft`.
pub(super) fn window_with(
    cx: &mut TestAppContext,
    draft: LaunchDraft,
    hosts: CollectionState,
    preferences: Preferences,
) -> (FakeHelm, &mut VisualTestContext) {
    window_with_more(cx, draft, hosts, preferences, Vec::new())
}

/// [`window_with`], with `more` collections in the helm's first snapshot: the store ignores a push to a collection
/// that snapshot did not install, so a test that pushes one (the recipe places) must list it here first.
pub(super) fn window_with_more(
    cx: &mut TestAppContext,
    draft: LaunchDraft,
    hosts: CollectionState,
    preferences: Preferences,
    more: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    let mut snapshot = vec![
        listing(&["a"], &[session("a")]),
        hosts,
        preferences_state(preferences),
        catalog(),
        no_runs(),
    ];
    snapshot.extend(more);
    helm.connect(snapshot);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    window.update(|window, cx| {
        let mark = Mark::take(cx);
        assert!(
            open(draft, &mark, window, cx).is_some(),
            "the launcher opened"
        );
    });
    settle(window);
    (helm, window)
}

/// [`command_draft`] in a new container of `web` on host1, as New container's segment enters it.
pub(crate) fn container_draft() -> LaunchDraft {
    let mut draft = command_draft();
    draft.choose_runs_on(
        RunsOn::New(ProviderKind::Podman),
        Some(LaunchTarget::Environment {
            recipe: "web".into(),
            place: Place::PodmanHost(1),
        }),
    );
    draft
}

/// The preferences every window here starts with: the Welcome card seen, no container host remembered.
pub(super) fn plain_preferences() -> Preferences {
    Preferences {
        welcome_seen: Some(true),
        ..Preferences::default()
    }
}

/// [`window_with`] on [`container_draft`], host1 to host3 and [`plain_preferences`].
fn window(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    window_with(
        cx,
        container_draft(),
        fixture_hosts(&[0, 1, 2]),
        plain_preferences(),
    )
}

/// Answer every command the window sent (an empty catalog for `launch-catalog`, an empty history, nothing for the rest)
/// round after round, except those `keep` picks, which are returned unanswered.
pub(crate) fn answer_except(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    keep: impl Fn(&FakeCommand) -> bool,
) -> Vec<FakeCommand> {
    let mut kept = Vec::new();
    loop {
        cx.run_until_parked();
        let mut answered = false;
        for command in helm.commands() {
            if keep(&command) {
                kept.push(command);
                continue;
            }
            let body = if command.path.ends_with("launch-catalog") {
                Some(json!([]))
            } else if command.path.starts_with("/api/launch-history") {
                Some(json!({ "launches": [], "folders": [], "checkout_config_revision": 1 }))
            } else {
                None
            };
            helm.reply(command.id, 200, body);
            answered = true;
        }
        settle(cx);
        if !answered {
            return kept;
        }
    }
}

/// Press Launch on the container draft and return the one environment create it sent, the launcher hidden.
fn launch_container(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    answer_except(helm, cx, |_| false);
    test_support::click(cx, "launcher-launch");
    let sent = answer_except(helm, cx, |command| {
        command.path == "/api/environment-sessions"
    });
    let [create] = &sent[..] else {
        panic!("one press sent one environment create, not {sent:?}");
    };
    assert!(!any_open(cx), "the launcher hid while its create is out");
    create.clone()
}

/// The run pushed, then the create answered 202 naming it, as the helm admits a launch.
fn accept(helm: &mut FakeHelm, cx: &mut VisualTestContext, create: &FakeCommand) {
    push_run(helm, cx, run("running", None, false));
    helm.reply(
        create.id,
        202,
        Some(json!({ "host_id": NEW_HOST, "run_id": RUN })),
    );
    settle(cx);
}

/// The run completed, with session `key` made on the new host: the helm lists it, and the window's read of the host's
/// sessions, sent once although the store changes again while it is out, is answered with it.
fn complete(helm: &mut FakeHelm, cx: &mut VisualTestContext, key: &'static str) {
    let rows = [session("a"), session(key)];
    helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: listing(&["a", key], &rows).meta,
        upsert: rows.iter().map(|row| row.entry()).collect(),
        remove: Vec::new(),
    });
    push_run(helm, cx, run("completed", None, false));
    // Another store change while the read is out must not send a second one (`RunWait::reading`).
    push_run(helm, cx, run("completed", None, false));
    let host = format!("/api/sessions?host={NEW_HOST}");
    let reads = answer_except(helm, cx, |command| command.path == host);
    let [read] = &reads[..] else {
        panic!("the completed run's host was read once, not {reads:?}");
    };
    helm.reply(
        read.id,
        200,
        Some(json!({ "sessions": [session(key).entry().value], "total": 1, "truncated": false })),
    );
    settle(cx);
}

/// The session the window has selected.
fn selected(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| local_state::selected(cx))
}

/// Whether the launcher counts as busy for New and for the host registry's changes.
fn busy(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| super::launch::busy(cx))
}

// ---- Launching ------------------------------------------------------------------------------------------------------

/// SPEC_impl.md, "Tensorlake sandbox sessions": a recipe launch has its own route and never falls through to the
/// ordinary create. Specified: the press sends one `POST /api/environment-sessions` naming `web` on host1's place,
/// unconfirmed, with a create in `~` naming no host.
#[gpui::test]
fn a_new_container_sends_one_environment_create(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    let body = create.body.expect("a create carries a body");
    assert_eq!(body["recipe"], json!("web"));
    assert_eq!(body["place"], json!({ "kind": "podman_host", "host": 1 }));
    assert_eq!(body["confirmed"], json!(false));
    assert_eq!(body["request"]["cwd"], json!("~"));
    assert_eq!(body["request"]["host"], Value::Null);
}

/// The session pane's New-as-focus-home relies on the session opening from the pushed outcome, never from the 202, and
/// SPEC.md's "A successful New or Clone opens its session only while ... the user has pressed no key" applies to the
/// whole create. Specified: after the 202 nothing opens and the launcher stays busy; once the run completes and the new
/// host's session `b` is read, `b` opens, as an ordinary create's reply opens its session.
#[gpui::test]
fn a_completed_run_opens_its_session_only_then(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    assert!(
        busy(cx),
        "the 202 settled nothing: the launch is still in flight"
    );
    assert!(!any_open(cx));
    complete(&mut helm, cx, "b");
    assert_eq!(
        selected(cx).as_deref(),
        Some("b"),
        "the run's session opened"
    );
    assert!(!busy(cx), "and the launch is over");
}

/// SPEC.md, "Creation": a success the user moved on from moves nothing, and "a display-only pill says Started “name”".
/// Specified: a key press after the press, then the run completing with `b`: the selection stays and the Started
/// report names `b`.
#[gpui::test]
fn a_completed_run_the_user_moved_on_from_reports_started(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    let before = selected(cx);
    keys(cx, "x");
    complete(&mut helm, cx, "b");
    assert_eq!(selected(cx), before, "the selection stayed");
    assert_eq!(
        name_of(cx, "launch-started-report").as_deref(),
        Some("Started “b”")
    );
}

/// SPEC.md, "Quick launch": "When New was never shown and success selects its session, the rename dialog opens over
/// it". An environment create's success goes through the same settle, so a hidden quick launch into a container is
/// named too. Specified: a launcher prepared out of sight on a container draft launches without showing; its run's
/// completion opens `b` with the naming editor over it.
#[gpui::test]
fn an_unseen_quick_launch_into_a_container_is_handed_to_naming(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    answer_except(&mut helm, cx, |_| false);
    test_support::click(cx, "launcher-cancel");
    let draft = container_draft();
    cx.update(|window, cx| {
        let finish = |_: &mut LaunchDraft, _: super::draft::Catalog<'_>, _: &gpui::App| {
            super::view::Prepared::Launch { notice: None }
        };
        assert!(super::view::prepare_unseen(
            draft,
            Mark::take(cx),
            finish,
            window,
            cx
        ));
    });
    let sent = answer_except(&mut helm, cx, |command| {
        command.path == "/api/environment-sessions"
    });
    let [create] = &sent[..] else {
        panic!("the quick launch sent one environment create, not {sent:?}");
    };
    assert!(!any_open(cx), "the launcher was never shown");
    accept(&mut helm, cx, create);
    complete(&mut helm, cx, "b");
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert!(
        name_of(cx, "rename-cancel").is_some(),
        "the naming editor opened over it, not the launcher"
    );
}

/// SPEC.md, "Sandbox sessions" (which containers follow): a failed launch "shows Not launched. with the reason", and a
/// failure "restores the same launcher with its draft". The helm's run already starts its reason with "Not launched.",
/// so the launcher must not say it twice, and the recipe and place must survive, or a retry means choosing them again.
/// Specified: the run fails with "Not launched. the image did not build"; the launcher comes back saying exactly that,
/// still in a container of `web` on host1.
#[gpui::test]
fn a_failed_run_brings_the_launcher_back_with_its_recipe_and_place(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    push_run(
        &mut helm,
        cx,
        run(
            "failed",
            Some("Not launched. the image did not build"),
            false,
        ),
    );
    assert!(any_open(cx), "the launcher came back");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. the image did not build")
    );
    assert_eq!(
        read(cx, |launcher, _, _| launcher.draft.target.clone()),
        Some(LaunchTarget::Environment {
            recipe: "web".into(),
            place: Place::PodmanHost(1),
        })
    );
    assert!(!busy(cx));
}

/// The helm pushes the run before its 202, so the look at the run made as the 202 arrives must find it: nothing else
/// may come to wake the launch. Specified: a run already failed when the 202 arrives brings the launcher back at once,
/// with no later push.
#[gpui::test]
fn the_look_at_the_202_sees_the_run(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    push_run(
        &mut helm,
        cx,
        run("failed", Some("Not launched. no room"), false),
    );
    helm.reply(
        create.id,
        202,
        Some(json!({ "host_id": NEW_HOST, "run_id": RUN })),
    );
    settle(cx);
    assert_eq!(shown_refusal(cx).as_deref(), Some("Not launched. no room"));
}

/// SPEC_impl.md, "The UI channel": "A run the client waits for that is no longer listed means the client cannot learn
/// how the launch ended; Hosts shows whatever environment it left." The launcher must say that rather than "Not
/// launched.", since the environment may exist. Specified: the run is listed, then goes; the launcher comes back with
/// its own words for that, without the lead.
#[gpui::test]
fn a_run_that_goes_away_comes_back_without_not_launched(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    drop_run(&mut helm, cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(
            "Farhelm can no longer see how this launch ended. Check Hosts for the environment before launching \
             again."
        )
    );
}

/// The notice offering a launcher whose launch ended unknown says to check Hosts, since the environment may exist; a
/// lost run is such an ending. Specified: a key press after the 202, then the run going away: nothing opens, and the
/// environment notice, not the not-launched one, is posted.
#[gpui::test]
fn a_lost_run_the_user_moved_on_from_has_the_environment_notice(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    keys(cx, "x");
    drop_run(&mut helm, cx);
    assert!(!any_open(cx));
    let posted = cx.update(|_, cx| notices::posted(cx));
    assert_eq!(posted, vec![Notice::EnvironmentLaunchUnknown]);
}

/// A 202 that names no run leaves nothing to follow, so the launch's end cannot be learned, and the launcher must say
/// so rather than wait for good. Specified: a 202 with an empty body brings the launcher back with its words for an
/// unknown outcome, without "Not launched.", and the launch is no longer in flight.
#[gpui::test]
fn a_202_naming_no_run_comes_back_as_unknown(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    helm.reply(create.id, 202, Some(json!({})));
    settle(cx);
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(
            "Farhelm can no longer see how this launch ended. Check Hosts for the environment before launching \
             again."
        )
    );
    assert!(!busy(cx));
}

/// A run the helm marks as ending without a known result (a launch task that ended without one) may have left an
/// environment behind, so the notice offering its launcher after the user moved on says to check Hosts, not that
/// nothing launched. Specified: a key press, then the run failing with the marker: nothing opens, and the
/// environment notice, not the not-launched one, is posted.
#[gpui::test]
fn an_unknown_outcome_the_user_moved_on_from_has_its_own_notice(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    keys(cx, "x");
    push_run(
        &mut helm,
        cx,
        run(
            "failed",
            Some(
                "The launch task ended without a result. Check Hosts for the named environment before launching again.",
            ),
            true,
        ),
    );
    assert!(!any_open(cx));
    let posted = cx.update(|_, cx| notices::posted(cx));
    assert_eq!(posted, vec![Notice::EnvironmentLaunchUnknown]);
}

/// Drafts do not survive a reset (decided by the maintainer for the migration), and the launch's wait goes with them:
/// nothing may act on a run of a UI session that is gone. Specified: a reset while the run runs leaves no launcher, no
/// notice and nothing busy, and the run's end pushed to the new session brings nothing back.
#[gpui::test]
fn a_reset_during_the_wait_leaves_nothing_behind(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    helm.break_channel();
    helm.connect(vec![listing(&["a"], &[session("a")]), no_runs()]);
    settle(cx);
    assert!(!busy(cx), "the wait went with the UI session");
    push_run(
        &mut helm,
        cx,
        run("failed", Some("Not launched. no room"), false),
    );
    assert!(!any_open(cx), "the run's end brought nothing back");
    let posted = cx.update(|_, cx| notices::posted(cx));
    for launch in [
        Notice::LaunchNotStarted,
        Notice::LaunchMayStillStart,
        Notice::EnvironmentLaunchUnknown,
    ] {
        assert!(
            !posted.contains(&launch),
            "{launch:?}: only the reset's own notice may show"
        );
    }
}

/// SPEC.md, "Creation": "Launches and changes to the host registry still exclude each other." An environment create's
/// operation key clears on the 202 while the launch goes on for minutes, so the launcher's busy state is what holds the
/// host registry's side back. Specified: after the 202, while the run runs, Add host's submission sends no probe and
/// says why, a host's Update sends nothing, and host2's Remove sends no removal; once the run ends, a launch no longer
/// counts as in flight.
#[gpui::test]
fn host_changes_wait_for_the_whole_environment_create(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    assert!(cx.update(|_, cx| crate::hosts::launch_in_flight(cx)));

    cx.update(|window, cx| {
        let opener = cx.focus_handle();
        assert!(crate::hosts::add::open(&opener, &Mark::take(cx), window, cx).is_some());
    });
    settle(cx);
    cx.simulate_input("dev@newbox");
    keys(cx, "enter");
    assert_eq!(helm.commands(), [], "Add host probed nothing");
    assert!(
        test_support::label(cx, "add-host-error")
            .is_some_and(|line| line.contains("a session launch is in progress")),
        "{:?}",
        test_support::label(cx, "add-host-error")
    );
    keys(cx, "escape");

    cx.update(|window, cx| crate::hosts::update::start(2, Mark::take(cx), window, cx));
    settle(cx);
    assert_eq!(helm.commands(), [], "Update sent nothing");
    let lines = cx.update(|_, cx| crate::hosts::setup::panel_lines(cx));
    assert!(
        lines.iter().any(|line| line
            .accessible_name()
            .contains(crate::hosts::LAUNCH_IN_FLIGHT)),
        "and the Hosts panel's line says why"
    );

    cx.update(|window, cx| {
        assert!(crate::hosts::remove::ask(2, &Mark::take(cx), window, cx).is_some());
    });
    settle(cx);
    test_support::click(cx, "prompt-confirm");
    assert_eq!(helm.commands(), [], "Remove sent nothing");
    assert!(
        test_support::label(cx, "host-remove-not-started")
            .is_some_and(|line| line.contains(crate::hosts::LAUNCH_IN_FLIGHT)),
        "and the question says why: {:?}",
        test_support::label(cx, "host-remove-not-started")
    );
    keys(cx, "escape");

    push_run(
        &mut helm,
        cx,
        run("failed", Some("Not launched. no room"), false),
    );
    assert!(!cx.update(|_, cx| crate::hosts::launch_in_flight(cx)));
}

/// SPEC.md, "Container sessions": "Launch progress names Preparing <recipe> on <host>…, … Creating a <recipe> container
/// on <host>…", each with elapsed time, rather than the plain "Launching … on <host>…", since the steps take minutes.
/// Specified: two seconds after the press of an accepted container launch, the progress pill is named by the run's
/// running step.
#[gpui::test]
fn the_progress_pill_names_the_runs_step(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    test_support::advance(cx, std::time::Duration::from_millis(2_100));
    assert_eq!(
        name_of(cx, "launch-progress").as_deref(),
        Some("Preparing web on host1…")
    );
}

/// The explanation [`accept_explained`] gives the run's running step.
const EXPLANATION: &str = "Pulling the base image";

/// [`accept`], with the run's running step carrying [`EXPLANATION`].
fn accept_explained(helm: &mut FakeHelm, cx: &mut VisualTestContext, create: &FakeCommand) {
    let mut running = run("running", None, false);
    running["steps"][0]["explanation"] = json!(EXPLANATION);
    push_run(helm, cx, running);
    helm.reply(
        create.id,
        202,
        Some(json!({ "host_id": NEW_HOST, "run_id": RUN })),
    );
    settle(cx);
}

/// SPEC.md, "Creation": the pending pill shows "elapsed seconds", and the run starts as the helm admits the launch,
/// just after the press. Counted from the pill's first draw instead, the first step would read "0 s" after the user
/// had already waited two seconds. Specified: two seconds after the press of an accepted container launch, the pill
/// is named by the run's first step and paints "2 s", not "0 s".
#[gpui::test]
fn the_progress_pills_first_step_counts_from_the_press(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept(&mut helm, cx, &create);
    test_support::advance(cx, std::time::Duration::from_millis(2_100));
    assert_eq!(
        name_of(cx, "launch-progress").as_deref(),
        Some("Preparing web on host1…"),
        "the premise: the pill shows the run's step"
    );
    let painted = crate::session_list::tests::painted(&test_support::frame(cx));
    assert!(
        painted.iter().any(|line| line == "2 s"),
        "the step counts from the press, in {painted:?}"
    );
}

/// The pill is a status named by its words, and a screen reader hears only that name: a step's explanation drawn beside
/// it but left out of the name would never be heard. Specified: two seconds after the press of an accepted container
/// launch whose running step has an explanation, the pill is named by the step and then its explanation.
#[gpui::test]
fn the_progress_pill_names_its_steps_explanation(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx);
    let create = launch_container(&mut helm, cx);
    accept_explained(&mut helm, cx, &create);
    test_support::advance(cx, std::time::Duration::from_millis(2_100));
    assert_eq!(
        name_of(cx, "launch-progress"),
        Some(format!("Preparing web on host1… {EXPLANATION}"))
    );
}

// ---- A quick launch on a container host with no Podman check result ------------------------------------------------

/// A Podman check with no result yet, as the helm pushes it for every host after it restarts.
fn unchecked() -> Value {
    json!({ "checked_at": null, "refreshing": false, "usable": false, "reason": null })
}

/// Host1 as the fixture has it, with `check` as its Podman check.
fn host1_with(check: Value) -> KeyedEntry {
    let mut host = fixture_hosts(&[0]).entries.remove(0);
    host.value["container_check"] = check;
    host
}

/// After a helm restart no host has a Podman check result until something asks, and a template's quick launch prepares
/// its launcher without showing it, so the launcher's opening never asked for it. Without its own ask it came back with
/// "this host needs a Podman check" and nothing checked the host until the user opened Settings or a new launcher.
/// Specified: a launcher prepared out of sight on host1's container draft, with host1 never checked, asks for host1's
/// check exactly once and sends no create; it comes back shown with that refusal; once host1's result is pushed usable,
/// Launch sends one environment create.
#[gpui::test]
fn a_quick_launch_asks_for_the_checks_and_comes_back_until_one_answers(cx: &mut TestAppContext) {
    let mut hosts = fixture_hosts(&[0, 1, 2]);
    hosts.entries[0] = host1_with(unchecked());
    let (mut helm, cx) = window_with(cx, container_draft(), hosts, plain_preferences());
    answer_except(&mut helm, cx, |_| false);
    test_support::click(cx, "launcher-cancel");
    assert!(!any_open(cx), "the premise: the opened launcher is gone");
    cx.update(|window, cx| {
        let finish = |_: &mut LaunchDraft, _: super::draft::Catalog<'_>, _: &gpui::App| {
            super::view::Prepared::Launch { notice: None }
        };
        assert!(super::view::prepare_unseen(
            container_draft(),
            Mark::take(cx),
            finish,
            window,
            cx
        ));
    });
    // Taken before anything is answered, so the asks can be counted; the fake helm answers strictly in order, so these
    // are answered here, in order, as `answer_except` would, before it answers the rest.
    cx.run_until_parked();
    let taken = helm.commands();
    let asks = taken
        .iter()
        .filter(|command| command.path == "/api/hosts/1/podman-probe")
        .count();
    let paths: Vec<&str> = taken.iter().map(|command| command.path.as_str()).collect();
    assert_eq!(
        asks, 1,
        "the quick launch asked for host1's check once, in {paths:?}"
    );
    for command in &taken {
        let body = command.path.ends_with("launch-catalog").then(|| json!([]));
        helm.reply(command.id, 200, body);
    }
    let sent = answer_except(&mut helm, cx, |command| {
        command.path == "/api/environment-sessions"
    });
    assert_eq!(sent, [], "and sent no create while host1 is unchecked");
    assert!(any_open(cx), "the refused quick launch came back shown");
    assert!(
        shown_refusal(cx).is_some_and(|refusal| refusal.contains("this host needs a Podman check")),
        "with the unchecked host's refusal, not {:?}",
        shown_refusal(cx)
    );

    let usable = host1_with(
        json!({ "checked_at": 1_700_000_400, "refreshing": false, "usable": true, "reason": null }),
    );
    helm.push(CollectionChanges {
        collection: Collection::Hosts,
        meta: meta(&["1", "2", "3"]),
        upsert: vec![usable],
        remove: Vec::new(),
    });
    settle(cx);
    test_support::click(cx, "launcher-launch");
    let sent = answer_except(&mut helm, cx, |command| {
        command.path == "/api/environment-sessions"
    });
    let [create] = &sent[..] else {
        panic!("Launch sent one environment create once host1 was usable, not {sent:?}");
    };
    assert_eq!(create.path, "/api/environment-sessions");
}
