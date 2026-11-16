//! The Announcements dialog: updates and information from Farhelm, shown from the helm's announcement queue.
//!
//! SPEC.md, "Announcements". The helm keeps the queue (pending entries and the history of what it has shown) and pushes
//! it as one entry of the `Announcements` collection; this module only presents it. Ported from the old app's
//! `announcements.rs` (`AnnouncementsDialog`) and the stylesheet's `.announcements-*` and `.announcement` rules at
//! reference commit `24bc0580`. Here's the TLDR of how it behaves:
//!
//! - **One dialog, two uses.** It shows a list of announcements newest first, each with its title, its optional detail
//!   line, its plain-text body (blank lines separate paragraphs; nothing is ever read as markup) and its optional
//!   labelled link, which opens in the browser through the app's one opener (`cx.open_url`). Got it, the header's ×
//!   and Escape close it.
//! - **The history, from Help.** After this helm has shown at least one announcement, the `?` menu offers
//!   Announcements, which opens the history ([`open_history`]): every shown announcement, newest shown first, as the
//!   helm orders it. Reopening the history writes nothing, and closing it returns focus to the `?` button.
//! - **Read from the store when opened.** The dialog draws the list it was opened with, and later pushes do not change
//!   it (why: [`AnnouncementsDialog`]). Nothing new is queued while it is open anyway (SPEC.md: "there is no live
//!   announcement event reader today"; the only producer runs at helm start), and an entry pushed after the window
//!   start's check opens nothing by itself: the check runs once per UI session.
//! - **The automatic batch, at window start and reset.** [`open_pending`] opens the pending entries, newest first, when
//!   the window-start owner ([`crate::welcome::StartDialogs`]) says the check is due: at once, or after the Welcome
//!   card and its feedback handoff close. An empty queue opens nothing, and a queue the store does not hold (a failed
//!   read) is logged and opens nothing; the next window start or reset checks again. Its dismissal (Got it, the ×,
//!   Escape, Enter on the dialog itself) records exactly the entries it showed with one keyless
//!   `POST /api/announcements/shown`; a failed write still closes it and leaves them pending. A reset closes it without
//!   recording anything, so the next UI session shows the same entries again. Closing it returns focus home, since it
//!   has no opener (SPEC.md: "closing an automatic batch restores ordinary initial focus").
//! - **A keyboard stop when nothing in the body is focusable.** A long announcement without a link holds nothing Tab
//!   reaches, so the body takes a stop of its own, named "Announcement content" as the old region was
//!   (`DialogFrame::scroll_stop`), in both uses.

#[cfg(test)]
pub(crate) mod tests;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::announcements::{Announcement, AnnouncementQueue};
use farhelm_proto::ui_channel::ANNOUNCEMENTS_KEY;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Div, FocusHandle, FontWeight, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, WeakEntity, Window, div, px, relative,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::{Mark, ReturnTo};
use crate::layers::{self, Dialog, DismissHook, Dismissal, LayerId, MainAction};
use crate::session;
use crate::theme::{self, metrics};

/// `.announcements-dialog`'s width.
const WIDTH: f32 = 560.;
/// `.announcements-dialog`'s text size.
const TEXT: f32 = 12.;
/// `.announcements-dialog`'s `line-height: 1.55`, relative to each text's own size as CSS's unitless value is.
const LINE: f32 = 1.55;
/// `.announcements-body`'s gap between announcements.
const GAP: f32 = 12.;
/// `.announcement`'s padding, vertical then horizontal.
const PAD: (f32, f32) = (8., 10.);
/// `.announcement h3`'s size.
const TITLE: f32 = 13.;
/// `.announcement .announcement-detail`'s size.
const DETAIL: f32 = 11.;
/// `.announcement p`'s top and bottom margin (`8px 0`). Neighbouring paragraphs' margins collapse into one gap, so each
/// paragraph takes it above itself, and a card ending in a paragraph rather than its link takes it once more at its
/// foot.
const PARAGRAPH_GAP: f32 = 8.;

/// `read` applied to the pushed announcement queue, or `None` before the queue has arrived.
fn with_queue<R>(cx: &App, read: impl FnOnce(&AnnouncementQueue) -> R) -> Option<R> {
    let store = session::current_store(cx)?;
    let store = store.read(cx);
    store
        .collections()
        .get::<AnnouncementQueue>(ANNOUNCEMENTS_KEY)
        .map(read)
}

/// The announcements this helm has shown, newest shown first, from the pushed queue; empty when none has been shown
/// or the queue has not arrived.
pub(crate) fn history(cx: &App) -> Vec<Announcement> {
    with_queue(cx, |queue| queue.history.clone()).unwrap_or_default()
}

