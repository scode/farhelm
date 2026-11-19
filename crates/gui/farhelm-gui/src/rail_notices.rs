//! The notices bell at the top of the host rail's foot, and the panel of notices about hosts and Farhelm it opens.
//!
//! SPEC.md, "Status" (the rail bell paragraphs and the trash cleanup notice) and "Managed checkouts" (a cleanup failure
//! "also appears in the navigation rail's notification bell"). Ported from the old app's `list/rail_notices.rs` (the
//! toggle, the panel, its actions) and `notices.rs` (the reader's read mark and Clear resolved), and the stylesheet's
//! `.rail-notices*` and `.rail-notice*` rules, all at reference commit `24bc0580`. Here's the TLDR of the rules:
//!
//! - **A view of its own, placed by the rail.** [`NoticesButton`] keeps the bell's focus handle and the panel it
//!   opened, so the session list only creates it and draws it at the top of the rail's foot, above Hosts, as it draws
//!   the Help button below the gear. The bell is the rail's quiet 16-pixel glyph while nothing is unread, and the
//!   filled bell in the strong red with the unread count beside it while something is; its accessible name and hover
//!   help report the count. While the host rail is expanded it is a row of the rail with "Notifications" beside its
//!   glyph, as every foot control is, and keeps its name and help; it reads the expansion in its own render from the
//!   entity the session list owns and hands it ([`Expansion`]), keeping no copy of it.
//! - **Read from the store, every frame.** The bell and the panel draw the helm's pushed notices ([`Notice`], the
//!   `Notices` collection) straight from the store, with no notice-center step (the migration's orchestrator decided
//!   that on 2026-11-14). A notice is unread while the helm says it is unread and it is unresolved; a resolved notice is
//!   quiet whether or not anyone opened the panel. What the open panel keeps of its own is only what its close and its
//!   Clear resolved need: the ids the last frame drew ([`NoticesPanel`]).
//! - **The panel is a popover beside the bell.** It opens through the layer manager with the mark the click took, 360
//!   pixels wide, newest first, in the row menus' surface with their header. Focus starts on the panel itself; Tab walks
//!   its controls as if they stood right after the bell and goes on out of it. Escape, a press outside it, the bell
//!   again, its Close, and keyboard focus leaving it each close it (the layer manager's popover that lets Tab through
//!   closes when focus leaves it, SPEC.md's "leaving the panel with keyboard focus"); Escape, the bell and Close return
//!   focus to the bell.
//! - **Every close ends in one read mark.** The panel's view goes with its layer, and its release sends
//!   `POST /api/notices/marks` marking read the unread notices the last frame drew, and nothing else, so a notice that
//!   arrived after that frame stays unread. It is a keyless send and best effort, as the session bell's mark is: a
//!   failure is only logged, and the bell stays loud until the panel is next closed (SPEC.md: "a failed best-effort mark
//!   may leave it loud until another close").
//! - **Clear resolved removes only the resolved notices the panel showed**, as a keyless send whose refusal the panel
//!   shows (SPEC.md: a failed clear surfaces like any other operation). The helm pushes the change before it answers.
//! - **The trash cleanup notice's actions belong to the Trash.** Retry now asks the Trash for one attempt through the
//!   dialog's own command and guards ([`TrashButton::retry`]), drawn unavailable whenever those guards refuse
//!   ([`listing::Snapshot::retry_target`]); a refusal the Trash keeps for that checkout shows on the notice too. Open Trash
//!   closes the panel and opens the Trash dialog through its entry point ([`TrashButton::open`]) naming the bell as
//!   where closing it returns focus.
//!
//! Host names, checkout names, repositories and reasons are host text, drawn through the host-text element.
//!
//! [`Expansion`]: crate::session_list::host_rail::expansion::Expansion

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::LIST_SESSIONS_CAP;
use farhelm_proto::notices::{Notice, NoticeContent};
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px, svg,
};
use serde_json::json;

use crate::assets::Icon;
use crate::components::{Button, HoverHelp, MenuHeader, Role, Tier, rail_entry};
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::local_time::{self, Zone};
use crate::notices_bell::{entry_frame, hairline, list_panel};
use crate::session_list::host_rail::expansion::Expansion;
use crate::theme::{self, Palette, Token};
use crate::trash::{TrashButton, listing};
use crate::{operations, session};

#[cfg(test)]
mod tests;

