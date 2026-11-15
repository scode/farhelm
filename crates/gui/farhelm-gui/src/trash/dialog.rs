//! The Trash dialog: archived checkouts grouped by host, newest first, each with its own immediate Delete.
//!
//! SPEC.md, "Managed checkouts" (the trash paragraphs). Ported from the dialog half of the old app's `list/trash.rs`
//! and its `.checkout-trash-*` and `.trash-*` rules in `app.css`, at reference commit `24bc0580`. Here's the TLDR of its
//! rules:
//!
//! - **What it shows.** Every registered host in the registry's order, with a section for each host that has
//!   something to say: its archives (the host lists them newest first), each with its repository, archive time and
//!   measured size; the folders to copy work out of before cleanup; what the host could not check; and the cleanup
//!   outcomes kept for it. An unreachable host stays visible, dimmed, and offers nothing. Totals that miss a size, a
//!   host still being read or an entry the host could not check say "measured · partial". Every value a host supplied
//!   is drawn as host text.
//! - **An entry's own Delete acts at once,** without a question, and the dialog stays open; a failed removal offers
//!   Retry instead, through the same command. It is unavailable under the host's guards
//!   ([`super::listing::HostSnapshot::entry_target`]: connected, its listing settled and within the batch limit, the
//!   entry not already removing, the host's own key free), while the registry cannot be read, and while any trash
//!   delete of this window's is in flight. A capable host's accepted row stays, marked removing, until a later listing
//!   confirms it gone.
//! - **Rows moving under the pointer pause the deletes.** An entry's Delete has no question, so a click aimed at one
//!   row must not delete the row a re-read moved under the pointer. Whenever what decides where rows are drawn changes,
//!   every entry's Delete is unavailable for [`SETTLE`] (the old app's `SETTLE_MS`, matching the approval cards'
//!   answer pause).
//! - **Each host's "Delete all on" and the footer's Delete all ask once, in place.** Both are inline questions of the
//!   prompt ([`prompt::ask_inline`]), frozen as they open: each host's IDs and connection ([`listing::Batch`]), and the
//!   counts, size and host names they name. A host's question is drawn inside its card, under its header; the global
//!   one in the frame's bounded region above the footer's actions ([`DialogFrame::notices`]), which scrolls on its own.
//!   Focus starts on Delete permanently (SPEC.md, "Opening and closing": permanent trash deletion starts on its
//!   confirming answer), scrolled into view by the focus coordinator; Cancel or Escape on the question returns focus to
//!   the button that asked it, Delete all for the global one. That button is drawn busy rather than disabled while it
//!   is unavailable (an entry's Delete pressed meanwhile, say), so it can still take focus back. Delete all waits until
//!   every reachable host's listing is settled and skips unreachable hosts ([`Snapshot::global_batches`]). Confirming
//!   closes the dialog at once and sends one request per host with its frozen IDs ([`TrashButton::delete_batches`]). A
//!   confirmation the guards refuse when pressed (another trash delete of this window's running, a host reconnected or
//!   busy) sends nothing and leaves the dialog open. The settle pause does not hold these buttons: a question stands
//!   between them and a delete.
//! - **Focus.** It starts on the footer's Close, and Tab goes header close, the archive list's controls (or, while
//!   none of them is available, the list's own stop, "Archived checkouts"), then the footer's Close and Delete all
//!   (SPEC.md, "Initial focus stays on footer Close"). Close, the header's close and Escape are never unavailable, and
//!   closing never cancels a delete the helm took.
//!
//! NOTE: Not yet as SPEC.md and the old app have them, each waiting for a foundation change and recorded with its
//! reason in the parity file ("### Trash"): the global question's region is not keyboard-scrollable while an answer
//! holds focus; and, until an owner can tell that its inline question is still open, the questions take the prompt's
//! loud look rather than the old band, and the rest of the dialog does not change while one is up.
//!
//! NOTE: A deviation from the old dialog, recorded in the parity file: the old dialog stacked its footer's actions as
//! full-width rows at phone widths, which the app's 720 by 560 minimum window never reaches, so that is not ported.

