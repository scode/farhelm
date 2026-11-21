//! Tests of Settings → Terminal's file-types field ([`super::file_types`]): the warnings a draft gets, ported from the
//! old field's unit tests, and, over the helm client's fake, what the field shows, what its Save sends and what it says
//! after.
//!
//! The dialog is opened from the gear as in the dialog's own tests (`super::tests`), whose helpers these share. Every
//! step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_input_mapping::Platform;
use farhelm_proto::open_extensions::{DEFAULT, WarningKind};
use farhelm_proto::ui_channel::Preferences;
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::Section;
use super::file_types::{ID, warnings};
use super::tests::{
    drawn, fill, one_save, open_rail, open_settings, painted, push, section_button, sent,
};
use crate::layers;
use crate::test_support::{click, keys, settle};

// ---- The warnings, from a draft alone ------------------------------------------------------------------------------

/// Recognized document, log and image types need no risk message beside the field, and neither does the default the
/// field starts from (the old `safe_draft_has_no_warnings`). Specified: `md, txt, json, log` and the default list warn
/// about nothing.
#[test]
fn a_safe_draft_has_no_warnings() {
    assert!(warnings("md, txt, json, log").is_empty());
    assert!(warnings(DEFAULT).is_empty());
}

/// Each unrecognized type gets its own warning, even in a draft that does not parse as a whole, so a risky type does
/// not disappear while the user corrects its neighbour (the old `unsafe_draft_warns_per_type`). Specified: in
/// `command, tar.gz, shell, COMMAND`, `command` and `shell` warn as other types, once each, and the malformed `tar.gz`
/// warns about nothing.
#[test]
fn an_unsafe_draft_warns_once_per_type() {
    assert_eq!(
        warnings("command, tar.gz, shell, COMMAND"),
        vec![
            ("command".into(), WarningKind::Other),
            ("shell".into(), WarningKind::Other)
        ]
    );
}

/// HTML-like types carry a risk of their own, scripts in the browser, so they get the browser warning (the old
/// `html_family_gets_specific_warning`). Specified: `html`, `htm`, `xhtml` and `svg` warn as HTML.
#[test]
fn the_html_family_gets_the_browser_warning() {
    assert_eq!(
        warnings("html, htm, xhtml, svg"),
        vec![
            ("html".into(), WarningKind::Html),
            ("htm".into(), WarningKind::Html),
            ("xhtml".into(), WarningKind::Html),
            ("svg".into(), WarningKind::Html),
        ]
    );
}

/// The warnings are bounded for an invalid, arbitrarily long draft, but repeated safe types must not use up that bound
/// before a risky type is reached (the old `duplicates_before_unsafe_types_do_not_hide_warnings`). Specified: after
/// forty `md` entries, `html` and `command` still warn.
#[test]
fn repeated_safe_types_do_not_hide_a_later_warning() {
    let draft = format!("{}html, command", "md,".repeat(40));
    assert_eq!(
        warnings(&draft),
        vec![
            ("html".into(), WarningKind::Html),
            ("command".into(), WarningKind::Other),
        ]
    );
}

// ---- The field, over the fake helm ---------------------------------------------------------------------------------

/// The field's Save.
const SAVE: &str = "settings-footer-save";

/// The preference row storing `list` as its file types; `None` never saved.
fn row(list: Option<&str>) -> Preferences {
    Preferences {
        open_extensions: list.map(str::to_owned),
        ..Preferences::default()
    }
}

/// Settings at Terminal over a row storing `list`, decided for `platform`, with nothing sent.
fn show_file_types<'a>(
    cx: &'a mut TestAppContext,
    list: Option<&str>,
    platform: Platform,
) -> (FakeHelm, &'a mut VisualTestContext) {
    let (mut helm, cx, _gear) = open_rail(cx, &row(list));
    open_settings(cx);
    cx.update(|_, cx| super::decide_tunnels_for_opened(platform, cx));
    click(cx, section_button(Section::Terminal));
    assert_eq!(sent(&mut helm), [], "the premise: Terminal reads nothing");
    (helm, cx)
}

/// What the field holds as typed.
fn typed(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        let dialog = layers::last_opened::<super::SettingsDialog>(cx).expect("Settings is open");
        let field = dialog
            .read(cx)
            .file_types
            .clone()
            .expect("Terminal was shown");
        field.read(cx).text(cx)
    })
}

/// Whether the field holds focus.
fn field_focused(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        let dialog = layers::last_opened::<super::SettingsDialog>(cx).expect("Settings is open");
        let field = dialog
            .read(cx)
            .file_types
            .clone()
            .expect("Terminal was shown");
        field.read(cx).focused(window, cx)
    })
}

/// Press Save the way a user does, scrolled into view first.
fn press_save(cx: &mut VisualTestContext) {
    click(cx, SAVE);
}

/// SPEC.md: the field "defaults to `md, txt, json, png, jpg, jpeg, gif, webp, bmp, avif`", is labelled for the modifier
/// that opens a download, and its value lives in the shared preference row, so a list saved elsewhere must show while
/// the user has not typed. Specified: with nothing saved, on the Mac, the field holds the default under "File types to
/// open with Cmd-click" with no warning; a pushed `md, html` shows in the field with the HTML warning; on Linux the
/// label says Ctrl-click.
#[gpui::test]
fn the_field_shows_the_default_then_follows_a_pushed_list(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_file_types(cx, None, Platform::Mac);
    assert_eq!(typed(cx), DEFAULT);
    assert!(painted(cx).contains("File types to open with Cmd-click"));
    assert!(!drawn(cx, "settings-file-types-warning-html"));

    push(&mut helm, &row(Some("md, html")), cx);
    assert_eq!(typed(cx), "md, html");
    let shown = painted(cx);
    assert!(
        shown.contains("html: An agent-written page may run in your browser"),
        "{shown}"
    );

    cx.update(|_, cx| super::decide_tunnels_for_opened(Platform::Linux, cx));
    settle(cx);
    assert!(painted(cx).contains("File types to open with Ctrl-click"));
}

