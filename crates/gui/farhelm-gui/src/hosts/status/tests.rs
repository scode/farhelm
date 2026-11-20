//! Tests of the per-host state the rail and the attention list read ([`super`]): first its rules without a window (the
//! update badge, live work, the warning's lifecycle and Always update's newest choice), then in the real main view over
//! the helm client's fake, where the sidebar's Hosts control opens and closes the panel, a refused Update and a reset
//! meet the warning, and Always update's save fails, survives Hosts, is tried again and is retired by Settings.

use std::collections::HashSet;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::provisioning::{ProvisioningOperation, ProvisioningView, RunStatus, StepStatus};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HostStateView, HostView, Preferences,
    RefreshView,
};
use gpui::{TestAppContext, VisualTestContext, px, size};
use serde_json::json;

use super::{
    Action, Activity, HostStatus, Live, Observed, State, Steps, auto_update_failed, host_status,
    live, update_badge, warned,
};
use crate::focus::Mark;
use crate::host_text::HostText;
use crate::hosts::progress::tests::step;
use crate::hosts::tests::{connected, connecting, host, hosts, progress, run, skew};
use crate::hosts::update::Urgency;
use crate::operations::{OpKey, Work};
use crate::session;
use crate::session_list::tests::{listing, open_main, preferences_state};
use crate::test_support::{click, meta, settle};

use ProvisioningOperation::{Add, Uninstall, Update};
use RunStatus::{Completed, Failed, Running};

// ---- Fixtures --------------------------------------------------------------------------------------------------------

/// A connected host on build `0.15.0`, older than the helm's, as the helm marks it.
fn old_build() -> HostStateView {
    HostStateView::Connected {
        identity: Some("id-old".into()),
        build_version: "0.15.0".into(),
        old_version: true,
        newer_version: false,
        refresh: RefreshView::Ok { sessions: 1 },
    }
}

/// A connected host on a build newer than the helm's.
fn newer_build() -> HostStateView {
    HostStateView::Connected {
        identity: Some("id-new".into()),
        build_version: "9.0.0".into(),
        old_version: false,
        newer_version: true,
        refresh: RefreshView::Ok { sessions: 0 },
    }
}

/// Host 2's retained run `id` of `operation` in `status`, as [`State::observe`] takes it.
fn seen(
    id: &str,
    operation: ProvisioningOperation,
    status: RunStatus,
) -> Vec<(i64, Option<Observed>)> {
    vec![(
        2,
        Some(Observed {
            run: id.to_owned(),
            operation,
            status,
        }),
    )]
}

/// The host progress collection with no entry: host 2's run is withdrawn.
fn withdrawn() -> Vec<(i64, Option<Observed>)> {
    Vec::new()
}

/// `state` after following `runs`, with no host list read and the preference off.
fn follow(state: &mut State, runs: &[(i64, Option<Observed>)]) {
    state.observe(Some(runs), None, false);
}

// ---- The update badge ------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md, "Session list": "Having an available Update action does not itself mean the host needs an
/// update: the rail's update badge and the attention list's Update rows require a compatible older build or a lower
/// protocol, as well as an available remote Update action". The old `classify` test crossed age, offers, busy state and
/// kind the same way. Specified: a remote host on an older build is optional and one with a lower protocol required; a
/// current host, a newer build, a higher protocol, an unreachable or connecting host, the helm's own machine on an older
/// build, a host with work in flight and one with a run going on get no badge.
#[test]
fn the_badge_needs_an_available_update_and_an_older_build_or_protocol() {
    let badge = |state: HostStateView| update_badge(&host(2, "ssh", state), None, false);
    assert_eq!(badge(old_build()), Some(Urgency::Optional));
    assert_eq!(badge(skew(1, 2)), Some(Urgency::Required));
    assert_eq!(badge(connected()), None, "current");
    assert_eq!(badge(newer_build()), None, "newer build");
    assert_eq!(badge(skew(3, 2)), None, "higher protocol");
    assert_eq!(
        badge(HostStateView::Unreachable {
            cause: "transport".into(),
            last_error: "offline".into(),
        }),
        None,
        "unreachable"
    );
    assert_eq!(badge(connecting(1, None)), None, "connecting");
    assert_eq!(
        update_badge(&host(1, "local", old_build()), None, false),
        None,
        "the helm's own machine is never updated from here"
    );
    let older = host(2, "ssh", old_build());
    assert_eq!(update_badge(&older, None, true), None, "work in flight");
    let running = run(2, Update, Running);
    assert_eq!(
        update_badge(&older, Some(&running), false),
        None,
        "a run going on"
    );
    let failed = run(2, Update, Failed);
    assert_eq!(
        update_badge(&older, Some(&failed), false),
        Some(Urgency::Optional),
        "a failed run leaves the host idle"
    );
}

