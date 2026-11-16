//! Tests of New's folder default in the real main view over the helm client's fake: New without an open session starts
//! in `~` and waits for the chosen host's launch history to name the folder its newest plain-folder launch started in,
//! Launch waits for it meanwhile, a failed read settles on `~`, and a quick launch prepared out of sight waits for it
//! too, or shows New when the history could not be read. A folder from a host's history, or New's default there,
//! stays bound to the installation it came from: once the host row names another, Launch refuses a folder the user
//! took, and an untouched default is decided again for the new one. The draft's own rules (once per opening or host
//! change, a typed folder kept, Clone and the held modes) are `draft_tests`'.
//!
//! SPEC.md, "Working directory"; SPEC_impl.md, "Launch templates" (the hidden quick launch). Ported from the old
//! `e2e/tests/destination-authority.spec.ts` ("the launcher's folder follows each host's last launch until the user
//! types one", "applied … retains installation authority through Launch"), `quick-launch.spec.ts` ("quick launch shows
//! New when the host's recent setups cannot be read") and `podman-environments.spec.ts`'s held-read wait, at reference
//! commit `24bc0580`.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::CollectionChanges;
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

// ---- A folder's installation ---------------------------------------------------------------------------------------

/// The refusal of a folder taken on an installation the host row no longer names, the old launcher's words.
const MOVED: &str = "The remembered folder belongs to a different installation; choose the host or folder again before launching";

/// Push the registry's hosts at `positions` (host-list fixture rows) with host `key`'s row on a new connection, and,
/// when `retarget` is set, retargeted onto another installation (a new recorded identity), as the helm pushes either.
fn push_host(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    positions: &[usize],
    key: &str,
    retarget: bool,
) {
    let mut state = super::view_tests::fixture_hosts(positions);
    let entry = state
        .entries
        .iter_mut()
        .find(|entry| entry.key == key)
        .expect("the premise: the host is in the fixture");
    if retarget {
        entry.value["identity"] = json!("replacement-install");
    }
    let incarnation = entry.value["incarnation"].as_u64().unwrap_or_default();
    entry.value["incarnation"] = json!(incarnation + 1);
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
}

/// Edit the open launcher's draft directly, and let it draw.
fn edit_draft(cx: &mut VisualTestContext, edit: impl FnOnce(&mut LaunchDraft)) {
    cx.update(|_, cx| {
        let launcher = cx
            .try_global::<super::view_tests::Opened>()
            .and_then(|opened| opened.0.upgrade())
            .expect("a launcher is open");
        launcher.update(cx, |launcher, cx| {
            edit(&mut launcher.draft);
            cx.notify();
        });
    });
    settle(cx);
}

/// A launcher on a draft like New's, on host1 in `/tmp`, where the strip offers its setups, whose history holds the two recent setups (Codex in
/// `/tmp` first) and the saved folder `/srv/saved`. The folder starts untouched and bound to nothing, so only taking a
/// folder from the history binds it.
fn new_with_history(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext) {
    let mut history = super::recents_view_tests::two_setups();
    history["folders"] = json!([{
        "host": 1,
        "canonical_cwd": "/srv/saved",
        "canonical_proven": true,
        "display_cwd": "/srv/saved",
        "created_at": 1,
        "creation_seq": null,
    }]);
    let (mut helm, cx) = super::view_tests::launcher_with_hosts(
        cx,
        LaunchDraft::new(Some(1), "/tmp".into(), None),
        super::view_tests::fixture_hosts(&[0]),
    );
    super::recents_view_tests::answer_history(&mut helm, cx, &history);
    (helm, cx)
}

/// Take a folder from host1's history one way, retarget host1's row onto another installation, and press Launch:
/// it is refused, beside Launch and under the host, and nothing is sent. Returns the folder taken.
fn refused_after_retarget(
    cx: &mut TestAppContext,
    take: impl FnOnce(&mut VisualTestContext),
) -> (FakeHelm, &mut VisualTestContext, String) {
    let (mut helm, cx) = new_with_history(cx);
    take(cx);
    let taken = draft(cx);
    assert!(
        taken.folder.is_touched() && taken.folder_install.is_some(),
        "the premise: a folder taken from the history, bound to its installation"
    );
    let folder = taken.folder.value().clone();
    push_host(&mut helm, cx, &[0], "1", true);
    assert!(answer(&mut helm, cx, Some(&remote_history())).is_empty());
    test_support::click(cx, "launcher-launch");
    assert_eq!(shown_refusal(cx), Some(format!("Not launched. {MOVED}")));
    assert_eq!(
        name_of(cx, "launcher-folder-installation-note").as_deref(),
        Some(MOVED),
        "said under the host too"
    );
    assert!(answer(&mut helm, cx, None).is_empty(), "nothing was sent");
    (helm, cx, folder)
}

/// A recent setup's folder taken while host1 named its first installation refuses Launch once the row is retargeted
/// onto another, and choosing the host again ends the refusal: Launch then goes out to that folder. Why: SPEC.md,
/// "Working directory": a path on two installations may hold different work, and the helm refuses only a missing one
/// (old `destination-authority.spec.ts`, "applied {ordinary recent|search recent|saved folder} retains installation
/// authority through Launch").
#[gpui::test]
fn a_recent_setups_folder_refuses_launch_on_a_retargeted_host(cx: &mut TestAppContext) {
    let (mut helm, cx, folder) =
        refused_after_retarget(cx, |cx| test_support::click(cx, "launcher-recent-0"));
    assert_eq!(folder, "/tmp");
    // The draft edit the host picker makes (`LaunchDraft::choose_host`), made directly.
    edit_draft(cx, |draft| draft.choose_host(1, "~".into()));
    assert_eq!(name_of(cx, "launcher-folder-installation-note"), None);
    test_support::click(cx, "launcher-launch");
    let creates = answer(&mut helm, cx, None);
    let [create] = &creates[..] else {
        panic!(
            "one create once the host is chosen again, not {}",
            creates.len()
        );
    };
    assert_eq!(create.body.as_ref().unwrap()["cwd"], json!("/tmp"));
}

