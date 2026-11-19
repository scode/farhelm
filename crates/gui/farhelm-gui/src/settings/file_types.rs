//! Settings → Terminal's **File types to open with Cmd-click** field (Ctrl-click off macOS): the helm's shared list of
//! final extensions that a modifier-click on a terminal file opens with the default app once the download is saved.
//!
//! SPEC.md, "Session list" (the Terminal paragraph) and "Local authority and trust between hosts" (the download
//! opening rule); SPEC_impl.md, "Session-host file reads". Ported from the old app's `OpenExtensionsSettings`
//! (`settings.rs` at reference commit `24bc0580`), with its words, its per-type warnings and its unit tests. Here's
//! the TLDR of its rules:
//!
//! - **The list is the helm's preference**, `open_extensions`, which the helm reads at hover and again at open time;
//!   this field only edits it. Unset reads as the shared default ([`DEFAULT`]); a saved empty list opens nothing while
//!   downloads still save. The field follows a pushed change while the user has not typed in it ([`DraftInput`]).
//! - **Save is explicit and acknowledged.** Nothing else shows the new list until the helm stores it: Save first parses
//!   the draft with the helm's own parser ([`OpenExtensions::parse`]) and shows its reason without sending anything if
//!   it does not parse, then sends the normalized list as a preference patch under the preference row's key, like the
//!   ntfy form. While it is out the field is read-only and Save says "Saving…"; a done save says "File types saved" and
//!   the field takes the helm's stored spelling from the store as the reply settles (the helm pushes the row before it
//!   replies, while the saving draft does not follow it); a refusal keeps the draft and shows the helm's words until
//!   the next Save. The parser's reason shows only while the draft is still the text it was found in. A press while
//!   another Settings save holds the key sends nothing and says so ([`Refused`]). Enter never saves, as in the old
//!   field.
//! - **Each risky type gets a standing warning**, beside the field and following the draft as typed, even when another
//!   entry in it is malformed ([`warnings`]): HTML-like types the browser warning, any other type outside the
//!   recognized set the default-app warning.

use farhelm_input_mapping::Platform;
use farhelm_proto::open_extensions::{DEFAULT, OpenExtensions, WarningKind};
use gpui::{
    Context, Div, ElementId, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, Stateful, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px,
};

use super::{Refused, choices};
use crate::components::{Button, HoverHelp, TextInput, TextInputState, Tier};
use crate::drafts::DraftInput;
use crate::operations::{OpKey, Outcome};
use crate::session;
use crate::store::HelmStore;
use crate::theme::{self, metrics};

/// The text field's element id; the field's column, its Save, lines and warnings are drawn under `-field`, `-save`,
/// `-error`, `-saved`, `-invalid`, `-busy` and `-warning-<type>` after it.
pub(super) const ID: &str = "settings-file-types";

/// The preference row's field, as the wire names it.
const FIELD: &str = "open_extensions";

/// The label, by the modifier that opens a download on each platform (SPEC.md: "Ctrl-click outside macOS").
const LABEL_MAC: &str = "File types to open with Cmd-click";
const LABEL_OTHER: &str = "File types to open with Ctrl-click";

/// Search uses the same modifier name as the field it opens.
pub(super) fn label(platform: Platform) -> &'static str {
    match platform {
        Platform::Mac => LABEL_MAC,
        Platform::Linux => LABEL_OTHER,
    }
}

/// The old words.
pub(super) const HELP: &str = "Comma-separated final extensions. A leading dot is optional. Save an empty field to stop opening \
                    downloaded files; downloads still save. This choice applies to this helm.";
const SAVE_HELP: &str = "Save the file types this helm may open";
const SAVED: &str = "File types saved";
pub(super) const HTML_HEADING: &str = "An agent-written page may run in your browser";
pub(super) const HTML_TEXT: &str = "An agent-written page can run scripts in your browser and open web addresses outside \
                         Farhelm's link permissions. It can send cross-site requests to services on your computer, \
                         including localhost on other ports, and those requests may carry the service's cookies even \
                         though the page cannot read them directly.";
pub(super) const OTHER_HEADING: &str = "No protection from the default app";
pub(super) const OTHER_TEXT: &str = "Farhelm does not restrict what your default app does with this file. An agent can write this \
                          type of file, and opening it may run code on your computer.";

/// The most warnings drawn: a valid list has at most 32 types, and a draft that is not valid is bounded the same way.
const MAX_WARNINGS: usize = 32;

/// `.open-extensions-field`'s gap between its parts, and a warning's padding and the gap between its two lines.
const FIELD_GAP: f32 = 8.;
const WARNING_PAD: (f32, f32) = (10., 12.);
const WARNING_GAP: f32 = 6.;
/// A warning's line height: `.open-extensions-warning p`'s 1.5 at the Settings rows' text size.
const WARNING_LINE_HEIGHT: f32 = choices::LABEL_TEXT * 1.5;

