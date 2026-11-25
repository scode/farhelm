//! Tests of setting up an SSH host over the helm client's fake: the confirmation a probe's plan opens (what it shows,
//! where focus starts, Copy plan), what each answer sends and under which key, setup without asking, and where a
//! refused setup shows, before and after the user moved on; then the rerun of a failed setup from a host's menu, and
//! the refusals of a setup and a rerun while a session launch is in flight.
//!
//! A new host's tests start where the Add host tests leave off, through the same root view and helpers
//! (`hosts/add/tests.rs`): Add host opened from its rail button, a destination typed, Add pressed, and the fake helm
//! answering the probe with [`plan_reply`]. The rerun's start from the Hosts panel's harness (`hosts/tests.rs`) with a
//! host whose setup failed. Every step ends in one of the shared harness's step helpers, so the focus and layer
//! invariants are checked after each.

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
    alias, alias_focused, destination, dialog_key, error_line, one_probe, open_dialog,
    open_rail_with, type_text,
};
use crate::hosts::tests::{nothing_in_flight, start_work};
use crate::operations::{OpKey, Work};
use crate::session;
use crate::test_support::{
    any_open, click, focused, focused_id, frame, keys, label, meta, one_command, settle,
};

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

/// Apply a preference change from elsewhere while the original probe is still outstanding.
fn turn_on_skip_after_probe(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    assert!(cx.update(|_, cx| super::asks_first(cx)));
    let preference = preferences(true);
    helm.push(CollectionChanges {
        collection: preference.collection,
        meta: preference.meta,
        upsert: preference.entries,
        remove: Vec::new(),
    });
    settle(cx);
    assert!(!cx.update(|_, cx| super::asks_first(cx)));
}

