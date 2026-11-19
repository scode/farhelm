//! Tests of the launcher's New and Clone entry points in the real main view over the helm client's fake: New's prefill
//! from the open session, New named's name, Cmd+N's action, Clone's prefill from its source and its host fallback, and
//! both refused while a launch is pending (New also while a host change is in flight; Clone shares its check). The last
//! section has Clone and New carrying a sandbox session's environment into a fresh one, and Replace with keeping it.
//!
//! The fleet: host1 (the helm's own host, kind `local`), host9 (a connected ssh host recorded as install `identity9`)
//! and host11 (a sandbox a session owns, naming no environment except in [`environment_window`]; the real helm builds a
//! host's sandbox binding and its environment view together, so host11 without one is a state the helm does not
//! serve, kept for the fallback it guards), with session `a` on
//! host9's install, `b` on host9's row but listed under an install the row no longer names, `c` on host11, and `e`, a
//! titled command session on host9's install that the list hides (as a filter would) while the helm still carries it.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{CollectionState, Preferences};
use farhelm_proto::{EnvironmentHostView, LaunchPermission, LaunchTarget, Place};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::draft::Mode;
use super::draft::{LaunchDraft, Tab};
use super::entry::HOST_NOT_CARRIED;
use super::entry::{new_available, open_clone, open_new, open_new_named, open_replace_with};
use super::environment_launch_tests::{answer_except, catalog};
use super::launch_tests::{HostChange, created, creates, start_host_change};
use super::view_tests::{draft, fixture_hosts, name_of, read, shown_refusal};
use crate::actions::NewSession;
use crate::layers;
use crate::local_state;
use crate::main_view::MainView;
use crate::session_list::tests::{change, listing, listing_meta, preferences_state, session};
use crate::test_support::{self, any_open, settle, started};

/// The sessions: `a` on host9 as its install `identity9`, with one terminal tab open, `b` on host9's row as the install
/// `old-install` it no longer names, `c` on the sandbox host11, and `d` on host5 as its recorded install `identity5`,
/// which host5 now contradicts (its state is an identity mismatch), and `e` on host9 as `identity9`, titled "fix login"
/// and launched as a command with a resume command, the shape Clone carries verbatim. `e` is outside the listing's
/// order: the session a filter hides, which the helm keeps in the pushed collection while it is open, so the open
/// session's actions still find it.
fn fleet() -> CollectionState {
    let mut state = listing(
        &["a", "b", "c", "d"],
        &[
            session("a").host(9).tabs(1),
            session("b").host(9),
            session("c").host(11),
            session("d").host(5),
            session("e")
                .host(9)
                .title("fix login")
                .unmanaged()
                .not_launched_as_checkout()
                .launch(json!({
                    "kind": "command",
                    "command": "make watch {farhelm_args}",
                    "yolo": false,
                    "agent": "codex",
                    "resume": "make watch --resume {conversation} {farhelm_args}",
                })),
        ],
    );
    let identities = [
        "identity9",
        "old-install",
        "identity11",
        "identity5",
        "identity9",
    ];
    for (entry, identity) in state.entries.iter_mut().zip(identities) {
        entry.value["host_identity"] = json!(identity);
    }
    state
}

/// The main view over the fake helm with hosts 1, 5, 9 and 11 and `yolo` as the remembered permission: with [`fleet`]
/// and `selected` selected, or, for `None`, with no sessions at all, since the selection rules open a listed session by
/// themselves and "no open session" needs nothing to open.
pub(super) fn window_with<'a>(
    cx: &'a mut TestAppContext,
    selected: Option<&str>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    window_on(cx, selected, fixture_hosts(&[0, 4, 8, 10]), Vec::new())
}

/// [`window_with`] over the hosts collection `hosts`, with `more` collections in the helm's first snapshot.
fn window_on<'a>(
    cx: &'a mut TestAppContext,
    selected: Option<&str>,
    hosts: CollectionState,
    more: Vec<CollectionState>,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    let sessions = if selected.is_some() {
        fleet()
    } else {
        listing(&[], &[])
    };
    let mut snapshot = vec![
        sessions,
        hosts,
        preferences_state(Preferences {
            remembered_permissions: Some("yolo".into()),
            // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
            welcome_seen: Some(true),
            ..Preferences::default()
        }),
    ];
    snapshot.extend(more);
    helm.connect(snapshot);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    if let Some(key) = selected {
        window.update(|_, cx| {
            local_state::select(key, cx);
        });
    }
    settle(window);
    (helm, window)
}

/// Run New in the window, as its button's handler would, and settle.
fn new(cx: &mut VisualTestContext) -> bool {
    let opened = cx.update(open_new);
    settle(cx);
    opened
}

