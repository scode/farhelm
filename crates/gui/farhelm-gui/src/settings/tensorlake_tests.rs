//! Tests of Settings → Tensorlake over the helm client's fake: the EXPERIMENTAL warning and its place, the account read
//! and its failure, the section's copy of the Tensorlake API key form beside Credentials' copy, the SSH key group, and
//! the create-without-asking switch.
//!
//! The dialog is opened from the gear as in the dialog's own tests (`super::tests`), whose helpers these share. Every
//! step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::Preferences;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::{Value, json};

use super::Section;
use super::tensorlake::{ASK_ID, EXPERIMENTAL_WARNING, USAGE_CREDITS_NOTE};
use super::tests::{
    available, drawn, fill, label_of, open_rail, open_settings, painted, preference_work, reach,
    section_button, sent, switch_on,
};
use crate::layers;
use crate::operations::Work;
use crate::test_support::{advance, any_open, bounds, click, focused_id, frame, keys, settle};
use std::time::Duration;

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The helm's Tensorlake settings as `GET /api/tensorlake` and a done `PUT` answer them.
pub(super) fn account(key_set: bool, create_without_asking: bool) -> Value {
    json!({ "api_key_set": key_set, "create_without_asking": create_without_asking, "helm_tag": "fixture",
            "ssh_key": { "state": "not_created" } })
}

/// [`account`] with no key saved and the switch off, and `ssh_key` as the SSH key's state.
fn account_with_ssh_key(ssh_key: Value) -> Value {
    json!({ "api_key_set": false, "create_without_asking": false, "helm_tag": "fixture", "ssh_key": ssh_key })
}

/// A ready key's state, with its registration command as the helm words it.
fn ready_ssh_key() -> Value {
    json!({ "state": "ready", "public_key": "ssh-ed25519 AAAAFixture farhelm-fixture",
            "public_key_path": "/state/tensorlake/ssh-key/id_ed25519.pub", "name": "farhelm-fixture",
            "register_command": REGISTER_COMMAND })
}

/// The registration command [`ready_ssh_key`] carries.
const REGISTER_COMMAND: &str =
    "tl sbx ssh keys add --name farhelm-fixture 'ssh-ed25519 AAAAFixture farhelm-fixture'";

/// The full-size fixture keeps the navigation buttons reachable even when the current page is scrolled.
fn show_section(cx: &mut VisualTestContext, section: Section) {
    click(cx, section_button(section));
}

/// Whether the window paints [`REGISTER_COMMAND`]. The well wraps the command wherever its width ends, so it is compared
/// with the painted lines' spaces taken out.
fn shows_command(cx: &mut VisualTestContext) -> bool {
    painted(cx)
        .replace(' ', "")
        .contains(&REGISTER_COMMAND.replace(' ', ""))
}

/// Open Settings and show Tensorlake, checking that showing it sends exactly one read, of the account, which is
/// returned unanswered.
pub(super) fn show_tensorlake(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    open_settings(cx);
    assert_eq!(sent(helm), [], "the premise: Sessions reads nothing");
    click(cx, section_button(Section::Tensorlake));
    let reads = sent(helm);
    assert_eq!(reads.len(), 1, "one read: {reads:?}");
    let read = reads.into_iter().next().unwrap();
    assert_eq!(
        (read.method, read.path.as_str()),
        (Method::Get, "/api/tensorlake")
    );
    read
}

/// The one command sent since the last call, which must be a `PUT /api/tensorlake`: its id and body.
fn one_account_save(helm: &mut FakeHelm) -> (u64, Value) {
    let sent = sent(helm);
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (Method::Put, "/api/tensorlake")
    );
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// Press the control with element id `id` the way a keyboard user does: Tab to it, which scrolls it into the dialog's
/// view, and Space. The switch sits below the fold, so a click cannot reach it.
pub(super) fn press(cx: &mut VisualTestContext, id: &str) {
    reach(cx, id);
    keys(cx, "space");
}

/// The key the Tensorlake section's key form holds as typed. The field is masked, so what it paints cannot show it.
fn typed_here(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        let dialog = layers::last_opened::<super::SettingsDialog>(cx).expect("Settings is open");
        let section = dialog
            .read(cx)
            .tensorlake_section
            .clone()
            .expect("Tensorlake was shown");
        let key = section.read(cx).key();
        key.read(cx).typed()
    })
}