/// The bell's element id, the old `.rail-notices-toggle`.
pub(crate) const BELL_ID: &str = "rail-notices-toggle";

/// The bell's unread count's id (the old `.rail-notices-count`), drawn only while something is unread.
pub(crate) const COUNT_ID: &str = "rail-notices-count";

/// The open panel's id, the old `#rail-notices-panel`.
pub(crate) const PANEL_ID: &str = "rail-notices-panel";

/// The footer's Clear resolved and Close.
pub(crate) const CLEAR_ID: &str = "rail-notices-clear";
pub(crate) const CLOSE_ID: &str = "rail-notices-close";

/// The id of the line that shows a refused Clear resolved.
pub(crate) const CLEAR_REFUSAL_ID: &str = "rail-notices-clear-refusal";

/// The id prefixes of a notice's entry, its Retry now and its Open Trash; the rest is the notice's id.
pub(crate) const ENTRY_ID_PREFIX: &str = "rail-notice-";
pub(crate) const RETRY_ID_PREFIX: &str = "rail-notice-retry-";
pub(crate) const OPEN_TRASH_ID_PREFIX: &str = "rail-notice-open-trash-";
pub(crate) const SEND_FEEDBACK_ID_PREFIX: &str = "rail-notice-send-feedback-";

/// `.rail-notices-toggle .bell-glyph`: the bell at the rail's 16-pixel size, the gear's and the `?`'s.
const GLYPH: f32 = 16.;

/// `.host-row-menu-flyout.rail-notices-flyout`'s width.
const PANEL_WIDTH: f32 = 360.;

/// The panel's text size and line height: the old panel set neither, so its prose took the page's 16 pixels at the
/// browser's `line-height: normal`, about 1.2 for Inter (GPUI's own default, about 1.6, spaced the lines visibly wider).
const PANEL_TEXT: f32 = 16.;
const PANEL_LINE: f32 = 19.;

/// `.rail-notice-checkout` and `.rail-notice-time`: the small quiet lines, at the same `normal` line height.
const SMALL_TEXT: f32 = 11.;
const SMALL_LINE: f32 = 13.;

/// `.rail-notices-count`: the unread count's text size and line height, and its least width.
const COUNT_TEXT: f32 = 10.;
const COUNT_LINE: f32 = 13.;
const COUNT_WIDTH: f32 = 12.;

/// The panel's header summary, the old `MenuHeader`'s.
const SUMMARY: &str = "About your hosts and Farhelm";

// ---- What the bell and the panel say -------------------------------------------------------------------------------

/// Whether `notice` makes the bell loud: unread, and unresolved (the helm serves a resolved notice as read, and a
/// resolved one is quiet whatever its read mark says).
pub(crate) fn is_unread(notice: &Notice) -> bool {
    !notice.read && !notice.resolved
}

/// The bell's accessible name, the old toggle's: "Notifications: 2 unread", "Notifications: 0 unread".
pub(crate) fn bell_label(unread: usize) -> String {
    format!("Notifications: {unread} unread")
}

/// The bell's hover help: its name, then what it opens, as the old toggle's tooltip.
pub(crate) fn bell_help(unread: usize) -> String {
    format!(
        "{}; open notices about your hosts and Farhelm",
        bell_label(unread)
    )
}

/// The bell's ink: the inverse ink on the open look's accent fill, the strong red while something is unread
/// (`.rail-notices-toggle.loud`), and the rail's quiet `--fg-1` otherwise.
pub(crate) fn bell_ink(palette: &Palette, loud: bool, open: bool) -> Token {
    if open {
        palette.on_fill
    } else if loud {
        palette.danger_strong
    } else {
        palette.fg_1
    }
}

/// `notices` newest first: by when each was raised, then by id, as the helm serves them; ordered here too so the panel
/// does not depend on arrival order.
pub(crate) fn newest_first(mut notices: Vec<&Notice>) -> Vec<&Notice> {
    notices.sort_by_key(|notice| std::cmp::Reverse((notice.raised_at, notice.id)));
    notices
}

// ---- The bell ------------------------------------------------------------------------------------------------------