// ---- Live work -------------------------------------------------------------------------------------------------------

/// Why this matters: SPEC.md, "Session list": "Live setup, update or uninstall progress takes precedence over a
/// problem", working rows name "the current step's name when known", and their bar fills "as steps complete when a step
/// count is available". Live work is derived from the pushed run and the operation store, so it cannot go stale.
/// Specified: a running Update with three steps is Update with 1 of 3 done and the running step named; a running
/// uninstall is Uninstall; a running run wins over requests out; without one, a confirmed setup's submission out is
/// Setup and an Update's requests Update, both without steps; a rerun's probe (nothing confirmed yet, as in the old
/// app), a removal, a host settings save, a sandbox's Resume, a failed or completed run and an idle host are no live
/// work.
#[test]
fn live_work_is_the_running_run_or_else_the_requests_out() {
    let mut update = run(2, Update, Running);
    update.steps = vec![
        step("Upload the farhelm binary", StepStatus::Completed),
        step("Install the farhelm binary", StepStatus::Running),
        step("Connect to the supervisor", StepStatus::Pending),
    ];
    assert_eq!(
        live(Some(&update), Some(Work::UpdateHost)),
        Some(Live {
            activity: Activity::Update,
            steps: Some(Steps {
                done: 1,
                total: 3,
                current: Some(HostText::from_host("Install the farhelm binary")),
            }),
        })
    );
    let uninstall = run(2, Uninstall, Running);
    assert_eq!(
        live(Some(&uninstall), None).map(|live| live.activity),
        Some(Activity::Uninstall)
    );
    let pending = |work| live(None, Some(work));
    for (work, activity) in [
        (Work::Setup, Activity::Setup),
        (Work::UpdateHost, Activity::Update),
    ] {
        assert_eq!(
            pending(work),
            Some(Live {
                activity,
                steps: None
            }),
            "{work:?}"
        );
    }
    assert_eq!(pending(Work::Probe), None, "a rerun's probe");
    assert_eq!(pending(Work::RemoveHost), None, "a removal");
    assert_eq!(pending(Work::Save), None, "a settings save");
    // SPEC.md's working rows and rings are setup, Update and uninstall only; the old attention list treated an
    // explicit lifecycle request as routine work.
    assert_eq!(
        pending(Work::Resume),
        None,
        "a sandbox's or container's Resume"
    );
    assert_eq!(live(Some(&run(2, Update, Failed)), None), None, "failed");
    assert_eq!(live(Some(&run(2, Add, Completed)), None), None, "completed");
    assert_eq!(live(None, None), None, "idle");
}

// ---- The warning, without a window -----------------------------------------------------------------------------------

/// Why this matters: SPEC.md, "Topology": "When setup or Update fails or has an uncertain outcome while Hosts is closed,
/// the host's rail avatar shows a warning mark until Hosts is opened". Opening acknowledges the mark, not the retained
/// failure, so the same failure seen again must not warn again (the old app's "otherwise the next read of a retained
/// failure would relatch it"). Specified: an Update run that fails while Hosts is closed warns; opening Hosts clears it;
/// the same failed run followed again after Hosts closes stays clear; a refused Update while Hosts is closed warns
/// again; a failure while Hosts is open, followed or refused, warns nothing.
#[test]
fn a_failure_while_hosts_is_closed_warns_until_hosts_opens() {
    let mut state = State::default();
    follow(&mut state, &seen("r1", Update, Running));
    assert!(!state.warned(2), "the premise: a running run warns nothing");
    follow(&mut state, &seen("r1", Update, Failed));
    assert!(state.warned(2));
    state.opened();
    assert!(!state.warned(2), "opening Hosts acknowledges it");
    state.open = false;
    follow(&mut state, &seen("r1", Update, Failed));
    assert!(
        !state.warned(2),
        "the acknowledged failure does not relatch"
    );
    state.raise(2, Action::Update);
    assert!(state.warned(2), "a refusal warns");

    let mut open = State::default();
    open.opened();
    follow(&mut open, &seen("r1", Update, Running));
    follow(&mut open, &seen("r1", Update, Failed));
    open.raise(2, Action::Setup);
    assert!(!open.warned(2), "Hosts shows the failure itself");
}

