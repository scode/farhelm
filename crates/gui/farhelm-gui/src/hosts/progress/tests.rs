//! Tests of what a host row makes of its provisioning run ([`super`]), without a window: what the panel keeps about a
//! run as the helm's progress changes, which runs open their row, what the inline status says and how its clock reads.
//! The panel's own tests (`hosts/tests.rs`) draw the same rules.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use farhelm_proto::provisioning::{
    ProvisioningOperation, ProvisioningView, RunStatus, StepStatus, StepView,
};

use super::{Run, Seen, Status, clock, described, fold, follow, opens_row, shown, status};
use crate::host_text::HostText;
use crate::hosts::tests::run;
use crate::operations::Work;

/// A step of a run, named `name`, in `status`.
pub(crate) fn step(name: &str, status: StepStatus) -> StepView {
    StepView {
        step: name.to_lowercase().replace(' ', "-"),
        name: name.to_owned(),
        paths: Vec::new(),
        explanation: None,
        status,
        message: None,
    }
}

/// An instant some way into the scheduler's day, and `secs` seconds after it.
fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000 + secs)
}

/// Host 2's setup in `status`, under the run id `id`, with three steps: one completed, one running, one pending.
fn setup(id: &str, status: RunStatus) -> ProvisioningView {
    let mut view = run(2, ProvisioningOperation::Add, status);
    view.run_id = Some(id.to_owned());
    view.steps = vec![
        step("Create directories", StepStatus::Completed),
        step("Upload the farhelm binary", StepStatus::Running),
        step("Install the farhelm binary", StepStatus::Pending),
    ];
    view
}

/// Why this matters: the elapsed time SPEC.md asks for is measured by the client, from when it first saw the run, and
/// the fold belongs to one run only ("folding one run cannot hide the next one"), so what the panel keeps must start
/// with a run, survive the run's progress, and go with it. Specified: a running run is recorded unfolded as first seen
/// at the time it is followed; following it again later keeps that time and a fold; another run id on the same host
/// replaces it with a fresh record; a run first seen already failed or completed is not recorded; a host whose run is
/// no longer pushed is forgotten.
#[test]
fn the_panel_keeps_a_runs_first_sight_and_fold_until_the_run_changes() {
    let mut seen = HashMap::new();
    follow(&mut seen, [&setup("run-a", RunStatus::Running)], at(0));
    let first = seen.get(&2).cloned().expect("a running run is recorded");
    follow(&mut seen, [&setup("run-a", RunStatus::Running)], at(30));
    assert_eq!(
        seen.get(&2),
        Some(&first),
        "a later push keeps the first sight"
    );
    fold(&mut seen, 2, "run-a");
    follow(&mut seen, [&setup("run-a", RunStatus::Failed)], at(40));
    assert!(
        !opens_row(Some(&setup("run-a", RunStatus::Running)), seen.get(&2)),
        "the fold is kept while the run is the same"
    );

    follow(&mut seen, [&setup("run-b", RunStatus::Running)], at(50));
    let next = seen.get(&2).expect("the next run is recorded");
    assert!(
        opens_row(Some(&setup("run-b", RunStatus::Running)), Some(next)),
        "a new run starts unfolded"
    );
    let Some(Status::Running { elapsed, .. }) = status(
        Some(&setup("run-b", RunStatus::Running)),
        Some(next),
        None,
        at(53),
    ) else {
        panic!("a running setup has an inline status");
    };
    assert_eq!(elapsed, Duration::from_secs(3), "with its own clock");

    let mut fresh = HashMap::new();
    follow(&mut fresh, [&setup("run-c", RunStatus::Failed)], at(0));
    follow(&mut fresh, [&setup("run-d", RunStatus::Completed)], at(0));
    assert!(fresh.is_empty(), "a finished run has no clock to start");

    follow(&mut seen, std::iter::empty(), at(60));
    assert!(seen.is_empty(), "a host with no run forgets its record");
}

/// Why this matters: SPEC.md, "Topology": "the running row expands once to show the helm's steps ... The user may fold
/// that run ... Success folds the row", while "While an update is running, its host row stays folded". Specified: a
/// running setup opens its row unless that very run was folded; a fold of the host's earlier run does not hold the new
/// one closed; a running uninstall opens its row like a setup and folds the same way; a completed or failed setup, a
/// running Update and the helm's idle view (no run id) open nothing here (a failure opens its row through the panel's
/// own rule); `shown` passes every run with an id.
#[test]
fn a_running_setup_opens_its_row_until_folded() {
    let running = setup("run-a", RunStatus::Running);
    assert!(opens_row(Some(&running), None));
    let mut seen = HashMap::new();
    follow(&mut seen, [&running], at(0));
    fold(&mut seen, 2, "run-a");
    assert!(!opens_row(Some(&running), seen.get(&2)), "folded");

    let mut older: HashMap<i64, Seen> = HashMap::new();
    follow(&mut older, [&setup("run-0", RunStatus::Running)], at(0));
    fold(&mut older, 2, "run-0");
    assert!(
        opens_row(Some(&running), older.get(&2)),
        "an earlier run's fold does not hold this one"
    );

    for status in [RunStatus::Completed, RunStatus::Failed] {
        assert!(!opens_row(Some(&setup("run-a", status)), None));
    }
    let update = run(2, ProvisioningOperation::Update, RunStatus::Running);
    assert!(!opens_row(Some(&update), None), "Update stays folded");
    assert!(
        shown(Some(&update)).is_some(),
        "an Update's steps are drawn"
    );
    let mut uninstall = run(2, ProvisioningOperation::Uninstall, RunStatus::Running);
    uninstall.run_id = Some("run-u".into());
    assert!(
        shown(Some(&uninstall)).is_some(),
        "an uninstall's steps are drawn"
    );
    assert!(
        opens_row(Some(&uninstall), None),
        "a running uninstall opens its row"
    );
    let mut seen = HashMap::new();
    follow(&mut seen, [&uninstall], at(0));
    fold(&mut seen, 2, "run-u");
    assert!(!opens_row(Some(&uninstall), seen.get(&2)), "and folds");
    let mut idle = setup("run-a", RunStatus::Running);
    idle.run_id = None;
    assert!(shown(Some(&idle)).is_none(), "the idle view is no run");
    assert!(shown(Some(&running)).is_some());
}

