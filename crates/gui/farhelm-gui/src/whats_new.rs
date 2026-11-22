//! What’s New follows the window's first-open dialogs and records a dismissal on the helm.
//!
//! The installed app owns the feed and its version selection. An asynchronous answer belongs to
//! the UI session and preferences that requested it. A reset or a failed lookup leaves the notes
//! eligible at the next launch, as does a user interaction that takes focus elsewhere before they open.

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    App, AppContext as _, Context, Div, FocusHandle, FontWeight, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledText, Task, WeakEntity, Window, div, px,
    relative,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::desktop::updates::{self, notes};
use crate::focus::{self, Mark, ReturnTo};
use crate::layers::{self, Dialog, Dismissal, LayerId, MainAction};
use crate::{assets, session, theme};

#[cfg(test)]
pub(crate) mod tests;

const TITLE: &str = "What's new in Farhelm";
const RELEASE_NOTES_URL: &str = crate::update_readout::view::RELEASE_NOTES_URL;

/// Start after Welcome and Announcements. The caller keeps the lookup for the lifetime of its UI session.
/// A response may only act on the connection and choices it read.
pub(crate) fn check(mark: Mark, window: &Window, cx: &App) -> Option<Task<()>> {
    let host = updates::upgrade_notes(cx)?;
    let store = session::current_store(cx)?;
    if focus::has_moved_on(&mark, cx) {
        return None;
    }
    let store_id = store.entity_id();
    let store = store.read(cx);
    let Some(preferences) = store.collections().get::<Preferences>(PREFERENCES_KEY) else {
        tracing::warn!("no shared preferences were read for upgrade highlights");
        return None;
    };
    let previous = preferences.last_whats_new_release.clone();
    let show = preferences.show_whats_new != Some(false);
    let running = host.running.clone();
    let answer = (host.check)(previous.clone(), show);
    Some(window.spawn(cx, async move |cx| {
        let outcome = match answer.await {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(%error, "could not read upgrade highlights");
                return;
            }
        };
        let _ = cx.update(|window, cx| {
            let Some(current) =
                session::current_store(cx).filter(|store| store.entity_id() == store_id)
            else {
                return;
            };
            let current = current.read(cx);
            if !current
                .collections()
                .get::<Preferences>(PREFERENCES_KEY)
                .is_some_and(|preferences| {
                    preferences.last_whats_new_release == previous
                        && (preferences.show_whats_new != Some(false)) == show
                })
            {
                return;
            }
            match outcome {
                notes::UpgradeNotes::Current => {}
                notes::UpgradeNotes::Skipped => record(&running, false, cx),
                // An automatic sign-in prompt can have taken focus while the website was answering,
                // without any user input to invalidate the mark.
                notes::UpgradeNotes::Show(releases) if !layers::focus_layer_open(window, cx) => {
                    open(running, releases, &mark, window, cx);
                }
                notes::UpgradeNotes::Show(_) => {}
            }
        });
    }))
}

/// Save the running release and, for Don't show again, the opt-out in the same sparse write.
/// A refused write keeps the old baseline so a later launch can retry.
fn record(running: &str, opt_out: bool, cx: &mut App) {
    let Ok(path) = ApiPath::api(&["preferences"]) else {
        return;
    };
    let patch = preferences_patch(running, opt_out.then_some(false));
    crate::operations::send(
        Method::Put,
        path,
        Some(patch),
        "PUT /api/preferences",
        cx,
        |answer, _| {
            if let Err(words) = answer {
                tracing::warn!(reason = %words.raw(), "could not record upgrade highlights");
            }
        },
    );
}

/// Dismissals can advance the baseline without overwriting the helm's current choice.
pub(crate) fn preferences_patch(running: &str, show: Option<bool>) -> serde_json::Value {
    let mut patch = serde_json::json!({ "last_whats_new_release": running });
    if let Some(show) = show {
        patch["show_whats_new"] = serde_json::Value::Bool(show);
    }
    patch
}