#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use farhelm_proto::ArchivedCheckout;
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, WeakEntity, Window, div, prelude::FluentBuilder as _, px, svg,
};

use super::TrashButton;
use super::listing::{self, Batch, HostSnapshot, Notice, Snapshot, Target};
use crate::assets::Icon;
use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, LayerId};
use crate::prompt::{self, Answer, Inline, Question};
use crate::theme::{self, Palette, metrics};
use crate::ticker;

/// How long every entry's Delete stays unavailable after the rows move: the old app's 700 milliseconds.
pub(crate) const SETTLE: Duration = Duration::from_millis(700);

/// `.checkout-trash-dialog`'s width.
const WIDTH: f32 = 680.;
/// `.checkout-trash-dialog p`: the dialog's 12-pixel text at a line height of 1.6.
const LINE: f32 = 19.2;
/// `.checkout-trash-host`'s margin between host cards.
const HOST_GAP: f32 = 14.;
/// The padding of a host's header, an entry, an issue and the folder line: `10px 12px`.
const PAD: (f32, f32) = (10., 12.);
/// The entry meta line's and the folder line's text, and the branch glyph's box.
const SMALL: f32 = 11.;
/// `.checkout-trash-host.unreachable`'s opacity.
const UNREACHABLE: f32 = 0.65;

/// The dialog's view: what it reads everything from, and the rows' settle pause.
pub(crate) struct TrashDialog {
    /// The button that opened it, which holds every read and starts the deletes.
    button: WeakEntity<TrashButton>,
    /// The footer's Close, where focus starts.
    close: FocusHandle,
    /// The footer's Delete all, which a cancelled global question returns focus to.
    delete_all: FocusHandle,
    /// Each host's "Delete all on" button, by registry id, which its cancelled question returns focus to.
    host_deletes: HashMap<i64, FocusHandle>,
    /// The confirmation asked last: the host it is about (`None` for Delete all) and the question. It draws nothing once
    /// answered, and a newer question replaces it.
    asked: Option<(Option<i64>, Inline<Vec<Target>>)>,
    /// How many questions this dialog has asked: part of the settle fence, so a question opening, which moves the rows
    /// under a host's, pauses the entries' Deletes as any move does.
    asks: u64,
    /// Set right after the layer opens.
    pub(super) layer: Option<LayerId>,
    /// The fingerprint of the layout last drawn ([`layout`]).
    drawn: Option<u64>,
    /// Whether the rows have stayed put for [`SETTLE`] since they last moved.
    settled: bool,
    /// The end of the current pause; replacing it cancels the older one, so an old pause never settles a newer layout.
    settle: Option<ticker::Deadline>,
    /// How long the pause lasts: [`SETTLE`], or nothing for the screenshot goldens, whose clock never moves.
    pause: Duration,
    /// Redraws the dialog whenever the button's reads change.
    _button: Subscription,
}

impl TrashDialog {
    /// A dialog reading from `button`.
    pub(super) fn new(button: Entity<TrashButton>, cx: &mut Context<Self>) -> Self {
        TrashDialog {
            _button: cx.observe(&button, |_, _, cx| cx.notify()),
            button: button.downgrade(),
            close: cx.focus_handle().tab_stop(true),
            delete_all: cx.focus_handle().tab_stop(true),
            host_deletes: HashMap::new(),
            asked: None,
            asks: 0,
            layer: None,
            drawn: None,
            settled: true,
            settle: None,
            pause: SETTLE,
        }
    }

    /// Where focus starts: the footer's Close.
    pub(super) fn start(&self) -> FocusHandle {
        self.close.clone()
    }