/// SPEC.md: "This field requires an explicit, acknowledged save, but its value lives in the shared preference row", and
/// the helm stores one normalized spelling. Specified: typing ` .JSON, md, HTML, ` warns about `html` at once and sends
/// nothing; Save sends one `PUT /api/preferences` with only `open_extensions: "json, md, html"`, with Save busy and
/// saying "Saving…" while it is out; the helm pushes the row with that list and then replies, as it does for every
/// save, and the field then holds the stored spelling and "File types saved" shows. The push comes while the field is
/// saving, when its draft does not follow, so this is the path the field takes up the stored list itself.
#[gpui::test]
fn save_sends_the_normalized_list_and_says_it_was_saved(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_file_types(cx, None, Platform::Mac);
    fill(cx, ID, " .JSON, md, HTML, ");
    assert!(drawn(cx, "settings-file-types-warning-html"));
    assert_eq!(sent(&mut helm), [], "typing sends nothing");

    press_save(cx);
    let (id, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "open_extensions": "json, md, html" }));
    assert!(painted(cx).contains("Saving…"));
    assert!(!drawn(cx, "settings-file-types-saved"));
    push(&mut helm, &row(Some("json, md, html")), cx);
    assert_eq!(
        typed(cx),
        " .JSON, md, HTML, ",
        "premise: the saving draft does not follow"
    );
    helm.reply(id, 200, None);
    settle(cx);
    assert_eq!(typed(cx), "json, md, html");
    assert!(drawn(cx, "settings-file-types-saved"));
    assert!(painted(cx).contains("File types saved"));
}

/// SPEC.md: "The warnings follow the draft as typed, including when another entry is malformed", and a list the helm
/// would refuse is not worth sending: the field checks it with the helm's own parser. Specified: typing `tar.gz,
/// command` warns about `command`; Save sends nothing and shows the parser's reason; the field keeps what was typed;
/// changing the list takes the reason away, as the old field cleared it on input.
#[gpui::test]
fn a_malformed_list_sends_nothing_and_says_why(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_file_types(cx, None, Platform::Mac);
    fill(cx, ID, "tar.gz, command");
    assert!(drawn(cx, "settings-file-types-warning-command"));
    press_save(cx);
    assert_eq!(sent(&mut helm), [], "nothing is sent");
    assert!(drawn(cx, "settings-file-types-invalid"));
    assert!(painted(cx).contains("no inner dot"));
    assert_eq!(typed(cx), "tar.gz, command");
    fill(cx, ID, "gz, command");
    assert!(
        !drawn(cx, "settings-file-types-invalid"),
        "the edit took the reason away"
    );
}

/// SPEC.md: "A refused save leaves the draft and current helm policy unchanged and shows the reason inline."
/// Specified: a Save of `md, sh` refused with 400 "fixture refusal" shows those words, no saved line, and the field
/// still holds `md, sh`.
#[gpui::test]
fn a_refused_save_keeps_the_draft_and_shows_the_helms_words(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_file_types(cx, None, Platform::Mac);
    fill(cx, ID, "md, sh");
    press_save(cx);
    let (id, _) = one_save(&mut helm);
    helm.reply(id, 400, Some(json!("fixture refusal")));
    settle(cx);
    assert!(drawn(cx, "settings-file-types-error"));
    assert!(painted(cx).contains("fixture refusal"));
    assert!(!drawn(cx, "settings-file-types-saved"));
    assert_eq!(typed(cx), "md, sh");
}

/// SPEC.md: "saving an empty list disables opening while downloads still save", so an empty field is a list to save,
/// not a missing one. Specified: Save of an empty field sends `open_extensions: ""`.
#[gpui::test]
fn an_empty_field_saves_an_empty_list(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_file_types(cx, None, Platform::Mac);
    fill(cx, ID, "");
    press_save(cx);
    let (_, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "open_extensions": "" }));
}

/// The old field swallowed Enter, so a list is saved only by its Save, where the warnings above it have been read.
/// Specified: Enter in the field after typing sends nothing.
#[gpui::test]
fn enter_never_saves(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_file_types(cx, None, Platform::Mac);
    fill(cx, ID, "md, txt");
    assert!(field_focused(cx), "premise: the field holds focus");
    keys(cx, "enter");
    assert_eq!(sent(&mut helm), [], "Enter saves nothing");
    assert_eq!(typed(cx), "md, txt");
}

/// The list is saved under the preference row's key, like every Settings preference, so a Save while another of those
/// saves is out cannot start, and must say so rather than look lost. Specified: with a link choice's save out, Save
/// sends nothing and draws the busy line under the field.
#[gpui::test]
fn a_save_while_another_preference_save_is_out_says_so(cx: &mut TestAppContext) {
    let (mut helm, cx) = show_file_types(cx, None, Platform::Mac);
    click(
        cx,
        "settings-terminal-links-Open in Farhelm and also pass the click on",
    );
    let (_, body) = one_save(&mut helm);
    assert_eq!(body, json!({ "terminal_link_click": "both" }), "premise");
    fill(cx, ID, "md");
    press_save(cx);
    assert_eq!(sent(&mut helm), [], "nothing is sent");
    assert!(drawn(cx, "settings-file-types-busy"));
}