/// Offer setup for a Tailscale draft so a refused setup has to restore the route along with its text fields.
fn probe_a_plan(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> OpKey {
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    keys(cx, "tab space tab");
    type_text(cx, "newbox1");
    let key = dialog_key(cx);
    click(cx, "add-host-submit");
    let (id, body) = one_probe(helm);
    assert_eq!(body["target"]["tailscale"], true);
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
        target: super::Target::New {
            destination: "dev@new-box".into(),
            alias: String::new(),
            tailscale: false,
            opener,
            from_agent: false,
        },
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
/// A preference change during the probe cannot give the earlier Add permission to install.
#[gpui::test]
fn a_plan_opens_the_setup_confirmation_in_place_of_add_host(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, vec![preferences(false)]);
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    click(cx, "add-host-submit");
    let (probe, _) = one_probe(&mut helm);
    turn_on_skip_after_probe(&mut helm, cx);
    helm.reply(probe, 200, Some(plan_reply()));
    settle(cx);
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

/// The operation store queues nothing, so a permanent answer given while another preference save is in flight (from
/// Settings, the templates dialog or a tunnel consent) sends nothing; dropped silently, the user would be asked again
/// next time with no word why. Specified: with a preference save of the test's own in flight, "Set up, don't ask again"
/// sends the provision request and no second preference save, and the panel shows "Set up new hosts without asking:"
/// with the words Settings uses for a press refused that way.
#[gpui::test]
fn a_permanent_answer_refused_for_another_preference_save_says_so_in_the_hosts_panel(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    probe_a_plan(&mut helm, cx);
    let started = cx.update(|_, cx| {
        let operations = session::current_operations(cx).expect("a UI session");
        let command = crate::settings::skip_host_setup_command().expect("the command builds");
        operations.update(cx, |operations, cx| {
            operations.start(&OpKey::preferences(), command, cx, |_, _| {})
        })
    });
    assert_eq!(
        started,
        Ok(()),
        "the premise: another preference save is out"
    );
    let _ = helm.commands();

    click(cx, "prompt-alternative-0");
    let paths: Vec<String> = helm
        .commands()
        .into_iter()
        .map(|command| command.path)
        .collect();
    assert_eq!(paths, ["/api/hosts/provision"], "no second preference save");
    assert_eq!(
        label(cx, "hosts-stop-asking-error"),
        Some(format!(
            "Set up new hosts without asking: {}",
            crate::settings::Refused::WORDS
        ))
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

/// Once "Set up new hosts without asking" is on, however it got there (Settings, a host dialog), a refusal to turn it
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
        let store = crate::hosts::current_operations_id(cx);
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
    assert!(
        frame(cx).iter().any(|node| {
            node.element_id.as_deref() == Some("add-host-tailscale")
                && node.toggled == Some(gpui::Toggled::True)
        }),
        "the reopened draft kept its Tailscale mode"
    );
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

// ---- The rerun -----------------------------------------------------------------------------------------------------

/// The Hosts panel over the reference hosts, `build-box` (host 2) with a failed setup, in a column as the app draws it
/// and a window as large as the reference screens'; with "Set up new hosts without asking" on when `without_asking` is.
fn panel_with_a_failed_setup(
    cx: &mut TestAppContext,
    without_asking: bool,
) -> (FakeHelm, &mut VisualTestContext) {
    use crate::hosts::tests::{hosts, open_panel_in_column, progress, reference_hosts, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus};
    let snapshot = vec![
        hosts(&reference_hosts()),
        progress(&[run(2, ProvisioningOperation::Add, RunStatus::Failed)]),
        preferences(without_asking),
        crate::sign_in::tests::questions(&[]),
        CollectionState {
            collection: Collection::SshSignInCompletions,
            meta: meta(&[]),
            entries: Vec::new(),
        },
    ];
    let (helm, _, cx) = open_panel_in_column(cx, snapshot);
    cx.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(1000.)));
    settle(cx);
    (helm, cx)
}

/// Choose Re-run from `build-box`'s menu and check that it sent its probe; the probe's id.
fn choose_rerun(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-rerun");
    one_probe(helm).0
}

/// A detached question must leave Remove available while the host is asking. A Ready that arrives during a refused
/// Remove still belongs to the same host and must replay once the Delete reply frees the command slot.
#[gpui::test]
fn remove_starts_during_host_setup_sign_in_and_defers_ready_replay(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::{SshConnectionPurpose, SshQuestion};

    let (mut helm, cx) = panel_with_a_failed_setup(cx, false);
    let _sign_in = cx.update(crate::sign_in::SignIn::attach);
    let probe = choose_rerun(&mut helm, cx);
    helm.reply(probe, 202, Some(json!({"attempt": "registered-check"})));
    settle(cx);
    let key = OpKey::host("2");
    assert_eq!(work(&key, cx), Some(Work::Probe));
    let question = SshQuestion {
        attempt: "registered-check".into(),
        purpose: SshConnectionPurpose::Setup,
        ..crate::sign_in::tests::question("registered-question", 2, "Verification code:", false)
    };
    let shown = crate::sign_in::tests::questions(std::slice::from_ref(&question));
    helm.push(CollectionChanges {
        collection: shown.collection,
        meta: shown.meta,
        upsert: shown.entries,
        remove: Vec::new(),
    });
    settle(cx);
    assert!(
        cx.update(|_, cx| {
            session::current_store(cx).is_some_and(|store| {
                store
                    .read(cx)
                    .collections()
                    .get::<SshQuestion>(&question.id)
                    .is_some()
            })
        }),
        "premise: the host is waiting for its SSH answer"
    );
    assert!(helm.commands().is_empty());

    // Close the sign-in popover to reach the host menu. The SSH question stays pending until answered or cancelled.
    assert!(
        label(cx, "ssh-sign-in-send").is_some(),
        "premise: the sign-in popover is open"
    );
    keys(cx, "escape");
    assert!(label(cx, "ssh-sign-in-send").is_none());
    assert!(
        helm.commands().is_empty(),
        "closing the popover leaves the check pending"
    );
    assert_eq!(work(&key, cx), Some(Work::Probe));
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-remove");
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Remove host · build-box")
    );
    click(cx, "prompt-confirm");
    let (removal, _) = one_command(&mut helm, Method::Delete, "/api/hosts/2");
    assert_eq!(work(&key, cx), Some(Work::RemoveHost));

    helm.push(CollectionChanges {
        collection: Collection::SshQuestions,
        meta: meta(&[]),
        upsert: Vec::new(),
        remove: vec![question.id],
    });
    helm.push(CollectionChanges {
        collection: Collection::SshSignInCompletions,
        meta: meta(&["registered-check"]),
        upsert: vec![KeyedEntry {
            key: "registered-check".into(),
            value: json!({"attempt": "registered-check", "result": "ready"}),
        }],
        remove: Vec::new(),
    });
    settle(cx);
    assert!(
        helm.commands().is_empty(),
        "a Ready waits for the removal reply"
    );
    assert_eq!(work(&key, cx), Some(Work::RemoveHost));

    // The sign-in is over even if its completion ages out before the Remove reply arrives.
    helm.push(CollectionChanges {
        collection: Collection::SshSignInCompletions,
        meta: meta(&[]),
        upsert: Vec::new(),
        remove: vec!["registered-check".into()],
    });
    // sleep-ok: cross the sign-in inactivity deadline while the fake helm still holds the Remove reply.
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(151));
    settle(cx);
    assert!(helm.commands().is_empty());
    assert_eq!(work(&key, cx), Some(Work::RemoveHost));

    helm.reply(removal, 409, Some(json!({"error": "host busy"})));
    settle(cx);
    let (replay, body) = one_probe(&mut helm);
    assert_eq!(
        body["target"],
        json!({"kind": "ssh-host", "host": 2, "destination": "dev@build-box"})
    );
    helm.reply(
        replay,
        404,
        Some(json!({"error": "host no longer registered"})),
    );
    settle(cx);
    assert_eq!(work(&key, cx), None);
}

