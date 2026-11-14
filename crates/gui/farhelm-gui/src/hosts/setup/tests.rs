//! Tests of setting up a new SSH host over the helm client's fake: the confirmation a probe's plan opens (what it
//! shows, where focus starts, Copy plan), what each answer sends and under which key, setup without asking, and where
//! a refused setup shows, before and after the user moved on.
//!
//! They start where the Add host tests leave off, through the same root view and helpers (`hosts/add/tests.rs`): Add
//! host opened from its rail button, a destination typed, Add pressed, and the fake helm answering the probe with
//! [`plan_reply`]. Every step ends in one of the shared harness's step helpers, so the focus and layer invariants are
//! checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::provisioning::{PlanStep, PlanStepPath, PlanSteps};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use crate::focus;
use crate::hosts::add::tests::{
    alias, any_open, destination, dialog_key, error_line, label, one_command, one_probe,
    open_dialog, open_rail_with, type_text,
};
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{click, focused, focused_id, frame, keys, meta, settle};

/// The plan the fake helm offers: two steps, the first with two paths and an explanation, in the helm's own shape.
const CONFIRMATION: &str = "Farhelm will set up dev@newbox (ubuntu, x86_64):\n- Create directories\n  \
                            /home/dev/.local/lib/farhelm\n  /home/dev/.local/state/farhelm\n  Existing shared \
                            directories keep their permissions.\n- Upload the farhelm binary\n";

/// A `provisionable` probe reply offering [`CONFIRMATION`]'s plan under the one-use id `p-1`.
fn plan_reply() -> Value {
    json!({
        "result": "provisionable",
        "probe_id": "p-1",
        "plan": { "the helm's own": "plan" },
        "confirmation": CONFIRMATION,
        "steps": {
            "lead": "Farhelm will set up dev@newbox (ubuntu, x86_64):",
            "steps": [
                {
                    "id": "create-directories",
                    "name": "Create directories",
                    "paths": [
                        { "full": "/home/dev/.local/lib/farhelm", "short": "~/.local/lib/farhelm" },
                        { "full": "/home/dev/.local/state/farhelm", "short": "~/.local/state/farhelm" },
                    ],
                    "explanation": "Existing shared directories keep their permissions.",
                },
                { "id": "upload-farhelm", "name": "Upload the farhelm binary", "paths": [], "explanation": null },
            ],
            "kept": null,
        },
    })
}

/// The preference row with "Set up new hosts without asking" set to `on`.
fn preferences(on: bool) -> CollectionState {
    CollectionState {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: json!({ "skip_host_setup_confirmation": on }),
        }],
    }
}

/// Open Add host, type `dev@newbox` and the alias `newbox1`, press Add, and answer the probe with [`plan_reply`]; the
/// dialog's host-add key.
fn probe_a_plan(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> OpKey {
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    keys(cx, "tab");
    type_text(cx, "newbox1");
    let key = dialog_key(cx);
    click(cx, "add-host-submit");
    let (id, _) = one_probe(helm);
    helm.reply(id, 200, Some(plan_reply()));
    settle(cx);
    key
}

/// The work in flight on `key`.
fn work(key: &OpKey, cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| session::current_operations(cx)?.read(cx).in_flight(key))
}

