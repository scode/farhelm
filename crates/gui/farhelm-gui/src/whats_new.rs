//! What’s New follows the window's first-open dialogs and records a dismissal on the helm.
//!
//! The installed app owns the feed and its version selection. An asynchronous answer belongs to
//! the UI session and preferences that requested it. A reset or a failed lookup leaves the notes
//! eligible at the next launch, as does a user interaction that takes focus elsewhere before they open.

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    App, AppContext as _, Context, FocusHandle, IntoElement, ParentElement as _, Render,
    Styled as _, Task, WeakEntity, Window, div, px,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier, return_key_content};
use crate::desktop::updates::{self, notes};
use crate::focus::{self, Mark, ReturnTo};
use crate::layers::{self, Dialog, Dismissal, LayerId, MainAction};
use crate::notices::{self, Notice};
use crate::{session, theme};

#[cfg(test)]
pub(crate) mod tests;
mod view;

const TITLE: &str = "What's new in Farhelm";
const RELEASE_NOTES_URL: &str = crate::update_readout::view::RELEASE_NOTES_URL;

/// Start after Welcome and Announcements. The caller keeps the lookup for the lifetime of its UI session.
/// A response may only act on the connection and choices it read. A startup prompt taking focus during the
/// read retires a Show answer even if the prompt has already closed.
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
    let last_focus_layer = layers::last_focus_layer(window, cx);
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
                notes::UpgradeNotes::Show(releases)
                    if !layers::focus_layer_open(window, cx)
                        && layers::last_focus_layer(window, cx) == last_focus_layer =>
                {
                    open(running, previous.as_deref(), releases, &mark, window, cx);
                }
                notes::UpgradeNotes::Show(_) => {}
            }
        });
    }))
}

/// Save the running release and, for Don't show again, the opt-out in the same sparse write.
/// A refused write keeps the old baseline so a later launch can retry. The user needs a notice when the
/// explicit opt-out fails because the dialog has already closed.
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
        move |answer, cx| {
            if let Err(words) = answer {
                tracing::warn!(reason = %words.raw(), "could not record upgrade highlights");
                if opt_out {
                    notices::post(Notice::UpgradeHighlightsOptOutFailed, cx);
                }
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
    previous: Option<&str>,
    releases: Vec<notes::ReleaseHighlights>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let version_range = previous.map_or_else(
        || running.clone(),
        |previous| format!("{previous} → {running}"),
    );
    let version_label = previous.map_or_else(
        || running.clone(),
        |previous| format!("From {previous} to {running}"),
    );
    let view = cx.new(|cx| HighlightsDialog {
        running: Some(running),
        version_range,
        version_label,
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
    version_range: String,
    version_label: String,
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

impl Render for HighlightsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.weak_entity();
        let got_it = weak.clone();
        let opt_out = weak.clone();
        let palette = theme::palette(cx);
        let body = div().flex().flex_col().gap(px(12.)).children(
            self.releases
                .iter()
                .map(|release| view::release_section(release, palette)),
        );
        DialogFrame::new(
            "whats-new",
            TITLE,
            body,
            vec![
                Button::new(
                    "whats-new-opt-out",
                    "Don't show again",
                    HoverHelp::tooltip("Turn off upgrade highlights"),
                    Role::Button,
                )
                .on_click(move |_, window, cx| dismiss(&opt_out, true, window, cx)),
                Button::new(
                    "whats-new-all",
                    "All release notes ↗",
                    HoverHelp::tooltip("Open the full release notes in your browser"),
                    Role::Link,
                )
                .tier(Tier::Link)
                .on_click(|_, _, cx| cx.open_url(RELEASE_NOTES_URL)),
                Button::new(
                    "whats-new-got-it",
                    "Got it",
                    HoverHelp::tooltip("Close these highlights"),
                    Role::Button,
                )
                .content(return_key_content("Got it", palette.on_accent_dim, palette))
                .tier(Tier::Primary)
                .focus(&self.got_it)
                .on_click(move |_, window, cx| dismiss(&got_it, false, window, cx)),
            ],
        )
        .heading(view::heading(
            &self.version_range,
            &self.version_label,
            &self.releases,
            palette,
        ))
        .footer_lead(view::footer_hints(palette))
        .width(px(664.))
        .scroll_stop("Release highlights")
        .close(
            "Close upgrade highlights",
            HoverHelp::tooltip("Close these highlights"),
            false,
            move |_, window, cx| dismiss(&weak, false, window, cx),
        )
    }
}