/// The launcher opened last, by its entity id.
fn opened(cx: &mut VisualTestContext) -> Option<gpui::EntityId> {
    cx.update(|_, cx| {
        layers::last_opened::<super::view::Launcher>(cx).map(|launcher| launcher.entity_id())
    })
}

/// The session the window has open.
fn selected(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| local_state::selected(cx))
}

/// The host and folder the open launcher's draft holds.
pub(super) fn destination(cx: &mut VisualTestContext) -> (Option<i64>, String) {
    let draft: LaunchDraft = draft(cx);
    (draft.host, draft.folder.value().clone())
}

/// SPEC.md, "Creation": the host "defaults to the host of the currently open session", and the folder to "the selected
/// session's directory when that session is on the target host's installation". New's whole point is to start next to
/// what the user is looking at. Specified: with a open on host9's install, New opens on host9 in a's folder, with the
/// remembered permission, and nothing touched.
#[gpui::test]
fn new_opens_beside_the_open_session(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert!(new(cx), "New opened");
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
    let draft = draft(cx);
    assert_eq!(
        draft.agent.permissions,
        Some(LaunchPermission::Yolo),
        "with the remembered permission"
    );
    assert!(
        !draft.folder.is_touched(),
        "a prefilled folder is a seed, not an edit"
    );
}

/// The open session's host and folder hold for a session the list hides too: a filter changes what the list shows,
/// not what is open (SPEC.md, "Host": the host "defaults to the host of the currently open session"), and the helm
/// keeps the hidden open session in what it pushes. Specified: with e open but outside the listing's order, New opens
/// on host9 in e's folder.
#[gpui::test]
fn new_opens_beside_an_open_session_the_list_hides(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("e"));
    assert_eq!(selected(cx).as_deref(), Some("e"), "the premise: e is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(9), "/home/user/src/e".to_owned()));
}

/// SPEC.md, "Host": with no open session, New uses "the helm's own host"; "Working directory": else "`~`, which each
/// host expands to its own home". Specified: nothing selected, New opens on host1 (the local row) in `~`.
#[gpui::test]
fn new_without_an_open_session_starts_on_the_helms_own_host(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, None);
    assert_eq!(
        cx.update(|_, cx| local_state::selected(cx)),
        None,
        "the premise: no session is open"
    );
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
}

/// SPEC.md, "Host": "a row retargeted or adopted onto a different install after the session was selected falls back to
/// the helm's own host rather than silently aiming the create at the successor". Aiming at the successor would start a
/// session on a machine the user never chose. Specified: with b open (listed under an install host9's row no longer
/// names), New opens on host1 in `~`, not on host9 in b's folder.
#[gpui::test]
fn new_does_not_follow_a_retargeted_host_row(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("b"));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
}

/// A sandbox a session owns is no place for an unrelated session, and the host picker leaves such hosts out, so New
/// from an open sandbox session never lands there; with no environment named on the host to preselect
/// ([`new_beside_an_open_environment_session_preselects_its_recipe_and_place`] has one), it is New on the helm's own
/// host. Specified: with c open on host11, which names no environment, New opens on host1 in `~` with no target.
#[gpui::test]
fn new_from_a_sandbox_session_starts_on_the_helms_own_host(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("c"));
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
    assert_eq!(draft(cx).target, None);
}

/// SPEC.md, the quick switcher's "New session named X": "Picking it opens New with that name and the default launch
/// target and directory (Creation), plus remembered permission defaults." Specified: New named "fix-login" with a open
/// opens New beside a with that name, entered as a hand edit.
#[gpui::test]
fn new_named_opens_new_with_the_name(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert!(cx.update(|window, cx| open_new_named("fix-login", window, cx)));
    settle(cx);
    let draft = draft(cx);
    assert_eq!(draft.name.value(), "fix-login");
    assert!(draft.name.is_touched(), "the name counts as typed");
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
}

/// SPEC.md, "Desktop window chrome": "Cmd+N in the Mac desktop app opens New with the same prefill ... unless a modal is
/// open". The chord is bound on the Mac only, so this dispatches its action, which is what the binding sends.
/// Specified: the action opens New; dispatched again while New is open, it opens nothing more.
#[gpui::test]
fn the_new_session_action_opens_new_unless_a_dialog_is_open(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, None);
    assert!(!any_open(cx), "the premise: nothing is open");
    cx.dispatch_action(NewSession);
    settle(cx);
    assert!(any_open(cx), "the action opened New");
    let first = opened(cx);
    cx.dispatch_action(NewSession);
    settle(cx);
    assert_eq!(
        opened(cx),
        first,
        "a second New did not open over the first"
    );
}