/// Every line of text the window painted, joined with single spaces.
fn painted(cx: &mut VisualTestContext) -> String {
    let lines: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect();
    lines
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The helm's refusal of a plan it no longer holds.
const USED: &str = "the provisioning plan is unknown or has already been used; probe again";

/// The reference screens' plan (`host-add-setup-plan`): the setup of `dev@new-box` as the old app showed it, eight
/// steps with their paths and explanations, for the screenshot golden.
pub(crate) fn reference_offer(opener: gpui::FocusHandle, mark: crate::focus::Mark) -> super::Offer {
    let tmp = "farhelm-tmp-35ff1630-e1a5-4759-96d9-9303ddf6a2fa";
    let step = |id: &str, name: &str, paths: &[&str], explanation: Option<&str>| PlanStep {
        id: id.into(),
        name: name.into(),
        paths: paths
            .iter()
            .map(|short| PlanStepPath {
                full: short.replacen('~', "/home/dev", 1),
                short: (*short).into(),
            })
            .collect(),
        explanation: explanation.map(str::to_owned),
    };
    let steps = PlanSteps {
        lead: "Farhelm will set up dev@new-box (ubuntu, x86_64):".into(),
        steps: vec![
            step(
                "create-directories",
                "Create directories",
                &[
                    "~/.local/lib/farhelm",
                    "~/.local/state/farhelm",
                    "~/.config/systemd/user",
                ],
                Some("Existing shared directories keep their permissions."),
            ),
            step(
                "upload-farhelm",
                "Upload the farhelm binary",
                &[&format!("~/.local/lib/farhelm/.farhelm.{tmp}")],
                None,
            ),
            step(
                "install-farhelm",
                "Install the farhelm binary",
                &[
                    "~/.local/lib/farhelm/farhelm",
                    &format!("~/.local/lib/farhelm/.farhelm.{tmp}"),
                ],
                None,
            ),
            step(
                "write-unit",
                "Write the systemd user unit",
                &[
                    "~/.config/systemd/user/farhelm-supervisor.service",
                    &format!("~/.config/systemd/user/.farhelm-supervisor.service.{tmp}"),
                ],
                Some("Unit: farhelm-supervisor.service"),
            ),
            step("reload-units", "Reload systemd user units", &[], None),
            step(
                "enable-supervisor",
                "Enable and start the supervisor",
                &["~/.config/systemd/user/farhelm-supervisor.service"],
                Some("The supervisor runs persistently under the systemd user manager"),
            ),
            step(
                "enable-linger",
                "Enable lingering, to start at boot",
                &[],
                Some(
                    "The supervisor starts at boot if linger succeeds; if lingering is refused, starts at login, \
                     not at boot",
                ),
            ),
            step(
                "remove-tmux",
                "Remove the bundled tmux",
                &["~/.local/lib/farhelm/tmux"],
                Some(
                    "Earlier releases installed their own tmux here; Farhelm no longer uses it, and tmux typed in a \
                     session afterwards no longer runs this copy.",
                ),
            ),
        ],
        kept: None,
    };
    super::Offer {
        key: OpKey::new_host_add(),
        probe_id: "p-reference".into(),
        confirmation: String::new(),
        steps,
        destination: "dev@new-box".into(),
        alias: String::new(),
        opener,
        mark,
    }
}

// ---- The confirmation ----------------------------------------------------------------------------------------------

/// SPEC.md, "Topology": the panel "shows a concrete installation plan ... and proceeds only on confirmation". The
/// confirmation is where the user reads what will change on their machine, so it must show the helm's plan, steps,
/// paths and all, offer the old app's three answers in their order, and start on Set up, the action the user asked
/// for. Specified: a `provisionable` reply closes Add host and opens "Set up host · dev@newbox" with its summary, the
/// plan's lead, step names, short paths and explanation, Copy plan and the footer note; the answers read Cancel, "Set
/// up, don't ask again" and Set up in that order; focus is on Set up; nothing more was sent.
#[gpui::test]
fn a_plan_opens_the_setup_confirmation_in_place_of_add_host(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    probe_a_plan(&mut helm, cx);
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Set up host · dev@newbox")
    );
    assert_eq!(destination_open(cx), None, "Add host closed");
    let text = painted(cx);
    for shown in [
        "Farhelm is not running there yet",
        "Farhelm will set up dev@newbox (ubuntu, x86_64):",
        "Create directories",
        "~/.local/lib/farhelm",
        "~/.local/state/farhelm",
        "Existing shared directories keep their permissions.",
        "Upload the farhelm binary",
        "Copy plan",
        "Undo \"don't ask again\" with the gear at the foot of the navigation rail",
    ] {
        assert!(text.contains(shown), "{shown:?} is drawn: {text}");
    }
    let answers: Vec<String> = frame(cx)
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
        .collect();
    assert_eq!(answers, ["Cancel", "Set up, don't ask again", "Set up"]);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(helm.commands(), [], "nothing was sent before an answer");
}

/// Whether Add host's destination field is drawn, and what it holds.
fn destination_open(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx)
        .into_iter()
        .any(|node| node.element_id.as_deref() == Some("add-host-destination"))
        .then(|| destination(cx))
}

/// The old app had a Copy plan button so the plan could be read or kept elsewhere before anything changes; it copies
/// the helm's own text, so what is pasted is exactly what the helm will do. Specified: Copy plan puts the helm's
/// `confirmation` text on the clipboard, trailing newline included, and leaves the question open with nothing sent.
#[gpui::test]
fn copy_plan_copies_the_helms_plan_text(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    probe_a_plan(&mut helm, cx);
    click(cx, "setup-copy-plan");
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some(CONFIRMATION)
    );
    assert!(any_open(cx), "the question stays");
    assert_eq!(helm.commands(), []);
}

// ---- The answers ---------------------------------------------------------------------------------------------------

