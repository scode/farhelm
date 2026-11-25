//! Tests of New sandbox in the real main view over the helm client's fake: the Tensorlake account the launcher reads as
//! it opens, the segment it shows, the recipe its segment enters on, its recipes listed with their mark, size and
//! refusals, the registration note, the footer's words when no recipe can be chosen (New container's too, from the
//! same window), and "Create a sandbox for this session?": a press asking it, its answers, the setting that skips it,
//! an edit retiring it, and a quick launch prepared out of sight waiting for the account and coming back to ask it.
//! SPEC.md, "Creation" (the Runs on bullet), "Sandbox sessions" and "Keyboard focus".
//!
//! The fleet is the helm's pinned fixture's first three hosts, as in `environment_view_tests`; the catalog holds
//! `plain`, with no snapshot, then `ready`, with one, both on managed Ubuntu at 2 CPUs, 4 GB memory and 20 GB disk,
//! except where a test pushes an empty one. The account read is answered with each test's [`Settings`] (`answer_with`).

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ProviderKind;
use farhelm_proto::tensorlake::{Settings, SshKeyStatus};
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, KeyedEntry};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{LaunchDraft, RunsOn};
use super::environment_launch_tests::{answer_with, plain_preferences, window_with_more};
use super::environment_tests::{account, built_json, ready_key, recipe_json, sandbox};
use super::launch_tests::command_draft;
use super::view_tests::{
    click_in_body, draft, fixture_hosts, name_of, pick_host_menu_row, shown_refusal,
};
use crate::focus::Mark;
use crate::session_list::tests::painted;
use crate::test_support::{self, any_open, focused_id, meta, settle};

/// The registration command the ready key in these tests carries. Its key name holds a zero-width space, which the note
/// shows escaped and the copy keeps.
const COMMAND: &str = "tl sbx ssh keys add --name fix\u{200b}ture 'ssh-ed25519 AAAA fixture'";

/// The pushed catalog: `plain` with no snapshot, then `ready` with one.
fn catalog() -> CollectionState {
    CollectionState {
        collection: Collection::Recipes,
        meta: meta(&["plain", "ready"]),
        entries: vec![
            KeyedEntry {
                key: "plain".into(),
                value: recipe_json("plain", json!({ "kind": "tensorlake_ubuntu" })),
            },
            KeyedEntry {
                key: "ready".into(),
                value: built_json("ready"),
            },
        ],
    }
}

/// The launch tests' window (`window_with_more`) on [`command_draft`] with [`catalog`], every command answered with
/// `account`.
fn opened<'a>(
    cx: &'a mut TestAppContext,
    account: &Settings,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let hosts = fixture_hosts(&[0, 1, 2]);
    let (mut helm, cx) = window_with_more(
        cx,
        command_draft(),
        hosts,
        plain_preferences(),
        vec![catalog()],
    );
    answer_with(&mut helm, cx, Some(account), |_| false);
    (helm, cx)
}

/// The account every press here can launch with: an API key and Farhelm's SSH key made.
fn usable() -> Settings {
    account(true, ready_key(COMMAND))
}

/// Without an API key none of the account's recipes can launch. The host menu still offers any usable container
/// host's New container row, while New sandbox is absent.
#[gpui::test]
fn new_sandbox_is_hidden_without_an_api_key(cx: &mut TestAppContext) {
    let (_helm, cx) = opened(cx, &account(false, ready_key(COMMAND)));
    click_in_body(cx, "launcher-host");
    assert!(
        name_of(cx, "launcher-host-option-New container").is_some(),
        "the premise: the New environment group is drawn"
    );
    assert_eq!(name_of(cx, "launcher-host-option-New sandbox"), None);
}