/// Why this matters: SPEC.md, "Topology": the mark lasts "until Hosts is opened or a later run of that same action
/// succeeds. Starting a retry does not acknowledge the warning, and an unrelated successful action does not clear it.
/// Setup recovery by discovery can resolve the same failed setup run." Specified: after a failed Update, a running
/// retry keeps the warning and its completion clears it; a refused Update's warning survives a completed setup run; a
/// failed setup run reported completed (discovery) clears its warning.
#[test]
fn only_a_later_success_of_the_same_action_clears_the_warning() {
    let mut state = State::default();
    follow(&mut state, &seen("r1", Update, Failed));
    follow(&mut state, &seen("r2", Update, Running));
    assert!(state.warned(2), "a retry does not acknowledge it");
    follow(&mut state, &seen("r2", Update, Completed));
    assert!(!state.warned(2), "the retry succeeded");

    state.raise(2, Action::Update);
    follow(&mut state, &seen("r3", Add, Completed));
    assert!(
        state.warned(2),
        "a setup's success does not clear an Update's warning"
    );

    let mut setup = State::default();
    follow(&mut setup, &seen("r1", Add, Failed));
    assert!(setup.warned(2), "the premise: the setup failed");
    follow(&mut setup, &seen("r1", Add, Completed));
    assert!(!setup.warned(2), "discovery resolved the same run");
}

/// Why this matters: an "uncertain outcome" warns as a failure does (SPEC.md, "Topology"). The helm keeps one run per
/// host, so a run seen running that the helm stops showing without a result ended where nobody saw how. The helm drops a
/// host with no retained run from the collection, so a withdrawn run is a missing entry. Specified: a running Update
/// whose entry is gone warns; a running setup replaced by another run warns; the same while Hosts is open warns
/// nothing; an uninstall that fails or vanishes never warns, since uninstall keeps its own notices.
#[test]
fn a_run_that_ends_unseen_is_uncertain_and_uninstall_never_warns() {
    let mut state = State::default();
    follow(&mut state, &seen("r1", Update, Running));
    follow(&mut state, &withdrawn());
    assert!(state.warned(2), "the Update ended unseen");

    let mut replaced = State::default();
    follow(&mut replaced, &seen("r1", Add, Running));
    follow(&mut replaced, &seen("r2", Add, Running));
    assert!(replaced.warned(2), "the first setup ended unseen");

    let mut open = State::default();
    open.opened();
    follow(&mut open, &seen("r1", Update, Running));
    follow(&mut open, &withdrawn());
    assert!(!open.warned(2));

    let mut uninstall = State::default();
    follow(&mut uninstall, &seen("r1", Uninstall, Running));
    follow(&mut uninstall, &withdrawn());
    follow(&mut uninstall, &seen("r2", Uninstall, Failed));
    assert!(!uninstall.warned(2));
}

/// Why this matters: a removed host has no avatar to warn, and a warning kept for its id would come back if the id
/// were ever shown again. Specified: a warned host the registry no longer lists is forgotten; while the host list has
/// not been read, nothing is forgotten.
#[test]
fn a_host_the_registry_no_longer_lists_is_forgotten() {
    let mut state = State::default();
    state.raise(2, Action::Setup);
    state.observe(None, None, false);
    assert!(state.warned(2), "no host list read yet");
    state.observe(None, Some(&HashSet::from([1])), false);
    assert!(!state.warned(2));
    assert!(state.hosts.is_empty());
}

