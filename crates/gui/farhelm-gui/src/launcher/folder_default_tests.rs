//! Tests of New's folder default in the real main view over the helm client's fake: New without an open session starts
//! in `~` and waits for the chosen host's launch history to name the folder its newest plain-folder launch started in,
//! Launch waits for it meanwhile, a failed read settles on `~`, and a quick launch prepared out of sight waits for it
//! too, or shows New when the history could not be read. The draft's own rules (once per opening or host change, a
//! typed folder kept, Clone and the held modes) are `draft_tests`'.
//!
//! SPEC.md, "Working directory"; SPEC_impl.md, "Launch templates" (the hidden quick launch). Ported from the old
//! `e2e/tests/destination-authority.spec.ts` ("the launcher's folder follows each host's last launch until the user
//! types one"), `quick-launch.spec.ts` ("quick launch shows New when the host's recent setups cannot be read") and
//! `podman-environments.spec.ts`'s held-read wait, at reference commit `24bc0580`.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::draft::{FolderDefault, LaunchDraft, Tab};
use super::entry::open_new;
use super::entry_tests::window_with;
use super::view_tests::{draft, name_of, read, shown_refusal};
use crate::test_support::{self, any_open, settle};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// A history whose newest launch went into a fresh managed checkout and whose next went into `/srv/remote-work`, the
/// folder New should start in.
fn remote_history() -> Value {
    let launch = |cwd: &str, repo: Option<Value>| {
        json!({
            "host": 1,
            "github_repo": repo,
            "canonical_cwd": cwd,
            "cwd": cwd,
            "selection": { "harness": "codex" },
            "created_at": 1,
            "creation_seq": null,
        })
    };
    json!({
        "launches": [
            launch("/srv/checkouts/bar-1", Some(json!({ "owner": "acme", "name": "bar" }))),
            launch("/srv/remote-work", None),
        ],
        "folders": [],
        "checkout_config_revision": 1,
    })
}

/// Answer every command the window sent since the last call: the history read with `history` (a failed read for
/// `None`), the catalog with an empty one, anything else with no body. Returns the creates, unanswered.
fn answer(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    history: Option<&Value>,
) -> Vec<FakeCommand> {
    cx.run_until_parked();
    let mut creates = Vec::new();
    for command in helm.commands() {
        if command.path == "/api/sessions" {
            creates.push(command);
        } else if command.path.starts_with("/api/launch-history") {
            match history {
                Some(history) => helm.reply(command.id, 200, Some(history.clone())),
                None => helm.reply(command.id, 500, None),
            }
        } else if command.path.ends_with("launch-catalog") {
            helm.reply(command.id, 200, Some(json!([])));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    creates
}

/// Every command the window sent since the last call, unanswered.
fn sent(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> Vec<FakeCommand> {
    cx.run_until_parked();
    helm.commands()
}

/// Fill the open launcher's Command tab with a complete command launch, so only the folder can hold Launch back.
fn complete_command(cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        let launcher = cx
            .try_global::<super::view_tests::Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher.draft.choose_tab(Tab::Command);
            launcher.draft.edit_command("true".into());
            launcher.draft.choose_command_yolo(false);
            cx.notify();
        });
    });
    settle(cx);
}

/// New, without an open session, on the helm's own host.
fn new_without_a_session(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let (helm, cx) = window_with(cx, None);
    assert!(cx.update(open_new), "the premise: New opened");
    settle(cx);
    (helm, cx)
}

// ---- A visible launcher --------------------------------------------------------------------------------------------

