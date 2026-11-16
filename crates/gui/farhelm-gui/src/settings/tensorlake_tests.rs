//! Tests of Settings → Tensorlake over the helm client's fake: the EXPERIMENTAL warning and its place, the account read
//! and its failure, the section's copy of the Tensorlake API key form beside Credentials' copy, and the
//! create-without-asking switch.
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
    Opened, available, drawn, fill, label_of, open_rail, open_settings, painted, preference_work,
    reach, section_button, sent, switch_on,
};
use crate::operations::Work;
use crate::test_support::{bounds, click, frame, keys, settle};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The helm's Tensorlake settings as `GET /api/tensorlake` and a done `PUT` answer them.
fn account(key_set: bool, create_without_asking: bool) -> Value {
    json!({ "api_key_set": key_set, "create_without_asking": create_without_asking, "helm_tag": "fixture",
            "ssh_key": { "state": "not_created" } })
}

/// Open Settings and show Tensorlake, checking that showing it sends exactly one read, of the account, which is
/// returned unanswered.
fn show_tensorlake(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> FakeCommand {
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
fn press(cx: &mut VisualTestContext, id: &str) {
    reach(cx, id);
    keys(cx, "space");
}

/// The key the Tensorlake section's key form holds as typed. The field is masked, so what it paints cannot show it.
fn typed_here(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        let dialog = cx.global::<Opened>().0.upgrade().expect("Settings is open");
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
        let dialog = cx.global::<Opened>().0.upgrade().expect("Settings is open");
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
    for control in [
        "tensorlake-key-test",
        "tensorlake-key-clear",
        "tensorlake-key-save",
        ASK_ID,
    ] {
        assert_eq!(available(cx, control), Some(false), "{control} is locked");
    }

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
    press(cx, "tensorlake-key-save");
    let (id, body) = one_account_save(&mut helm);
    assert_eq!(body, json!({ "api_key": "fictional-key-saved-there" }));
    helm.reply(id, 200, Some(account(true, false)));
    settle(cx);
    assert_eq!(typed_in_credentials(cx), "", "the saving form was emptied");

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
    press(cx, "tensorlake-key-save");
    let (_, body) = one_account_save(&mut helm);
    assert_eq!(body, json!({ "api_key": "fictional-key-typed-here" }));
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