    /// Ask to delete everything on `host` (its "Delete all on"), or on every reachable host (`None`, Delete all), as the
    /// state is now: nothing opens while the registry is unread, a trash delete of this window's runs, or the guards
    /// leave nothing to delete. The question returns focus to the button that asked it when cancelled, so that button
    /// takes focus first (a click on macOS leaves it unfocused). Visible to the crate for the screenshot goldens.
    pub(crate) fn ask(&mut self, host: Option<i64>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(button) = self.button.upgrade() else {
            return;
        };
        let snapshot = button.update(cx, |button, cx| button.snapshot(cx));
        if !snapshot.hosts_known || snapshot.delete_in_flight {
            return;
        }
        let batches = match host {
            Some(id) => snapshot
                .hosts
                .iter()
                .find(|shown| shown.id == id)
                .and_then(HostSnapshot::batch)
                .map(|batch| vec![batch]),
            None => snapshot.global_batches(),
        };
        let Some(batches) = batches else {
            return;
        };
        let mark = Mark::take(cx);
        let opener = match host {
            Some(id) => self.host_deletes.get(&id).cloned(),
            None => Some(self.delete_all.clone()),
        };
        if let Some(opener) = opener {
            focus::focus_in_layer(&opener, &mark, window, cx);
        }
        let dialog = cx.weak_entity();
        let button = self.button.clone();
        let inline = prompt::ask_inline(
            confirmation(&batches),
            &mark,
            move |targets: Vec<Target>, window, cx| {
                // SPEC.md: "Confirming a bulk delete closes the dialog immediately and sends one request per host". A
                // confirmation the guards refuse sends nothing and leaves the dialog open on what changed.
                let layer = dialog.upgrade().and_then(|dialog| dialog.read(cx).layer);
                let sent = button
                    .update(cx, |button, cx| button.delete_batches(targets, cx))
                    .unwrap_or(false);
                if let Some(layer) = layer.filter(|_| sent) {
                    layers::close(layer, window, cx);
                }
            },
            window,
            cx,
        );
        self.asked = Some((host, inline));
        self.asks += 1;
        cx.notify();
    }

    /// Draw every entry's Delete as if the rows had always been settled: the screenshot goldens' clock never moves, so
    /// a pause would never end in their pictures.
    #[cfg(test)]
    pub(crate) fn without_pause(&mut self) {
        self.pause = Duration::ZERO;
    }

    /// Start the settle pause when `snapshot`'s layout differs from the one last drawn. Called while drawing, so the
    /// frame that first shows moved rows already draws their Deletes unavailable.
    fn fence(&mut self, snapshot: &Snapshot, cx: &mut Context<Self>) {
        let layout = layout(snapshot, self.asks);
        if self.drawn == Some(layout) {
            return;
        }
        self.drawn = Some(layout);
        if self.pause.is_zero() {
            return;
        }
        self.settled = false;
        let dialog = cx.weak_entity();
        self.settle = Some(ticker::after(self.pause, cx, move |cx| {
            let _ = dialog.update(cx, |dialog, cx| {
                dialog.settled = true;
                cx.notify();
            });
        }));
    }
}

/// A fingerprint of everything that decides where entries' Deletes are drawn: the questions asked (`asks`), the
/// registry's state, a delete in flight, and per host its reachability, whether it is being read, its rows (with their
/// removing and failure details, which change a row's height), its diagnostics and its kept outcomes.
fn layout(snapshot: &Snapshot, asks: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    // Opening a host's question moves the rows under it. Its closing moves them back unseen: the dialog cannot tell
    // when an inline question closes.
    asks.hash(&mut hasher);
    (snapshot.hosts_known, snapshot.delete_in_flight).hash(&mut hasher);
    for host in &snapshot.hosts {
        (
            host.id,
            host.connected,
            host.pending,
            host.notices.len(),
            host.notice_error.is_some(),
        )
            .hash(&mut hasher);
        match &host.listing {
            Some(Ok(listing)) => {
                for checkout in &listing.checkouts {
                    (
                        &checkout.id,
                        checkout.removing,
                        checkout
                            .stuck
                            .as_ref()
                            .map(|stuck| (&stuck.reason, stuck.at)),
                    )
                        .hash(&mut hasher);
                }
                listing.issues.len().hash(&mut hasher);
            }
            Some(Err(_)) => 1.hash(&mut hasher),
            None => 2.hash(&mut hasher),
        }
    }
    hasher.finish()
}