/// The key the Credentials section's key form holds as typed.
fn typed_in_credentials(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        let dialog = layers::last_opened::<super::SettingsDialog>(cx).expect("Settings is open");
        let credentials = dialog
            .read(cx)
            .credentials
            .clone()
            .expect("Credentials was shown");
        let key = credentials.read(cx).key();
        key.read(cx).typed()
    })
}

// ---- The warning and the account read ------------------------------------------------------------------------------

/// The maintainer wants every Tensorlake user to meet the EXPERIMENTAL warning before using it, and everyone has to
/// open this section to save an API key, so the warning heads it; a section that showed the warning only once its
/// settings loaded would hide it exactly when something is already wrong. A failed read must also lock every control,
/// or the switch would show off and the key field "No key saved" for an account that may have both (the old "a failed
/// load seeds nothing editable"). Specified: before the read answers, the maintainer's exact warning is painted, above
/// the key field; after a refused read, the warning is still painted, the helm's words show under the key form, the
/// field takes no typing and Test, Clear, Save and the switch are unavailable; Retry loading sends the read again, and
/// once it answers with no key saved and the switch off, the field says "No key saved", takes a key, and the switch is
/// available and off.
#[gpui::test]
fn the_warning_heads_the_section_and_a_failed_read_locks_it_until_a_retry(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    assert!(painted(cx).contains(EXPERIMENTAL_WARNING));
    assert!(
        bounds(cx, "tensorlake-experimental").bottom() <= bounds(cx, "tensorlake-key").top(),
        "the warning is drawn above the API key"
    );

    helm.reply(
        read.id,
        500,
        Some(json!("could not read Tensorlake settings")),
    );
    settle(cx);
    assert!(
        painted(cx).contains(EXPERIMENTAL_WARNING),
        "the warning stays"
    );
    assert_eq!(
        label_of(cx, "tensorlake-key-load-error").as_deref(),
        Some("could not read Tensorlake settings")
    );
    fill(cx, "tensorlake-key", "fictional-early-key");
    assert_eq!(typed_here(cx), "", "the field takes no typing");
    for control in ["tensorlake-key-test", "tensorlake-key-clear", ASK_ID] {
        assert_eq!(available(cx, control), Some(false), "{control} is locked");
    }
    assert!(!drawn(cx, super::tests::TENSORLAKE_SAVE));

    press(cx, "tensorlake-key-retry");
    let reads = sent(&mut helm);
    assert_eq!(reads.len(), 1, "Retry loading reads again: {reads:?}");
    assert_eq!(reads[0].path, "/api/tensorlake");
    helm.reply(reads[0].id, 200, Some(account(false, false)));
    settle(cx);
    assert!(!drawn(cx, "tensorlake-key-load-error"));
    assert!(painted(cx).contains("No key saved"));
    assert_eq!(available(cx, ASK_ID), Some(true));
    assert_eq!(switch_on(cx, ASK_ID), Some(false));
    fill(cx, "tensorlake-key", "fictional-tensorlake-key");
    assert_eq!(typed_here(cx), "fictional-tensorlake-key");
}

// ---- The two key forms ---------------------------------------------------------------------------------------------