/// SPEC.md, "Sandbox sessions": a missing "local Farhelm SSH key also disables" a recipe; the old switch still entered
/// New sandbox on the first recipe with a snapshot, so the user sees which key to create rather than an empty choice
/// (kept by the launcher area on 2026-11-16). Specified: with an API key and no SSH key, New sandbox shows; entering it
/// chooses `ready`, the first built recipe, over the unbuilt `plain`; Folder is the sandbox's home; a press sends
/// nothing and says "Create Farhelm's SSH key in Settings → Tensorlake" after "Not launched."; and with no key there is
/// no registration note.
#[gpui::test]
fn entering_new_sandbox_chooses_the_first_built_recipe_and_names_the_missing_key(
    cx: &mut TestAppContext,
) {
    let keyless = account(true, SshKeyStatus::NotCreated);
    let (mut helm, cx) = opened(cx, &keyless);
    pick_host_menu_row(cx, "launcher-host-option-New sandbox");
    let entered = draft(cx);
    assert_eq!(entered.target, Some(sandbox("ready")));
    assert_eq!(entered.folder.value(), "~");
    test_support::click(cx, "launcher-launch");
    let sent = answer_with(&mut helm, cx, Some(&keyless), |command| {
        command.path == "/api/environment-sessions"
    });
    assert_eq!(sent, [], "nothing is sent");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Create Farhelm's SSH key in Settings → Tensorlake")
    );
    assert_eq!(cx.debug_bounds("launcher-registration-note"), None);
}

/// SPEC.md, "Sandbox sessions": New sandbox "lists recipes with a cube mark and CPU and memory size", and "A recipe
/// without a built snapshot is disabled with No snapshot yet". A disabled option that could still be chosen would only
/// lead to a refused press. Specified: in New sandbox with a usable account, the open Recipe list shows `plain` as
/// "plain · 2 CPUs · 4 GB memory · No snapshot yet", unavailable, and with the mark (how the select draws a mark and
/// refuses an unavailable option is the component's, `components::tests::select`).
#[gpui::test]
fn sandbox_recipes_are_listed_with_their_mark_size_and_refusal(cx: &mut TestAppContext) {
    let (_helm, cx) = opened(cx, &usable());
    pick_host_menu_row(cx, "launcher-host-option-New sandbox");
    test_support::click(cx, "launcher-recipe");
    let lines = painted(&test_support::frame(cx));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("plain · 2 CPUs · 4 GB memory · No snapshot yet")),
        "{lines:?}"
    );
    assert!(
        cx.debug_bounds("launcher-recipe-option-plain-mark")
            .is_some()
    );
}

/// SPEC.md, "Sandbox sessions": "The selected recipe shows the existing copyable SSH-key registration command and
/// explains that Farhelm cannot check account registration." A key that exists but was never registered fails only at
/// launch, minutes later, so the note is the one place the user learns the step. Specified: in New sandbox with the key
/// made, the note says the old sentences and names the helm's command shown escaped (it holds a zero-width space) in
/// its command well, and the well's "Copy registration command" puts the raw command on the clipboard byte for byte;
/// back on Existing host the note is gone.
#[gpui::test]
fn the_registration_note_shows_and_copies_the_command(cx: &mut TestAppContext) {
    let (_helm, cx) = opened(cx, &usable());
    pick_host_menu_row(cx, "launcher-host-option-New sandbox");
    assert_eq!(
        name_of(cx, "launcher-registration-words").as_deref(),
        Some(
            "Register Farhelm's SSH key with Tensorlake before launching. Farhelm cannot check account \
             registration."
        )
    );
    let escaped = farhelm_proto::text::escaped(COMMAND);
    assert_ne!(
        escaped, COMMAND,
        "the premise: the command holds a character shown escaped"
    );
    assert_eq!(
        name_of(cx, "launcher-registration-command"),
        Some(escaped),
        "shown escaped"
    );
    assert_eq!(
        name_of(cx, "launcher-registration-command-copy").as_deref(),
        Some("Copy registration command")
    );
    test_support::click(cx, "launcher-registration-command-copy");
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some(COMMAND)
    );
    assert!(
        cx.debug_bounds("launcher-registration-note").is_some(),
        "the premise: the note is drawn"
    );
    pick_host_menu_row(cx, "launcher-host-option-1");
    assert_eq!(cx.debug_bounds("launcher-registration-note"), None);
}

