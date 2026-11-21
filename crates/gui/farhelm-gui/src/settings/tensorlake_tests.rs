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
    assert!(!drawn(cx, "settings-footer-save"));

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
/// unsent; and that form's Save sends its own key.
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
    click(cx, "settings-footer-save");
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
    helm.reply(reads[0].id, 200, Some(json!([])));
    settle(cx);

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
    click(cx, "settings-footer-save");
    let (_, body) = one_account_save(&mut helm);
    assert_eq!(body, json!({ "api_key": "fictional-key-typed-here" }));
}

/// The two key fields write one account. A close Save cannot choose between different replacement drafts.
#[gpui::test]
fn closing_with_two_different_key_drafts_never_chooses_one(cx: &mut TestAppContext) {
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
    click(cx, "settings-close");
    click(cx, "prompt-confirm");
    assert!(
        painted(cx)
            .contains("Two different Tensorlake keys are typed. Save the one you want first.")
    );
    assert_eq!(typed_in_credentials(cx), "fictional-second-key");
    assert!(
        sent(&mut helm)
            .iter()
            .all(|request| request.method == Method::Get)
    );
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

/// The helm answers Create SSH key within its host reply bound and lets a generation still running then carry on,
/// saying the key shows once it exists; the account is read once per opening and not pushed, so without reading it
/// again the key would never show in this opening. Specified: a Create SSH key refused with a 504 shows the helm's
/// words and reads the account once; answered with no key yet, the group still offers Create SSH key with the words
/// under it; showing another section and Tensorlake again reads the account once more, and its ready key shows the
/// command and takes the refusal's words away; showing Tensorlake a third time reads nothing. The re-reads while the
/// section stays shown are `a_carried_on_create_shows_the_key_where_it_was_pressed`'s.
#[gpui::test]
fn a_refused_create_reads_the_account_now_and_on_the_next_showing(cx: &mut TestAppContext) {
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
    assert_eq!(create.len(), 1, "one command: {create:?}");
    let words = "Farhelm's SSH key was not ready within 20 seconds; its creation carries on, and Settings shows the \
                 key once it exists";
    helm.reply(create[0].id, 504, Some(json!(words)));
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-ssh-key-error").as_deref(),
        Some(words)
    );
    let reads = sent(&mut helm);
    assert_eq!(
        reads
            .iter()
            .map(|read| read.path.as_str())
            .collect::<Vec<_>>(),
        ["/api/tensorlake"],
        "the refusal reads the account once"
    );
    helm.reply(
        reads[0].id,
        200,
        Some(account_with_ssh_key(json!({ "state": "not_created" }))),
    );
    settle(cx);
    assert!(drawn(cx, "tensorlake-ssh-key-create"));

    show_section(cx, Section::Sessions);
    assert_eq!(sent(&mut helm), [], "the premise: Sessions reads nothing");
    show_section(cx, Section::Tensorlake);
    let reads = sent(&mut helm);
    assert_eq!(
        reads
            .iter()
            .map(|read| read.path.as_str())
            .collect::<Vec<_>>(),
        ["/api/tensorlake"],
        "showing Tensorlake again reads the account once more"
    );
    helm.reply(
        reads[0].id,
        200,
        Some(account_with_ssh_key(ready_ssh_key())),
    );
    settle(cx);
    assert!(shows_command(cx));
    assert!(
        !drawn(cx, "tensorlake-ssh-key-error"),
        "the key took the refusal's words away"
    );

    show_section(cx, Section::Sessions);
    show_section(cx, Section::Tensorlake);
    assert_eq!(sent(&mut helm), [], "a third showing reads nothing");
}

/// The words a Create SSH key refused at the helm's bound carry, while the generation goes on.
const CARRIES_ON: &str = "Farhelm's SSH key was not ready within 20 seconds; its creation carries on, and Settings \
                          shows the key once it exists";