/// A saved folder taken from the search box is bound to its installation as a recent setup's is. Why: the same rule
/// for every history pick (the old spec's "saved folder" case).
#[gpui::test]
fn a_saved_folder_from_search_refuses_launch_on_a_retargeted_host(cx: &mut TestAppContext) {
    let (_helm, _cx, folder) = refused_after_retarget(cx, |cx| {
        test_support::click(cx, "launcher-search-row");
        cx.simulate_input("folder:saved");
        settle(cx);
        test_support::keys(cx, "enter");
    });
    assert_eq!(folder, "/srv/saved");
}

/// A recent folder taken from the folder field's list is bound to its installation as a recent setup's is. Why: the
/// same rule for every history pick.
#[gpui::test]
fn a_recent_folder_from_the_folder_list_refuses_launch_on_a_retargeted_host(
    cx: &mut TestAppContext,
) {
    let (_helm, _cx, folder) = refused_after_retarget(cx, |cx| {
        test_support::click(cx, "launcher-folder-row");
        assert_eq!(
            name_of(cx, "launcher-folder-0").as_deref(),
            Some("/srv/saved"),
            "the premise: the list offers the saved folder first"
        );
        test_support::click(cx, "launcher-folder-0");
    });
    assert_eq!(folder, "/srv/saved");
}

/// A reconnect to the same installation (a new connection, the same recorded identity) keeps a taken folder: Launch
/// goes out to it. Why: only another installation may hold different work; refusing on every reconnect would make the
/// rule a nuisance (the old spec's control step).
#[gpui::test]
fn a_reconnect_to_the_same_installation_keeps_the_folder(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_with_history(cx);
    test_support::click(cx, "launcher-recent-0");
    assert!(
        draft(cx).folder_install.is_some(),
        "the premise: a bound folder"
    );
    push_host(&mut helm, cx, &[0], "1", false);
    assert!(answer(&mut helm, cx, Some(&remote_history())).is_empty());
    assert_eq!(name_of(cx, "launcher-folder-installation-note"), None);
    test_support::click(cx, "launcher-launch");
    let creates = answer(&mut helm, cx, None);
    let [create] = &creates[..] else {
        panic!("one create, not {}", creates.len());
    };
    assert_eq!(create.body.as_ref().unwrap()["cwd"], json!("/tmp"));
}

/// New's default from host1's history is decided again once the row is retargeted onto another installation: Launch
/// waits for the new installation's history, whose answer moves the untouched folder to its own last folder. Why: the
/// old installation's history says nothing about the new one (SPEC.md, "Working directory").
#[gpui::test]
fn a_default_is_decided_again_on_a_retargeted_host(cx: &mut TestAppContext) {
    let (mut helm, cx) = new_without_a_session(cx);
    assert!(answer(&mut helm, cx, Some(&remote_history())).is_empty());
    assert_eq!(draft(cx).folder.value(), "/srv/remote-work", "the premise");
    push_host(&mut helm, cx, &[0, 4, 8, 10], "1", true);
    assert_eq!(draft(cx).folder_default, FolderDefault::Pending);
    let mut replacement = remote_history();
    replacement["launches"][1]["cwd"] = json!("/srv/replacement");
    assert!(answer(&mut helm, cx, Some(&replacement)).is_empty());
    assert_eq!(draft(cx).folder.value(), "/srv/replacement");
    assert_eq!(draft(cx).folder_default, FolderDefault::Settled);
}

/// The folder New took from the open session, untouched, is decided again too once that session's host row is
/// retargeted: from the new installation's history, since the session is on the old one. Why: it is New's default,
/// not a folder the user chose (the old launcher re-armed it on any change of target).
#[gpui::test]
fn an_inherited_folder_is_decided_again_on_a_retargeted_host(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(cx.update(open_new), "the premise: New opened");
    settle(cx);
    let opened = draft(cx);
    assert_eq!(
        opened.host,
        Some(9),
        "the premise: beside session a on host9"
    );
    assert!(
        !opened.folder.is_touched() && opened.folder_install.is_some(),
        "the premise: session a's folder, untouched, bound to host9's installation"
    );
    let inherited = opened.folder.value().clone();
    assert_ne!(
        inherited, "/srv/remote-work",
        "the premise: a folder the history would change"
    );
    assert!(answer(&mut helm, cx, Some(&remote_history())).is_empty());
    push_host(&mut helm, cx, &[0, 4, 8, 10], "9", true);
    assert_eq!(draft(cx).folder_default, FolderDefault::Pending);
    assert_eq!(
        name_of(cx, "launcher-folder-installation-note"),
        None,
        "an untouched folder is decided again, not refused"
    );
    assert!(answer(&mut helm, cx, Some(&remote_history())).is_empty());
    assert_eq!(draft(cx).folder.value(), "/srv/remote-work");
}