/// With no recipe in the catalog, entering New sandbox or New container chooses none (an empty name), and the footer
/// and Launch's name used to say "new  sandbox", with two spaces around the missing recipe, as the old footer did too.
/// SPEC.md, "Creation": Launch's accessible name describes the target. With no recipe chosen it names only the
/// environment, its host when known, and the path.
/// New sandbox also shows the SSH-key registration note with no recipe chosen, as the old launcher did.
#[gpui::test]
fn a_new_environment_without_a_recipe_is_named_without_one(cx: &mut TestAppContext) {
    let empty = CollectionState {
        collection: Collection::Recipes,
        meta: meta(&[]),
        entries: Vec::new(),
    };
    let hosts = fixture_hosts(&[0, 1, 2]);
    let (mut helm, cx) =
        window_with_more(cx, command_draft(), hosts, plain_preferences(), vec![empty]);
    answer_with(&mut helm, cx, Some(&usable()), |_| false);
    for (segment, named) in [
        ("launcher-host-option-New sandbox", "new sandbox"),
        (
            "launcher-host-option-New container",
            "new container on host1",
        ),
    ] {
        pick_host_menu_row(cx, segment);
        let recipe = draft(cx).target.map(|target| target.recipe().to_owned());
        assert_eq!(
            recipe.as_deref(),
            Some(""),
            "the premise: {segment} with no recipe"
        );
        let words = format!("Command · {named} · ~");
        assert_eq!(
            name_of(cx, "launcher-launch"),
            Some(format!("Launch {words}"))
        );
        // The old launcher showed the registration note with no recipe chosen too (its check passed for the empty
        // name), so New sandbox keeps it here; a container has no account to register with.
        let note = cx.debug_bounds("launcher-registration-note").is_some();
        assert_eq!(
            note,
            segment == "launcher-host-option-New sandbox",
            "{segment}: the registration note"
        );
    }
}

// ---- "Create a sandbox for this session?" ---------------------------------------------------------------------------

/// The pricing page the question links to, the old question's.
const PRICING: &str = "https://www.tensorlake.ai/pricing";

/// Enter New sandbox (on `ready`, the first built recipe) and verify the form discloses billing and deletion before
/// pressing Launch. Return what was sent, which is nothing while the confirmation is open.
fn press_in_new_sandbox(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    account: &Settings,
) -> Vec<FakeCommand> {
    pick_host_menu_row(cx, "launcher-host-option-New sandbox");
    assert_eq!(
        draft(cx).target,
        Some(sandbox("ready")),
        "the premise: New sandbox on `ready`"
    );
    assert_eq!(
        name_of(cx, "launcher-environment-notice").as_deref(),
        Some(
            "This session's sandbox bills while running. Suspend stops CPU and memory billing. Deleting its last session removes the sandbox and its files."
        ),
        "the form shows the notice even if the confirmation will be skipped"
    );
    test_support::click(cx, "launcher-launch");
    environment_creates(helm, cx, account)
}

/// The environment creates sent since the last call, every other command answered (the account read with `account`).
fn environment_creates(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    account: &Settings,
) -> Vec<FakeCommand> {
    answer_with(helm, cx, Some(account), |command| {
        command.path == "/api/environment-sessions"
    })
}

/// The one environment create in `sent`, and its body.
fn the_create(sent: &[FakeCommand]) -> (&FakeCommand, Value) {
    let [create] = sent else {
        panic!("one environment create, not {sent:?}");
    };
    let body = create.body.clone().expect("a create carries a body");
    (create, body)
}