/// SPEC.md: Credentials "includes the same Tensorlake API key form as Tensorlake ... labelled **Tensorlake API key**
/// there and **API key** in Tensorlake", "in Tensorlake the key form shows its own warning", and "A save or clear in
/// either updates both forms' saved-key metadata immediately; each keeps its own unsaved replacement draft." One shared
/// account per opening is what keeps the forms from disagreeing about whether a key is saved, and separate drafts keep
/// a key typed in one from being sent by the other. This is the Tensorlake half of the old "the two Tensorlake key
/// forms share acknowledged metadata and separate drafts". Specified: Tensorlake's form is labelled "API key", draws
/// one "Not encrypted on disk" warning and points to Credentials; showing Credentials afterwards reads only the
/// catalog, not the account again; a key saved in Credentials makes Tensorlake's form know a key is saved (its Test,
/// unavailable before, becomes available), while the key typed into Tensorlake's form beforehand is still there,
/// unsent. Discarding that draft must preserve the saved key and the other form's draft. A subsequent Save still
/// sends only the value typed on its own page.
#[gpui::test]
fn a_save_in_either_key_form_shows_in_both_and_each_keeps_its_own_typed_key(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(read.id, 200, Some(account(false, false)));
    settle(cx);
    let lines: Vec<String> = frame(cx)
        .into_iter()
        .flat_map(|node| node.texts.into_iter().map(|line| line.text.to_string()))
        .collect();
    assert!(
        lines.iter().any(|line| line == "API key")
            && !lines.iter().any(|line| line == "Tensorlake API key"),
        "the field's label: {lines:?}"
    );
    let text = painted(cx);
    assert_eq!(text.matches("Not encrypted on disk").count(), 1, "{text}");
    assert!(text.contains("You can also change it in Settings → Credentials."));
    fill(cx, "tensorlake-key", "fictional-key-typed-here");
    assert_eq!(
        available(cx, "tensorlake-key-test"),
        Some(false),
        "the premise: no key saved yet"
    );

    click(cx, section_button(Section::Credentials));
    let reads = sent(&mut helm);
    let paths: Vec<&str> = reads.iter().map(|read| read.path.as_str()).collect();
    assert_eq!(
        paths,
        ["/api/credentials"],
        "the account is read once per opening"
    );
    helm.reply(reads[0].id, 200, Some(json!([])));
    settle(cx);
    assert!(painted(cx).contains("No key saved"));
    fill(cx, "tensorlake-key", "fictional-key-saved-there");
    click(cx, super::tests::CREDENTIALS_SAVE);
    let (id, body) = one_account_save(&mut helm);
    assert_eq!(body, json!({ "api_key": "fictional-key-saved-there" }));
    helm.reply(id, 200, Some(account(true, false)));
    settle(cx);
    assert_eq!(typed_in_credentials(cx), "", "the saving form was emptied");
    // The saved key makes Tensorlake's list of Sandboxes not on this helm read, though the section is not shown.
    let reads = sent(&mut helm);
    assert_eq!(
        reads
            .iter()
            .map(|read| read.path.as_str())
            .collect::<Vec<_>>(),
        ["/api/tensorlake/orphans"]
    );
    helm.reply(
        reads[0].id,
        200,
        Some(super::tensorlake_sandbox_tests::leftover_list(&[], &[])),
    );
    settle(cx);
    fill(cx, "tensorlake-key", "fictional-credentials-draft");

    click(cx, section_button(Section::Tensorlake));
    assert_eq!(
        sent(&mut helm),
        [],
        "showing Tensorlake again reads nothing"
    );
    // The field holds the typed key, so its placeholder is not drawn; Test, available only with a key saved, says it.
    assert_eq!(
        available(cx, "tensorlake-key-test"),
        Some(true),
        "the save in Credentials shows here"
    );
    assert_eq!(
        typed_here(cx),
        "fictional-key-typed-here",
        "the other form's draft is its own"
    );
    click(cx, super::tests::TENSORLAKE_DISCARD);
    assert!(
        sent(&mut helm).is_empty(),
        "Discard does not clear the saved key"
    );
    assert!(typed_here(cx).is_empty());
    assert_eq!(available(cx, "tensorlake-key-test"), Some(true));
    assert_eq!(typed_in_credentials(cx), "fictional-credentials-draft");

    fill(cx, "tensorlake-key", "fictional-key-typed-here");
    click(cx, super::tests::TENSORLAKE_SAVE);
    let (_, body) = one_account_save(&mut helm);
    assert_eq!(body, json!({ "api_key": "fictional-key-typed-here" }));
}

/// The two key fields write one account. A close Save cannot choose between different replacement drafts.
#[gpui::test]
fn closing_with_two_different_key_drafts_never_chooses_one(cx: &mut TestAppContext) {
    assert_closing_with_two_different_key_drafts(cx, false);
}

/// A close asked while the first key's page Save is out waits for it (SPEC.md, "Settings") instead of approving
/// drafts over it. Its success empties the first field, so the question then names only the other draft, which the
/// person sees and answers afresh; nothing replaces the saved key without that answer.
#[gpui::test]
fn closing_while_a_key_save_is_out_waits_and_asks_about_the_other_draft(cx: &mut TestAppContext) {
    assert_closing_with_two_different_key_drafts(cx, true);
}