/// The list a field shows for the stored choice: the stored text, or the default when nothing was ever saved.
fn shown(stored: Option<&str>) -> &str {
    stored.unwrap_or(DEFAULT)
}

/// The risky types in `draft`, each once, in the order typed, with the warning each gets: every comma-separated entry
/// that parses on its own is judged, so a malformed neighbour never hides a warning while the user is correcting the
/// list (the old `open_extension_warnings`). Bounded at [`MAX_WARNINGS`] for a draft that is not a valid list.
pub(super) fn warnings(draft: &str) -> Vec<(String, WarningKind)> {
    let mut warnings: Vec<(String, WarningKind)> = Vec::new();
    for token in draft.split(',') {
        let Ok(list) = OpenExtensions::parse(token) else {
            continue;
        };
        for (extension, kind) in list.entries() {
            if kind != WarningKind::Safe && !warnings.iter().any(|(seen, _)| seen == extension) {
                warnings.push((extension.to_owned(), kind));
                if warnings.len() == MAX_WARNINGS {
                    return warnings;
                }
            }
        }
    }
    warnings
}

/// The field, a view of its own that Settings makes the first time Terminal is shown in an opening and keeps until
/// the dialog closes.
pub(super) struct FileTypes {
    input: DraftInput,
    /// Which modifier the label names ([`FileTypes::set_platform`]).
    platform: Platform,
    /// Why Save sent nothing, the parser's reason, and the draft it was found in: drawn only while the draft is still
    /// that text, as the old field cleared it on input.
    invalid: Option<(String, String)>,
    /// Whether the last save was done and nothing has been typed since, which the saved line says.
    saved: bool,
    /// The line saying a Save sent nothing because another Settings save held the preference row's key.
    refused: Refused,
    _pushes: Option<Subscription>,
}

impl FileTypes {
    /// The field over the stored list, labelled for `platform`, following the store while it lives.
    pub(super) fn new(platform: Platform, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = session::current_store(cx);
        let stored = store.as_ref().and_then(|store| Self::stored(store, cx));
        let input = DraftInput::new(
            shown(stored.as_deref()),
            TextInputState::new(window, cx),
            |field: &mut Self| &mut field.input,
            // Enter saves nothing, as in the old field (see the module docs).
            |_, _, _| {},
            window,
            cx,
        );
        let pushes = store.map(|store| {
            cx.observe_in(&store, window, |field, _, window, cx| {
                field.follow_store(window, cx)
            })
        });
        FileTypes {
            input,
            platform,
            invalid: None,
            saved: false,
            refused: Refused::default(),
            _pushes: pushes,
        }
    }

    /// Label the field for `platform`: the dialog's own choice, which tests and goldens set to draw another platform's
    /// section (`SettingsDialog::decide_tunnels_for`).
    #[cfg(test)]
    pub(super) fn set_platform(&mut self, platform: Platform, cx: &mut Context<Self>) {
        self.platform = platform;
        cx.notify();
    }

    /// What the field holds as typed, for tests: it is a text field, whose text the frame does not report.
    #[cfg(test)]
    pub(super) fn text(&self, cx: &gpui::App) -> String {
        self.input.input().text(cx).to_string()
    }

    /// Whether the text field holds focus in `window`, for tests: the frame's focus names the field's own element.
    #[cfg(test)]
    pub(super) fn focused(&self, window: &Window, cx: &gpui::App) -> bool {
        self.input.input().focus_handle(cx).is_focused(window)
    }

    /// The focus target for a result about file-opening types.
    pub(super) fn search_focus(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.input.input().focus_handle(cx)
    }

    /// The stored list, `None` while unset (and while no preference row has been pushed).
    fn stored(store: &Entity<HelmStore>, cx: &gpui::App) -> Option<String> {
        super::preferences(store.read(cx).collections())
            .and_then(|preferences| preferences.open_extensions.clone())
    }