/// The notices bell at the top of the rail's foot, with the panel it opens (see the module docs). Created once by the
/// session list, which draws it in the foot for the window's life.
pub struct NoticesButton {
    /// The bell's focus handle, a tab stop, where Escape, the bell and Close return focus, and where closing the Trash
    /// dialog that Open Trash opened returns it.
    focus: FocusHandle,
    /// Where the bell was last drawn, for the panel to open beside: element state, so it is taken while rendering.
    anchor: Option<Anchor>,
    /// The panel this bell opened last; checked with the layer manager whenever it is read, so a panel closed some other
    /// way (Escape, a press outside, focus leaving, a reset) needs no bookkeeping here.
    panel: Option<LayerId>,
    /// The Trash, whose dialog and Retry the trash cleanup notice's actions use.
    trash: WeakEntity<TrashButton>,
    /// The host rail's expansion, the session list's, read as the bell draws: expanded, the bell is a row with its
    /// "Notifications" label beside the glyph (SPEC.md, "Session list": expanded, "the foot controls show labels"; the
    /// old label's word).
    rail: Entity<Expansion>,
}

impl NoticesButton {
    /// A bell with no panel open, whose notices' actions go to `trash`, at the foot of the rail whose expansion is
    /// `rail`.
    pub(crate) fn new(
        trash: WeakEntity<TrashButton>,
        rail: Entity<Expansion>,
        cx: &mut Context<Self>,
    ) -> Self {
        NoticesButton {
            focus: cx.focus_handle().tab_stop(true),
            anchor: None,
            panel: None,
            trash,
            rail,
        }
    }

    /// The panel this bell opened, if it is still open.
    fn open_panel(&self, window: &Window, cx: &App) -> Option<LayerId> {
        self.panel
            .filter(|panel| layers::is_open(*panel, window, cx))
    }

    /// The bell was pressed: close the panel if it is open, otherwise open it beside the bell with the mark this press
    /// took. Visible to the crate for the screenshot goldens, which open the panel without a click.
    pub(crate) fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_panel(window, cx).is_some() {
            self.close(window, cx);
            return;
        }
        let Some(anchor) = self.anchor.clone() else {
            return;
        };
        // Taken in the press's handler, after the focus coordinator counted this press or key.
        let mark = Mark::take(cx);
        let button = cx.weak_entity();
        let (bell, trash) = (self.focus.clone(), self.trash.clone());
        let view = cx.new(|cx| NoticesPanel::new(button, bell, trash, cx));
        let container = view.read(cx).container.clone();
        // The layer holds the view's only strong handle, so its release, which sends the read mark, is the panel's
        // close, whatever closed it. Focus starts on the panel itself, as the old panel took focus (`tabindex: -1`), so
        // a stray Enter presses nothing; Tab walks the panel's controls after the bell and on out of it, and focus
        // leaving it closes it: the old panel closed on `focusout` once focus was outside both it and its toggle.
        let popover = Popover::menu(view, container.clone(), anchor.beside(), container)
            .returns_to(ReturnTo::Control(self.focus.clone()))
            .tab_through();
        self.panel = layers::open_popover(popover, &mark, window, cx);
        cx.notify();
    }

    /// Close the panel if it is still open, which returns focus to the bell.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(panel) = self.open_panel(window, cx) {
            layers::close(panel, window, cx);
        }
        self.panel = None;
        cx.notify();
    }
}

impl Render for NoticesButton {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let anchor = Anchor::for_element("rail-notices-anchor", window, cx);
        self.anchor = Some(anchor.clone());
        let open = self.open_panel(window, cx).is_some();
        let rail_expanded = self.rail.read(cx).shown(cx);
        let unread = session::current_store(cx).map_or(0, |store| {
            store
                .read(cx)
                .collections()
                .ordered::<Notice>()
                .into_iter()
                .filter(|notice| is_unread(notice))
                .count()
        });
        let loud = unread > 0;
        let ink = theme::color(bell_ink(palette, loud, open));
        // `.host-rail-icon` with the count over its top right corner (`.rail-notices-count`).
        let glyph = div()
            .relative()
            .size(px(GLYPH))
            .child(
                svg()
                    .path(if loud { Icon::BellFilled } else { Icon::Bell }.path())
                    .size(px(GLYPH))
                    .text_color(ink),
            )
            .when(loud, |glyph| {
                glyph.child(
                    div()
                        .id(COUNT_ID)
                        .absolute()
                        .top(px(-6.))
                        .right(px(-8.))
                        .min_w(px(COUNT_WIDTH))
                        .text_size(px(COUNT_TEXT))
                        .line_height(px(COUNT_LINE))
                        .font_weight(FontWeight::BOLD)
                        .text_color(ink)
                        .child(unread.to_string()),
                )
            });
        let button = Button::new(
            BELL_ID,
            bell_label(unread),
            HoverHelp::tooltip(bell_help(unread)),
            Role::Button,
        )
        .tier(Tier::Rail)
        .expanded(open)
        .focus(&self.focus)
        .content(rail_entry(
            glyph,
            rail_expanded.then(|| {
                div()
                    .text_color(ink)
                    .child("Notifications")
                    .into_any_element()
            }),
        ))
        .when(rail_expanded, Button::rail_row)
        .on_click(cx.listener(|bell, _, window, cx| bell.toggle(window, cx)));
        div()
            .relative()
            .when(rail_expanded, |wrapper| wrapper.w_full())
            .child(button)
            .child(anchor.track())
    }
}