/// Why this matters: SPEC.md, "Session list": Always update's error "stops applying after a newer choice for that
/// setting", and SPEC_impl.md: "a later choice on either surface supersedes it". The old app's preference queue did
/// this with its newest-write check; here each choice is numbered. Specified: a failed save is an error while it is the
/// newest choice; a later choice retires it; the save of an older choice failing after a newer one started is no error;
/// the preference read on retires the failure for good, so it does not return when the preference reads off again.
#[test]
fn only_the_newest_choices_failed_save_is_an_error() {
    let mut state = State::default();
    let first = state.choose();
    state.save_failed(first);
    assert!(state.save_error());
    state.choose();
    assert!(!state.save_error(), "a newer choice retires it");

    let older = state.choose();
    let newer = state.choose();
    state.save_failed(older);
    assert!(!state.save_error(), "an older choice's failure is stale");
    state.save_failed(newer);
    assert!(state.save_error());
    state.observe(None, None, true);
    assert!(!state.save_error(), "someone turned it on since");
    state.observe(None, None, false);
    assert!(!state.save_error(), "and it stays retired");
}

// ---- In the main view ------------------------------------------------------------------------------------------------

/// Two remote hosts on an older build (2 and 3) beside the local host, as the registry lists them.
fn listed() -> Vec<HostView> {
    vec![
        host(1, "local", connected()),
        host(2, "ssh", old_build()),
        host(3, "ssh", old_build()),
    ]
}

/// The real main view over the fake helm with [`listed`], no retained runs and `preferences`, in a window as large as
/// the reference screens'. Sessions shows.
fn main(cx: &mut TestAppContext, preferences: Preferences) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, cx) = open_main(cx, snapshot(preferences));
    cx.simulate_resize(size(px(1600.), px(1000.)));
    settle(cx);
    answer_trash_reads(&mut helm, cx);
    (helm, cx)
}

/// Answer what the main view reads at startup, each connected host's checkout trash, with nothing, so the tests see only
/// the commands their own steps send: the fake answers commands in the order they were sent, so these go first.
fn answer_trash_reads(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    for read in helm.commands() {
        assert_eq!(
            read.path, "/api/checkout-trash/list",
            "the premise: only trash reads went out at startup"
        );
        helm.reply(read.id, 200, None);
    }
    settle(cx);
}

/// [`main`]'s snapshot.
fn snapshot(preferences: Preferences) -> Vec<CollectionState> {
    snapshot_with(preferences, &[])
}

/// [`snapshot`] with `runs` retained.
fn snapshot_with(preferences: Preferences, runs: &[ProvisioningView]) -> Vec<CollectionState> {
    vec![
        listing(&[], &[]),
        // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
        preferences_state(Preferences {
            welcome_seen: Some(true),
            ..preferences
        }),
        hosts(&listed()),
        progress(runs),
    ]
}

/// Remove host `host`'s entry from the host progress collection, as the helm does when it withdraws a run, and settle.
fn withdraw_run(helm: &mut FakeHelm, host: i64, cx: &mut VisualTestContext) {
    helm.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&[]),
        upsert: Vec::new(),
        remove: vec![HostView::key(host)],
    });
    settle(cx);
}

/// Open Hosts and close it again with the Hosts control, acknowledging every warning, and check that host `host` no
/// longer warns.
fn acknowledge(host: i64, cx: &mut VisualTestContext) {
    click(cx, "hosts-panel-toggle");
    click(cx, "hosts-panel-toggle");
    assert!(!hosts_shown(cx), "the premise: Sessions shows again");
    assert!(!is_warned(host, cx), "the premise: acknowledged");
}

/// Push `view` as its host's retained run, and settle.
fn push_run(helm: &mut FakeHelm, view: ProvisioningView, cx: &mut VisualTestContext) {
    let key = HostView::key(view.host_id);
    helm.push(CollectionChanges {
        collection: Collection::HostProgress,
        meta: meta(&[key.as_str()]),
        upsert: progress(&[view]).entries,
        remove: Vec::new(),
    });
    settle(cx);
}

/// Push `preferences` as the preference row, and settle.
fn push_preferences(helm: &mut FakeHelm, preferences: Preferences, cx: &mut VisualTestContext) {
    let state = preferences_state(preferences);
    helm.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
}