/// What the dialog's controls need to act, cloned into each handler.
#[derive(Clone)]
struct Handles {
    button: WeakEntity<TrashButton>,
    dialog: WeakEntity<TrashDialog>,
}

impl Handles {
    /// Close the dialog through the layer manager, as Escape does: both Close controls.
    fn dismiss(&self) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let dialog = self.dialog.clone();
        move |_, window, cx| {
            let layer = dialog.upgrade().and_then(|dialog| dialog.read(cx).layer);
            if let Some(layer) = layer {
                layers::dismiss(layer, window, cx);
            }
        }
    }

    /// Ask the confirmation for `host`'s "Delete all on", or Delete all's for `None` ([`TrashDialog::ask`]).
    fn ask(
        &self,
        host: Option<i64>,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let dialog = self.dialog.clone();
        move |_, window, cx| {
            let _ = dialog.update(cx, |dialog, cx| dialog.ask(host, window, cx));
        }
    }

    /// Delete `id` on `host` at once, unless the rows have moved since the frame this press was aimed at.
    fn delete(
        &self,
        host: i64,
        id: String,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let Handles { button, dialog } = self.clone();
        move |_, _, cx| {
            if !dialog
                .upgrade()
                .is_some_and(|dialog| dialog.read(cx).settled)
            {
                return;
            }
            let _ = button.update(cx, |button, cx| button.delete(host, &id, cx));
        }
    }
}