/// SPEC.md, "Sandbox sessions": "Launching asks Create a sandbox for this session?, naming recipe, CPU, memory and
/// disk, linking Tensorlake pricing, and explaining suspension, the idle timeout and irreversible file deletion", and
/// "Keyboard focus": a risky action that cannot be undone "starts on Cancel". A sandbox bills from the moment it
/// exists, so nothing may be sent before the user answers. Specified: a press in New sandbox on `ready` sends nothing
/// and asks the question in the launcher, titled "Create a sandbox for this session?", naming "Recipe ready" and "2 CPU
/// · 4 GB memory · 20 GB disk", with the old billing paragraph; its answers are Cancel and "Create and launch", focus
/// on Cancel; its "Tensorlake pricing" link opens the pricing page, leaving the question up and nothing sent.
#[gpui::test]
fn a_press_asks_before_creating_a_sandbox_with_cancel_focused(cx: &mut TestAppContext) {
    let (mut helm, cx) = opened(cx, &usable());
    let sent = press_in_new_sandbox(&mut helm, cx, &usable());
    assert_eq!(sent, [], "nothing is sent before the answer");
    assert!(any_open(cx), "the launcher stays open");
    assert_eq!(
        name_of(cx, "prompt").as_deref(),
        Some("Create a sandbox for this session?")
    );
    assert_eq!(
        name_of(cx, "sandbox-question-recipe").as_deref(),
        Some("Recipe ready")
    );
    assert_eq!(
        name_of(cx, "sandbox-question-size").as_deref(),
        Some("2 CPU · 4 GB memory · 20 GB disk")
    );
    assert_eq!(
        name_of(cx, "sandbox-question-consequence").as_deref(),
        Some(
            "The sandbox bills while running. Suspend preserves its processes and files and stops CPU and memory \
             billing; snapshot storage still bills. Tensorlake suspends it after 30 minutes without a connection \
             from Farhelm. Deleting its last session irreversibly removes the sandbox, every process, its files and \
             snapshots."
        )
    );
    assert_eq!(name_of(cx, "prompt-cancel").as_deref(), Some("Cancel"));
    assert_eq!(
        name_of(cx, "prompt-confirm").as_deref(),
        Some("Create and launch")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    assert_eq!(
        name_of(cx, "prompt-link").as_deref(),
        Some("Tensorlake pricing")
    );
    test_support::click(cx, "prompt-link");
    assert_eq!(cx.opened_url().as_deref(), Some(PRICING));
    assert_eq!(
        environment_creates(&mut helm, cx, &usable()),
        [],
        "the link sends nothing"
    );
    assert!(
        cx.debug_bounds("prompt-confirm").is_some(),
        "and leaves the question up"
    );
}

/// SPEC.md, "Sandbox sessions": "Cancel creates nothing"; the old Cancel's help, "create no sandbox and keep the
/// launcher open", is the promise. Specified: Cancel sends nothing, takes the question away and leaves the launcher
/// open on its draft, still New sandbox on `ready`, with no refusal shown.
#[gpui::test]
fn cancel_creates_nothing_and_keeps_the_draft(cx: &mut TestAppContext) {
    let (mut helm, cx) = opened(cx, &usable());
    press_in_new_sandbox(&mut helm, cx, &usable());
    assert!(
        cx.debug_bounds("prompt-cancel").is_some(),
        "the premise: the question is up"
    );
    test_support::click(cx, "prompt-cancel");
    assert_eq!(environment_creates(&mut helm, cx, &usable()), []);
    assert_eq!(cx.debug_bounds("prompt-cancel"), None, "the question went");
    assert!(any_open(cx), "the launcher stays open");
    assert_eq!(draft(cx).target, Some(sandbox("ready")));
    assert_eq!(shown_refusal(cx), None);
}

/// A recipe can be edited elsewhere while its confirmation is open. The answer must send the size shown in the
/// question, even if the catalog already holds a new one. A refusal restores the same launch draft, and the next
/// press asks about the new size without sending a request.
#[gpui::test]
fn create_and_launch_sends_the_size_shown_before_a_catalog_edit(cx: &mut TestAppContext) {
    let (mut helm, cx) = opened(cx, &usable());
    press_in_new_sandbox(&mut helm, cx, &usable());
    let original_size = "2 CPU · 4 GB memory · 20 GB disk";
    assert_eq!(
        name_of(cx, "sandbox-question-size").as_deref(),
        Some(original_size)
    );
    let title = draft(cx).name.value().clone();
    let mut edited = built_json("ready");
    edited["size"]["cpus"] = json!(4);
    helm.push(CollectionChanges {
        collection: Collection::Recipes,
        meta: meta(&["plain", "ready"]),
        upsert: vec![KeyedEntry {
            key: "ready".into(),
            value: edited,
        }],
        remove: Vec::new(),
    });
    settle(cx);
    let live_size = cx.update(|_, cx| {
        crate::session::current_store(cx)
            .unwrap()
            .read(cx)
            .collections()
            .get::<farhelm_proto::recipes::RecipeView>("ready")
            .unwrap()
            .size
    });
    assert_eq!(
        live_size.cpus, 4,
        "the pushed edit has reached the live catalog"
    );
    assert_eq!(
        name_of(cx, "sandbox-question-size").as_deref(),
        Some(original_size)
    );
    test_support::click(cx, "prompt-confirm");
    let sent = environment_creates(&mut helm, cx, &usable());
    let (create, body) = the_create(&sent);
    assert_eq!(body["recipe"], json!("ready"));
    assert_eq!(body["place"], json!({ "kind": "tensorlake_account" }));
    assert_eq!(body["confirmed"], json!(true));
    assert_eq!(
        body["confirmed_size"],
        json!({"cpus": 2, "memory_gb": 4, "disk_gb": 20})
    );
    assert!(!any_open(cx), "the launcher hid while its create is out");
    let refusal = "The sandbox size changed. Review the updated size before launching.";
    helm.reply(create.id, 409, Some(json!(refusal)));
    settle(cx);
    assert!(any_open(cx), "the refusal brought the launcher back");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some(format!("Not launched. {refusal}").as_str())
    );
    assert_eq!(draft(cx).target, Some(sandbox("ready")));
    assert_eq!(draft(cx).name.value(), &title);
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        environment_creates(&mut helm, cx, &usable()),
        [],
        "the consent went with the create it sent"
    );
    assert_eq!(
        name_of(cx, "prompt").as_deref(),
        Some("Create a sandbox for this session?"),
        "the press asks again"
    );
    assert_eq!(
        name_of(cx, "sandbox-question-size").as_deref(),
        Some("4 CPU · 4 GB memory · 20 GB disk")
    );
}