/// Host 2's Update run `id` in `status`.
fn update_run(id: &str, status: RunStatus) -> ProvisioningView {
    let mut view = run(2, Update, status);
    view.run_id = Some(id.to_owned());
    view
}

/// Whether host `host` shows the warning now.
fn is_warned(host: i64, cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| warned(host, cx))
}

/// Whether the attention list would show Always update's failure now.
fn save_failed(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| auto_update_failed(cx))
}

/// Whether the Hosts panel is drawn.
fn hosts_shown(cx: &mut VisualTestContext) -> bool {
    crate::test_support::frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some("hosts-panel"))
}

/// The commands sent since the last call, as (method, path, body), and their ids in the order sent.
fn sent(helm: &mut FakeHelm) -> (Vec<(Method, String, serde_json::Value)>, Vec<u64>) {
    helm.commands()
        .into_iter()
        .map(
            |FakeCommand {
                 id,
                 method,
                 path,
                 body,
             }| ((method, path, body.unwrap_or_default()), id),
        )
        .unzip()
}

/// The helm's plan reply to an Update's plan request, offering the one-use plan `probe_id`; the client reads only the
/// id.
fn plan(probe_id: &str) -> serde_json::Value {
    json!({ "probe_id": probe_id, "plan": {}, "confirmation": "", "steps": { "lead": "", "steps": [], "kept": null } })
}

/// The id of the one preference write sent since the last call, checking that it turns automatic updates on and that
/// nothing else was sent.
fn one_preference_write(helm: &mut FakeHelm) -> u64 {
    let commands = helm.commands();
    assert_eq!(commands.len(), 1, "one command: {commands:?}");
    let command = &commands[0];
    assert_eq!(
        (&command.method, command.path.as_str(), command.body.clone()),
        (
            &Method::Put,
            "/api/preferences",
            Some(json!({ "auto_update_remote_hosts": true }))
        )
    );
    command.id
}

/// Why this matters: the warning exists for failures the user did not see, and opening Hosts is where they see them,
/// so the sidebar's Hosts control must tell the panel when it shows and hides it. Specified: in the main view with
/// Sessions showing, host 2's Update run failing warns host 2 only, and the rail's one read of host 2 carries the warning
/// beside its update badge; pressing Hosts clears it; another failed run while Hosts shows warns nothing; after pressing
/// Hosts again (Sessions shows), a third failed run warns again.
#[gpui::test]
fn the_hosts_control_acknowledges_the_warning_and_failures_while_it_shows_warn_nothing(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = main(cx, Preferences::default());
    assert!(!hosts_shown(cx), "the premise: Sessions shows");
    push_run(&mut helm, update_run("r1", Running), cx);
    push_run(&mut helm, update_run("r1", Failed), cx);
    assert!(is_warned(2, cx));
    assert!(!is_warned(3, cx), "only the failed host");
    assert_eq!(
        cx.update(|_, cx| host_status(&listed()[1], cx)),
        HostStatus {
            sign_in: None,
            update: Some(Urgency::Optional),
            live: None,
            warned: true,
        },
        "the rail's one read of the host agrees: idle again, so its update badge shows beside the warning"
    );

    click(cx, "hosts-panel-toggle");
    assert!(hosts_shown(cx), "the premise: Hosts shows");
    assert!(!is_warned(2, cx), "opening Hosts acknowledged it");
    push_run(&mut helm, update_run("r2", Failed), cx);
    assert!(!is_warned(2, cx), "Hosts shows the failure itself");

    click(cx, "hosts-panel-toggle");
    assert!(!hosts_shown(cx), "the premise: Sessions shows again");
    push_run(&mut helm, update_run("r3", Failed), cx);
    assert!(is_warned(2, cx));
}