/// SPEC.md, "Creation": while a launch is pending "the same launcher remains busy and cannot accept a second launch or be
/// replaced with another draft", and Cmd+N works "unless ... New is disabled". Specified: after a press of Launch sends
/// a create, New is unavailable and opens nothing; once the helm refuses the create after the user moved on, New is
/// available again and brings back that launcher with its refusal instead of a fresh one.
#[gpui::test]
fn new_waits_for_a_pending_launch_and_then_brings_back_its_refusal(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, None);
    assert!(new(cx));
    cx.update(|_, cx| {
        let launcher = layers::last_opened::<super::view::Launcher>(cx);
        launcher
            .expect("New opened a launcher")
            .update(cx, |launcher, cx| {
                launcher.draft.choose_tab(Tab::Command);
                launcher.draft.edit_command("true".into());
                launcher.draft.choose_command_yolo(false);
                cx.notify();
            });
    });
    settle(cx);
    // The launcher's reads answered first: New waits for the host's history to settle its folder.
    let early = creates(&mut helm, cx);
    assert!(early.is_empty(), "the premise: nothing pressed yet");
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("the press sent one create, not {}", sent.len());
    };
    let create = create.id;
    assert!(!any_open(cx), "the premise: the launcher hid");
    assert!(
        !cx.update(|_, cx| new_available(cx)),
        "New is unavailable while the create is out"
    );
    assert!(!new(cx), "and opens nothing");
    assert!(!any_open(cx));
    test_support::keys(cx, "x");
    helm.reply(create, 409, Some(json!("host1 is not connected")));
    settle(cx);
    assert!(
        cx.update(
            |_, cx| crate::notices::posted(cx).contains(&crate::notices::Notice::LaunchNotStarted)
        ),
        "the premise: the refusal waits behind its notice"
    );
    assert!(!any_open(cx), "and nothing opened");
    assert!(
        cx.update(|_, cx| new_available(cx)),
        "New is available again"
    );
    assert!(new(cx), "New opened");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. host1 is not connected"),
        "New brought back the refused launcher, not a fresh one"
    );
}

/// SPEC.md, "Host": New aims at the install the user was looking at. A host whose reported identity contradicts its
/// recorded one is waiting for the user to decide which install it is, so it cannot be that install yet, whatever its
/// registry row still records (the old app's comparison excluded it too). Specified: with d open on host5, in an
/// identity mismatch, New opens on host1 in `~`.
#[gpui::test]
fn new_does_not_follow_a_host_in_an_identity_mismatch(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("d"));
    assert_eq!(selected(cx).as_deref(), Some("d"), "the premise: d is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
}

/// SPEC.md, "Working directory": the folder is prefilled "for the target host", and "Changing the host re-seeds a
/// folder the user has not typed or picked". So the open session's folder belongs to its host, not to the draft: it
/// goes when another host is picked and comes back when its own is picked again. Specified: with a open, New opens on
/// host9 in a's folder; picking host1 re-seeds `~`; picking host9 again brings a's folder back.
#[gpui::test]
fn picking_a_host_re_seeds_its_own_folder(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert!(new(cx));
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-1");
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-9");
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
}

/// New is unavailable while a host change is in flight, as the old New button was disabled while the page's one
/// operation token was held by a host change (SPEC.md, "Creation": "Launches and changes to the host registry still
/// exclude each other"); a probe counts by the migration's orchestrator's decision, though the old token left it out. Specified: with a host probe in flight New is unavailable and opens nothing; once it is answered New opens.
#[gpui::test]
fn new_waits_for_a_host_change(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, None);
    let path = start_host_change(cx, HostChange::Probe);
    cx.run_until_parked();
    let sent = helm.commands();
    let probe = sent
        .iter()
        .position(|command| command.path == path)
        .expect("the premise: the probe is in flight");
    assert!(
        !cx.update(|_, cx| new_available(cx)),
        "New is unavailable while the probe is out"
    );
    assert!(!new(cx), "and opens nothing");
    // The fake answers in order: what the window sent before the probe (each connected host's trash read) goes first.
    for earlier in &sent[..probe] {
        assert_eq!(
            earlier.path, "/api/checkout-trash/list",
            "the premise: only trash reads went out before the probe"
        );
        helm.reply(earlier.id, 200, None);
    }
    helm.reply(sent[probe].id, 200, Some(json!({})));
    settle(cx);
    assert!(new(cx), "with the probe answered, New opens");
}

/// Run Clone of `session` in the window, as the row menu's handler would, and settle.
fn clone(cx: &mut VisualTestContext, session: &str) -> bool {
    let opened = cx.update(|window, cx| open_clone(session, window, cx));
    settle(cx);
    opened
}