/// SPEC.md, "Sandbox sessions": "Only Create Tensorlake sandboxes without asking skips this question". The create then
/// goes unconfirmed, as the old launcher sent it, and the helm checks the setting again, so a setting turned off since
/// the launcher read it is refused there rather than billed (accepted by the launcher area on 2026-11-16 in place of
/// the old re-read at the press). Specified: with the account read saying the setting is on, a press sends one
/// environment create for `ready` with `confirmed` false and asks nothing; the helm's refusal of it comes back as any
/// refused launch does, "Not launched." and its words, the draft kept.
#[gpui::test]
fn the_setting_skips_the_question_with_an_unconfirmed_create(cx: &mut TestAppContext) {
    let mut skipping = usable();
    skipping.create_without_asking = true;
    let (mut helm, cx) = opened(cx, &skipping);
    let sent = press_in_new_sandbox(&mut helm, cx, &skipping);
    let (create, body) = the_create(&sent);
    assert_eq!(body["recipe"], json!("ready"));
    assert_eq!(body["confirmed"], json!(false));
    assert!(body.get("confirmed_size").is_none());
    assert!(!any_open(cx), "the launcher hid, asking nothing");
    helm.reply(
        create.id,
        400,
        Some(json!(
            "confirm creation of a billed sandbox before launching"
        )),
    );
    settle(cx);
    assert!(any_open(cx), "the launcher came back");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. confirm creation of a billed sandbox before launching")
    );
    assert_eq!(draft(cx).target, Some(sandbox("ready")));
}

/// The launcher reads the skip setting once, as it opens; turned off since, the helm refuses the unconfirmed create,
/// and a launcher that trusted its copy again would send the same create into the same refusal at every press, a dead
/// end short of closing it and losing the draft (found by the gate review, 2026-11-16). Specified: with the setting on
/// as read, the first press sends one unconfirmed create; after the helm refuses it, a press in the same launcher sends
/// nothing, reads the account no more, and asks the question; its Create and launch then sends one confirmed create.
#[gpui::test]
fn a_press_after_a_create_on_the_skip_setting_asks(cx: &mut TestAppContext) {
    let mut skipping = usable();
    skipping.create_without_asking = true;
    let (mut helm, cx) = opened(cx, &skipping);
    let sent = press_in_new_sandbox(&mut helm, cx, &skipping);
    let (create, body) = the_create(&sent);
    assert_eq!(
        body["confirmed"],
        json!(false),
        "the premise: sent on the skip"
    );
    helm.reply(
        create.id,
        400,
        Some(json!(
            "confirm creation of a billed sandbox before launching"
        )),
    );
    settle(cx);
    assert!(
        any_open(cx),
        "the premise: the refusal brought the launcher back"
    );
    test_support::click(cx, "launcher-launch");
    let sent = answer_with(&mut helm, cx, Some(&skipping), |command| {
        command.path == "/api/environment-sessions" || command.path == "/api/tensorlake"
    });
    assert_eq!(
        sent,
        [],
        "nothing is sent, and the account is not read again"
    );
    assert_eq!(
        name_of(cx, "prompt").as_deref(),
        Some("Create a sandbox for this session?")
    );
    test_support::click(cx, "prompt-confirm");
    let sent = environment_creates(&mut helm, cx, &skipping);
    let (_, body) = the_create(&sent);
    assert_eq!(body["confirmed"], json!(true));
}