/// Show Tensorlake with no key yet, press Create SSH key, refuse it with [`CARRIES_ON`], and answer the account read
/// the refusal sends with no key yet: the state a carried-on generation leaves the group in.
fn refuse_create(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    let read = show_tensorlake(helm, cx);
    let not_created = || Some(account_with_ssh_key(json!({ "state": "not_created" })));
    helm.reply(read.id, 200, not_created());
    settle(cx);
    press(cx, "tensorlake-ssh-key-create");
    let create = sent(helm);
    assert_eq!(create.len(), 1, "one command: {create:?}");
    helm.reply(create[0].id, 504, Some(json!(CARRIES_ON)));
    settle(cx);
    let read = one_account_read(helm);
    helm.reply(read.id, 200, not_created());
    settle(cx);
    assert!(
        drawn(cx, "tensorlake-ssh-key-create"),
        "premise: no key yet"
    );
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

/// The old app's Create SSH key waited for `ssh-keygen`, so the key showed where the user pressed. The helm now answers
/// at its bound while the generation carries on, and nothing pushed says when the key is written, so a user who stays
/// on Tensorlake must see it appear without leaving the section. Specified: after a carried-on create and its read
/// finding no key, nothing is read before five seconds; at five seconds one read goes out; answered with the ready key,
/// the command shows, the refusal's words go, and focus moves to Copy; a minute more reads nothing.
#[gpui::test]
fn a_carried_on_create_shows_the_key_where_it_was_pressed(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    refuse_create(&mut helm, cx);
    advance(cx, Duration::from_millis(4_900));
    assert_eq!(sent(&mut helm), [], "not before five seconds");
    advance(cx, Duration::from_millis(100));
    let read = one_account_read(&mut helm);
    helm.reply(read.id, 200, Some(account_with_ssh_key(ready_ssh_key())));
    settle(cx);
    assert!(shows_command(cx));
    assert!(!drawn(cx, "tensorlake-ssh-key-error"));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("tensorlake-ssh-key-command-copy")
    );
    advance(cx, Duration::from_secs(60));
    assert_eq!(sent(&mut helm), [], "the key showed, so the re-reads stop");
}

/// The re-reads are bounded: a generation that never writes a key must not leave Settings reading the account for as
/// long as it stays open, and the helm's own bounds end any generation about 10 seconds after the refusal. Specified:
/// with every read answered with no key yet, re-reads go out at 5, 15 and 30 seconds after the refusal, one each, and
/// nothing more in the next two minutes.
#[gpui::test]
fn the_re_reads_after_a_carried_on_create_end_after_half_a_minute(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    refuse_create(&mut helm, cx);
    for wait in [5, 10, 15] {
        advance(cx, Duration::from_secs(wait));
        let read = one_account_read(&mut helm);
        helm.reply(
            read.id,
            200,
            Some(account_with_ssh_key(json!({ "state": "not_created" }))),
        );
        settle(cx);
    }
    advance(cx, Duration::from_secs(120));
    assert_eq!(sent(&mut helm), [], "the schedule has ended");
}

/// The refusal's words promise that Settings shows the key once it exists, which is false when the generation failed
/// after the helm's reply (the helm only logs that), and the re-reads are the only way the group would learn of a key.
/// So once they end with no key, the group must say to press Create SSH key again rather than leave the promise
/// standing. Specified: after a carried-on create, no try-again line is drawn while re-reads remain; once the last one
/// (at 30 seconds) is sent, "No key has appeared since. Press Create SSH key to try again." shows, and still does when
/// it answers with no key; pressing Create SSH key again takes it away.
#[gpui::test]
fn re_reads_that_find_no_key_say_to_try_again(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    refuse_create(&mut helm, cx);
    let not_created = || Some(account_with_ssh_key(json!({ "state": "not_created" })));
    for wait in [5, 10] {
        advance(cx, Duration::from_secs(wait));
        let read = one_account_read(&mut helm);
        helm.reply(read.id, 200, not_created());
        settle(cx);
        assert!(
            !drawn(cx, "tensorlake-ssh-key-try-again"),
            "a re-read remains"
        );
    }
    advance(cx, Duration::from_secs(15));
    let last = one_account_read(&mut helm);
    assert!(
        drawn(cx, "tensorlake-ssh-key-try-again"),
        "the re-reads are over once the last is sent"
    );
    helm.reply(last.id, 200, not_created());
    settle(cx);
    assert_eq!(
        label_of(cx, "tensorlake-ssh-key-try-again").as_deref(),
        Some("No key has appeared since. Press Create SSH key to try again.")
    );
    press(cx, "tensorlake-ssh-key-create");
    assert!(
        !drawn(cx, "tensorlake-ssh-key-try-again"),
        "a new press takes it away"
    );
}