/// Exercise the same two independent drafts while either neither key or just the first key is already saving.
fn assert_closing_with_two_different_key_drafts(cx: &mut TestAppContext, saving_first: bool) {
    let (mut helm, cx, _) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(read.id, 200, Some(account(false, false)));
    settle(cx);
    fill(cx, "tensorlake-key", "fictional-first-key");
    show_section(cx, Section::Credentials);
    let reads = sent(&mut helm);
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].path, "/api/credentials");
    helm.reply(reads[0].id, 200, Some(json!([])));
    settle(cx);
    fill(cx, "tensorlake-key", "fictional-second-key");
    assert_eq!(typed_in_credentials(cx), "fictional-second-key");
    show_section(cx, Section::Tensorlake);
    assert_eq!(typed_here(cx), "fictional-first-key");
    let pending = saving_first.then(|| {
        click(cx, super::tests::TENSORLAKE_SAVE);
        let (id, body) = one_account_save(&mut helm);
        assert_eq!(body, json!({ "api_key": "fictional-first-key" }));
        assert_eq!(preference_work(cx), Some(Work::Save));
        id
    });
    click(cx, "settings-close");
    if let Some(id) = pending {
        assert!(!drawn(cx, "prompt"), "Close waits for the first key's save");
        helm.reply(id, 200, Some(account(true, false)));
        settle(cx);
        assert_eq!(preference_work(cx), None, "the first save settled");
        assert!(
            painted(cx).contains("Unsaved: Tensorlake API key in Credentials"),
            "the question names only the draft still unsaved"
        );
        assert!(!painted(cx).contains("You entered two different Tensorlake keys."));
        assert_eq!(typed_in_credentials(cx), "fictional-second-key");
        assert!(
            sent(&mut helm)
                .iter()
                .all(|request| request.method == Method::Get),
            "the old save's success did not authorize a replacement key"
        );
        return;
    }
    click(cx, "prompt-confirm");
    assert!(painted(cx).contains("You entered two different Tensorlake keys."));
    assert_eq!(typed_in_credentials(cx), "fictional-second-key");
    assert!(
        sent(&mut helm)
            .iter()
            .all(|request| request.method == Method::Get)
    );
}

// ---- Closing while work is out -------------------------------------------------------------------------------------

/// SPEC.md, "Settings": a close waits for the helm's answer. The create-without-asking switch saves at once from the
/// Tensorlake section's own view, and the key form's Test holds the form even with no draft, so neither gives the
/// close question anything to name; a close that went on would drop their outcome unseen. Specified: with no key saved,
/// turning the switch on and pressing Escape keeps Settings open with nothing asked, and the answer closes it; in a new
/// opening, Test on the clean key form and Escape keep Settings open, and the test's answer closes it.
#[gpui::test]
fn a_close_waits_for_the_switch_and_a_test_on_a_clean_key_form(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(read.id, 200, Some(account(false, false)));
    settle(cx);
    press(cx, ASK_ID);
    let (save, body) = one_account_save(&mut helm);
    assert_eq!(body, json!({ "create_without_asking": true }));
    keys(cx, "escape");
    assert!(any_open(cx), "Escape waits for the switch's save");
    assert!(!drawn(cx, "prompt"));
    helm.reply(save, 200, Some(account(false, true)));
    settle(cx);
    assert!(!any_open(cx), "the answered save let the close go on");

    let read = show_tensorlake(&mut helm, cx);
    helm.reply(read.id, 200, Some(account(true, true)));
    settle(cx);
    // A saved key also starts the leftover-sandbox read, answered first on the ordered lane.
    for read in sent(&mut helm) {
        assert_eq!(read.path, "/api/tensorlake/orphans");
        helm.reply(
            read.id,
            200,
            Some(super::tensorlake_sandbox_tests::leftover_list(&[], &[])),
        );
    }
    settle(cx);
    assert!(
        !drawn(cx, "settings-unsaved"),
        "the premise: the key form is clean"
    );
    reach(cx, "tensorlake-key-test");
    keys(cx, "space");
    let requests = sent(&mut helm);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/api/tensorlake/test");
    keys(cx, "escape");
    assert!(any_open(cx), "Escape waits for the test");
    assert!(!drawn(cx, "prompt"));
    helm.reply(requests[0].id, 200, Some(json!({ "sandboxes": 0 })));
    settle(cx);
    assert!(!any_open(cx), "the answered test let the close go on");
}

