//! Tests of New sandbox in the real main view over the helm client's fake: the Tensorlake account the launcher reads as
//! it opens, the segment it shows, the recipe its segment enters on, its recipes listed with their mark, size and
//! refusals, the registration note, a press sending the create as the helm takes it today, and a quick launch prepared
//! out of sight waiting for the account. SPEC.md, "Creation" (the Runs on bullet) and "Sandbox sessions".
//!
//! The fleet is the helm's pinned fixture's first three hosts, as in `environment_view_tests`; the catalog holds
//! `plain`, with no snapshot, then `ready`, with one, both on managed Ubuntu at 2 CPUs and 4 GB memory. The account read
//! is answered with each test's [`Settings`] (`answer_with`).

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ProviderKind;
use farhelm_proto::tensorlake::{Settings, SshKeyStatus};
use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{LaunchDraft, RunsOn};
use super::environment_launch_tests::{answer_with, plain_preferences, window_with_more};
use super::environment_tests::{account, built_json, ready_key, recipe_json, sandbox};
use super::launch_tests::command_draft;
use super::view_tests::{draft, fixture_hosts, name_of, shown_refusal};
use crate::focus::Mark;
use crate::session_list::tests::painted;
use crate::test_support::{self, any_open, meta, settle};

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

/// SPEC.md, "Creation": "A segment with no usable place is hidden"; the old switch showed New sandbox only once the
/// account had an API key, since without one every recipe is refused. Specified: with the account read saying no API
/// key is saved, the switch shows Existing host and New container but no New sandbox.
#[gpui::test]
fn new_sandbox_is_hidden_without_an_api_key(cx: &mut TestAppContext) {
    let (_helm, cx) = opened(cx, &account(false, ready_key(COMMAND)));
    assert!(
        name_of(cx, "launcher-runs-on-New container").is_some(),
        "the premise: the switch is drawn"
    );
    assert_eq!(name_of(cx, "launcher-runs-on-New sandbox"), None);
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
    test_support::click(cx, "launcher-runs-on-New sandbox");
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
    test_support::click(cx, "launcher-runs-on-New sandbox");
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
/// made, the note says the old sentences and names the helm's command shown escaped (it holds a zero-width space), and
/// its "Copy registration command" puts the raw command on the clipboard byte for byte; back on Existing host the note
/// is gone.
#[gpui::test]
fn the_registration_note_shows_and_copies_the_command(cx: &mut TestAppContext) {
    let (_helm, cx) = opened(cx, &usable());
    test_support::click(cx, "launcher-runs-on-New sandbox");
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
        name_of(cx, "launcher-registration-copy").as_deref(),
        Some("Copy registration command")
    );
    test_support::click(cx, "launcher-registration-copy");
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
    test_support::click(cx, "launcher-runs-on-Existing host");
    assert_eq!(cx.debug_bounds("launcher-registration-note"), None);
}

/// Until the launcher asks "Create a sandbox for this session?", a press sends what main sends: an unconfirmed create,
/// which the helm refuses unless "Create Tensorlake sandboxes without asking" is on, and that refusal must reach the user
/// as any refused launch does, with the draft kept. Specified: a press in New sandbox on `ready` sends one environment
/// create on the Tensorlake account, unconfirmed (the rest of the body is shared with containers, whose tests check
/// it); the helm's refusal brings the launcher back saying "Not launched." and the helm's words, still on `ready`.
#[gpui::test]
fn a_press_sends_the_sandbox_create_unconfirmed_and_its_refusal_comes_back(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = opened(cx, &usable());
    test_support::click(cx, "launcher-runs-on-New sandbox");
    test_support::click(cx, "launcher-launch");
    let sent = answer_with(&mut helm, cx, Some(&usable()), |command| {
        command.path == "/api/environment-sessions"
    });
    let [create] = &sent[..] else {
        panic!("one press sent one environment create, not {sent:?}");
    };
    let body = create.body.clone().expect("a create carries a body");
    assert_eq!(body["place"], json!({ "kind": "tensorlake_account" }));
    assert_eq!(body["confirmed"], json!(false));
    assert!(
        !any_open(cx),
        "the premise: the launcher hid while its create is out"
    );
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

/// A quick launch decides out of sight whether to launch or show New, and a sandbox template's draft is decided against
/// the account; deciding before the account read answers would show New saying "Loading Tensorlake settings…" for a
/// launch that could start. Specified: a launcher prepared out of sight on a sandbox draft of `ready`, with everything
/// but its account read answered, sends no create and shows nothing; once the read answers with a usable account, it
/// sends one environment create for `ready` on the Tensorlake account, still without showing.
#[gpui::test]
fn a_hidden_quick_launch_waits_for_the_account_read(cx: &mut TestAppContext) {
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
    let sent = answer_with(&mut helm, cx, Some(&usable()), |command| {
        command.path == "/api/environment-sessions"
    });
    let [create] = &sent[..] else {
        panic!(
            "the quick launch sent one environment create once the account answered, not {sent:?}"
        );
    };
    let body: Value = create.body.clone().expect("a create carries a body");
    assert_eq!(body["recipe"], json!("ready"));
    assert_eq!(body["place"], json!({ "kind": "tensorlake_account" }));
    assert!(!any_open(cx), "the launcher was never shown");
}