/// SPEC.md, "Clone": an ordinary, editable create "pre-filled from an existing session's host, title, and launch", where
/// a command launch carries "its command, YOLO assertion, declared agent type, and resume command" verbatim, and "Other
/// sessions keep their working directory". Clone is a copy of what the user is looking at, so anything it drops or
/// rewrites starts a different session than the one copied. Specified: Clone of e opens the Command tab in Clone mode
/// on host9 in e's folder, named "fix login", with e's command, assertion, agent type and resume command, nothing
/// touched and no prompt.
#[gpui::test]
fn clone_prefills_the_sources_host_folder_title_and_launch(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert!(clone(cx, "e"), "Clone opened");
    let draft = draft(cx);
    assert_eq!(draft.mode, Mode::Clone);
    assert_eq!(draft.tab, Tab::Command);
    assert_eq!(destination(cx), (Some(9), "/home/user/src/e".to_owned()));
    assert_eq!(draft.name.value(), "fix login");
    assert_eq!(draft.command.command, "make watch {farhelm_args}");
    assert_eq!(draft.command.yolo, Some(false));
    assert_eq!(
        draft.command.agent,
        Some(farhelm_proto::LaunchHarness::Codex)
    );
    assert!(draft.command.resume_on, "with its resume command on");
    assert_eq!(
        draft.command.resume,
        "make watch --resume {conversation} {farhelm_args}"
    );
    assert!(draft.prompt.is_empty() && draft.conversation.is_empty());
    assert!(
        !draft.folder.is_touched() && !draft.name.is_touched(),
        "prefilled values are seeds, not edits"
    );
}

/// A Clone spells out invisible characters in the copied fields, so the user can see what they are about to launch.
/// Typing beside the spellings keeps the hidden characters in both the draft and the launch request. A hidden character
/// typed into the field is spelled out at once, and a spelling typed by hand becomes the character it names.
#[gpui::test]
fn clone_shows_its_copied_folder_and_name_escaped_and_editable(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    let source = session("e")
        .host(9)
        .cwd("/srv/work\u{200b}box")
        .title("fix\u{202e} login")
        .unmanaged()
        .not_launched_as_checkout()
        .launch(json!({"kind": "command", "command": "true", "yolo": false}));
    let mut changes = change(
        listing_meta(&["a", "b", "c", "d"], 5, false),
        &[source],
        &[],
    );
    changes.upsert[0].value["host_identity"] = json!("identity9");
    helm.push(changes);
    settle(cx);
    assert!(clone(cx, "e"));
    let shown = |cx: &mut VisualTestContext| {
        read(cx, |launcher, _, cx| {
            (
                launcher.folder.text(cx).to_string(),
                launcher.name.text(cx).to_string(),
            )
        })
    };
    assert_eq!(
        shown(cx),
        ("/srv/work<U+200B>box".into(), "fix<U+202E> login".into())
    );

    test_support::click(cx, "launcher-folder-row");
    assert!(read(cx, |launcher, window, cx| launcher
        .folder
        .focus_handle(cx)
        .is_focused(window)));
    test_support::keys(cx, "end x");
    assert_eq!(draft(cx).folder.value(), "/srv/work\u{200b}boxx");
    // The recent-folder list covers the name field. Escape dismisses the list while keeping Clone open.
    test_support::keys(cx, "backspace escape");

    test_support::click(cx, "launcher-name-row");
    assert!(read(cx, |launcher, window, cx| launcher
        .name
        .focus_handle(cx)
        .is_focused(window)));
    test_support::keys(cx, "end x");
    assert_eq!(draft(cx).name.value(), "fix\u{202e} loginx");
    test_support::keys(cx, "backspace");
    assert_eq!(
        shown(cx),
        ("/srv/work<U+200B>box".into(), "fix<U+202E> login".into())
    );
    let edited = draft(cx);
    assert!(edited.folder.is_touched() && edited.name.is_touched());
    assert_eq!(edited.folder.value(), "/srv/work\u{200b}box");
    assert_eq!(edited.name.value(), "fix\u{202e} login");

    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one copy was expected, got {sent:?}")
    };
    let body = create.body.as_ref().expect("a create carries its draft");
    assert_eq!(body["cwd"], json!("/srv/work\u{200b}box"));
    assert_eq!(body["title"], json!("fix\u{202e} login"));
    created(&mut helm, cx, create.id, "copy");

    // A hidden character typed or pasted into the field is spelled out at once (the edit resyncs the field), and a
    // spelling typed by hand stands for the character it names.
    assert!(clone(cx, "e"), "the source can be copied again");
    test_support::click(cx, "launcher-name-row");
    assert!(read(cx, |launcher, window, cx| launcher
        .name
        .focus_handle(cx)
        .is_focused(window)));
    test_support::keys(cx, "end");
    cx.simulate_input("\u{200b}");
    settle(cx);
    assert_eq!(shown(cx).1, "fix<U+202E> login<U+200B>");
    assert_eq!(draft(cx).name.value(), "fix\u{202e} login\u{200b}");
    cx.simulate_input("<U+200B>");
    settle(cx);
    assert_eq!(shown(cx).1, "fix<U+202E> login<U+200B><U+200B>");
    assert_eq!(draft(cx).name.value(), "fix\u{202e} login\u{200b}\u{200b}");
}