impl Render for TrashDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = match self.button.upgrade() {
            Some(button) => button.update(cx, |button, cx| button.snapshot(cx)),
            None => Snapshot::default(),
        };
        self.fence(&snapshot, cx);
        let palette = theme::palette(cx);
        let handles = Handles {
            button: self.button.clone(),
            dialog: cx.weak_entity(),
        };
        let deletes_allowed = self.settled && snapshot.hosts_known && !snapshot.delete_in_flight;
        // The bulk deletes ask first, so the settle pause that guards an entry's immediate Delete does not hold them.
        let asks_allowed = snapshot.hosts_known && !snapshot.delete_in_flight;
        let (count, _) = snapshot.count();
        let (bytes, partial) = snapshot.total();

        let mut body = div()
            .flex()
            .flex_col()
            .line_height(px(LINE))
            .child(div().child(
                "Folders of managed checkouts whose last session was deleted. Before cleanup starts, you can copy work \
                 out of the folder shown under its host. Once cleanup is accepted, files may already be partly \
                 removed. Failed cleanup stays listed with its reason and Retry.",
            ));
        if let Some(error) = &snapshot.hosts_error {
            body = body.child(block(palette).mt(px(12.)).child(
                HostSentence::new("trash-hosts-error")
                    .text("Host reachability could not be refreshed. Cleanup is disabled until it can be checked: ")
                    .host(error),
            ));
        }
        let empty = snapshot.hosts_known
            && count == 0
            && snapshot
                .hosts
                .iter()
                .filter(|host| host.connected)
                .all(|host| {
                    !host.pending
                        && host
                            .listed()
                            .is_some_and(|listing| listing.issues.is_empty())
                });
        if empty {
            // `.checkout-trash-empty` is a paragraph, 12 pixels below the intro.
            body = body.child(div().mt(px(12.)).child(line(
                "trash-empty",
                "The trash is empty on reachable hosts.",
            )));
        }
        let global = match &self.asked {
            Some((None, inline)) => Some(inline.clone()),
            _ => None,
        };
        for host in &snapshot.hosts {
            let focus = self
                .host_deletes
                .entry(host.id)
                .or_insert_with(|| cx.focus_handle().tab_stop(true))
                .clone();
            let asked = match &self.asked {
                Some((Some(id), inline)) if *id == host.id => Some(inline.clone()),
                _ => None,
            };
            let ask = HostAsk {
                allowed: asks_allowed,
                asker: matches!(self.asked, Some((Some(id), _)) if id == host.id),
                focus: &focus,
                asked,
            };
            if let Some(section) =
                host_section(host, &snapshot, deletes_allowed, ask, &handles, palette)
            {
                body = body.child(section);
            }
        }
        // Hosts gone from the registry take their buttons' focus handles with them.
        let present: Vec<i64> = snapshot.hosts.iter().map(|host| host.id).collect();
        self.host_deletes.retain(|id, _| present.contains(id));

        let ink = theme::color(palette.fg_0);
        let heading = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .text_size(metrics::TEXT_TITLE)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(ink)
            .child(super::can(snapshot.summary().0, false, 0., 0., ink))
            .child("Trash");
        let close = Button::new(
            "trash-close",
            "Close",
            HoverHelp::tooltip(
                "Close: return to the session list; cleanup already accepted goes on",
            ),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .focus(&self.close)
        .on_click(handles.dismiss());
        let delete_all = Button::new(
            "trash-delete-all",
            "Delete all",
            HoverHelp::tooltip(
                "Delete all: after you confirm, remove every archived checkout on every reachable host for good",
            ),
            Role::Button,
        )
        .tier(Tier::Danger)
        .focus(&self.delete_all)
        // Busy rather than disabled: a cancelled question returns focus to the button that asked it, which must still
        // be able to take it when an entry's Delete started meanwhile. It sits in the footer, so the list's own stop is
        // unaffected.
        .busy(!asks_allowed || snapshot.global_batches().is_none())
        .on_click(handles.ask(None));
        let mut frame = DialogFrame::new(
            "checkout-trash-dialog",
            "Trash",
            body,
            vec![close, delete_all],
        );
        if let Some(global) = global {
            // The old `.checkout-trash-global-confirmation`: directly above the footer's actions, bounded, and
            // scrolling on its own, which is what the frame's notices region is.
            frame = frame.notices(
                div()
                    .debug_selector(|| "trash-global-confirmation".into())
                    .child(global),
            );
        }
        frame
            .heading(heading)
            .summary(format!(
                "{count} {} · {} on reachable hosts",
                listing::checkouts(count),
                listing::space(bytes, partial)
            ))
            .width(px(WIDTH))
            // The old list was a stop of its own under this name, so the archives stay keyboard-scrollable while every
            // action in them is unavailable (SPEC.md, "Reaching everything by keyboard": a region holding nothing
            // focusable gets its own tab stop).
            .scroll_stop("Archived checkouts")
            .close(
                "Close trash",
                HoverHelp::tooltip(
                    "Close: return to the session list; cleanup already accepted goes on",
                ),
                false,
                handles.dismiss(),
            )
    }
}