// ---- Creating sandboxes --------------------------------------------------------------------------------------------

/// The switch authorizes creating billed sandboxes without a question, so SPEC.md has it save "only when the helm
/// acknowledges it and show only the stored choice": a switch that showed the clicked value while its save was out, or
/// after a refusal, would claim an authorization the helm never stored. Its save must name only its own field, or it
/// could undo a key saved in another form. Specified: with the account answered off, the switch is available and off,
/// and the Usage Credits note is painted; turning it on sends `PUT /api/tensorlake` with only `create_without_asking:
/// true`, under the preference row's key, and while that is out the switch still shows off, and a second press sends
/// nothing and says why; the helm's answer turns it on; turning it off after that, refused, leaves it on with the
/// helm's words under it; and Settings opened again reads the account again and shows the stored choice.
#[gpui::test]
fn the_create_without_asking_switch_shows_only_the_stored_choice(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(read.id, 200, Some(account(false, false)));
    settle(cx);
    assert_eq!(available(cx, ASK_ID), Some(true));
    assert_eq!(switch_on(cx, ASK_ID), Some(false));
    assert!(painted(cx).contains(USAGE_CREDITS_NOTE));

    press(cx, ASK_ID);
    let (id, body) = one_account_save(&mut helm);
    assert_eq!(body, json!({ "create_without_asking": true }));
    assert_eq!(preference_work(cx), Some(Work::Save));
    assert_eq!(
        switch_on(cx, ASK_ID),
        Some(false),
        "unacknowledged, it shows the stored choice"
    );
    keys(cx, "space");
    assert_eq!(sent(&mut helm), [], "a press during the save sends nothing");
    assert!(drawn(cx, &format!("{ASK_ID}-busy")), "and says why");
    helm.reply(id, 200, Some(account(false, true)));
    settle(cx);
    assert_eq!(switch_on(cx, ASK_ID), Some(true));

    keys(cx, "space");
    let (id, body) = one_account_save(&mut helm);
    assert_eq!(body, json!({ "create_without_asking": false }));
    helm.reply(id, 500, Some(json!("could not save Tensorlake settings")));
    settle(cx);
    assert_eq!(
        switch_on(cx, ASK_ID),
        Some(true),
        "a refusal leaves the stored choice"
    );
    assert_eq!(
        label_of(cx, "tensorlake-create-without-asking-error").as_deref(),
        Some("could not save Tensorlake settings")
    );

    keys(cx, "escape");
    settle(cx);
    let read = show_tensorlake(&mut helm, cx);
    assert_eq!(
        available(cx, ASK_ID),
        Some(false),
        "a new opening waits for its own read"
    );
    helm.reply(read.id, 200, Some(account(false, true)));
    settle(cx);
    assert_eq!(switch_on(cx, ASK_ID), Some(true));
    assert!(!drawn(cx, "tensorlake-create-without-asking-error"));
}

/// The account is read once per opening, so a done save whose answer the GUI cannot decode would leave the switch on
/// the choice from before the save, with no failure line, until Settings is opened again. The helm always answers with
/// its settings, so this guards a helm the GUI does not understand. Specified: a done save of the switch answered with
/// something that is not the settings sends one read of the account, and the switch then shows what that read says.
#[gpui::test]
fn a_done_save_whose_answer_does_not_decode_reads_the_account_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(read.id, 200, Some(account(false, false)));
    settle(cx);
    press(cx, ASK_ID);
    let (id, _) = one_account_save(&mut helm);
    helm.reply(id, 200, Some(json!("saved")));
    settle(cx);
    let reads = sent(&mut helm);
    assert_eq!(reads.len(), 1, "one read: {reads:?}");
    assert_eq!(
        (reads[0].method, reads[0].path.as_str()),
        (Method::Get, "/api/tensorlake")
    );
    helm.reply(reads[0].id, 200, Some(account(false, true)));
    settle(cx);
    assert_eq!(switch_on(cx, ASK_ID), Some(true));
}

// ---- The SSH key group ---------------------------------------------------------------------------------------------