/// SPEC.md, "Topology": "an existing entry is set up again from its own row", and the old app's rerun probed "the
/// registered destination" before asking again; the drive of the first Add host port found no way to try a failed setup
/// again. The question names the host as its row does, and confirming hands the helm its one-use plan under the host's
/// own key. Specified: the menu of a host whose setup failed starts on Retry and offers Re-run ("Try the failed setup
/// again"); choosing it closes the menu and sends `POST /api/hosts/probe` for `dev@build-box` with the recorded install
/// paths (none) and no new entry, under the host's key, while the row reads "Setting up…"; the plan opens "Set up host
/// · build-box" with focus on Set up; Set up sends `POST /api/hosts/provision` for `p-1` under the host's key, focus
/// returns to the row's toggle, and a refusal of it shows under the row with nothing opened.
/// The prompt uses the preference at Re-run, even if a preferences push turns on skipping while the probe is out.
#[gpui::test]
fn re_run_probes_the_registered_destination_and_sets_up_under_the_hosts_key(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = panel_with_a_failed_setup(cx, false);
    click(cx, "host-menu-toggle-2");
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));
    assert_eq!(label(cx, "host-menu-rerun").as_deref(), Some("Re-run"));
    assert!(painted(cx).contains("Try the failed setup again"));
    click(cx, "host-menu-rerun");
    assert!(!any_open(cx), "the menu closed");
    let (probe, body) = one_probe(&mut helm);
    assert_eq!(
        body,
        json!({
            "target": { "kind": "ssh-host", "host": 2, "destination": "dev@build-box" },
            "remote_farhelm": null,
            "remote_state_dir": null,
        })
    );
    let key = OpKey::host("2");
    assert_eq!(work(&key, cx), Some(Work::Probe));
    assert_eq!(label(cx, "host-status-2").as_deref(), Some("Setting up…"));

    turn_on_skip_after_probe(&mut helm, cx);
    helm.reply(probe, 200, Some(plan_reply()));
    settle(cx);
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Set up host · build-box")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    click(cx, "prompt-confirm");
    assert!(!any_open(cx), "the question closed");
    let (setup, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "p-1" }));
    assert_eq!(work(&key, cx), Some(Work::Setup));
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-toggle-2"));

    helm.reply(setup, 409, Some(json!(USED)));
    settle(cx);
    assert_eq!(label(cx, "host-error-2").as_deref(), Some(USED));
    assert!(!any_open(cx), "a rerun's refusal opens nothing");
}