/// Confirming hands the helm its own one-use plan id and nothing else, so it can act only on the plan it showed; the
/// work goes through the operation store under the key the probe used, so it goes on after the question closes and a
/// second setup for the same add is refused. Specified: Set up closes the question, focus goes back to Add host, and
/// `POST /api/hosts/provision` with `{"probe_id": "p-1"}` is the dialog's key's work; the helm's 202 frees the key and
/// opens nothing.
#[gpui::test]
fn set_up_submits_the_plan_under_the_probes_key_and_closes(cx: &mut TestAppContext) {
    let (mut helm, cx, add) = open_rail_with(cx, Vec::new());
    let key = probe_a_plan(&mut helm, cx);
    click(cx, "prompt-confirm");
    assert!(!any_open(cx), "the question closed at once");
    assert!(focused(cx, &add), "focus went back to Add host");
    let (id, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "p-1" }));
    assert_eq!(work(&key, cx), Some(Work::Setup));

    helm.reply(id, 202, Some(json!({ "host_id": 3, "run_id": "run-3" })));
    settle(cx);
    assert_eq!(work(&key, cx), None, "accepted frees the key");
    assert!(!any_open(cx), "and opens nothing");
}

/// "Set up, don't ask again" is the old app's permanent answer: it sets this host up and turns on the shared preference
/// that skips the question next time, which Settings shows and can turn off. Specified: it sends the provision request
/// for `p-1` and `PUT /api/preferences` with only `{"skip_host_setup_confirmation": true}`, in either order, and closes
/// the question.
#[gpui::test]
fn the_permanent_answer_also_stops_asking(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    probe_a_plan(&mut helm, cx);
    click(cx, "prompt-alternative-0");
    assert!(!any_open(cx));
    let mut sent: Vec<(String, Option<Value>)> = helm
        .commands()
        .into_iter()
        .map(|command| {
            (
                format!("{:?} {}", command.method, command.path),
                command.body,
            )
        })
        .collect();
    // Sorted, so the assertion does not depend on which request the answer sends first.
    sent.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        sent,
        [
            (
                format!("{:?} /api/hosts/provision", Method::Post),
                Some(json!({ "probe_id": "p-1" }))
            ),
            (
                format!("{:?} /api/preferences", Method::Put),
                Some(json!({ "skip_host_setup_confirmation": true }))
            ),
        ]
    );
}

/// A failed save is reported, never treated as saved (the goal's rule for every save). The permanent answer's
/// preference save outlives the question that sent it, so its refusal shows on the Hosts panel's line. Specified: the
/// helm refusing the preference save puts "Set up new hosts without asking: " and its words on the panel, while the
/// setup itself goes on; opening Add host again takes the line away.
#[gpui::test]
fn a_refused_permanent_answer_shows_in_the_hosts_panel(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    let key = probe_a_plan(&mut helm, cx);
    click(cx, "prompt-alternative-0");
    let preference = helm
        .commands()
        .into_iter()
        .find(|command| command.path == "/api/preferences")
        .expect("the preference save was sent");
    helm.reply(
        preference.id,
        500,
        Some(json!("the helm's database refused")),
    );
    settle(cx);
    assert_eq!(
        label(cx, "hosts-stop-asking-error"),
        Some("Set up new hosts without asking: the helm's database refused".to_owned())
    );
    assert_eq!(work(&key, cx), Some(Work::Setup), "the setup goes on");
    open_dialog(cx);
    assert_eq!(
        label(cx, "hosts-stop-asking-error"),
        None,
        "a new add takes the line away"
    );
}

/// Refuse the permanent answer's preference save with `words`, after "Set up, don't ask again" on a probed plan, and
/// check the premise: the panel shows the refusal.
fn refuse_stop_asking(helm: &mut FakeHelm, cx: &mut VisualTestContext, words: &str) {
    probe_a_plan(helm, cx);
    click(cx, "prompt-alternative-0");
    let preference = helm
        .commands()
        .into_iter()
        .find(|command| command.path == "/api/preferences")
        .expect("the preference save was sent");
    helm.reply(preference.id, 500, Some(json!(words)));
    settle(cx);
    assert_eq!(
        label(cx, "hosts-stop-asking-error"),
        Some(format!("Set up new hosts without asking: {words}")),
        "the premise: the refusal shows"
    );
}