/// SPEC.md: "Until Farhelm's own Tensorlake SSH key ... exists, the section offers **Create SSH key**; opening Settings
/// never creates it. Once it exists, the section shows the one-time command ... with a **Copy** button." The key is a
/// credential the helm keeps unencrypted, so only the user's press may make it, and the command is the user's one way
/// to register it, so what Copy puts on the clipboard must be exactly the helm's command. Specified: with no key,
/// showing the section sends only the account read and the group says the helm has no key yet, with Create SSH key;
/// its press sends `POST /api/tensorlake/ssh-key` with no body; the done answer, the settings with a ready key, takes
/// Create SSH key away, shows the registration command and moves focus to Copy, so a second Space copies and leaves
/// the create-without-asking switch off (the control the focus coordinator's recovery would otherwise pick); Copy puts
/// the command on the clipboard byte for byte and shows "Command copied" for exactly 1.5 seconds; and a copy while
/// it shows restarts the 1.5 seconds.
#[gpui::test]
fn create_ssh_key_shows_the_registration_command_and_copy_copies_it(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(
        read.id,
        200,
        Some(account_with_ssh_key(json!({ "state": "not_created" }))),
    );
    settle(cx);
    assert!(painted(cx).contains("This helm has no SSH key for Tensorlake yet."));

    press(cx, "tensorlake-ssh-key-create");
    let sent_now = sent(&mut helm);
    assert_eq!(sent_now.len(), 1, "one command: {sent_now:?}");
    assert_eq!(
        (
            sent_now[0].method,
            sent_now[0].path.as_str(),
            sent_now[0].body.clone()
        ),
        (Method::Post, "/api/tensorlake/ssh-key", None)
    );
    helm.reply(
        sent_now[0].id,
        200,
        Some(account_with_ssh_key(ready_ssh_key())),
    );
    settle(cx);
    assert!(!drawn(cx, "tensorlake-ssh-key-create"));
    assert!(shows_command(cx), "{}", painted(cx));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("tensorlake-ssh-key-command-copy"),
        "a done create moves focus to Copy"
    );

    assert_eq!(
        cx.read_from_clipboard(),
        None,
        "the premise: nothing copied yet"
    );
    assert_eq!(
        switch_on(cx, ASK_ID),
        Some(false),
        "the premise: the switch starts off"
    );
    keys(cx, "space");
    assert_eq!(
        switch_on(cx, ASK_ID),
        Some(false),
        "the second Space did not reach the switch"
    );
    assert_eq!(
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some(REGISTER_COMMAND)
    );
    assert_eq!(
        label_of(cx, "tensorlake-ssh-key-copied").as_deref(),
        Some("Command copied")
    );
    advance(cx, Duration::from_millis(1499));
    assert!(drawn(cx, "tensorlake-ssh-key-copied"), "still at 1499 ms");
    advance(cx, Duration::from_millis(1));
    assert!(!drawn(cx, "tensorlake-ssh-key-copied"), "gone at 1500 ms");

    keys(cx, "space");
    advance(cx, Duration::from_millis(1000));
    keys(cx, "space");
    advance(cx, Duration::from_millis(1000));
    assert!(
        drawn(cx, "tensorlake-ssh-key-copied"),
        "the second copy restarted the 1.5 seconds"
    );
    advance(cx, Duration::from_millis(500));
    assert!(!drawn(cx, "tensorlake-ssh-key-copied"));
}

/// The words a Create SSH key refused at the helm's bound carry, while the generation goes on: the helm's own, from
/// `create_ssh_key` in `crates/farhelm-helm/src/tensorlake/mod.rs`.
const CARRIES_ON: &str = "Farhelm's SSH key was not ready within 20 seconds; its creation carries on. Select Create \
                          SSH key again in a few seconds to show the key";

/// Show Tensorlake with no key yet and press Create SSH key: the one command it sends.
fn press_create(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
    let read = show_tensorlake(helm, cx);
    helm.reply(
        read.id,
        200,
        Some(account_with_ssh_key(json!({ "state": "not_created" }))),
    );
    settle(cx);
    press(cx, "tensorlake-ssh-key-create");
    let create = sent(helm);
    assert_eq!(create.len(), 1, "one command: {create:?}");
    create.into_iter().next().unwrap()
}