/// The managed-checkout name is calculated from the copied title. Its automatic suffix must keep the title's
/// spelling through draws. Editing the numbered suggestion makes it an exact name with the same hidden characters.
#[gpui::test]
fn a_managed_clones_numbered_name_spells_out_its_source_title(cx: &mut TestAppContext) {
    let mut source = super::draft_tests::session(farhelm_proto::SessionLaunch::Command(
        farhelm_proto::CommandLaunch {
            command: "true".into(),
            yolo: false,
            agent: None,
            resume: None,
        },
    ));
    source.title = "fix\u{202e} login".into();
    source.github_repo = Some(farhelm_proto::GithubRepo {
        owner: "acme".into(),
        name: "project".into(),
    });
    let mut start = LaunchDraft::clone_of(&source, Some(1));
    assert!(
        start.advance_checkout_name("fix\u{202e} login-clone"),
        "the copied title seeds a numbered name"
    );
    let (_helm, cx) = super::view_tests::launcher_on(cx, start);
    assert_eq!(draft(cx).checkout_title(), "fix\u{202e} login-clone-2");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.name.text(cx).to_string()),
        "fix<U+202E> login-clone-2"
    );
    test_support::click(cx, "launcher-name-row");
    assert!(read(cx, |launcher, window, cx| launcher
        .name
        .focus_handle(cx)
        .is_focused(window)));
    test_support::keys(cx, "end x backspace");
    assert!(draft(cx).name.is_touched());
    assert_eq!(draft(cx).checkout_title(), "fix\u{202e} login-clone-2");
    assert_eq!(
        read(cx, |launcher, _, cx| launcher.name.text(cx).to_string()),
        "fix<U+202E> login-clone-2"
    );
}

/// The note the open launcher shows under Host, if any.
fn host_note(cx: &mut VisualTestContext) -> Option<String> {
    name_of(cx, "launcher-host-note")
}

/// SPEC.md, "Host": a create aims at the install the user was looking at, and a row retargeted onto another install
/// "falls back to the helm's own host rather than silently aiming the create at the successor". The old launcher bound
/// a Clone to its source's host only once the registry confirmed that install, else used New's host and said so under
/// Host, so the fallback is never silent. "Other sessions keep their working directory" holds either way. Specified:
/// with b open (listed under an install host9's row no longer names), Clone of b opens on host1 with b's folder and the
/// old note; picking a host takes the note away, since the host is then the user's choice.
#[gpui::test]
fn clone_of_a_session_on_a_retargeted_host_row_falls_back_and_says_so(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("b"));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    assert!(clone(cx, "b"));
    assert_eq!(destination(cx), (Some(1), "/home/user/src/b".to_owned()));
    assert_eq!(host_note(cx).as_deref(), Some(HOST_NOT_CARRIED));
    test_support::click(cx, "launcher-host");
    test_support::click(cx, "launcher-host-option-9");
    assert_eq!(host_note(cx), None, "the picked host is the user's");
}

/// A host a sandbox or container owns is no place for another session, so a Clone of a session there that cannot say
/// which environment to make afresh (its host names none) falls back to New's host with the old note, as for a host it
/// cannot confirm, rather than landing silently elsewhere (decided by the launcher area on 2026-11-16). Specified: with
/// c (on the sandbox host11, which names no environment) open, Clone of c opens on host1 with c's folder, no target and
/// the note; Clone of e, whose host is confirmed, shows none.
#[gpui::test]
fn clone_of_a_sandbox_session_with_nothing_to_carry_falls_back_and_says_so(
    cx: &mut TestAppContext,
) {
    let (_helm, cx) = window_with(cx, Some("c"));
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c is open");
    assert!(clone(cx, "c"));
    assert_eq!(destination(cx), (Some(1), "/home/user/src/c".to_owned()));
    assert_eq!(draft(cx).target, None);
    assert_eq!(host_note(cx).as_deref(), Some(HOST_NOT_CARRIED));
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed the Clone");
    assert!(clone(cx, "e"));
    assert_eq!(host_note(cx), None);
}

/// Clone's refusals are New's: it replaces the launcher's draft, which SPEC.md, "Creation" forbids while a launch is
/// pending ("cannot accept a second launch or be replaced with another draft"), it does not open over another dialog,
/// and a session the store no longer holds has nothing to copy. Specified: Clone opens nothing for an unknown key, nor
/// while New is open.
#[gpui::test]
fn clone_opens_nothing_over_a_dialog_or_for_an_unknown_session(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert!(!clone(cx, "gone"), "nothing to copy");
    assert!(!any_open(cx));
    assert!(new(cx), "the premise: New opened");
    let first = opened(cx);
    assert!(!clone(cx, "e"), "Clone does not open over New");
    assert_eq!(opened(cx), first);
}