// ---- The panel -----------------------------------------------------------------------------------------------------

/// The open panel: drawn from the store and the Trash every frame; what it keeps is only what its close and Clear
/// resolved need.
struct NoticesPanel {
    /// The bell that opened it, which closes it before opening another layer.
    button: WeakEntity<NoticesButton>,
    /// The control that receives focus when a dialog opened from the panel closes.
    bell: FocusHandle,
    trash: WeakEntity<TrashButton>,
    /// The panel's own focus handle, where focus starts.
    container: FocusHandle,
    /// The entries' scroll position, for a panel taller than the window allows.
    scroll: ScrollHandle,
    /// The unread notices the last frame drew: what the close marks read.
    unread_shown: Vec<i64>,
    /// The resolved notices the last frame drew: what Clear resolved removes.
    resolved_shown: Vec<i64>,
    /// Whether Clear resolved's request is waiting for its answer; the button is unavailable meanwhile.
    clearing: bool,
    /// The helm's words for the last refused Clear resolved, until the next one.
    refusal: Option<HostText>,
    /// Sends the read mark when the view is released, which is when the panel closes.
    _mark_on_close: Subscription,
}

impl NoticesPanel {
    fn new(
        button: WeakEntity<NoticesButton>,
        bell: FocusHandle,
        trash: WeakEntity<TrashButton>,
        cx: &mut Context<Self>,
    ) -> Self {
        NoticesPanel {
            button,
            bell,
            trash,
            container: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            unread_shown: Vec::new(),
            resolved_shown: Vec::new(),
            clearing: false,
            refusal: None,
            // Kept in the view: GPUI takes an entity's release callbacks before it drops the entity.
            _mark_on_close: cx.on_release(|panel: &mut NoticesPanel, cx| {
                let ids = std::mem::take(&mut panel.unread_shown);
                send_mark("read", ids, cx, |answer, _| {
                    if let Err(reason) = answer {
                        tracing::warn!(?reason, "could not mark the shown notices read");
                    }
                });
            }),
        }
    }

    /// Clear resolved was pressed: remove the resolved notices the last frame drew. The helm pushes what is left
    /// before it answers, so a success needs nothing here; a refusal is kept to show.
    fn clear(&mut self, cx: &mut Context<Self>) {
        if self.clearing || self.resolved_shown.is_empty() {
            return;
        }
        let panel = cx.weak_entity();
        self.refusal = None;
        self.clearing = send_mark(
            "clear_resolved",
            self.resolved_shown.clone(),
            cx,
            move |answer, cx| {
                // The panel may have closed meanwhile; then nobody is told.
                let _ = panel.update(cx, |panel, cx| {
                    panel.clearing = false;
                    panel.refusal = answer.err();
                    cx.notify();
                });
            },
        );
        cx.notify();
    }
}

/// `POST /api/notices/marks` with `action` (`read` or `clear_resolved`) for `ids`, as a keyless send handing `then` the
/// answer. Nothing is sent for no ids, and then `false` comes back, as it does while there is no UI session.
fn send_mark(
    action: &str,
    ids: Vec<i64>,
    cx: &mut App,
    then: impl FnOnce(Result<Option<serde_json::Value>, HostText>, &mut App) + 'static,
) -> bool {
    if ids.is_empty() {
        return false;
    }
    let Ok(path) = ApiPath::api(&["notices", "marks"]) else {
        return false;
    };
    operations::send(
        Method::Post,
        path,
        Some(json!({ "action": action, "ids": ids })),
        "POST /api/notices/marks",
        cx,
        then,
    )
}