/// New without an open session starts in `~` and waits for the host's history: Launch says it waits for the host's last
/// folder and sends nothing. The answer moves the folder, and its field, to where the newest launch into a plain folder
/// started (a newer launch into a fresh managed checkout does not count), and Launch then launches there. Why: SPEC.md,
/// "Working directory" (the folder prefill's middle step), and the old app's refusal while the read was out (old
/// "the launcher's folder follows each host's last launch until the user types one").
#[gpui::test]
fn new_waits_for_the_hosts_last_folder_and_starts_there(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_without_a_session(cx);
    let opened: LaunchDraft = draft(cx);
    assert_eq!(opened.folder.value(), "~");
    assert_eq!(opened.folder_default, FolderDefault::Pending);
    complete_command(cx);
    // The reads are out but not answered yet.
    let reads = sent(&mut helm, cx);
    assert!(
        reads
            .iter()
            .any(|command| command.path.starts_with("/api/launch-history")),
        "the premise: New asked for the host's history"
    );
    test_support::click(cx, "launcher-launch");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. Wait for this host's last folder before launching")
    );
    for command in reads {
        if command.path.starts_with("/api/launch-history") {
            helm.reply(command.id, 200, Some(remote_history()));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    assert!(
        sent(&mut helm, cx).is_empty(),
        "the refused press sent nothing"
    );
    assert_eq!(draft(cx).folder.value(), "/srv/remote-work");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "/srv/remote-work",
        "the field shows it"
    );
    test_support::click(cx, "launcher-launch");
    let creates = answer(&mut helm, cx, Some(&remote_history()));
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    assert_eq!(
        create.body.as_ref().unwrap()["cwd"],
        json!("/srv/remote-work")
    );
}

/// A host chosen after New settled makes the folder wait again for that host's own default, and its history settles it
/// as the first host's did. Why: SPEC.md, "Changing the host re-seeds a folder the user has not typed or picked" (old
/// "the launcher's folder follows each host's last launch until the user types one", its host change).
#[gpui::test]
fn a_host_change_waits_for_the_new_hosts_folder(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_without_a_session(cx);
    assert!(answer(&mut helm, cx, Some(&remote_history())).is_empty());
    assert_eq!(draft(cx).folder.value(), "/srv/remote-work", "the premise");
    // The draft edit the host picker makes (`LaunchDraft::choose_host`), made directly.
    cx.update(|_, cx| {
        let launcher = cx
            .try_global::<super::view_tests::Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            launcher.draft.choose_host(9, "~".into());
            cx.notify();
        });
    });
    settle(cx);
    let waiting = draft(cx);
    assert_eq!(waiting.folder.value(), "~");
    assert_eq!(waiting.folder_default, FolderDefault::Pending);
    let mut nine = remote_history();
    nine["launches"][1]["cwd"] = json!("/srv/nine");
    let reads = sent(&mut helm, cx);
    assert!(
        reads
            .iter()
            .any(|command| command.path.starts_with("/api/launch-history?host=9")),
        "the premise: the new host's history was asked for"
    );
    for command in reads {
        if command.path.starts_with("/api/launch-history") {
            helm.reply(command.id, 200, Some(nine.clone()));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    assert_eq!(draft(cx).folder.value(), "/srv/nine");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.folder.text(cx).to_string()),
        "/srv/nine",
        "the field shows it"
    );
}

/// A history read that fails settles New's folder on `~`, which a visible launcher launches into as any `~`. Why: the
/// read is advisory; failing it must not hold Launch for good (old `folder_default_step`'s unreadable arm).
#[gpui::test]
fn a_failed_history_read_settles_on_home(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_without_a_session(cx);
    complete_command(cx);
    assert!(answer(&mut helm, cx, None).is_empty());
    let settled = draft(cx);
    assert_eq!(settled.folder.value(), "~");
    assert_eq!(settled.folder_default, FolderDefault::Unread);
    test_support::click(cx, "launcher-launch");
    let creates = answer(&mut helm, cx, None);
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    assert_eq!(create.body.as_ref().unwrap()["cwd"], json!("~"));
}

/// New beside an open session on the chosen host's installation takes that session's folder and waits for nothing.
/// Why: SPEC.md puts "the selected session's directory" first, ahead of the history.
#[gpui::test]
fn new_beside_an_open_session_waits_for_nothing(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert!(cx.update(open_new), "the premise: New opened");
    settle(cx);
    let opened = draft(cx);
    assert_ne!(
        opened.folder.value(),
        "~",
        "the premise: the session's folder"
    );
    assert_eq!(opened.folder_default, FolderDefault::Settled);
    assert!(!opened.folder_default_waiting());
}