/// One host's card (`.checkout-trash-host`), or `None` when the host has nothing to say: a connected, settled host with
/// an empty listing and nothing kept for it.
fn host_section(
    host: &HostSnapshot,
    snapshot: &Snapshot,
    deletes_allowed: bool,
    ask: HostAsk,
    handles: &Handles,
    palette: &Palette,
) -> Option<AnyElement> {
    let listing = host.listed();
    let show = snapshot.deleting == Some(host.id)
        || !host.connected
        || host.pending
        || listing
            .is_none_or(|listing| !listing.checkouts.is_empty() || !listing.issues.is_empty())
        || !host.notices.is_empty()
        || host.notice_error.is_some();
    if !show {
        return None;
    }
    let id = host.id;
    let totals = listing.map(listing::totals);
    let count = totals.map_or_else(|| "Count unknown".to_owned(), |totals| totals.count_label());
    let size = if host.connected && snapshot.hosts_known && host.pending {
        "Measuring…".to_owned()
    } else {
        totals.map_or_else(
            || "Size unknown".to_owned(),
            |totals| listing::space(totals.bytes, totals.partial),
        )
    };
    let named = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .child(HostSentence::new(format!("trash-host-name-{id}")).host(&host.name)),
        )
        .child(
            div()
                .id(SharedString::from(format!("trash-host-totals-{id}")))
                .role(Role::Label)
                .aria_label(SharedString::from(format!("{count} · {size}")))
                .text_color(theme::color(palette.fg_1))
                .child(format!("{count} · {size}")),
        );
    let delete_label = HostSentence::new(format!("trash-host-delete-label-{id}"))
        .text("Delete all on ")
        .host(&host.name);
    let delete = Button::new(
        format!("trash-host-delete-{id}"),
        "Delete all on",
        HoverHelp::tooltip(
            "Delete all on this host: after you confirm, remove every archived checkout listed here for good",
        ),
        Role::Button,
    )
    .tier(Tier::DangerOutline)
    .accessible_name(delete_label.accessible_name())
    .content(delete_label)
    .focus(ask.focus)
    .on_click(handles.ask(Some(id)));
    // While unavailable, the host that asked last stays able to take focus back from its question, as Delete all does;
    // any other is disabled, so a list whose actions are all unavailable keeps its own stop.
    let unavailable = !ask.allowed || host.batch().is_none();
    let delete = if ask.asker {
        delete.busy(unavailable)
    } else {
        delete.disabled(unavailable)
    };
    // `.checkout-trash-host-header`: the name and totals, then the host's own Delete all, spread apart and wrapping.
    let header = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(10.))
        .py(px(PAD.0))
        .px(px(PAD.1))
        .bg(theme::color(palette.bg_1))
        .child(named)
        .child(div().flex_none().child(delete));
    let mut card = div()
        .id(SharedString::from(format!("trash-host-{id}")))
        .debug_selector(move || format!("trash-host-{id}"))
        .flex()
        .flex_col()
        // `margin-block: 14px` collapsed between cards in the old page; flex margins do not collapse, so only the top.
        .mt(px(HOST_GAP))
        .border_1()
        .border_color(theme::color(palette.border))
        .rounded(metrics::RADIUS_MD)
        .overflow_hidden()
        .when(!host.connected, |card| card.opacity(UNREACHABLE))
        .child(header)
        // The old band sat right under the header: a host's question stays inside its own card.
        .children(ask.asked.map(|inline| {
            div()
                .debug_selector(move || format!("trash-host-confirmation-{id}"))
                .child(inline)
        }));
    if !host.connected {
        card = card.child(paragraph(
            format!("trash-unreachable-{id}"),
            "Not reachable, so they can't be checked or deleted",
        ));
    }
    if let Some(Err(error)) = &host.listing {
        card = card.child(
            block(palette).child(HostSentence::new(format!("trash-read-error-{id}")).host(error)),
        );
    }
    if let Some(error) = &host.notice_error {
        card = card.child(
            div()
                .px(px(PAD.1))
                .my(px(12.))
                .text_color(theme::color(palette.danger_strong))
                .child(
                    HostSentence::new(format!("trash-notice-error-{id}"))
                        .text("The notification bell could not be updated: ")
                        .host(error),
                ),
        );
    }
    if let Some(listing) = listing {
        if listing::selection_size(listing) > listing::MAX_SELECTION {
            card = card.child(paragraph(
                format!("trash-over-limit-{id}"),
                "This host exceeds the 10,000-entry cleanup limit. Cleanup is unavailable for this listing.",
            ));
        }
        for checkout in &listing.checkouts {
            let enabled = deletes_allowed && host.entry_target(&checkout.id).is_some();
            card = card.child(entry(host, checkout, enabled, handles, palette));
        }
        for (at, issue) in listing.issues.iter().enumerate() {
            card = card.child(issue_block(
                format!("trash-issue-{id}-{at}"),
                (!issue.path.is_empty()).then(|| HostText::from_host(issue.path.as_str())),
                None,
                &HostText::from_host(issue.message.as_str()),
                palette,
            ));
        }
        for (at, folder) in listing::folders(listing).iter().enumerate() {
            card = card.child(
                div()
                    .py(px(PAD.0))
                    .px(px(PAD.1))
                    .bg(theme::color(palette.bg_1))
                    .border_t_1()
                    .border_color(theme::color(palette.border))
                    .text_size(px(SMALL))
                    .text_color(theme::color(palette.fg_1))
                    .child(
                        HostSentence::new(format!("trash-folder-{id}-{at}"))
                            .text("On disk: ")
                            .mono(&HostText::from_host(folder.as_str())),
                    ),
            );
        }
    }
    for (at, notice) in host.notices.iter().enumerate() {
        card = card.child(notice_block(
            format!("trash-notice-{id}-{at}"),
            notice,
            palette,
        ));
    }
    Some(card.into_any_element())
}