/// The one command sent since the last call, checked to be a read of the account.
fn one_account_read(helm: &mut FakeHelm) -> FakeCommand {
    let reads = sent(helm);
    assert_eq!(
        reads
            .iter()
            .map(|read| (read.method, read.path.as_str()))
            .collect::<Vec<_>>(),
        [(Method::Get, "/api/tensorlake")],
        "one read of the account"
    );
    reads.into_iter().next().unwrap()
}

/// SPEC.md, "Settings": the helm is a presumed low-latency remote, so a refused Create SSH key is not polled for; the
/// helm's own words tell the user to select Create SSH key again, which waits for the running generation and answers
/// its key. The account is still read once, in case the key is already there. Specified: a 504 shows the helm's words
/// verbatim and reads the account once, with no try-again line until that read answers; answered with no key yet, the
/// group still offers Create SSH key and says to try again; a minute later nothing more is read, nor when Tensorlake is
/// shown again; a second Create SSH key, done with the ready key, shows the command and takes both lines away.
#[gpui::test]
fn a_refused_create_reads_the_account_once_and_a_second_create_shows_the_key(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let create = press_create(&mut helm, cx);
    helm.reply(create.id, 504, Some(json!(CARRIES_ON)));
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-ssh-key-error").as_deref(),
        Some(CARRIES_ON)
    );
    let read = one_account_read(&mut helm);
    assert!(
        !drawn(cx, "tensorlake-ssh-key-try-again"),
        "nothing is known of the key before the read answers"
    );
    helm.reply(
        read.id,
        200,
        Some(account_with_ssh_key(json!({ "state": "not_created" }))),
    );
    settle(cx);
    assert!(drawn(cx, "tensorlake-ssh-key-create"));
    assert!(
        drawn(cx, "tensorlake-ssh-key-try-again"),
        "the read found no key"
    );

    advance(cx, Duration::from_secs(60));
    assert_eq!(sent(&mut helm), [], "no re-read later");
    show_section(cx, Section::Sessions);
    show_section(cx, Section::Tensorlake);
    assert_eq!(sent(&mut helm), [], "nor on the next showing");

    press(cx, "tensorlake-ssh-key-create");
    let again = sent(&mut helm);
    assert_eq!(
        again
            .iter()
            .map(|command| (command.method, command.path.as_str()))
            .collect::<Vec<_>>(),
        [(Method::Post, "/api/tensorlake/ssh-key")]
    );
    assert!(
        !drawn(cx, "tensorlake-ssh-key-try-again"),
        "a new press starts afresh"
    );
    helm.reply(
        again[0].id,
        200,
        Some(account_with_ssh_key(ready_ssh_key())),
    );
    settle(cx);
    assert!(shows_command(cx));
    assert!(!drawn(cx, "tensorlake-ssh-key-error"));
    assert!(!drawn(cx, "tensorlake-ssh-key-try-again"));
}

/// Each press starts afresh: the try-again line speaks for the read its own refusal sent, so a second refusal must not
/// show the first refusal's line before its read answers. Specified: a refusal whose read finds no key shows the line;
/// a second press takes it away, and its refusal shows no line until its own read answers with no key, which shows it
/// again.
#[gpui::test]
fn a_second_refused_create_waits_for_its_own_read_before_saying_to_try_again(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let not_created = || Some(account_with_ssh_key(json!({ "state": "not_created" })));
    let create = press_create(&mut helm, cx);
    helm.reply(create.id, 504, Some(json!(CARRIES_ON)));
    settle(cx);
    let read = one_account_read(&mut helm);
    helm.reply(read.id, 200, not_created());
    settle(cx);
    assert!(
        drawn(cx, "tensorlake-ssh-key-try-again"),
        "the premise: the first refusal's read found no key"
    );

    press(cx, "tensorlake-ssh-key-create");
    let again = sent(&mut helm);
    assert_eq!(again.len(), 1, "{again:?}");
    assert!(
        !drawn(cx, "tensorlake-ssh-key-try-again"),
        "the press took the line away"
    );
    helm.reply(again[0].id, 504, Some(json!(CARRIES_ON)));
    settle(cx);
    let read = one_account_read(&mut helm);
    assert!(
        !drawn(cx, "tensorlake-ssh-key-try-again"),
        "nothing is said before the second refusal's read answers"
    );
    helm.reply(read.id, 200, not_created());
    settle(cx);
    assert!(drawn(cx, "tensorlake-ssh-key-try-again"));
}