// ---- A quick launch out of sight -----------------------------------------------------------------------------------

/// Prepare a quick launch of a complete command launch out of sight, from New's own prefill, as a template shortcut
/// does, and answer its catalog read; returns what it sent next, unanswered.
fn quick_launch(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, Vec<FakeCommand>) {
    let (mut helm, cx) = window_with(cx, None);
    cx.update(|window, cx| {
        let draft = super::entry::new_prefill(cx).expect("New's prefill");
        assert_eq!(draft.folder_default, FolderDefault::Pending, "the premise");
        let finish = |draft: &mut LaunchDraft, _: super::draft::Catalog<'_>, _: &gpui::App| {
            draft.choose_tab(Tab::Command);
            draft.edit_command("true".into());
            draft.choose_command_yolo(false);
            super::view::Prepared::Launch { notice: None }
        };
        assert!(super::view::prepare_unseen(
            draft,
            crate::focus::Mark::take(cx),
            finish,
            window,
            cx
        ));
    });
    settle(cx);
    let mut read = false;
    for command in sent(&mut helm, cx) {
        if command.path.ends_with("launch-catalog") {
            read = true;
            helm.reply(command.id, 200, Some(json!([])));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    assert!(read, "the premise: the quick launch read the catalog");
    settle(cx);
    let next = sent(&mut helm, cx);
    (helm, cx, next)
}

/// A quick launch prepared out of sight waits, still unseen, for the host's history, then launches into the folder it
/// names. Why: SPEC_impl.md, "Launch templates": "a hidden quick launch waits for it", so a template's shortcut starts
/// in the host's last folder rather than in `~` or opening New to say it waits.
#[gpui::test]
fn a_quick_launch_waits_out_of_sight_for_the_hosts_last_folder(cx: &mut TestAppContext) {
    let (mut helm, cx, next) = quick_launch(cx);
    assert!(
        next.iter().all(|command| command.path != "/api/sessions"),
        "no create before the history"
    );
    let reads: Vec<&FakeCommand> = next
        .iter()
        .filter(|command| command.path.starts_with("/api/launch-history"))
        .collect();
    assert_eq!(
        reads.len(),
        1,
        "the quick launch asked for the history itself"
    );
    assert!(!any_open(cx), "nothing is shown while it waits");
    for command in &next {
        if command.path.starts_with("/api/launch-history") {
            helm.reply(command.id, 200, Some(remote_history()));
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    let creates: Vec<FakeCommand> = sent(&mut helm, cx)
        .into_iter()
        .filter(|command| command.path == "/api/sessions")
        .collect();
    let [create] = &creates[..] else {
        panic!(
            "one create once the history answered, not {}",
            creates.len()
        );
    };
    assert_eq!(
        create.body.as_ref().unwrap()["cwd"],
        json!("/srv/remote-work")
    );
    assert!(!any_open(cx), "the launcher was never shown");
}

/// A quick launch whose host's history cannot be read shows New saying why, and launches nothing. Why: the
/// folder it would start in is only a stand-in then, and the old app's quick launch let the user check it (old "quick
/// launch shows New when the host's recent setups cannot be read").
#[gpui::test]
fn a_quick_launch_without_the_hosts_history_shows_new(cx: &mut TestAppContext) {
    let (mut helm, cx, next) = quick_launch(cx);
    assert_eq!(
        next.iter()
            .filter(|command| command.path.starts_with("/api/launch-history"))
            .count(),
        1,
        "the premise: the quick launch asked for the history"
    );
    for command in &next {
        if command.path.starts_with("/api/launch-history") {
            helm.reply(command.id, 500, None);
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    settle(cx);
    assert!(any_open(cx), "New is shown");
    assert_eq!(
        name_of(cx, "launcher-note").as_deref(),
        Some(
            "Couldn't prepare quick launch: this host's recent setups could not be read, so its last folder is \
             unknown; check the folder and launch"
        )
    );
    assert!(
        answer(&mut helm, cx, None).is_empty(),
        "and nothing was launched"
    );
}