    /// Save the draft (the module docs). Does nothing while a save is out, when Save is drawn busy.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input.draft().is_saving() {
            return;
        }
        self.saved = false;
        let list = match OpenExtensions::parse(self.input.draft().value()) {
            Ok(list) => list.normalized(),
            Err(reason) => {
                self.invalid = Some((reason, self.input.draft().value().clone()));
                cx.notify();
                return;
            }
        };
        self.invalid = None;
        let handle = window.window_handle();
        let started = DraftInput::save(
            self,
            |field: &mut Self| &mut field.input,
            &OpKey::preferences(),
            |_| choices::patch(FIELD, serde_json::Value::String(list)),
            window,
            cx,
            move |field, settled, cx| {
                if let Outcome::Done(_) = settled.outcome {
                    field.saved = true;
                    // The helm pushes the row before it replies, so the stored spelling came while the field was
                    // saving, when the draft does not follow; take it up now that it does.
                    let field = cx.weak_entity();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            let _ = field.update(cx, |field, cx| field.follow_store(window, cx));
                        });
                    });
                }
            },
        );
        // Another Settings save may hold the key, which the field then says.
        self.refused.record(ID, &started);
        if let Err(not_started) = started {
            tracing::debug!(?not_started, "file types not saved");
        }
        cx.notify();
    }

    /// Follow the stored list where the draft allows.
    fn follow_store(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let stored = session::current_store(cx).and_then(|store| Self::stored(&store, cx));
        self.input.follow(shown(stored.as_deref()), window, cx);
        cx.notify();
    }

    /// The field under its label (`.open-extensions-label`: semibold, over the field), as wide as the section.
    fn labelled(label: &'static str, input: TextInput) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(choices::LABEL_GAP))
            .w_full()
            .min_w_0()
            .child(
                div()
                    .text_size(px(choices::LABEL_TEXT))
                    .line_height(px(choices::LABEL_LINE_HEIGHT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label),
            )
            .child(div().w_full().debug_selector(|| ID.to_owned()).child(input))
    }

    /// One standing warning for `extension` (`.open-extensions-warning`): its heading line, with the warning sign, and
    /// what the risk is.
    fn warning(extension: &str, kind: WarningKind, cx: &Context<Self>) -> Stateful<Div> {
        let palette = theme::palette(cx);
        let (heading, text) = match kind {
            WarningKind::Html => (HTML_HEADING, HTML_TEXT),
            WarningKind::Safe | WarningKind::Other => (OTHER_HEADING, OTHER_TEXT),
        };
        let heading = format!("{extension}: {heading}");
        div()
            .id(ElementId::Name(format!("{ID}-warning-{extension}").into()))
            .debug_selector({
                let extension = extension.to_owned();
                move || format!("{ID}-warning-{extension}")
            })
            .role(Role::Alert)
            .aria_label(format!("{heading}. {text}"))
            .flex()
            .flex_col()
            .gap(px(WARNING_GAP))
            .w_full()
            .py(px(WARNING_PAD.0))
            .px(px(WARNING_PAD.1))
            .bg(theme::color(palette.danger_bg_dim))
            .border_2()
            .border_color(theme::color(palette.danger_strong))
            .rounded(metrics::RADIUS_MD)
            .text_color(theme::color(palette.fg_0))
            .text_size(px(choices::LABEL_TEXT))
            .line_height(px(WARNING_LINE_HEIGHT))
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .font_weight(FontWeight::BOLD)
                    .child(
                        div()
                            .text_color(theme::color(palette.danger_strong))
                            .child("⚠"),
                    )
                    .child(heading),
            )
            .child(div().child(text))
    }
}

impl Render for FileTypes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let draft = self.input.draft();
        let saving = draft.is_saving();
        let label = label(self.platform);
        let input = TextInput::new(ID, self.input.input(), label, HoverHelp::tooltip(HELP))
            .height(px(choices::FIELD_HEIGHT));
        let warnings: Vec<_> = warnings(draft.value())
            .into_iter()
            .map(|(extension, kind)| Self::warning(&extension, kind, cx))
            .collect();
        let weak = cx.weak_entity();
        let save = Button::new(
            format!("{ID}-save"),
            if saving {
                "Saving…"
            } else {
                "Save file types"
            },
            HoverHelp::tooltip(SAVE_HELP),
            Role::Button,
        )
        .tier(Tier::Neutral)
        // Busy, not disabled, so a pressed Save keeps focus through its save.
        .busy(saving)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |field, cx| field.save(window, cx));
        });
        let invalid = self
            .invalid
            .as_ref()
            .filter(|(_, found_in)| found_in == draft.value())
            .map(|(reason, _)| {
                choices::outcome_line(
                    format!("{ID}-invalid"),
                    Role::Alert,
                    theme::color(palette.danger),
                )
                .aria_label(reason.clone())
                .child(reason.clone())
            });
        let failure = draft
            .refusal()
            .map(|reason| choices::failure_outcome(format!("{ID}-error"), reason, cx));
        let refused = choices::busy_line(&self.refused, ID, cx);
        let saved = (self.saved && !saving && !draft.is_touched()).then(|| {
            choices::outcome_line(
                format!("{ID}-saved"),
                Role::Status,
                theme::color(palette.fg_1),
            )
            .aria_label(SAVED)
            .child(SAVED)
        });
        div()
            .id(ElementId::Name(format!("{ID}-field").into()))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(FIELD_GAP))
            .w_full()
            .min_w_0()
            .child(Self::labelled(label, input))
            .child(choices::help(HELP, cx))
            .children(warnings)
            .child(save)
            .children(invalid)
            .children(failure)
            .children(refused)
            .children(saved)
    }
}