/// The rerun's probe can find the host is now a manual install, and its answer outlives the menu that asked, so it has
/// to show where the host is; a refusal of the probe shows there the same way. Specified: a `manual` reply puts its
/// reason under `build-box`'s row and opens nothing; a second Re-run whose probe the helm refuses replaces it with the
/// refusal's words.
#[gpui::test]
fn a_rerun_that_leads_nowhere_says_why_under_the_row(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel_with_a_failed_setup(cx, false);
    let probe = choose_rerun(&mut helm, cx);
    helm.reply(
        probe,
        200,
        Some(json!({ "result": "manual", "reason": "no systemd user manager" })),
    );
    settle(cx);
    assert_eq!(
        label(cx, "host-error-2").as_deref(),
        Some("no systemd user manager")
    );
    assert!(!any_open(cx));

    let probe = choose_rerun(&mut helm, cx);
    helm.reply(probe, 504, Some(json!("the host timed out")));
    settle(cx);
    assert_eq!(
        label(cx, "host-error-2").as_deref(),
        Some("the host timed out")
    );
}

/// A sandbox can become suspended between the painted Re-run menu and the
/// helm's probe. Hosts must show how to continue while retaining the failure
/// that originally made Re-run available.
#[gpui::test]
fn a_refused_tensorlake_setup_rerun_keeps_the_failed_progress(cx: &mut TestAppContext) {
    use crate::hosts::tests::{connected, host, hosts, open_panel_in_column, progress, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus};

    let mut failed = run(2, ProvisioningOperation::Add, RunStatus::Failed);
    failed.message = Some("the earlier install failed".into());
    let (mut helm, _, cx) = open_panel_in_column(
        cx,
        vec![
            hosts(&[host(2, "tensorlake", connected())]),
            progress(&[failed]),
        ],
    );
    cx.simulate_resize(gpui::size(gpui::px(1600.), gpui::px(1000.)));
    settle(cx);
    let earlier = Some("the earlier install failed");
    assert_eq!(label(cx, "host-run-message-2").as_deref(), earlier);

    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-rerun");
    let (probe, body) = one_probe(&mut helm);
    assert_eq!(
        body,
        json!({ "target": { "kind": "tensorlake-host", "host": 2 } })
    );
    let reason = "this host's Tensorlake sandbox is suspended, so Farhelm cannot reach it; resume the sandbox first, then try again";
    helm.reply(probe, 409, Some(json!(reason)));
    settle(cx);

    assert_eq!(label(cx, "host-error-2").as_deref(), Some(reason));
    assert_eq!(label(cx, "host-run-message-2").as_deref(), earlier);
    assert!(!any_open(cx));
    assert_eq!(helm.commands(), [], "the refusal sends no confirmation");
}

/// "Set up new hosts without asking" covers reruns too (the old app submitted a rerun's plan on the remembered answer).
/// Specified: with the preference on, the rerun's plan goes straight to `POST /api/hosts/provision` under the host's
/// key and no question opens.
#[gpui::test]
fn with_the_preference_on_a_rerun_sets_up_without_asking(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel_with_a_failed_setup(cx, true);
    let probe = choose_rerun(&mut helm, cx);
    helm.reply(probe, 200, Some(plan_reply()));
    settle(cx);
    assert!(!any_open(cx), "no question");
    let (_, body) = one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(body, json!({ "probe_id": "p-1" }));
    assert_eq!(work(&OpKey::host("2"), cx), Some(Work::Setup));
}

/// SPEC.md, "Keyboard focus": a result that arrives after the user moved on opens nothing over what they are doing
/// (the goal's "No queue for results that arrive later"). The rerun's plan is such a result, and with nothing to open
/// its row is where the user learns why nothing happened. Specified: after Re-run, a key press, then the probe's plan:
/// no question opens, `build-box`'s row says "Farhelm is not running on this host. Choose Re-run again ...", and no
/// setup is sent.
#[gpui::test]
fn a_reruns_plan_after_the_user_moved_on_opens_nothing_and_says_so(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel_with_a_failed_setup(cx, false);
    let probe = choose_rerun(&mut helm, cx);
    keys(cx, "shift-tab");
    helm.reply(probe, 200, Some(plan_reply()));
    settle(cx);
    assert_eq!(label(cx, "prompt"), None, "no setup question");
    assert!(!any_open(cx));
    assert_eq!(
        label(cx, "host-error-2").as_deref(),
        Some(super::RERUN_MOVED_ON)
    );
    assert_eq!(helm.commands(), [], "nothing was provisioned");
}

// ---- During a session launch ---------------------------------------------------------------------------------------