/// Why this matters: the inline status is the row's summary while a setup or an Update runs, folded or not (SPEC.md: it
/// "still says setting up or uninstalling and shows the step count, current step and elapsed time", and an Update
/// shows "`Updating…` until a progress snapshot is available, then ... the current step, completed-step count, and
/// client-measured elapsed time inline"), and a rerun's probe and submission show "Setting up…" before any run exists,
/// as the old row's planning status did. Specified: a running setup reads "Setting up", 1 done of 3 (a completed step
/// counts; a running or pending one does not), the running step's name, and the time since first sight; unchanged,
/// warning and failed steps count as done too; a running Update is counted and timed the same way as an Update's; with
/// a probe or a setup in flight and no running run it is pending setup, with an Update's request in flight pending
/// Update, also over the host's earlier failed run, and with an uninstall's pending uninstall ("Setup and uninstall
/// planning show their action in the folded row"); a save in flight, a completed run and no run give none.
#[test]
fn the_inline_status_counts_steps_names_the_current_one_and_times_the_run() {
    let running = setup("run-a", RunStatus::Running);
    let mut seen = HashMap::new();
    follow(&mut seen, [&running], at(0));
    assert_eq!(
        status(Some(&running), seen.get(&2), None, at(7)),
        Some(Status::Running {
            run: Run::Setup,
            done: 1,
            total: 3,
            current: Some(HostText::from_host("Upload the farhelm binary")),
            elapsed: Duration::from_secs(7),
        })
    );
    let mut finished = running.clone();
    finished.steps = vec![
        step("Remove the bundled tmux", StepStatus::Skipped),
        step("Enable lingering", StepStatus::Degraded),
        step("Connect", StepStatus::Failed),
    ];
    let Some(Status::Running { done, current, .. }) = status(Some(&finished), None, None, at(0))
    else {
        panic!("a running run has a status");
    };
    assert_eq!((done, current), (3, None));

    for work in [Work::Probe, Work::Setup] {
        assert_eq!(
            status(None, None, Some(work), at(0)),
            Some(Status::Pending(Run::Setup))
        );
    }
    let mut update = running.clone();
    update.operation = Some(ProvisioningOperation::Update);
    let Some(Status::Running {
        run: Run::Update,
        done: 1,
        total: 3,
        elapsed,
        ..
    }) = status(Some(&update), seen.get(&2), None, at(5))
    else {
        panic!("a running Update is counted as an Update");
    };
    assert_eq!(elapsed, Duration::from_secs(5));
    let failed = run(2, ProvisioningOperation::Update, RunStatus::Failed);
    assert_eq!(
        status(Some(&failed), None, Some(Work::UpdateHost), at(0)),
        Some(Status::Pending(Run::Update)),
        "a new Update's requests over the failed one"
    );
    let failed = run(2, ProvisioningOperation::Uninstall, RunStatus::Failed);
    assert_eq!(
        status(Some(&failed), None, Some(Work::UninstallHost), at(0)),
        Some(Status::Pending(Run::Uninstall)),
        "an uninstall's requests, the continuation's over its failed run"
    );
    assert_eq!(status(None, None, Some(Work::Save), at(0)), None);
    assert_eq!(
        status(
            Some(&setup("run-a", RunStatus::Completed)),
            None,
            None,
            at(0)
        ),
        None
    );
    assert_eq!(status(None, None, None, at(0)), None);
}

/// Why this matters: the clock is read at a glance beside a step name, and the old status's `m:ss` grew hours rather
/// than capping, so a stuck hour-long setup reads as one. When the panel redraws it is the ticker's one-second clock's
/// business (`hosts/tests.rs` pins that the count moves on by itself). Specified: 6 seconds read "0:06", 75 "1:15",
/// 3909 "1:05:09", and a part second is dropped rather than rounded up (2.75 s reads "0:02").
#[test]
fn the_clock_reads_minutes_and_seconds_and_grows_hours() {
    assert_eq!(clock(Duration::from_secs(6)), "0:06");
    assert_eq!(clock(Duration::from_secs(75)), "1:15");
    assert_eq!(clock(Duration::from_secs(3_909)), "1:05:09");
    assert_eq!(clock(Duration::from_millis(2_750)), "0:02");
}

/// Why this matters: a running status's hover is the progress card, whose words ("Progress of the run on <host>") are
/// the only place a screen reader learns what the hover holds, so the status must be described by them; a pending
/// status's help is its own name again, which a screen reader would read twice as a description. The check is on the
/// rule `status_mark` applies, because GPUI's test platform never turns its accessibility tree on and the frame's
/// record has no field for a description. Specified: a running status (setup or Update) is described by its help; a
/// pending one is not.
#[test]
fn a_running_status_is_described_by_its_cards_words_and_a_pending_one_is_not() {
    let running = |run| Status::Running {
        run,
        done: 1,
        total: 3,
        current: None,
        elapsed: Duration::from_secs(5),
    };
    assert!(described(&running(Run::Setup)));
    assert!(described(&running(Run::Update)));
    assert!(!described(&Status::Pending(Run::Setup)));
    assert!(!described(&Status::Pending(Run::Update)));
}