/// SPEC_impl.md, "One failure path: the reset": a reset drops everything that came from the helm, and the permanent
/// answer's refusal is the helm's words kept outside the operation store. Specified: after the refusal shows, the
/// channel breaking and the helm reconnecting with its hosts leaves the panel without the line.
#[gpui::test]
fn a_refused_permanent_answer_is_gone_after_a_reset(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    refuse_stop_asking(&mut helm, cx, "the helm's database refused");
    helm.break_channel();
    settle(cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    settle(cx);
    assert_eq!(label(cx, "hosts-stop-asking-error"), None);
}

/// Once "Set up new hosts without asking" is on, however it got there (Settings, another client), a refusal to turn it
/// on no longer describes anything, and a line saying so would contradict the switch. Specified: after the refusal
/// shows, the helm pushing the preference row with the switch on takes the line away.
#[gpui::test]
fn a_refused_permanent_answer_is_gone_once_the_preference_is_on(cx: &mut TestAppContext) {
    // The preference row is in the snapshot, off, so the push below changes a collection the store holds.
    let (mut helm, cx, _add) = open_rail_with(cx, vec![preferences(false)]);
    refuse_stop_asking(&mut helm, cx, "the helm's database refused");
    let on = preferences(true);
    helm.push(CollectionChanges {
        collection: on.collection,
        meta: on.meta,
        upsert: on.entries,
        remove: Vec::new(),
    });
    settle(cx);
    assert_eq!(label(cx, "hosts-stop-asking-error"), None);
}

/// The panel's line lives in a global, which notifies no view by itself; the panel observes it. Every writer today also
/// redraws the window some other way, so only a write with nothing else going on shows the observer at work, as a
/// future writer or a cached panel would need it. Specified: with the panel drawn, an update that changes nothing
/// redraws it zero times (the premise); a bare write of a refusal to the global, with nothing else notified, redraws
/// it.
#[gpui::test]
fn the_panel_redraws_when_its_line_changes_alone(cx: &mut TestAppContext) {
    let (_helm, cx, _add) = open_rail_with(cx, Vec::new());
    let renders = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| crate::render_count::count("hosts-panel", cx))
    };
    let before = renders(cx);
    cx.update(|_, _| {});
    cx.run_until_parked();
    assert_eq!(
        renders(cx),
        before,
        "the premise: nothing redraws the panel by itself"
    );
    cx.update(|_, cx| {
        let store = session::current_operations(cx).map(|operations| operations.entity_id());
        let words = crate::host_text::HostText::from_host("refused");
        super::show(cx, |lines| {
            lines.stop_asking = store.map(|store| (store, words))
        });
    });
    cx.run_until_parked();
    assert!(renders(cx) > before, "the write redrew the panel");
}

/// Cancel must change nothing on the host or in the preferences (`provisioning.spec.ts`, "cancel and a setup answer in
/// one burst add nothing and change no preference"). Specified: Cancel closes the question, sends nothing, and focus
/// goes back to Add host.
#[gpui::test]
fn cancel_sends_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, add) = open_rail_with(cx, Vec::new());
    probe_a_plan(&mut helm, cx);
    click(cx, "prompt-cancel");
    assert!(!any_open(cx));
    assert!(focused(cx, &add));
    assert_eq!(helm.commands(), []);
}

/// With "Set up new hosts without asking" on, the user has already answered: the old app submitted at once without
/// showing the plan. Specified: with the preference on, a `provisionable` reply opens nothing, closes Add host, and
/// sends the provision request for `p-1` under the dialog's key.
#[gpui::test]
fn with_the_preference_on_a_plan_is_submitted_without_asking(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, vec![preferences(true)]);
    let key = probe_a_plan(&mut helm, cx);
    assert!(!any_open(cx), "no question, and Add host closed");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "p-1" }));
    assert_eq!(work(&key, cx), Some(Work::Setup));
}

// ---- A refused setup -----------------------------------------------------------------------------------------------

/// The old app reopened the form after a definite refusal ("a refused ADD attempt reopens the form"), so the user can
/// fix what was refused and try again; the plan is spent, so Add probes again. The reopened dialog still belongs to the
/// Add host button that opened the first one, so closing it returns focus there; run under macOS's click rule, where
/// no click focused that button and nothing focuses it before the dialog opens. Specified: a 409 to the provision
/// request reopens Add host holding `dev@newbox` and `newbox1` with the helm's words in its error line, and the setup
/// confirmation does not come back; Escape then closes it and focus goes back to Add host.
#[gpui::test]
fn a_refused_setup_reopens_add_host_with_the_destination_alias_and_reason(cx: &mut TestAppContext) {
    let (mut helm, cx, add) = open_rail_with(cx, Vec::new());
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    probe_a_plan(&mut helm, cx);
    click(cx, "prompt-confirm");
    let (id, _) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    helm.reply(id, 409, Some(json!(USED)));
    settle(cx);
    assert_eq!(destination_open(cx).as_deref(), Some("dev@newbox"));
    assert_eq!(error_line(cx).as_deref(), Some(USED));
    assert_eq!(label(cx, "prompt"), None, "no setup question");
    assert_eq!(alias(cx), "newbox1");
    keys(cx, "escape");
    assert!(
        !any_open(cx),
        "the premise: Escape closed the reopened dialog"
    );
    assert!(
        focused(cx, &add),
        "focus went back to the Add host button that opened the first dialog, which the click never focused"
    );
}