/// The refusal every host registry change gives during a session launch, after the action's name.
fn during_launch(action: &str) -> String {
    format!("{action}: {}", crate::hosts::LAUNCH_IN_FLIGHT)
}

/// Why this matters: a setup is a host registry change, which the migration keeps out of the way of a session launch,
/// and the setup question can stay open while a launch starts, so checking only at Add would let Set up put a new host
/// beside the launch. The refusal takes the path a helm refusal of the setup takes, so the user can try again with the
/// fields as they were. Specified: with the setup question open and a launcher `Launch` in flight, Set up sends no
/// provision request and reopens Add host holding `dev@newbox` and `newbox1`, its error line reading "Add host: not
/// started, a session launch is in progress; try again when it finishes".
#[gpui::test]
fn set_up_during_a_launch_sends_nothing_and_reopens_add_host_saying_why(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    probe_a_plan(&mut helm, cx);
    assert_eq!(
        label(cx, "prompt").as_deref(),
        Some("Set up host · dev@newbox"),
        "the premise: the setup question is open"
    );
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    click(cx, "prompt-confirm");
    assert_eq!(helm.commands(), [], "no setup was sent");
    assert_eq!(destination_open(cx).as_deref(), Some("dev@newbox"));
    assert_eq!(alias(cx), "newbox1");
    assert_eq!(error_line(cx), Some(during_launch("Add host")));
    assert_eq!(
        label(cx, "hosts-setup-not-started"),
        None,
        "said once, in Add host"
    );
    assert_eq!(label(cx, "hosts-add-error"), None, "said once, in Add host");
}

/// Why this matters: a permanent answer whose setup was refused must not leave half its effect behind, or the next Add
/// would skip the very question the user was answering. Specified: with the setup question open and a launcher
/// `Launch` in flight, "Set up, don't ask again" sends neither the provision request nor the preference, and Add host
/// reopens saying why.
#[gpui::test]
fn the_permanent_answer_during_a_launch_leaves_the_preference_off(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, Vec::new());
    probe_a_plan(&mut helm, cx);
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    click(cx, "prompt-alternative-0");
    assert_eq!(
        helm.commands(),
        [],
        "neither the setup nor the preference was sent"
    );
    assert_eq!(error_line(cx), Some(during_launch("Add host")));
}

/// Why this matters: without asking, Add host closes as the plan goes out, and a submission refused at once must reopen
/// it after that close, or the close would take the reopened dialog with it and the refusal would show nowhere.
/// Specified: with "Set up new hosts without asking" on, a launcher `Launch` started (by no input) while the probe is
/// out, and then a plan, nothing is provisioned, Add host is open holding `dev@newbox` and `newbox1` with "Add host:
/// not started, …" in its error line, and the panel shows no line.
#[gpui::test]
fn without_asking_a_setup_refused_during_a_launch_reopens_add_host(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail_with(cx, vec![preferences(true)]);
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    keys(cx, "tab tab");
    assert!(alias_focused(cx), "the premise: Tab reached the alias");
    type_text(cx, "newbox1");
    click(cx, "add-host-submit");
    let (probe, _) = one_probe(&mut helm);
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    helm.reply(probe, 200, Some(plan_reply()));
    settle(cx);
    assert_eq!(helm.commands(), [], "no setup was sent");
    assert_eq!(destination_open(cx).as_deref(), Some("dev@newbox"));
    assert_eq!(alias(cx), "newbox1");
    assert_eq!(error_line(cx), Some(during_launch("Add host")));
    assert_eq!(label(cx, "hosts-setup-not-started"), None);
}

/// Why this matters: the same rule for a registered host's setup, which has no Add host to reopen, so the Hosts panel's
/// line says it, naming the host as Remove's line does. Specified: with a rerun's setup question open for `build-box`
/// and a session's `Restart` in flight, Set up sends no provision request, opens nothing, and the panel's line says
/// "Set up build-box: not started, a session launch is in progress; try again when it finishes", with nothing under the
/// row.
#[gpui::test]
fn set_up_of_a_rerun_during_a_launch_says_why_on_the_panel(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel_with_a_failed_setup(cx, false);
    let probe = choose_rerun(&mut helm, cx);
    helm.reply(probe, 200, Some(plan_reply()));
    settle(cx);
    assert!(
        label(cx, "prompt").is_some(),
        "the premise: the setup question is open"
    );
    start_work(&mut helm, cx, Work::Restart, OpKey::session("s1"));
    click(cx, "prompt-confirm");
    assert_eq!(helm.commands(), [], "no setup was sent");
    assert!(!any_open(cx), "nothing opened");
    assert_eq!(
        label(cx, "hosts-setup-not-started"),
        Some(during_launch("Set up build-box"))
    );
    assert_eq!(label(cx, "host-error-2"), None, "no record under the row");
}