/// A refused launch waiting behind its notice arrived after the user moved on, so they have not seen it; replacing its
/// draft with a Clone would lose it unseen (SPEC.md, "Who may move focus": the notice "brings back the launcher with
/// its draft and error"). So Clone brings it back as New does, and Clone works again once it is closed. Before the
/// refusal, Clone waits for the pending launch like New. Specified: while a create is out Clone opens nothing; once
/// the helm refuses it after the user moved on, Clone of e brings back that launcher with its refusal and withdraws the
/// notice; after Escape, Clone of e opens e's copy.
#[gpui::test]
fn clone_brings_back_a_refused_launch_waiting_behind_its_notice(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(new(cx));
    cx.update(|_, cx| {
        let launcher = layers::last_opened::<super::view::Launcher>(cx);
        launcher
            .expect("New opened a launcher")
            .update(cx, |launcher, cx| {
                launcher.draft.choose_tab(Tab::Command);
                launcher.draft.edit_command("true".into());
                launcher.draft.choose_command_yolo(false);
                cx.notify();
            });
    });
    settle(cx);
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("the press sent one create, not {}", sent.len());
    };
    let create = create.id;
    assert!(!clone(cx, "e"), "Clone waits while the create is out");
    assert!(!any_open(cx));
    test_support::keys(cx, "x");
    helm.reply(create, 409, Some(json!("host9 is not connected")));
    settle(cx);
    let notice_up = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            crate::notices::posted(cx).contains(&crate::notices::Notice::LaunchNotStarted)
        })
    };
    assert!(
        notice_up(cx),
        "the premise: the refusal waits behind its notice"
    );
    assert!(clone(cx, "e"), "Clone opened a launcher");
    assert_eq!(draft(cx).mode, Mode::New, "the refused New, not a Clone");
    assert_eq!(
        shown_refusal(cx).as_deref(),
        Some("Not launched. host9 is not connected")
    );
    assert!(!notice_up(cx), "the notice has nothing left to offer");
    test_support::keys(cx, "escape");
    assert!(!any_open(cx), "the premise: Escape closed it");
    assert!(clone(cx, "e"));
    assert_eq!(draft(cx).mode, Mode::Clone);
    assert_eq!(shown_refusal(cx), None);
}

/// Each opening of Clone starts from its source again, and leaves nothing behind for the next New (the old
/// `clone.spec.ts` cases "each clone restores every field after an intervening draft edit" and "closing a clone without
/// submitting, or submitting it, both leave the next New Session with fresh defaults"): a copy that kept an earlier
/// edit, or a New that inherited a Clone's settings, would start a session the user did not ask for. Specified: Clone of
/// e renamed by hand and closed, then Clone of e again, shows e's title; closing it, New opens beside a with no launch
/// choices of e's.
#[gpui::test]
fn each_clone_starts_from_its_source_and_new_starts_fresh_after_closing(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert!(clone(cx, "e"));
    cx.update(|_, cx| {
        let launcher = layers::last_opened::<super::view::Launcher>(cx);
        launcher.expect("Clone opened").update(cx, |launcher, cx| {
            launcher.draft.edit_name("renamed".into());
            cx.notify();
        });
    });
    settle(cx);
    assert_eq!(
        draft(cx).name.value(),
        "renamed",
        "the premise: the edit took"
    );
    test_support::keys(cx, "escape");
    assert!(clone(cx, "e"));
    assert_eq!(draft(cx).name.value(), "fix login");
    test_support::keys(cx, "escape");
    assert!(new(cx));
    let fresh = draft(cx);
    assert_eq!(fresh.mode, Mode::New);
    assert_eq!(fresh.tab, Tab::Agent);
    assert!(fresh.command.command.is_empty() && fresh.name.value().is_empty());
    assert_eq!(destination(cx), (Some(9), "/home/user/src/a".to_owned()));
}

/// One submit path: a Clone launches as any create does, carrying the copied launch and destination in its request
/// (SPEC.md, "Clone": "cloning starts a brand-new, independent create through the same form"), and leaves nothing behind
/// for the next New. Specified: Launch on Clone of e sends one create to host9 in e's folder titled "fix login" with
/// e's command, agent type and resume command; once it succeeds (and opens the new session), New opens with none of
/// e's launch.
#[gpui::test]
fn a_clone_launch_sends_the_copied_launch(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(clone(cx, "e"));
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("the press sent one create, not {}", sent.len());
    };
    let body = create.body.clone().expect("a create carries a body");
    assert_eq!(body["host"], json!(9));
    assert_eq!(body["cwd"], json!("/home/user/src/e"));
    assert_eq!(body["title"], json!("fix login"));
    assert_eq!(
        body["command"]["command"],
        json!("make watch {farhelm_args}")
    );
    assert_eq!(
        body["command"]["resume"],
        json!("make watch --resume {conversation} {farhelm_args}")
    );
    assert_eq!(body["command"]["agent"], json!("codex"));
    created(&mut helm, cx, create.id, "copy");
    assert_eq!(
        selected(cx).as_deref(),
        Some("copy"),
        "the premise: the copy opened"
    );
    assert!(new(cx), "with the create answered, New opens");
    let fresh = draft(cx);
    assert_eq!((fresh.mode, fresh.tab), (Mode::New, Tab::Agent));
    assert!(fresh.command.command.is_empty() && fresh.name.value().is_empty());
}