impl Render for NoticesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let panel = list_panel(PANEL_ID, px(PANEL_WIDTH), window, cx)
            .aria_label(SharedString::from("Notifications"))
            .text_size(px(PANEL_TEXT))
            .line_height(px(PANEL_LINE))
            .text_color(theme::color(palette.fg_0))
            .child(MenuHeader::app("Notifications", SUMMARY));
        let Some(store) = session::current_store(cx) else {
            return panel;
        };
        // Taken out of the store first, since asking the Trash for its state needs the app mutably.
        let (notices, state) = {
            let collections = store.read(cx).collections();
            let notices: Vec<(Notice, Option<HostText>)> =
                newest_first(collections.ordered::<Notice>())
                    .into_iter()
                    .map(|notice| {
                        let host = notice
                            .host
                            .and_then(|host| collections.get::<HostView>(&HostView::key(host)))
                            .map(|host| HostText::from_host(host.name.as_str()));
                        (notice.clone(), host)
                    })
                    .collect();
            // What the collection's own state says, above the entries, as the old panel's lines did.
            let state = match collections
                .meta(Collection::Notices)
                .map(|meta| &meta.availability)
            {
                None => {
                    Some(HostSentence::new("rail-notices-loading").text("Loading notifications…"))
                }
                Some(Availability::Unavailable { error }) => Some(
                    HostSentence::new("rail-notices-error")
                        .text("Could not refresh notifications: ")
                        .host(&HostText::from_host(error.as_str())),
                ),
                Some(_) if notices.is_empty() => {
                    Some(HostSentence::new("rail-notices-empty").text("No notifications"))
                }
                Some(_) => None,
            };
            (notices, state)
        };
        self.unread_shown = notices
            .iter()
            .filter(|(notice, _)| is_unread(notice))
            .map(|(notice, _)| notice.id)
            .collect();
        self.resolved_shown = notices
            .iter()
            .filter(|(notice, _)| notice.resolved)
            .map(|(notice, _)| notice.id)
            .collect();
        // One look at the Trash per frame, which every notice's actions read.
        let trash = self
            .trash
            .upgrade()
            .map(|trash| trash.update(cx, |trash, cx| trash.snapshot(cx)));
        let zone = local_time::zone(cx);
        let mut body = div()
            .id("rail-notices-entries")
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_y_scroll();
        for (at, (notice, host)) in notices.into_iter().enumerate() {
            if at > 0 {
                body = body.child(hairline(palette));
            }
            body = body.child(entry(
                &notice,
                host,
                trash.as_ref(),
                &self.button,
                &self.bell,
                &self.trash,
                zone,
                palette,
            ));
        }
        let clear = Button::new(
            CLEAR_ID,
            "Clear resolved",
            HoverHelp::tooltip("Remove only the resolved notifications shown in this panel"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(self.resolved_shown.is_empty() || self.clearing)
        .on_click(cx.listener(|panel, _, _, cx| panel.clear(cx)));
        let button = self.button.clone();
        let close = Button::new(
            CLOSE_ID,
            "Close",
            HoverHelp::tooltip("Close notifications and mark the shown items read"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .on_click(move |_, window, cx| {
            let _ = button.update(cx, |button, cx| button.close(window, cx));
        });
        panel
            .children(state.map(|state| div().flex_none().mx(px(12.)).my(px(10.)).child(state)))
            .children(self.refusal.as_ref().map(|refusal| {
                div()
                    .flex_none()
                    .mx(px(12.))
                    .my(px(10.))
                    .text_color(theme::color(palette.danger_strong))
                    .child(HostSentence::new(CLEAR_REFUSAL_ID).host(refusal))
            }))
            .child(focus::scroll_region(body, &self.scroll))
            // `.rail-notices-footer`: Clear resolved and Close at the two ends, under a hairline.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .justify_between()
                    .py(px(10.))
                    .px(px(12.))
                    .border_t_1()
                    .border_color(theme::color(palette.border_row))
                    .child(clear)
                    .child(close),
            )
    }
}

/// What a trash cleanup notice's two actions can do now, from the Trash's state at this frame.
struct Actions {
    /// Retry now would go ahead ([`listing::Snapshot::retry_target`]).
    retry: bool,
    /// The refusal the Trash keeps for this checkout, from its last delete or retry, if any.
    refused: Option<HostText>,
}

impl Actions {
    /// The actions of `notice` with the Trash as `trash` shows it this frame; with no Trash (the window is going away)
    /// nothing is offered.
    fn of(notice: &Notice, checkout_id: &str, trash: Option<&listing::Snapshot>) -> Actions {
        let Some((trash, host)) = trash.zip(notice.host) else {
            return Actions {
                retry: false,
                refused: None,
            };
        };
        let refused = trash
            .hosts
            .iter()
            .find(|entry| entry.id == host)
            .and_then(|host| {
                host.notices
                    .iter()
                    .find(|kept| kept.ids.iter().any(|id| id == checkout_id))
            })
            .map(|kept| kept.message.clone());
        Actions {
            retry: !notice.resolved && trash.retry_target(host, checkout_id).is_some(),
            refused,
        }
    }
}

/// Draw one record and the actions appropriate to its condition. A trash cleanup notice (`.rail-notice`) shows the host
/// and what happened, the checkout's repository and last measured size, the reason in red (greyed with the rest once
/// resolved), when it last failed (to the minute, in the viewer's local time), and for an unresolved notice the promise
/// of a retry at the host's supervisor restart with Retry now beside Open Trash. A session-limit notice says which list
/// was cut and offers Send feedback. A host the registry does not list is named "Unknown host", as the old panel said;
/// trash reports keep their original host name while the registry load is pending.
#[allow(
    clippy::too_many_arguments,
    reason = "one notice's parts and the handles its actions reach, passed through once"
)]
fn entry(
    notice: &Notice,
    host: Option<HostText>,
    trash_snapshot: Option<&listing::Snapshot>,
    button: &WeakEntity<NoticesButton>,
    bell: &FocusHandle,
    trash: &WeakEntity<TrashButton>,
    zone: Zone,
    palette: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let (checkout_id, name, repository, bytes, reason, at) = match &notice.content {
        NoticeContent::StuckTrash {
            checkout_id,
            name,
            repository,
            bytes,
            reason,
            at,
        } => (checkout_id, name, repository, bytes, reason, at),
        NoticeContent::HostSessionLimit | NoticeContent::FleetSessionLimit => {
            return session_limit_entry(notice, host, button, bell, palette);
        }
    };
    let actions = Actions::of(notice, checkout_id, trash_snapshot);
    let id = notice.id;
    let quiet = |line: gpui::Div| {
        line.text_size(px(SMALL_TEXT))
            .line_height(px(SMALL_LINE))
            .text_color(theme::color(palette.fg_2))
    };
    let mut title = HostSentence::new(format!("{ENTRY_ID_PREFIX}{id}-title"));
    title = match &host {
        Some(host) => title.host(host),
        None => title.text("Unknown host"),
    };
    let size = match bytes {
        Some(bytes) => format!(" · {} last measured", listing::space(*bytes, false)),
        None => " · size unknown".to_owned(),
    };
    let reason_ink = if notice.resolved {
        palette.fg_1
    } else {
        palette.danger_strong
    };
    let content = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .when(notice.resolved, |content| {
            content.text_color(theme::color(palette.fg_1))
        })
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.text(": trash could not be freed")),
        )
        .when(notice.resolved, |content| {
            content.child(div().font_weight(FontWeight::SEMIBOLD).child("Resolved"))
        })
        .child(
            HostSentence::new(format!("{ENTRY_ID_PREFIX}{id}-name"))
                .text("Farhelm could not finish removing ")
                .host(&HostText::from_host(name.as_str()))
                .text("."),
        )
        .child(quiet(div()).child(
            HostSentence::new(format!("{ENTRY_ID_PREFIX}{id}-checkout"))
                .host(&HostText::from_host(repository.as_str()))
                .text(size),
        ))
        .child(
            div()
                .text_color(theme::color(reason_ink))
                .child(host_text::element(&HostText::from_host(reason.as_str()))),
        )
        .child(quiet(div()).child(
            HostSentence::new(format!("{ENTRY_ID_PREFIX}{id}-time"))
                .text("Last failed ")
                .mono(&HostText::from_host(local_time::minute(*at, zone))),
        ))
        .when(!notice.resolved, |content| {
            content.child(
                "Farhelm tries again when this host\u{2019}s supervisor restarts. You can also retry now.",
            )
        })
        .child(actions_row(notice, checkout_id, &actions, button, bell, trash))
        .children(actions.refused.map(|refused| {
            div()
                .text_color(theme::color(palette.danger_strong))
                .child(HostSentence::new(format!("{ENTRY_ID_PREFIX}{id}-refused")).host(&refused))
        }));
    entry_frame(
        SharedString::from(format!("{ENTRY_ID_PREFIX}{id}")),
        is_unread(notice),
        palette,
        content,
    )
}