/// Why this matters: Re-run starts a host registry change too (its probe can register an answering supervisor, and its
/// plan leads to a setup), so it waits for a launch as Add does, and says why. Specified: with a session's `Replace` in
/// flight, choosing Re-run on `build-box` sends nothing and the panel's line says "Re-run build-box: not started, a
/// session launch is in progress; try again when it finishes"; once the Replace settles, Re-run sends its probe and the
/// line goes.
#[gpui::test]
fn re_run_during_a_launch_sends_nothing_and_says_why_on_the_panel(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel_with_a_failed_setup(cx, false);
    let replace = start_work(&mut helm, cx, Work::Replace, OpKey::session("s1"));
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-rerun");
    assert_eq!(helm.commands(), [], "no probe was sent");
    assert_eq!(
        label(cx, "hosts-setup-not-started"),
        Some(during_launch("Re-run build-box"))
    );

    helm.reply(replace, 200, None);
    settle(cx);
    assert!(nothing_in_flight(cx), "the premise: the Replace settled");
    choose_rerun(&mut helm, cx);
    assert_eq!(label(cx, "hosts-setup-not-started"), None, "the line went");
}

/// Why this matters: once the user has moved on, a new host's setup refused during a launch has no dialog to reopen, so
/// the Hosts panel says it, and in the words the panel gives a helm refusal of the same setup ("Add host: "), so one add
/// does not read two ways depending on who refused it. Specified: with "Set up new hosts without asking" on, a key press
/// while the probe is out, a launcher `Launch` in flight, and then a plan, nothing is provisioned, no layer opens, and
/// the panel's line reads "Add host: not started, a session launch is in progress; try again when it finishes".
#[gpui::test]
fn a_setup_refused_during_a_launch_after_the_user_moved_on_says_add_host_on_the_panel(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _add) = open_rail_with(cx, vec![preferences(true)]);
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    click(cx, "add-host-submit");
    let (probe, _) = one_probe(&mut helm);
    keys(cx, "tab");
    start_work(&mut helm, cx, Work::Launch, OpKey::new_launch());
    helm.reply(probe, 200, Some(plan_reply()));
    settle(cx);
    assert_eq!(helm.commands(), [], "no setup was sent");
    assert!(!any_open(cx), "nothing opened");
    assert_eq!(
        label(cx, "hosts-setup-not-started"),
        Some(during_launch("Add host"))
    );
}

/// Why this matters: the panel's line says a setup was not started, which stops being true once one is, so a setup that
/// goes out must take it away, as a rerun that goes out does. Specified: after a Re-run refused during a session's
/// `Replace` put "Re-run build-box: not started, …" on the panel, and the Replace settled, a setup submission that the
/// operation store starts takes the line away.
#[gpui::test]
fn a_setup_that_goes_out_takes_the_not_started_line_away(cx: &mut TestAppContext) {
    let (mut helm, cx) = panel_with_a_failed_setup(cx, false);
    let replace = start_work(&mut helm, cx, Work::Replace, OpKey::session("s1"));
    click(cx, "host-menu-toggle-2");
    click(cx, "host-menu-rerun");
    assert!(
        label(cx, "hosts-setup-not-started").is_some(),
        "the premise: the line shows"
    );
    helm.reply(replace, 200, None);
    settle(cx);
    assert!(nothing_in_flight(cx), "the premise: the Replace settled");

    cx.update(|window, cx| {
        let offer = reference_offer(cx.focus_handle(), focus::Mark::take(cx));
        super::submit(offer, window.window_handle(), cx);
    });
    settle(cx);
    one_command(&mut helm, Method::Post, "/api/hosts/provision");
    assert_eq!(label(cx, "hosts-setup-not-started"), None, "the line went");
}