/// Clone creates an independent session. If the user starts working in another terminal while its create waits,
/// the copied session can appear without taking that terminal's keyboard.
#[gpui::test]
fn a_clones_success_leaves_the_keyboard_in_the_session_switched_to(cx: &mut TestAppContext) {
    let (mut helm, cx) = window_with(cx, Some("a"));
    assert!(clone(cx, "e"));
    assert_eq!(draft(cx).mode, Mode::Clone);
    let create = super::launch_tests::press(&mut helm, cx, "/api/sessions");
    test_support::click(cx, "session-row-c");
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c opened");
    assert!(
        test_support::terminal_focused(cx),
        "c's terminal has the keyboard"
    );

    super::launch_tests::created_as(
        &mut helm,
        cx,
        create.id,
        &["a", "b", "c", "d", "copy"],
        session("copy"),
    );
    assert!(crate::session_list::menu_tests::drawn(
        cx,
        "session-row-copy"
    ));
    assert_eq!(
        name_of(cx, "launch-started-report").as_deref(),
        Some("Started “copy”")
    );
    assert_eq!(selected(cx).as_deref(), Some("c"));
    assert!(
        test_support::terminal_focused(cx),
        "the keyboard stayed in c's terminal"
    );
}

/// When the source's host is not carried over, the Clone takes New's host, which is the open session's when there is
/// one (the old launcher's "ordinary host default"), not always the helm's own. With a open on host9's install, that is
/// host9 even for a Clone of b, whose listing named another install on host9's row: the user is looking at that
/// install, and the note says the source's own host was not carried over. Specified: with a open, Clone of b opens on
/// host9 in b's folder with the note.
#[gpui::test]
fn clone_falls_back_to_the_open_sessions_host(cx: &mut TestAppContext) {
    let (_helm, cx) = window_with(cx, Some("a"));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    assert!(clone(cx, "b"));
    assert_eq!(destination(cx), (Some(9), "/home/user/src/b".to_owned()));
    assert_eq!(host_note(cx).as_deref(), Some(HOST_NOT_CARRIED));
}

// ---- Carrying an environment ----------------------------------------------------------------------------------------

/// A new sandbox of `recipe` on the Tensorlake account.
fn sandbox(recipe: &str) -> LaunchTarget {
    LaunchTarget::Environment {
        recipe: recipe.into(),
        place: Place::TensorlakeAccount,
    }
}

/// [`window_with`] with `selected` open, where host11's sandbox is described as the helm describes an environment's
/// host (`HostView::environment`), made from `recipe` on the Tensorlake account, and the pushed recipe catalog holds
/// `web` alone. host1's Podman is usable, so New container is offered too.
pub(super) fn environment_window<'a>(
    cx: &'a mut TestAppContext,
    selected: &str,
    recipe: &str,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let mut hosts = fixture_hosts(&[0, 4, 8, 10]);
    let environment = EnvironmentHostView {
        name: format!("{recipe} sandbox"),
        place_name: None,
        target: sandbox(recipe),
        tunnels: None,
        credential_warning: None,
    };
    let host11 = hosts
        .entries
        .iter_mut()
        .find(|entry| entry.key == "11")
        .expect("the fixture lists host11");
    host11.value["environment"] = serde_json::to_value(environment).unwrap();
    window_on(cx, Some(selected), hosts, vec![catalog()])
}

/// SPEC.md, "Sandbox sessions": "Clone targets the recipe for a fresh sandbox with the source's launch and repository
/// intent, never a copy of its disk"; "Container sessions" has Clone create "a new container from the recipe at its
/// selected place". The old launcher took the source host's environment target, by the host's id, with Folder `~`.
/// Specified: with c open on host11, a `web` sandbox, Clone of c opens in Clone mode on a new `web` sandbox in `~`,
/// with New's host (host1) held behind Existing host; choosing Existing host gives host1 back with no note under Host,
/// since nothing fell back.
#[gpui::test]
fn clone_of_an_environment_session_starts_a_fresh_one_from_its_recipe(cx: &mut TestAppContext) {
    let (_helm, cx) = environment_window(cx, "c", "web");
    assert!(clone(cx, "c"), "Clone opened");
    let opened = draft(cx);
    assert_eq!(opened.mode, Mode::Clone);
    assert_eq!(opened.target, Some(sandbox("web")));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
    test_support::click(cx, "launcher-runs-on-Existing host");
    assert_eq!(draft(cx).target, None, "the premise: Existing host chosen");
    assert_eq!(destination(cx).0, Some(1));
    assert_eq!(host_note(cx), None, "the environment was carried");
}