/// The question asks about the draft as it was pressed; answering it after an edit would create a sandbox for a launch
/// the user never confirmed, as the YOLO question's own retirement guards against. Specified: with the question up,
/// choosing Existing host takes it away and sends nothing; back in New sandbox, a press asks again rather than sending.
#[gpui::test]
fn an_edit_retires_the_question(cx: &mut TestAppContext) {
    let (mut helm, cx) = opened(cx, &usable());
    press_in_new_sandbox(&mut helm, cx, &usable());
    assert!(
        cx.debug_bounds("prompt-confirm").is_some(),
        "the premise: the question is up"
    );
    pick_host_menu_row(cx, "launcher-host-option-1");
    assert_eq!(
        cx.debug_bounds("prompt-confirm"),
        None,
        "the edit retired it"
    );
    assert_eq!(environment_creates(&mut helm, cx, &usable()), []);
    let sent = press_in_new_sandbox(&mut helm, cx, &usable());
    assert_eq!(sent, [], "a new press asks first again");
    assert_eq!(
        name_of(cx, "prompt").as_deref(),
        Some("Create a sandbox for this session?")
    );
}

/// The old app's rule for a quick launch that would create a sandbox without the launcher on screen: reveal the
/// launcher with the question instead of creating (`create_form.rs` at the reference commit). A quick launch decides
/// out of sight whether to launch or show New, and a sandbox template's draft is decided against the account, so it
/// also waits for the account read, as a decision before it would show New saying "Loading Tensorlake settings…".
/// Specified: a launcher prepared out of sight on a sandbox draft of `ready`, with everything but its account read
/// answered, sends nothing and shows nothing; once the read answers with a usable account, it still sends nothing, and
/// the launcher is shown with the question, focus on Cancel; Create and launch then sends one confirmed environment
/// create for `ready`.
#[gpui::test]
fn a_hidden_quick_launch_waits_for_the_account_and_comes_back_to_ask(cx: &mut TestAppContext) {
    let (mut helm, cx) = opened(cx, &usable());
    test_support::click(cx, "launcher-cancel");
    assert!(!any_open(cx), "the premise: the opened launcher is gone");
    let mut prepared = command_draft();
    prepared.choose_runs_on(
        RunsOn::New(ProviderKind::Tensorlake),
        Some(sandbox("ready")),
    );
    cx.update(|window, cx| {
        let finish = |_: &mut LaunchDraft, _: super::draft::Catalog<'_>, _: &gpui::App| {
            super::view::Prepared::Launch { notice: None }
        };
        assert!(super::view::prepare_unseen(
            prepared,
            Mark::take(cx),
            finish,
            window,
            cx
        ));
    });
    let held = answer_with(&mut helm, cx, Some(&usable()), |command| {
        command.path == "/api/tensorlake" || command.path == "/api/environment-sessions"
    });
    let paths: Vec<&str> = held.iter().map(|command| command.path.as_str()).collect();
    assert_eq!(paths, ["/api/tensorlake"], "only the account read is out");
    assert!(!any_open(cx), "nothing is shown while it is out");
    helm.reply(
        held[0].id,
        200,
        Some(serde_json::to_value(usable()).unwrap()),
    );
    settle(cx);
    assert_eq!(
        environment_creates(&mut helm, cx, &usable()),
        [],
        "nothing is created before the answer"
    );
    assert!(any_open(cx), "the launcher came back to ask");
    assert_eq!(
        name_of(cx, "prompt").as_deref(),
        Some("Create a sandbox for this session?")
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-cancel"));
    test_support::click(cx, "prompt-confirm");
    let sent = environment_creates(&mut helm, cx, &usable());
    let (_, body) = the_create(&sent);
    assert_eq!(body["recipe"], json!("ready"));
    assert_eq!(body["confirmed"], json!(true));
}