/// Whether this helm has shown any announcement yet, which is when the `?` menu offers the history. Reads the pushed
/// queue without copying it, since the menu asks on every draw.
pub(crate) fn has_history(cx: &App) -> bool {
    with_queue(cx, |queue| !queue.history.is_empty()).unwrap_or(false)
}

/// Open the history of shown announcements, from the `?` menu: newest shown first, writing nothing. `opener` is the `?`
/// button, where closing returns focus (SPEC.md: "Closing this history view returns focus to Help"); `mark` is the
/// [`Mark`] taken in the menu item's click handler. Opens nothing when the history is empty, the user has moved on
/// since `mark` or the window is inert. Returns the dialog's layer.
pub fn open_history(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let shown = history(cx);
    if shown.is_empty() {
        return None;
    }
    open(
        shown,
        ReturnTo::Control(opener.clone()),
        false,
        mark,
        window,
        cx,
    )
}

/// Open the automatic batch: this helm's pending announcements, newest first (the helm sends them oldest first), whose
/// dismissal records exactly those as shown. Called by the window-start owner ([`crate::welcome::StartDialogs`]) once
/// per UI session, when nothing from the window's start is open ahead of it. `mark` is the window-start mark, or the
/// one taken when the Welcome card or its feedback handoff closed; nothing opens when the user has moved on since.
///
/// Opens nothing when nothing is pending, and nothing when the store holds no queue, which means the helm could not
/// read it: SPEC.md has that failure "logged silently and retried only at a later window start or reset". Closing the
/// batch returns focus home. Returns the dialog's layer.
pub(crate) fn open_pending(mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    let Some(mut pending) = with_queue(cx, |queue| queue.pending.clone()) else {
        tracing::warn!(
            "no announcement queue was read; announcements are checked again at the next window start"
        );
        return None;
    };
    if pending.is_empty() {
        return None;
    }
    pending.reverse();
    open(pending, ReturnTo::Home, true, mark, window, cx)
}

/// Record `ids` as shown: one keyless, best-effort `POST /api/announcements/shown` naming exactly those entries, so
/// newer ones the dialog did not show stay pending. A failure is logged and nothing else, since the dialog has closed:
/// the helm keeps the entries pending, and a later window start or reset shows them again.
fn record_shown(ids: Vec<String>, cx: &mut App) {
    let path = match ApiPath::api(&["announcements", "shown"]) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(%error, "could not record announcements as shown");
            return;
        }
    };
    crate::operations::send(
        Method::Post,
        path,
        Some(serde_json::json!({ "ids": ids })),
        "POST /api/announcements/shown",
        cx,
        |answer, _| {
            if let Err(words) = answer {
                tracing::warn!(reason = %words.raw(), "could not record announcements as shown");
            }
        },
    );
}

/// Open the dialog over `entries`, in the order given, with closing returning focus as `returns_to` says. With
/// `records`, the first dismissal records `entries` as shown ([`record_shown`]): the automatic batch. The history
/// records nothing.
fn open(
    entries: Vec<Announcement>,
    returns_to: ReturnTo,
    records: bool,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let dialog = cx.new(|cx| AnnouncementsDialog {
        entries,
        got_it: cx.focus_handle().tab_stop(true),
        layer: None,
        unrecorded: records,
    });
    // Every user close (Got it, the ×, Escape, Enter on the dialog itself) is a dismissal and asks this hook; a reset
    // closes the dialog without asking, so it records nothing and the entries stay pending for the next UI session.
    let on_dismiss: Option<DismissHook> = records.then(|| {
        let weak = dialog.downgrade();
        let hook: DismissHook = Rc::new(move |_, cx| {
            let ids = weak.update(cx, |dialog, _| {
                // Cleared, so a second dismissal racing the close cannot record twice.
                std::mem::take(&mut dialog.unrecorded).then(|| {
                    dialog
                        .entries
                        .iter()
                        .map(|announcement| announcement.id.clone())
                        .collect::<Vec<_>>()
                })
            });
            if let Ok(Some(ids)) = ids {
                record_shown(ids, cx);
            }
            Dismissal::Close
        });
        hook
    });
    // Enter on the dialog itself (after a click on its text) presses Got it, the old dialog's main action.
    let weak = dialog.downgrade();
    let main_action: MainAction = Rc::new(move |window, cx| close(&weak, window, cx));
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: cx.focus_handle(),
            // Got it, as the old dialog started there.
            start: dialog.read(cx).got_it.clone(),
            subject: None,
            main_action: Some(main_action),
            keeps_focus_on_empty_click: false,
            returns_to,
            on_dismiss,
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    Some(layer)
}