/// What a host's card needs for its "Delete all on": whether asking is allowed at all, whether it asked last, the
/// button's focus handle, and the host's question while one is asked.
struct HostAsk<'a> {
    allowed: bool,
    /// Whether this host's button asked the dialog's latest question, which returns focus to it when cancelled.
    asker: bool,
    focus: &'a FocusHandle,
    asked: Option<Inline<Vec<Target>>>,
}

/// The confirmation for `batches`, frozen with their targets: the old band's words under its accessible name, "Confirm
/// permanent checkout deletion", which the loud look draws as the heading; then Cancel and Delete permanently, where
/// focus starts (SPEC.md, "Opening and closing").
fn confirmation(batches: &[Batch]) -> Question<Vec<Target>> {
    let targets = batches.iter().map(|batch| batch.target.clone()).collect();
    let mut question = Question::new(
        "Confirm permanent checkout deletion",
        Answer {
            label: "Delete permanently".into(),
            help: HoverHelp::tooltip(
                "Delete permanently: remove these folders for good; work never committed or pushed is lost",
            ),
            tier: Tier::Danger,
        },
        targets,
    )
    .cancel_answer(Answer {
        label: "Cancel".into(),
        help: HoverHelp::tooltip("Cancel: delete nothing and go back to the trash"),
        tier: Tier::Neutral,
    })
    .loud();
    for line in listing::confirmation_text(batches) {
        question = question.line(line);
    }
    question
}

/// One archived checkout (`.checkout-trash-entry`): its name, measured size and own Delete, Retry or Removing… on the
/// first line; the repository and archive time under it; then whether removal runs or why it failed.
fn entry(
    host: &HostSnapshot,
    checkout: &ArchivedCheckout,
    enabled: bool,
    handles: &Handles,
    palette: &Palette,
) -> AnyElement {
    let key = format!("{}-{}", host.id, checkout.id);
    let name = HostText::from_host(checkout.name.as_str());
    let named = |words: &str, after: &str| {
        HostSentence::new(SharedString::from(format!("trash-entry-label-{key}")))
            .text(words.to_owned())
            .host(&name)
            .text(after.to_owned())
            .accessible_name()
    };
    let (label, accessible, help) = if checkout.removing {
        (
            "Removing…",
            named("Removing ", ""),
            "Removing: the host is freeing this folder's space",
        )
    } else if checkout.stuck.is_some() {
        (
            "Retry",
            named("Retry cleanup of ", ""),
            "Retry: ask the host for one more attempt to free this folder, through the same checks",
        )
    } else {
        (
            "Delete",
            named("Delete ", " permanently"),
            "Delete: remove this folder for good at once, without asking; work in it that was never committed or \
             pushed is lost",
        )
    };
    let action = Button::new(
        format!("trash-entry-delete-{key}"),
        label,
        HoverHelp::tooltip(help),
        Role::Button,
    )
    .tier(Tier::DangerOutline)
    .accessible_name(accessible)
    .disabled(!enabled || checkout.removing)
    .on_click(handles.delete(host.id, checkout.id.clone()));
    let heading = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_weight(FontWeight::BOLD)
                .child(HostSentence::new(format!("trash-entry-name-{key}")).mono(&name)),
        )
        .children(
            checkout
                .bytes
                .filter(|_| !host.pending)
                .map(|bytes| div().child(listing::space(bytes, false))),
        )
        .child(div().flex_none().child(action));
    let archived = match checkout.archived_at {
        Some(seconds) => HostSentence::new(format!("trash-entry-archived-{key}"))
            .text("Archived ")
            .mono(&HostText::from_host(listing::stamp(seconds))),
        None => {
            HostSentence::new(format!("trash-entry-archived-{key}")).text("Archive time unknown")
        }
    };
    let meta = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .mt(px(4.))
        .text_size(px(SMALL))
        .text_color(theme::color(palette.fg_1))
        .child(branch(theme::color(palette.fg_1)))
        .child(
            HostSentence::new(format!("trash-entry-repository-{key}"))
                .host(&HostText::from_host(checkout.repository.as_str())),
        )
        .child(div().ml_auto().child(archived));
    let mut drawn = div()
        .id(SharedString::from(format!("trash-entry-{key}")))
        .flex()
        .flex_col()
        .py(px(PAD.0))
        .px(px(PAD.1))
        .border_t_1()
        .border_color(theme::color(palette.border))
        .child(heading)
        .child(meta);
    if checkout.removing {
        drawn = drawn.child(
            div()
                .mt(px(4.))
                .text_size(px(SMALL))
                .text_color(theme::color(palette.fg_1))
                .child(line(
                    format!("trash-entry-removing-{key}"),
                    "Freeing space; some files may already be gone.",
                )),
        );
    } else if let Some(stuck) = &checkout.stuck {
        drawn = drawn.child(
            block(palette)
                .child(
                    HostSentence::new(format!("trash-entry-stuck-{key}"))
                        .text("Could not free space · ")
                        .mono(&HostText::from_host(listing::stamp(stuck.at))),
                )
                .child(
                    HostSentence::new(format!("trash-entry-stuck-reason-{key}"))
                        .host(&HostText::from_host(stuck.reason.as_str())),
                ),
        );
    }
    drawn.into_any_element()
}