/// SPEC.md, "Creation": "Missing recipes or removed place hosts make that placement inapplicable", and "Sandbox
/// sessions": "A missing recipe stays unavailable rather than silently choosing another host". Quietly swapping in
/// another recipe would start something the user did not copy. Specified: with c's sandbox made from `gone`, a recipe
/// the catalog no longer lists, Clone of c keeps `gone` chosen, drawn as "Unavailable recipe (retained)", and a press
/// says why and sends nothing.
#[gpui::test]
fn a_clone_whose_recipe_is_gone_keeps_it_unavailable(cx: &mut TestAppContext) {
    let (mut helm, cx) = environment_window(cx, "c", "gone");
    assert!(clone(cx, "c"));
    assert_eq!(draft(cx).target, Some(sandbox("gone")));
    assert!(
        crate::session_list::tests::painted(&test_support::frame(cx))
            .iter()
            .any(|line| line.contains("Unavailable recipe (retained)")),
        "the Recipe select keeps it as unavailable"
    );
    let is_create =
        |command: &FakeCommand| command.path.ends_with("sessions") && command.body.is_some();
    answer_except(&mut helm, cx, is_create);
    test_support::click(cx, "launcher-launch");
    assert!(
        shown_refusal(cx).is_some_and(|refusal| refusal.contains("Recipe \"gone\" is unavailable")),
        "{:?}",
        shown_refusal(cx)
    );
    assert_eq!(
        answer_except(&mut helm, cx, is_create),
        Vec::new(),
        "nothing was sent"
    );
}

/// SPEC.md, "Sandbox sessions": "When a sandbox session is open, New and quick-switcher creation preselect its recipe
/// for a new sandbox and use the new sandbox's home Folder default, never the disposable host or its old folder". A
/// quick launch keeps its template's choices and New's defaults instead, or a template naming no placement would
/// launch into a fresh environment (decided by the launcher area on 2026-11-16). Specified: with c open on a `web`
/// sandbox, New and New named "fix-login" each open on a new `web` sandbox in `~` with New's host1 behind it, the
/// latter named; New's prefill, which quick launches start from, holds no target.
#[gpui::test]
fn new_beside_an_open_environment_session_preselects_its_recipe_and_place(cx: &mut TestAppContext) {
    let (_helm, cx) = environment_window(cx, "c", "web");
    assert_eq!(selected(cx).as_deref(), Some("c"), "the premise: c is open");
    assert!(new(cx), "New opened");
    assert_eq!(draft(cx).target, Some(sandbox("web")));
    assert_eq!(destination(cx), (Some(1), "~".to_owned()));
    test_support::keys(cx, "escape");
    assert!(cx.update(|window, cx| open_new_named("fix-login", window, cx)));
    settle(cx);
    let named = draft(cx);
    assert_eq!(
        (named.target, named.name.value().as_str()),
        (Some(sandbox("web")), "fix-login")
    );
    test_support::keys(cx, "escape");
    let prefill = cx.update(|_, cx| super::entry::new_prefill(cx));
    assert_eq!(
        prefill.map(|draft| draft.target),
        Some(None),
        "a quick launch's prefill keeps New's defaults"
    );
}

/// SPEC.md, "Sandbox sessions": "Restart, Restart with, Replace and Replace with stay in the existing sandbox". The
/// draft refuses a target in Replace with (`environment_tests`); this shows the launcher offers none either, where New
/// beside the same session does. Specified: with c open on a `web` sandbox and New container usable, New shows the Runs
/// on switch; Replace with of c holds host11 with no target and draws no switch.
#[gpui::test]
fn replace_with_of_an_environment_session_keeps_its_environment(cx: &mut TestAppContext) {
    let (_helm, cx) = environment_window(cx, "c", "web");
    assert!(new(cx));
    assert!(
        name_of(cx, "launcher-runs-on-Existing host").is_some(),
        "the premise: New beside c shows the switch"
    );
    test_support::keys(cx, "escape");
    assert!(cx.update(|window, cx| open_replace_with("c", window, cx)));
    settle(cx);
    let draft = draft(cx);
    assert!(matches!(draft.mode, Mode::ReplaceWith { .. }));
    assert_eq!((draft.target, draft.host), (None, Some(11)));
    assert_eq!(
        name_of(cx, "launcher-runs-on-Existing host"),
        None,
        "no switch"
    );
}