/// Why this matters: a refused Update plan (a newer database schema, a host below the upgrade floor) is a failed Update
/// even though no run exists, and the attention list's Update sends it while Hosts is closed. The warning lives in UI
/// memory only, so a reset forgets it, as SPEC.md's update dismissal is forgotten; a failure the helm still retains is
/// seen afresh by the new UI session and warns again, as the old app's reload did. Specified: with Sessions showing, an
/// Update of host 2 whose plan the helm refuses warns host 2; after the channel breaks and the helm connects again with
/// the same state, host 2 no longer warns; after another reset whose snapshot retains a failed Update run for host 2,
/// it warns again.
#[gpui::test]
fn a_refused_update_warns_and_a_reset_forgets_it(cx: &mut TestAppContext) {
    let (mut helm, cx) = main(cx, Preferences::default());
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        crate::hosts::update::start(2, mark, window, cx);
    });
    settle(cx);
    let (plan, _) =
        crate::test_support::one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    assert!(!is_warned(2, cx), "the premise: nothing has failed yet");
    helm.reply(
        plan,
        409,
        Some(json!(
            "the host's database schema is newer than this helm's"
        )),
    );
    settle(cx);
    assert!(is_warned(2, cx));

    helm.break_channel();
    settle(cx);
    helm.connect(snapshot(Preferences::default()));
    settle(cx);
    assert!(!is_warned(2, cx), "the reset forgot it");

    helm.break_channel();
    settle(cx);
    helm.connect(snapshot_with(
        Preferences::default(),
        &[update_run("r1", Failed)],
    ));
    settle(cx);
    assert!(
        is_warned(2, cx),
        "a failure the helm still retains warns again in the new UI session"
    );
}

/// Why this matters: the helm withdraws a run it already showed running (a refusal after the run was announced) by
/// removing the host's progress entry, never by pushing an idle one, so this is the only way the rail sees an
/// uncertain ending through the real pipeline; and the withdrawn run must not linger as "running" and make the host's
/// next run warn. Specified: with Sessions showing, host 2's running Update whose entry the helm removes warns host 2;
/// once acknowledged, the host's next run pushed running warns nothing.
#[gpui::test]
fn a_withdrawn_run_warns_and_the_next_run_does_not(cx: &mut TestAppContext) {
    let (mut helm, cx) = main(cx, Preferences::default());
    push_run(&mut helm, update_run("r1", Running), cx);
    assert!(
        !is_warned(2, cx),
        "the premise: a running run warns nothing"
    );
    withdraw_run(&mut helm, 2, cx);
    assert!(is_warned(2, cx), "the withdrawn run ended unseen");
    acknowledge(2, cx);
    push_run(&mut helm, update_run("r2", Running), cx);
    assert!(!is_warned(2, cx), "the next run is no news");
}

/// Why this matters: a registered host's setup rerun reaches the helm twice (the probe, then the submission) and each
/// can fail with Hosts closed once the rail's avatar menu offers the rerun; nothing pushed records either failure, so
/// only the completion's own warning tells the rail. Specified: with Sessions showing and "Set up new hosts without
/// asking" on, host 2's rerun probe refused by the helm warns host 2; after acknowledging, a rerun whose probe returns a
/// plan sends the setup at once, warning nothing yet, and that submission's refusal warns host 2.
#[gpui::test]
fn a_refused_setup_rerun_and_a_refused_submission_warn(cx: &mut TestAppContext) {
    let skips = Preferences {
        skip_host_setup_confirmation: Some(true),
        ..Preferences::default()
    };
    let (mut helm, cx) = main(cx, skips);
    let rerun = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            let mark = Mark::take(cx);
            crate::hosts::setup::rerun(&listed()[1], mark, window, cx);
        });
        settle(cx);
    };
    rerun(cx);
    let (probe, _) = crate::test_support::one_command(&mut helm, Method::Post, "/api/hosts/probe");
    helm.reply(probe, 409, Some(json!("the host refused the connection")));
    settle(cx);
    assert!(is_warned(2, cx), "the refused probe");
    acknowledge(2, cx);

    rerun(cx);
    let (probe, _) = crate::test_support::one_command(&mut helm, Method::Post, "/api/hosts/probe");
    helm.reply(
        probe,
        200,
        Some(json!({
            "result": "provisionable",
            "probe_id": "p-1",
            "plan": {},
            "confirmation": "",
            "steps": { "lead": "", "steps": [], "kept": null },
        })),
    );
    settle(cx);
    let (submission, _) =
        crate::test_support::one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert!(!is_warned(2, cx), "the premise: nothing has failed yet");
    helm.reply(submission, 409, Some(json!("the plan was already used")));
    settle(cx);
    assert!(is_warned(2, cx), "the refused submission");
}