/// The try-again line claims the read found no key, which a failed read did not establish. Specified: a refused
/// Create SSH key whose read fails shows the refusal and no try-again line.
#[gpui::test]
fn a_failed_read_after_a_refused_create_says_nothing_about_the_key(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let create = press_create(&mut helm, cx);
    helm.reply(create.id, 504, Some(json!(CARRIES_ON)));
    settle(cx);
    let read = one_account_read(&mut helm);
    helm.reply(read.id, 500, Some(json!("fixture account read refused")));
    settle(cx);
    assert!(drawn(cx, "tensorlake-ssh-key-error"));
    assert!(!drawn(cx, "tensorlake-ssh-key-try-again"));
}

/// A key folder the helm cannot read is a state of the settings, not a failed read (SPEC_impl.md, "Tensorlake
/// account"), and Create SSH key must not be offered over it: the helm never replaces a key the user may have
/// registered. Specified: with the key unavailable, the group says Farhelm has no usable SSH key for Tensorlake with
/// the helm's reason, and draws neither Create SSH key nor a command.
#[gpui::test]
fn an_unavailable_key_shows_the_helms_reason_and_offers_nothing(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(
        read.id,
        200,
        Some(account_with_ssh_key(
            json!({ "state": "unavailable", "reason": "the key folder is unreadable" }),
        )),
    );
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-ssh-key-unavailable").as_deref(),
        Some("Farhelm has no usable SSH key for Tensorlake: the key folder is unreadable")
    );
    assert!(!drawn(cx, "tensorlake-ssh-key-create"));
    assert!(!drawn(cx, "tensorlake-ssh-key-command-copy"));
}

/// The key can appear by a read rather than by the done answer: a refused create whose generation finished anyway is
/// found by the read the refusal sends. Create then vanishes under focus just as after a done answer, and the focus
/// coordinator's recovery would put focus on the switch that creates sandboxes without asking. Specified: a Create SSH
/// key refused and its read answered with a ready key shows the command and moves focus to Copy.
#[gpui::test]
fn a_key_found_by_the_read_after_a_refusal_moves_focus_to_copy(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(
        read.id,
        200,
        Some(account_with_ssh_key(json!({ "state": "not_created" }))),
    );
    settle(cx);
    press(cx, "tensorlake-ssh-key-create");
    let create = sent(&mut helm);
    assert_eq!(create.len(), 1, "{create:?}");
    helm.reply(
        create[0].id,
        504,
        Some(json!("Farhelm's SSH key was not ready within 20 seconds")),
    );
    settle(cx);
    let reads = sent(&mut helm);
    assert_eq!(reads.len(), 1, "the refusal reads the account: {reads:?}");
    helm.reply(
        reads[0].id,
        200,
        Some(account_with_ssh_key(ready_ssh_key())),
    );
    settle(cx);
    assert!(shows_command(cx));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("tensorlake-ssh-key-command-copy")
    );
}

/// A result must not move focus once the user has moved on since the action (SPEC.md, "Who may move focus"): a user
/// who tabbed away while the key was being made is working elsewhere. Specified: Create SSH key pressed, then Tab
/// before the done answer, leaves focus where Tab put it when the answer shows the command.
#[gpui::test]
fn a_done_create_after_the_user_moved_on_moves_no_focus(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    helm.reply(
        read.id,
        200,
        Some(account_with_ssh_key(json!({ "state": "not_created" }))),
    );
    settle(cx);
    press(cx, "tensorlake-ssh-key-create");
    let create = sent(&mut helm);
    assert_eq!(create.len(), 1, "{create:?}");
    keys(cx, "tab");
    let moved_to = focused_id(cx);
    assert_ne!(
        moved_to.as_deref(),
        Some("tensorlake-ssh-key-create"),
        "the premise: Tab moved focus on"
    );
    helm.reply(
        create[0].id,
        200,
        Some(account_with_ssh_key(ready_ssh_key())),
    );
    settle(cx);
    assert!(shows_command(cx));
    assert_eq!(focused_id(cx), moved_to, "focus stays where Tab put it");
}