/// The open dialog: the announcements it was opened with, Got it's focus handle (where it starts) and its layer.
///
/// `entries` is a copy of the store's queue taken at opening, an exception to the rule that a view reads shared state
/// from the store when it draws. The automatic batch must record exactly the entries its dialog showed when it is
/// dismissed (SPEC.md: "dismissal records only the entries that dialog showed"), so what is drawn cannot follow later
/// pushes. The history uses the same dialog: an announcement the helm pushes while this history is open appears at
/// its next opening.
struct AnnouncementsDialog {
    entries: Vec<Announcement>,
    got_it: FocusHandle,
    /// Set right after the layer opens; every way of closing goes through the layer manager's dismissal.
    layer: Option<LayerId>,
    /// Whether this is the automatic batch and its dismissal has not yet recorded `entries` as shown; never set for the
    /// history, which records nothing.
    unrecorded: bool,
}

/// Dismiss the dialog `weak` through the layer manager, as Escape does; nothing once it is gone.
fn close(weak: &WeakEntity<AnnouncementsDialog>, window: &mut Window, cx: &mut App) {
    let layer = weak.upgrade().and_then(|dialog| dialog.read(cx).layer);
    if let Some(layer) = layer {
        layers::dismiss(layer, window, cx);
    }
}

/// A click handler that dismisses the dialog `weak`, for Got it and the header's Close.
fn dismiss(
    weak: WeakEntity<AnnouncementsDialog>,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| close(&weak, window, cx)
}

/// `body`'s paragraphs: the text between blank lines, with paragraphs of only whitespace dropped, as the old dialog
/// split it (`split("\n\n")`). The text is shown as it is, never interpreted as markup.
fn paragraphs(body: &str) -> Vec<&str> {
    body.split("\n\n")
        .filter(|part| !part.trim().is_empty())
        .collect()
}

/// One announcement's card (`.announcement`): its title, detail, paragraphs and link, in that order. The link's element
/// id is built from the announcement's own id, which the helm never reuses, not from its place in the list.
fn card(announcement: &Announcement, cx: &App) -> Div {
    let palette = theme::palette(cx);
    let detail = announcement.detail.as_ref().map(|detail| {
        div()
            .mt(px(PARAGRAPH_GAP))
            .text_size(px(DETAIL))
            .text_color(theme::color(palette.fg_2))
            .child(SharedString::from(detail.clone()))
    });
    let body = paragraphs(&announcement.body);
    let has_paragraph = detail.is_some() || !body.is_empty();
    let body = body.into_iter().map(|paragraph| {
        div()
            .mt(px(PARAGRAPH_GAP))
            .child(SharedString::from(paragraph.to_owned()))
    });
    let link = announcement.link.as_ref().map(|link| {
        let url = link.url.clone();
        // A row, so the link sits at the start of its line as the old inline link did, not stretched and centered.
        // The old inline link followed the last paragraph's bottom margin, or sat right under the title without one.
        let gap = if has_paragraph { PARAGRAPH_GAP } else { 0. };
        div().mt(px(gap)).flex().child(
            Button::new(
                SharedString::from(format!("announcement-link-{}", announcement.id)),
                format!("{} ↗", link.label),
                HoverHelp::tooltip(format!("{}: open in your browser", link.label)),
                Role::Link,
            )
            .tier(Tier::Link)
            .on_click(move |_, _, cx| cx.open_url(&url)),
        )
    });
    // A last paragraph's bottom margin, which the old card kept inside its padding.
    let foot = if link.is_none() && has_paragraph {
        PARAGRAPH_GAP
    } else {
        0.
    };
    div()
        .pt(px(PAD.0))
        .pb(px(PAD.0 + foot))
        .px(px(PAD.1))
        .border_1()
        .border_color(theme::color(palette.border_dim))
        .rounded(metrics::RADIUS_SM)
        .child(
            div()
                .text_size(px(TITLE))
                // `h3`'s browser default weight; the old rule set none of its own.
                .font_weight(FontWeight::BOLD)
                .text_color(theme::color(palette.fg_0))
                .child(SharedString::from(announcement.title.clone())),
        )
        .children(detail)
        .children(body)
        .children(link)
}

impl Render for AnnouncementsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let body = div()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .text_size(px(TEXT))
            .line_height(relative(LINE))
            .text_color(theme::color(palette.fg_dim))
            .children(
                self.entries
                    .iter()
                    .map(|announcement| card(announcement, cx)),
            );
        let got_it = Button::new(
            "announcements-got-it",
            "Got it",
            HoverHelp::tooltip("Got it: close these announcements"),
            Role::Button,
        )
        .tier(Tier::Primary)
        .focus(&self.got_it)
        .on_click(dismiss(weak.clone()));
        DialogFrame::new("announcements", "Announcements", body, vec![got_it])
            .summary("Updates and information from Farhelm.")
            .width(px(WIDTH))
            .compact()
            // The old body was a stop of its own under this name, so a long announcement without a link can still be
            // scrolled from the keyboard.
            .scroll_stop("Announcement content")
            .close(
                "Close announcements",
                HoverHelp::tooltip("Close: done with these announcements"),
                false,
                dismiss(weak),
            )
    }
}