/// Why this matters: an Update fails without a dialog in two ways that only the row records, a plan reply the app
/// cannot use and a refused confirming request, and either can happen with Hosts closed from the attention list's
/// Update. Specified: with Sessions showing, an Update of host 2 whose plan reply names no plan warns host 2; after
/// acknowledging, an Update whose plan is answered sends the confirming request, warning nothing yet, and that
/// request's refusal warns host 2.
#[gpui::test]
fn an_unreadable_plan_and_a_refused_confirmation_warn(cx: &mut TestAppContext) {
    let (mut helm, cx) = main(cx, Preferences::default());
    let start = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            let mark = Mark::take(cx);
            crate::hosts::update::start(2, mark, window, cx);
        });
        settle(cx);
    };
    start(cx);
    let (request, _) =
        crate::test_support::one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    helm.reply(request, 200, Some(json!({ "not": "a plan" })));
    settle(cx);
    assert!(is_warned(2, cx), "the unreadable plan");
    acknowledge(2, cx);

    start(cx);
    let (request, _) =
        crate::test_support::one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    helm.reply(request, 200, Some(plan("u-1")));
    settle(cx);
    let (confirmation, body) =
        crate::test_support::one_command(&mut helm, Method::Post, "/api/hosts/2/update");
    assert_eq!(
        body,
        json!({ "probe_id": "u-1" }),
        "the premise: the confirmation"
    );
    assert!(!is_warned(2, cx), "the premise: nothing has failed yet");
    helm.reply(confirmation, 409, Some(json!("the plan was already used")));
    settle(cx);
    assert!(is_warned(2, cx), "the refused confirmation");
}

/// Why this matters: SPEC.md, "Session list": Always update "requests the same displayed hosts as Update all, then
/// turns on that preference ... A failed save leaves the requested updates running and, while the preference still
/// reads off, shows an error ... with Try again saving only the preference. The error survives opening and closing
/// Hosts". Specified: Always update for hosts 2 and 3 sends each host's Update plan request and then the preference
/// write turning automatic updates on, under the preference row's key, host 2 reading as updating with no badge
/// meanwhile; once both plans are answered and the write refused, both Updates go on to their confirming requests and
/// the error shows; pressing Hosts twice keeps it; Try again sends the preference write alone and hides the error while
/// it is out; its refusal shows it again; a done save with the preference pushed on hides it.
#[gpui::test]
fn always_update_requests_the_hosts_then_saves_and_a_failed_save_waits_for_try_again(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = main(
        cx,
        Preferences {
            auto_update_remote_hosts: Some(false),
            ..Preferences::default()
        },
    );
    cx.update(|window, cx| {
        let mark = Mark::take(cx);
        super::always_update(&[2, 3], mark, window, cx);
    });
    settle(cx);
    let (commands, ids) = sent(&mut helm);
    assert_eq!(
        commands,
        vec![
            (
                Method::Post,
                "/api/hosts/2/update".into(),
                serde_json::Value::Null
            ),
            (
                Method::Post,
                "/api/hosts/3/update".into(),
                serde_json::Value::Null
            ),
            (
                Method::Put,
                "/api/preferences".into(),
                json!({ "auto_update_remote_hosts": true })
            ),
        ]
    );
    let in_flight = |key: OpKey, cx: &mut VisualTestContext| {
        cx.update(|_, cx| session::current_operations(cx)?.read(cx).in_flight(&key))
    };
    assert_eq!(in_flight(OpKey::preferences(), cx), Some(Work::Save));
    assert!(!save_failed(cx), "no error while the save is out");
    assert_eq!(
        cx.update(|_, cx| host_status(&listed()[1], cx)),
        HostStatus {
            sign_in: None,
            update: None,
            live: Some(Live {
                activity: Activity::Update,
                steps: None,
            }),
            warned: false,
        },
        "while its plan request is out, host 2 is updating and its badge is gone"
    );
    // The fake answers in the order sent: both plans (each Update goes on to its confirming request), then the write.
    helm.reply(ids[0], 200, Some(plan("u-2")));
    helm.reply(ids[1], 200, Some(plan("u-3")));
    helm.reply(ids[2], 500, Some(json!("disk full")));
    settle(cx);
    let (confirmations, confirming) = sent(&mut helm);
    assert_eq!(
        confirmations,
        vec![
            (
                Method::Post,
                "/api/hosts/2/update".into(),
                json!({ "probe_id": "u-2" })
            ),
            (
                Method::Post,
                "/api/hosts/3/update".into(),
                json!({ "probe_id": "u-3" })
            ),
        ]
    );
    assert!(save_failed(cx));
    assert_eq!(
        in_flight(OpKey::host("2"), cx),
        Some(Work::UpdateHost),
        "the Updates go on"
    );
    assert_eq!(in_flight(OpKey::host("3"), cx), Some(Work::UpdateHost));
    for id in confirming {
        helm.reply(id, 202, Some(json!({ "run_id": "run" })));
    }
    settle(cx);

    click(cx, "hosts-panel-toggle");
    assert!(save_failed(cx), "it survives opening Hosts");
    click(cx, "hosts-panel-toggle");
    assert!(save_failed(cx), "and closing it");

    cx.update(|_, cx| super::retry_auto_update(cx));
    settle(cx);
    let again = one_preference_write(&mut helm);
    assert!(!save_failed(cx), "a newer choice is out");
    helm.reply(again, 500, Some(json!("disk full")));
    settle(cx);
    assert!(save_failed(cx), "Try again failed too");

    cx.update(|_, cx| super::retry_auto_update(cx));
    settle(cx);
    let last = one_preference_write(&mut helm);
    push_preferences(
        &mut helm,
        Preferences {
            auto_update_remote_hosts: Some(true),
            ..Preferences::default()
        },
        cx,
    );
    helm.reply(last, 200, None);
    settle(cx);
    assert!(!save_failed(cx));
}