/// SPEC.md, "Keyboard focus": once the user has moved on, a result opens no dialog and shows where it belongs. A
/// refused new host has no row, so the Hosts panel says it, until Add host opens again, as the old panel's error went
/// when the form opened. Specified: after Set up, a key press, then a 409, no layer opens and the panel shows "Add
/// host: " and the helm's words; opening Add host again takes the line away.
#[gpui::test]
fn a_setup_refused_after_the_user_moved_on_shows_in_the_hosts_panel(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    probe_a_plan(&mut helm, cx);
    click(cx, "prompt-confirm");
    let (id, _) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    keys(cx, "tab");
    helm.reply(id, 409, Some(json!(USED)));
    settle(cx);
    assert!(!any_open(cx), "nothing opened");
    assert_eq!(
        label(cx, "hosts-add-error"),
        Some(format!("Add host: {USED}"))
    );
    open_dialog(cx);
    assert_eq!(
        label(cx, "hosts-add-error"),
        None,
        "a new add takes the line away"
    );
}

/// The old app reopened the form after a refused setup on the remembered-answer path too ("a refused ADD attempt reopens
/// the form"); judging that path by Add's mark must not lose it when the user did nothing since. Specified: with the
/// preference on and no input after Add, a 409 to the plan that went out without asking reopens Add host holding
/// `dev@newbox` and `newbox1` with the helm's words in its error line, and the panel shows nothing.
#[gpui::test]
fn without_asking_a_refusal_reopens_add_host_when_the_user_stayed(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, vec![preferences(true)]);
    probe_a_plan(&mut helm, cx);
    let (id, _) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert!(
        !any_open(cx),
        "the premise: the plan went out without asking"
    );
    helm.reply(id, 409, Some(json!(USED)));
    settle(cx);
    assert_eq!(destination_open(cx).as_deref(), Some("dev@newbox"));
    assert_eq!(alias(cx), "newbox1");
    assert_eq!(error_line(cx).as_deref(), Some(USED));
    assert_eq!(
        label(cx, "hosts-add-error"),
        None,
        "the panel shows nothing"
    );
}

/// Without asking, the plan goes out on the strength of the Add, so whether the user moved on is judged from the Add,
/// not from when the plan arrived: a refusal must not reopen Add host over whatever the user turned to while the probe
/// was out. Specified: with the preference on, Tab while the probe is out, a plan that goes out at once, and a 409 to
/// it, no layer opens and the panel shows "Add host: " and the helm's words.
#[gpui::test]
fn without_asking_a_refusal_after_the_user_moved_on_opens_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, vec![preferences(true)]);
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    click(cx, "add-host-submit");
    let (id, _) = one_probe(&mut helm);
    keys(cx, "tab");
    helm.reply(id, 200, Some(plan_reply()));
    settle(cx);
    let (id, _) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert!(
        !any_open(cx),
        "the premise: the plan went out without asking"
    );
    helm.reply(id, 409, Some(json!(USED)));
    settle(cx);
    assert!(!any_open(cx), "nothing opened");
    assert_eq!(
        label(cx, "hosts-add-error"),
        Some(format!("Add host: {USED}"))
    );
}

/// The setup confirmation is a dialog the user did not just ask for once they have moved on, so it must not open then
/// (the goal's "No queue for results that arrive later"); the dialog still open says what Add found. Specified:
/// Shift+Tab during the probe, then a plan, opens no question and sends nothing; Add host shows that Farhelm is not
/// running there and Add shows the plan.
#[gpui::test]
fn a_plan_after_the_user_moved_on_opens_no_confirmation(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    click(cx, "add-host-submit");
    let (id, _) = one_probe(&mut helm);
    keys(cx, "shift-tab");
    helm.reply(id, 200, Some(plan_reply()));
    settle(cx);
    assert_eq!(label(cx, "prompt"), None, "no setup question");
    assert_eq!(error_line(cx).as_deref(), Some(super::MOVED_ON));
    assert_eq!(helm.commands(), []);
}