/// Explain which list was cut and take the user to the regular feedback
/// dialog. Closing the panel also uses its usual read mark and return target.
fn session_limit_entry(
    notice: &Notice,
    host: Option<HostText>,
    button: &WeakEntity<NoticesButton>,
    bell: &FocusHandle,
    palette: &Palette,
) -> gpui::Stateful<gpui::Div> {
    let id = notice.id;
    let mut title = HostSentence::new(format!("{ENTRY_ID_PREFIX}{id}-title"));
    let detail = if notice.content == NoticeContent::HostSessionLimit {
        title = match host.as_ref() {
            Some(host) => title.host(host),
            None => title.text("Unknown host"),
        }
        .text(": session limit reached");
        if notice.resolved {
            format!(
                "This host had more than {LIST_SESSIONS_CAP} sessions. Farhelm showed only its newest {LIST_SESSIONS_CAP}."
            )
        } else {
            format!(
                "This host has more than {LIST_SESSIONS_CAP} sessions. The list shows only its newest {LIST_SESSIONS_CAP}; Farhelm cannot reach the others."
            )
        }
    } else {
        title = title.text("Farhelm session limit reached");
        if notice.resolved {
            format!("The list across your hosts showed only {LIST_SESSIONS_CAP} sessions.")
        } else {
            format!("The list across your hosts shows only {LIST_SESSIONS_CAP} sessions.")
        }
    };
    let (button, bell) = (button.clone(), bell.clone());
    let feedback = Button::new(
        format!("{SEND_FEEDBACK_ID_PREFIX}{id}"),
        "Send feedback",
        HoverHelp::tooltip("Open the Farhelm feedback dialog"),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .on_click(move |_, window, cx| {
        let mark = Mark::take(cx);
        let _ = button.update(cx, |button, cx| button.close(window, cx));
        crate::feedback::open_returning(ReturnTo::Control(bell.clone()), None, &mark, window, cx);
    });
    let content = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .when(notice.resolved, |content| {
            content.text_color(theme::color(palette.fg_1))
        })
        .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
        .when(notice.resolved, |content| {
            content.child(div().font_weight(FontWeight::SEMIBOLD).child("Resolved"))
        })
        .child(detail)
        .child("Farhelm is built for tens of sessions. Send feedback if you need this many or think a bug caused it.")
        .child(div().flex().child(feedback));
    entry_frame(
        SharedString::from(format!("{ENTRY_ID_PREFIX}{id}")),
        is_unread(notice),
        palette,
        content,
    )
}

/// A notice's actions (`.rail-notice-actions`): Retry now, while the notice is unresolved, and Open Trash.
fn actions_row(
    notice: &Notice,
    checkout_id: &str,
    actions: &Actions,
    button: &WeakEntity<NoticesButton>,
    bell: &FocusHandle,
    trash: &WeakEntity<TrashButton>,
) -> impl IntoElement {
    let (host, checkout) = (notice.host, checkout_id.to_owned());
    let retry_trash = trash.clone();
    let retry = host.filter(|_| !notice.resolved).map(|host| {
        Button::new(
            format!("{RETRY_ID_PREFIX}{}", notice.id),
            "Retry now",
            HoverHelp::tooltip(if actions.retry {
                "Retry removing this checkout's files with the usual cleanup checks"
            } else {
                "Retry needs a connected host, a current stuck entry and no other operation in progress"
            }),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(!actions.retry)
        .on_click(move |_, _, cx| {
            let _ = retry_trash.update(cx, |trash, cx| trash.retry(host, &checkout, cx));
        })
    });
    let (button, bell, trash) = (button.clone(), bell.clone(), trash.clone());
    let open_trash = Button::new(
        format!("{OPEN_TRASH_ID_PREFIX}{}", notice.id),
        "Open Trash",
        HoverHelp::tooltip("Open Trash to inspect archived checkouts on all hosts"),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .on_click(move |_, window, cx| {
        // Taken in the handler, after the focus coordinator counted this press or key.
        let mark = Mark::take(cx);
        let _ = button.update(cx, |button, cx| button.close(window, cx));
        let returns_to = ReturnTo::Control(bell.clone());
        let _ = trash.update(cx, |trash, cx| trash.open(returns_to, &mark, window, cx));
    });
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(8.))
        .children(retry)
        .child(open_trash)
}