/// Open the selected releases. Only dismissal calls the recording hook, so a reset never counts as seeing the notes.
pub(crate) fn open(
    running: String,
    releases: Vec<notes::ReleaseHighlights>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let view = cx.new(|cx| HighlightsDialog {
        running: Some(running),
        releases,
        opt_out: false,
        got_it: cx.focus_handle().tab_stop(true),
        layer: None,
    });
    let weak = view.downgrade();
    let on_dismiss = Rc::new(move |_: &mut Window, cx: &mut App| {
        let record_for = weak.update(cx, |dialog, _| {
            dialog
                .running
                .take()
                .map(|running| (running, dialog.opt_out))
        });
        if let Ok(Some((running, opt_out))) = record_for {
            record(&running, opt_out, cx);
        }
        Dismissal::Close
    });
    let weak = view.downgrade();
    let main_action: MainAction = Rc::new(move |window, cx| dismiss(&weak, false, window, cx));
    let layer = layers::open_dialog(
        Dialog {
            view: view.clone().into(),
            container: cx.focus_handle(),
            start: view.read(cx).got_it.clone(),
            subject: None,
            main_action: Some(main_action),
            keeps_focus_on_empty_click: false,
            returns_to: ReturnTo::Home,
            on_dismiss: Some(on_dismiss),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    view.update(cx, |view, _| view.layer = Some(layer));
    Some(layer)
}

/// Hold the releases that were presented together. A later preference push does not rewrite the open dialog.
struct HighlightsDialog {
    running: Option<String>,
    releases: Vec<notes::ReleaseHighlights>,
    opt_out: bool,
    got_it: FocusHandle,
    layer: Option<LayerId>,
}

/// Header Close, Escape, and all confirming buttons go through the same layer dismissal.
fn dismiss(view: &WeakEntity<HighlightsDialog>, opt_out: bool, window: &mut Window, cx: &mut App) {
    let layer = view
        .update(cx, |dialog, _| {
            dialog.opt_out |= opt_out;
            dialog.layer
        })
        .ok()
        .flatten();
    if let Some(layer) = layer {
        layers::dismiss(layer, window, cx);
    }
}

/// Tell the user which releases actually arrived. The saved baseline can precede that range.
fn subtitle(releases: &[notes::ReleaseHighlights]) -> String {
    match releases {
        [] => String::new(),
        [release] => format!("Highlights from {}", release.version),
        [newest, older] => format!("Highlights from {} and {}", older.version, newest.version),
        [newest, .., oldest] => format!("Highlights from {} to {}", oldest.version, newest.version),
    }
}

/// A wrapping paragraph gets one accessible label for the entire entry. Code ranges split the drawing's
/// text runs so GPUI can apply the font override at their edges.
fn paragraph(id: SharedString, pieces: &[notes::HighlightPiece]) -> impl IntoElement {
    let mut text = String::new();
    let mut highlights = Vec::new();
    let mut code = Vec::new();
    for piece in pieces {
        let start = text.len();
        text.push_str(&piece.text);
        let range = start..text.len();
        match piece.style {
            notes::HighlightStyle::Plain => {}
            notes::HighlightStyle::Bold => highlights.push((
                range,
                HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..HighlightStyle::default()
                },
            )),
            notes::HighlightStyle::Code => {
                highlights.push((range.clone(), HighlightStyle::default()));
                code.push((range, SharedString::from(assets::TERMINAL_FONT)));
            }
        }
    }
    let text = SharedString::from(text);
    let styled = StyledText::new(text.clone())
        .with_highlights(highlights)
        .with_font_family_overrides(code);
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(text)
        .w_full()
        .child(styled)
}

/// The source already orders categories and entries. Keep screenshot links with the text they illustrate.
fn release_section(release: &notes::ReleaseHighlights, cx: &App) -> Div {
    let palette = theme::palette(cx);
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .id(SharedString::from(format!(
                    "whats-new-release-{}",
                    release.version
                )))
                .role(Role::Heading)
                .aria_label(format!("{}, {}", release.version, release.date))
                .flex()
                .items_baseline()
                .gap(px(10.))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(release.version.clone()),
                )
                .child(
                    div()
                        .text_color(theme::color(palette.fg_2))
                        .child(release.date.clone()),
                ),
        )
        .children(
            release
                .highlights
                .iter()
                .enumerate()
                .map(|(category_index, category)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "whats-new-category-{}-{category_index}",
                                    release.version
                                )))
                                .role(Role::Heading)
                                .aria_label(category.name.clone())
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(category.name.clone()),
                        )
                        .children(category.entries.iter().enumerate().map(|(index, entry)| {
                            let link = entry.screenshot_url.clone().map(|url| {
                                Button::new(
                                    SharedString::from(format!(
                                        "whats-new-shot-{}-{category_index}-{index}",
                                        release.version
                                    )),
                                    "See screenshot ↗",
                                    HoverHelp::tooltip("Open this entry on the Release notes page"),
                                    Role::Link,
                                )
                                .tier(Tier::Link)
                                .on_click(move |_, _, cx| cx.open_url(&url))
                            });
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .pl(px(12.))
                                .items_start()
                                .child(paragraph(
                                    SharedString::from(format!(
                                        "whats-new-entry-{}-{category_index}-{index}",
                                        release.version
                                    )),
                                    &entry.pieces,
                                ))
                                .children(link)
                        }))
                }),
        )
}

impl Render for HighlightsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let got_it = weak.clone();
        let opt_out = weak.clone();
        let body = div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .text_size(px(13.))
            .line_height(relative(1.45))
            .children(
                self.releases
                    .iter()
                    .map(|release| release_section(release, cx)),
            );
        let count = self
            .releases
            .iter()
            .map(|release| release.total_changes)
            .fold(0, u64::saturating_add);
        let changes = if count == 1 { "change" } else { "changes" };
        DialogFrame::new("whats-new", TITLE, body, vec![
            Button::new("whats-new-opt-out", "Don't show again", HoverHelp::tooltip("Turn off upgrade highlights"), Role::Button)
                .on_click(move |_, window, cx| dismiss(&opt_out, true, window, cx)),
            Button::new("whats-new-all", "All release notes ↗", HoverHelp::tooltip("Open the full release notes in your browser"), Role::Link)
                .tier(Tier::Link).on_click(|_, _, cx| cx.open_url(RELEASE_NOTES_URL)),
            Button::new("whats-new-got-it", "Got it", HoverHelp::tooltip("Close these highlights"), Role::Button)
                .tier(Tier::Primary).focus(&self.got_it)
                .on_click(move |_, window, cx| dismiss(&got_it, false, window, cx)),
        ])
        .summary(subtitle(&self.releases))
        .note(format!("{count} {changes} in total. See the release notes for the full detail."))
        .width(px(664.))
        .scroll_stop("Release highlights")
        .close("Close upgrade highlights", HoverHelp::tooltip("Close these highlights"), false,
            move |_, window, cx| dismiss(&weak, false, window, cx))
    }
}