/// A kept cleanup outcome, as an issue block: an answer that was not a cleanup report says first that some files may
/// already be gone.
fn notice_block(id: String, notice: &Notice, palette: &Palette) -> AnyElement {
    let lead = notice.unconfirmed.then_some(
        "The cleanup reply could not be confirmed; some folders may already be deleted. Rechecking the trash: ",
    );
    issue_block(id, notice.path.clone(), lead, &notice.message, palette)
}

/// An issue (`.checkout-trash-issue`): the folder it is about in the monospace face when there is one, then the
/// words, after `lead` when the app has something to say first.
fn issue_block(
    id: String,
    path: Option<HostText>,
    lead: Option<&'static str>,
    message: &HostText,
    palette: &Palette,
) -> AnyElement {
    let mut words = HostSentence::new(format!("{id}-message"));
    if let Some(lead) = lead {
        words = words.text(lead);
    }
    let path = path.map(|path| HostSentence::new(format!("{id}-path")).mono(&path));
    block(palette)
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .children(path)
        .child(words.host(message))
        .into_any_element()
}

/// The shared look of an issue (`.checkout-trash-issue`): the card's padding on the well, under a rule.
fn block(palette: &Palette) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .py(px(PAD.0))
        .px(px(PAD.1))
        .bg(theme::color(palette.bg_1))
        .border_t_1()
        .border_color(theme::color(palette.border))
}

/// A paragraph inside a host's card (`.checkout-trash-host > p`): inset to the card's padding.
fn paragraph(id: String, words: &'static str) -> impl IntoElement {
    div().px(px(PAD.1)).my(px(12.)).child(line(id, words))
}

/// A line of the app's own words that tests and assistive technology find by `id`.
fn line(id: impl Into<SharedString>, words: &'static str) -> impl IntoElement {
    let id = id.into();
    let selector = id.clone();
    div()
        .id(id)
        .debug_selector(move || selector.to_string())
        .role(Role::Label)
        .aria_label(words)
        .child(words)
}

/// The managed-checkout glyph before a repository (the old `BranchIcon`), in `ink`.
fn branch(ink: Hsla) -> gpui::Svg {
    svg()
        .path(Icon::Branch.path())
        .size(px(SMALL))
        .flex_none()
        .text_color(ink)
}