/// Why this matters: SPEC.md, "Session list": Always update's error "stops applying after a newer choice for that
/// setting", and SPEC_impl.md: "a later choice on either surface supersedes it", so a user who goes to Settings → Hosts
/// and chooses there is not shown a stale alert. Specified: after Always update's save fails, pressing the Settings
/// switch "Update remote hosts automatically" retires the error at once, and the error stays retired when that save is
/// refused too, Settings showing its own failure line instead; and after another failed Always update, the preference
/// pushed on from elsewhere and then off again leaves no error.
#[gpui::test]
fn a_newer_choice_in_settings_or_elsewhere_retires_the_failed_save(cx: &mut TestAppContext) {
    let off = Preferences {
        auto_update_remote_hosts: Some(false),
        ..Preferences::default()
    };
    let (mut helm, cx) = main(cx, off.clone());
    cx.update(|_, cx| super::retry_auto_update(cx));
    settle(cx);
    let write = one_preference_write(&mut helm);
    helm.reply(write, 500, Some(json!("disk full")));
    settle(cx);
    assert!(save_failed(cx), "the premise: the save failed");

    click(cx, "app-settings-toggle");
    click(cx, "settings-section-hosts");
    click(cx, "settings-auto-update-remote-hosts");
    assert!(!save_failed(cx), "the Settings press is a newer choice");
    let settings = one_preference_write(&mut helm);
    helm.reply(settings, 500, Some(json!("disk full")));
    settle(cx);
    assert!(
        !save_failed(cx),
        "Settings' own failure is not Always update's"
    );
    assert!(
        crate::test_support::frame(cx)
            .iter()
            .any(|node| node.element_id.as_deref()
                == Some("settings-auto-update-remote-hosts-error")),
        "Settings shows its failure under its switch"
    );
    crate::test_support::keys(cx, "escape");

    cx.update(|_, cx| super::retry_auto_update(cx));
    settle(cx);
    let write = one_preference_write(&mut helm);
    helm.reply(write, 500, Some(json!("disk full")));
    settle(cx);
    assert!(save_failed(cx), "the premise: failed again");
    let on = Preferences {
        auto_update_remote_hosts: Some(true),
        ..Preferences::default()
    };
    push_preferences(&mut helm, on, cx);
    push_preferences(&mut helm, off, cx);
    assert!(!save_failed(cx), "another client's choice came since");
}