/// A second press starts a new attempt, so the first refusal's re-reads must not decide the try-again line for it:
/// left running, their last read would put "No key has appeared since" under a busy button, or straight under the
/// second refusal's promise that the key shows once it exists. Specified: after a carried-on create and its re-reads at
/// 5 and 15 seconds finding no key, Create SSH key pressed again at 25 seconds and left unanswered sends nothing more
/// by 35 seconds (the first schedule's last read, due at 30, is cancelled) and draws no try-again line; the second
/// refusal and the read it sends, answered with no key, draw none either.
#[gpui::test]
fn a_second_press_cancels_the_first_refusals_re_reads(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    refuse_create(&mut helm, cx);
    let not_created = || Some(account_with_ssh_key(json!({ "state": "not_created" })));
    for wait in [5, 10] {
        advance(cx, Duration::from_secs(wait));
        let read = one_account_read(&mut helm);
        helm.reply(read.id, 200, not_created());
        settle(cx);
    }
    advance(cx, Duration::from_secs(10));
    press(cx, "tensorlake-ssh-key-create");
    let create = sent(&mut helm);
    assert_eq!(create.len(), 1, "the second create: {create:?}");
    advance(cx, Duration::from_secs(10));
    assert_eq!(
        sent(&mut helm),
        [],
        "the first schedule's last read is cancelled"
    );
    assert!(
        !drawn(cx, "tensorlake-ssh-key-try-again"),
        "not under the busy button"
    );
    helm.reply(create[0].id, 504, Some(json!(CARRIES_ON)));
    settle(cx);
    let read = one_account_read(&mut helm);
    helm.reply(read.id, 200, not_created());
    settle(cx);
    assert!(
        !drawn(cx, "tensorlake-ssh-key-try-again"),
        "not under the second refusal's promise"
    );
}

/// A last re-read that fails learned nothing about the key, so it must not claim that none has appeared; the API key
/// forms already show its failure with Retry loading. Specified: after a carried-on create and its re-reads at 5 and 15
/// seconds finding no key, the last one at 30 seconds answered with a refusal shows the key form's Retry loading and
/// draws no try-again line.
#[gpui::test]
fn a_failed_last_re_read_says_nothing_about_the_key(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    refuse_create(&mut helm, cx);
    for wait in [5, 10] {
        advance(cx, Duration::from_secs(wait));
        let read = one_account_read(&mut helm);
        helm.reply(
            read.id,
            200,
            Some(account_with_ssh_key(json!({ "state": "not_created" }))),
        );
        settle(cx);
    }
    advance(cx, Duration::from_secs(15));
    let last = one_account_read(&mut helm);
    helm.reply(
        last.id,
        500,
        Some(json!("the helm could not read its settings")),
    );
    settle(cx);
    assert!(
        drawn(cx, "tensorlake-key-retry"),
        "premise: the failed read reached the account"
    );
    assert!(!drawn(cx, "tensorlake-ssh-key-try-again"));
}

/// The re-reads belong to the open Settings: once it closes, a read would reach nobody and only ask the helm again.
/// Specified: after a carried-on create, closing Settings with Escape and letting two minutes pass sends nothing. This
/// passes with no re-reads at all; it guards against re-reads that outlive Settings, and
/// `a_carried_on_create_shows_the_key_where_it_was_pressed` supplies its premise, that a read goes out at five seconds
/// while Settings stays open.
#[gpui::test]
fn closing_settings_stops_the_re_reads(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    refuse_create(&mut helm, cx);
    keys(cx, "escape");
    assert!(!any_open(cx), "premise: Settings closed");
    advance(cx, Duration::from_secs(120));
    assert_eq!(sent(&mut helm), [], "nothing is read after Settings closes");
}

/// A refusal asks for a read on the next showing, but once a later Create SSH key is done the key is known and that
/// read would only ask the helm again for nothing. Specified: after a refused create and its read finding no key, a
/// second Create SSH key answered done shows the command, takes the refusal's words away and moves focus to Copy, and
/// showing another section and Tensorlake again reads nothing.
#[gpui::test]
fn a_done_create_after_a_refusal_drops_the_read_on_the_next_showing(cx: &mut TestAppContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &Preferences::default());
    let read = show_tensorlake(&mut helm, cx);
    let not_created = account_with_ssh_key(json!({ "state": "not_created" }));
    helm.reply(read.id, 200, Some(not_created.clone()));
    settle(cx);
    press(cx, "tensorlake-ssh-key-create");
    let create = sent(&mut helm);
    helm.reply(create[0].id, 500, Some(json!("ssh-keygen failed: fixture")));
    settle(cx);
    let reads = sent(&mut helm);
    assert_eq!(reads.len(), 1, "the refusal reads the account: {reads:?}");
    helm.reply(reads[0].id, 200, Some(not_created));
    settle(cx);
    assert!(drawn(cx, "tensorlake-ssh-key-error"));

    press(cx, "tensorlake-ssh-key-create");
    let create = sent(&mut helm);
    assert_eq!(create.len(), 1, "{create:?}");
    helm.reply(
        create[0].id,
        200,
        Some(account_with_ssh_key(ready_ssh_key())),
    );
    settle(cx);
    assert!(shows_command(cx));
    assert!(!drawn(cx, "tensorlake-ssh-key-error"));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("tensorlake-ssh-key-command-copy")
    );
    show_section(cx, Section::Sessions);
    show_section(cx, Section::Tensorlake);
    assert_eq!(sent(&mut helm), [], "the key is known: nothing to read");
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
