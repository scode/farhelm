//! A session row's notification bell and the list of the session's notifications it opens.
//!
//! SPEC.md, "Status" (the bell paragraphs) and "Session list" (the bell sits on the row's first line, before the last
//! activity time). Ported from the old app's `list/bell.rs` (the bell, its accessible name, the list, `notification_runs`
//! and `DOCS_LINK_PREFIX`), its `ListView`'s one read-on-close effect and clear (`list/view.rs`), and the stylesheet's
//! `.session-bell*` and `.session-row-bell` rules, all at reference commit `24bc0580`. Here's the TLDR of the rules:
//!
//! - **Only a row with notifications has a bell.** [`bell`] draws nothing for a session with none, so its row keeps its
//!   full title width. The bell is grey while everything in it is read and red and filled while anything is unread
//!   ([`is_unread`]: unresolved and above the helm's read mark); its accessible name and hover help say how many are
//!   unread, since a screen reader cannot hear the red.
//! - **Everything is read from the store, every frame.** The bell and the list draw from the session's pushed entry
//!   (its `notifications` and `notifications_read_through`), never from a copy. What the open list keeps of its own is
//!   only what its close needs: the newest sequence number the last frame drew ([`NotificationList::shown`]). So an
//!   entry pushed while the list is open is marked read by the close only once a frame has shown it; one that lands in
//!   the same moment as the close, before any frame drew it, stays unread and makes the bell loud again.
//! - **The bell is its own control inside the row.** It sits inside the row's open control, before the age, and keeps
//!   its press from it, so clicking the bell never opens or selects the session (the old app laid it over the open
//!   button for the same reason). It is a real button: Tab reaches it and Enter or Space opens the list, since nothing
//!   else offers the list by keyboard.
//! - **The list is a popover beside the bell.** It opens through the layer manager with the mark the click took,
//!   placed beside the bell as the old flyout was (left edge 10 pixels past the bell, so it reaches past the sidebar
//!   rather than being clipped by it), in the row menu's surface and header look, 288 pixels wide. Focus starts on the
//!   list itself, not on Clear all, so a stray Enter clears nothing; Tab walks its links and Clear all as if they stood
//!   right after the bell and goes on out of it, which closes it, as the old list closed when focus left it. Escape and
//!   the bell again close it with focus back on the bell.
//! - **Every close ends in one read mark.** However the list closes (Escape, a press outside it, the bell again, focus
//!   leaving it, the bell going away with its row, a reset), the layer manager drops its view, and the view's own
//!   release sends `PUT /api/sessions/{id}/notifications/read` through the newest entry it showed, when that is above
//!   the read mark the store holds by then. Watching the release rather than marking at each close site is what keeps a
//!   close path added later from forgetting the mark, the old `ListView`'s reason for its one effect. The mark is a
//!   keyless send and best effort: a failure is only logged (SPEC.md, "Errors and diagnostics"), and the bell stays
//!   loud until the list is next closed, which tries again.
//! - **Clear all is something the user asked for.** It sends `PUT /api/sessions/{id}/notifications/cleared` through
//!   the newest entry shown, as a keyless send. The helm pushes the emptied row before it answers, so on success the
//!   bell goes, and the list with it, as its anchor is no longer drawn. A refusal is shown in the list, under the
//!   button (SPEC.md: a failed clear "surfaces like any other operation").
//! - **Host text, with one exception.** An entry's text is the supervisor's, so it is drawn through host text, escaped
//!   and direction-isolated, except a link to a page of Farhelm's docs ([`DOCS_LINK_PREFIX`], [`notification_runs`]),
//!   which is a link button that opens through `cx.open_url`, the opener the app's other docs links use. Any other
//!   address stays plain text, because a host's supervisor must not be able to put a link to anywhere it likes in front
//!   of the user.
//!
//! The rail's notices bell ([`crate::rail_notices`]) is a separate surface with state of its own, which draws its panel
//! with this list's pieces: the floating panel ([`list_panel`]), an entry's frame with its red edge ([`entry_frame`])
//! and the hairline between entries ([`hairline`]).

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::SessionNotification;
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px, svg,
};
use serde_json::json;

use crate::assets::Icon;
use crate::components::{Button, HoverHelp, MenuHeader, Role, StatusMark, Tier};
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::local_time::{self, Zone};
use crate::session_list::age;
use crate::theme::{self, Palette, Token, metrics};
use crate::{operations, session, ticker};

#[cfg(test)]
mod tests;

/// The id prefix of a session row's bell; the rest is the session's key. Deliberately not under the row's own prefix
/// (`session-row-`), which tests and the bridge use to find the rows' open controls.
pub(crate) const BELL_ID_PREFIX: &str = "session-bell-";

/// The open list's panel id.
pub(crate) const LIST_ID: &str = "notification-list";

/// The list's Clear all button's id.
pub(crate) const CLEAR_ID: &str = "notification-clear";

/// The id of the line under Clear all that shows a refused clear.
pub(crate) const CLEAR_REFUSAL_ID: &str = "notification-clear-refusal";

/// The id prefix of one entry of the list; the rest is its sequence number, which is unique within the session.
pub(crate) const ENTRY_ID_PREFIX: &str = "notification-entry-";

/// The id prefix of an entry's age; the rest is the entry's sequence number.
pub(crate) const AGE_ID_PREFIX: &str = "notification-age-";

/// The id prefix of a docs link in an entry's text; the rest is the entry's sequence number, a dash and the link's
/// place among the text's runs.
pub(crate) const LINK_ID_PREFIX: &str = "notification-link-";

/// The only place notification text may link to: a page of Farhelm's own docs. A fixed scheme, host and path start,
/// so nothing after it can point the link at another site (see [`notification_runs`] for what may follow it).
pub(crate) const DOCS_LINK_PREFIX: &str = "https://farhelm.io/docs/";

/// The bell's box: `.session-row-bell`'s 10 by 12 pixels, the row's glyph cell (`.sidebar-glyph`). The 12-unit drawing
/// is a 10-pixel square centred in it, as the old `viewBox 0 0 12 12` in that box drew it.
const BELL_WIDTH: f32 = 10.;
const BELL_HEIGHT: f32 = 12.;

/// The gap between the bell and the age after it (`.session-row-bell`'s `margin-right`).
const BELL_GAP: f32 = 4.;

/// The list's width, `.host-row-menu-flyout.session-bell-flyout`'s: wider than the row menu's 246 pixels because its
/// entries are sentences rather than command names.
const LIST_WIDTH: f32 = 288.;

/// The room a floating panel keeps free below the height of the window's content area
/// ([`crate::desktop::linux_title_bar::content_area`]): a panel is at most that height less this, and one taller
/// scrolls ([`window_cap`]). The old flyouts' `max-height: calc(100vh - 16px)` took it from the viewport, which was the
/// old page and so is the content area here; the name still says window from then.
const WINDOW_ROOM: f32 = 16.;

/// An entry's text size (`.session-bell-text`).
const ENTRY_TEXT: f32 = 13.;

/// The width of a new entry's red edge (`.session-bell-entry`'s `border-left`).
const NEW_EDGE: f32 = 3.;

// ---- What the bell and the list say -------------------------------------------------------------------------------

/// Whether `notification` is unread under the helm's read mark `read_through`: unresolved and newer than the mark.
/// It decides both whether the bell is loud and which entries the list marks New. A resolved entry counts as read even
/// when nobody opened the list (SPEC.md: resolved entries "never make the bell loud or appear as new").
pub(crate) fn is_unread(notification: &SessionNotification, read_through: u64) -> bool {
    !notification.resolved && notification.seq > read_through
}

/// How many of `row`'s notifications are unread ([`is_unread`]); the bell is loud exactly when this is nonzero.
pub(crate) fn unread(row: &SessionRow) -> usize {
    row.info
        .notifications
        .iter()
        .filter(|notification| is_unread(notification, row.notifications_read_through))
        .count()
}

/// "1 unread", "3 unread" or "none unread", shared by the bell's accessible name and its hover help so the two never
/// disagree about the count.
fn unread_phrase(unread: usize) -> String {
    match unread {
        0 => "none unread".to_owned(),
        count => format!("{count} unread"),
    }
}

/// "Notifications for <title>", the open list's accessible name and the start of the bell's. The title is a host value
/// (agents may rename any session), so it is named escaped and cut short, as the row menu's name is
/// ([`MenuHeader::clamp_name`]).
fn list_label(title: &HostText) -> String {
    format!("Notifications for {}", MenuHeader::clamp_name(title))
}

/// The bell's accessible name: whose bell it is and how many of its notifications are unread ("Notifications for
/// api-refactor: 2 unread"), the old `bell_label`.
pub(crate) fn bell_label(title: &HostText, unread: usize) -> String {
    format!("{}: {}", list_label(title), unread_phrase(unread))
}

/// The bell's hover help, the old tooltip: the count, then what a click does.
pub(crate) fn bell_help(unread: usize) -> String {
    format!(
        "Notifications: {}; click to list them",
        unread_phrase(unread)
    )
}

/// `notifications` newest first, by sequence number, which only grows: the list's order (SPEC.md: "The list is newest
/// first"). The helm already sends them so; ordering here keeps the list right whatever order arrives.
fn newest_first(notifications: &[SessionNotification]) -> Vec<&SessionNotification> {
    let mut ordered: Vec<&SessionNotification> = notifications.iter().collect();
    ordered.sort_by_key(|notification| std::cmp::Reverse(notification.seq));
    ordered
}

/// The bell's ink. On the selected row's accent fill it is the inverse ink whatever it says (the old
/// `.session-row.selected .session-row-bell`, which outranked the loud red too; the filled drawing still tells loud
/// from quiet there). Elsewhere it is the strong red while anything is unread, the primary ink while its list is open,
/// and the quiet `--fg-2` otherwise, which the pointer turns to the primary ink ([`bell_hover_ink`]).
pub(crate) fn bell_ink(palette: &Palette, loud: bool, selected: bool, open: bool) -> Token {
    if selected {
        palette.on_fill
    } else if loud {
        palette.danger_strong
    } else if open {
        palette.fg_0
    } else {
        palette.fg_2
    }
}

/// The bell's ink under the pointer, or `None` when the pointer changes nothing: only a quiet bell off the selected
/// row brightens (`.session-row-bell:hover`); a loud one stays red and a selected one keeps the inverse ink.
pub(crate) fn bell_hover_ink(palette: &Palette, loud: bool, selected: bool) -> Option<Token> {
    (!loud && !selected).then_some(palette.fg_0)
}

/// The bell's drawing: filled while anything is unread, outlined otherwise.
pub(crate) fn bell_icon(loud: bool) -> Icon {
    if loud { Icon::BellFilled } else { Icon::Bell }
}

// ---- Links in notification text -----------------------------------------------------------------------------------

/// One run of a notification's text, as [`notification_runs`] cuts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextRun<'a> {
    /// Host text, drawn escaped like the rest of the notification.
    Plain(&'a str),
    /// A docs link, drawn as a link with the address as its words.
    DocsLink(&'a str),
}

/// Cut `text` into plain runs and docs links.
///
/// A docs link is a run of non-whitespace that starts with [`DOCS_LINK_PREFIX`], minus any trailing sentence
/// punctuation (so "see <url>." links the address, not the full stop), whose remainder after the prefix uses only ASCII
/// letters and digits and `-._~/#?=&+`, and whose path has no `.` or `..` segment. Anything else stays plain text
/// whole: a run with a quote, angle bracket, backslash, percent sign or invisible character in it, and one like
/// `https://farhelm.io/docs/../api/` that a browser would resolve to a page outside the docs. Leaving out `%` is what
/// makes the dot-segment check enough, since `%2e` is a dot to a browser. This is a narrow recogniser for links
/// Farhelm itself writes, not a URL parser, and refusing an odd case costs only a link. Text with no docs link comes
/// back as a single plain run, empty text included.
///
/// The narrow character set also means a link's address holds nothing host text's escape table would change, so the
/// link may show the address as its words without escaping it.
///
/// Ported unchanged from the old app's `notification_runs`.
pub(crate) fn notification_runs(text: &str) -> Vec<TextRun<'_>> {
    let mut runs = Vec::new();
    let mut plain_from = 0;
    let mut search_from = 0;
    while let Some(found) = text[search_from..].find(DOCS_LINK_PREFIX) {
        let start = search_from + found;
        let word_end = text[start..]
            .find(char::is_whitespace)
            .map_or(text.len(), |offset| start + offset);
        // The prefix ends in '/', which is not trimmed, so the link always keeps at least the whole prefix.
        let url = text[start..word_end].trim_end_matches(['.', ',', ';', ':', '!', '?', ')']);
        if is_plain_docs_tail(&url[DOCS_LINK_PREFIX.len()..]) {
            if plain_from < start {
                runs.push(TextRun::Plain(&text[plain_from..start]));
            }
            runs.push(TextRun::DocsLink(url));
            plain_from = start + url.len();
        }
        search_from = word_end;
    }
    if plain_from < text.len() || runs.is_empty() {
        runs.push(TextRun::Plain(&text[plain_from..]));
    }
    runs
}

/// Whether `tail`, what follows [`DOCS_LINK_PREFIX`] in a candidate link, is one [`notification_runs`] may link: only
/// the narrow character set, and no `.` or `..` segment in the path (the part before any `?` or `#`), so the link
/// cannot leave the docs once a browser resolves it.
fn is_plain_docs_tail(tail: &str) -> bool {
    let allowed = tail
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"-._~/#?=&+".contains(&byte));
    let path = tail.split(['?', '#']).next().unwrap_or("");
    allowed
        && !path
            .split('/')
            .any(|segment| segment == "." || segment == "..")
}

// ---- The bell ------------------------------------------------------------------------------------------------------

/// What a drawn bell keeps between frames: the list it opened last. Kept in the bell's keyed element state, so it lives
/// as long as the row draws the bell; the list closes with its anchor when the bell stops being drawn, so nothing here
/// outlives what it is about. The layer is checked with the layer manager whenever it is read, so a list closed some
/// other way (Escape, a press outside, focus leaving) needs no bookkeeping here.
struct BellState {
    open: Option<LayerId>,
}

/// The bell of `key`'s row, or `None` when the session has no notifications (or is not in the store), and the row
/// draws no bell. `selected` is whether the row is the selected one, whose accent fill takes the inverse ink. Called
/// from the session list's item callback, once per drawn row and frame: the bell's anchor, focus handle and state are
/// element state, so they are made while the row's elements are.
///
/// The bell keeps a primary press to itself, so the row's open control around it neither arms its click nor applies
/// its focus rule: clicking the bell never opens or selects the session. Its Enter and Space stop at the button too.
pub(crate) fn bell(
    key: &str,
    selected: bool,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    let store = session::current_store(cx)?;
    let (title, unread) = {
        let row = store.read(cx).collections().get::<SessionRow>(key)?;
        if row.info.notifications.is_empty() {
            return None;
        }
        (HostText::from_host(row.info.title.as_str()), unread(row))
    };
    let id: SharedString = format!("{BELL_ID_PREFIX}{key}").into();
    let anchor =
        Anchor::for_element(SharedString::from(format!("{id}-anchor")), window, cx).beside();
    let focus = focus::control_handle(SharedString::from(format!("{id}-focus-handle")), window, cx);
    let state = window.use_keyed_state(SharedString::from(format!("{id}-state")), cx, |_, _| {
        BellState { open: None }
    });
    let open = state
        .read(cx)
        .open
        .is_some_and(|layer| layers::is_open(layer, window, cx));
    let palette = theme::palette(cx);
    let loud = unread > 0;
    let group: SharedString = format!("{id}-group").into();
    let hover = bell_hover_ink(palette, loud, selected);
    let glyph = div()
        .w(px(BELL_WIDTH))
        .h(px(BELL_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(bell_icon(loud).path())
                .size(px(BELL_WIDTH))
                .flex_none()
                .text_color(theme::color(bell_ink(palette, loud, selected, open)))
                .when_some(hover.filter(|_| !open), |glyph, hover| {
                    glyph.group_hover(group.clone(), move |style| {
                        style.text_color(theme::color(hover))
                    })
                }),
        );
    let toggled = key.to_owned();
    let (opener, bell_focus) = (anchor.clone(), focus.clone());
    // The readout tier draws no padding, edge or fill, at rest or under the pointer, so the button is exactly the old
    // bell's box and the glyph's ink is all that changes; keyboard focus still draws every control's ring.
    // Beside the sidebar, as every session-row hover is (SPEC.md, "Session list": "including the summary card and
    // notification bell"), the ring in the on-fill ink on the selected row's accent fill.
    let help = HoverHelp::tooltip(bell_help(unread)).beside();
    let help = if selected { help.ring_on_fill() } else { help };
    let button = Button::new(id.clone(), bell_label(&title, unread), help, Role::Button)
        .tier(Tier::Readout)
        .expanded(open)
        .focus(&focus)
        .content(glyph)
        .on_click(move |_, window, cx| toggle(&toggled, &state, &opener, &bell_focus, window, cx));
    Some(
        div()
            .relative()
            .flex_none()
            .mr(px(BELL_GAP))
            .group(group)
            // GPUI runs a press's bubble listeners innermost first: the button has armed its own click and applied its
            // focus rule by now, and stopping the press here keeps the row's open control from doing either (the same
            // reason the status dot stops its press, `components::MarkToggle`).
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(button)
            .child(anchor.track())
            .into_any_element(),
    )
}

/// The bell of `key`'s row was activated: close its list if it is open, otherwise open the list beside the bell with
/// the mark this press took. Closing returns focus to the bell, which the list names, also on macOS, where the click
/// that opened it left the bell unfocused.
fn toggle(
    key: &str,
    state: &Entity<BellState>,
    anchor: &Anchor,
    bell: &FocusHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let open = state
        .read(cx)
        .open
        .filter(|layer| layers::is_open(*layer, window, cx));
    if let Some(layer) = open {
        layers::close(layer, window, cx);
        state.update(cx, |state, _| state.open = None);
        return;
    }
    // Taken in the press's handler, after the focus coordinator counted this press or key.
    let mark = Mark::take(cx);
    let view = cx.new(|cx| NotificationList::new(key, cx));
    let container = view.read(cx).container.clone();
    // The layer holds the view's only strong handle, so the view is released exactly when the list closes, which is
    // what sends the read mark ([`NotificationList::mark_read`]). Focus starts on the list itself, as the old panel
    // took focus, so a stray Enter cannot clear anything; Tab walks the list's links and Clear all after the bell and
    // goes on out of it, closing it, as the old list closed when focus left it for anything but its bell.
    let popover = Popover::menu(view, container.clone(), anchor.clone(), container)
        .returns_to(focus::ReturnTo::Control(bell.clone()))
        .tab_through();
    let opened = layers::open_popover(popover, &mark, window, cx);
    state.update(cx, |state, _| state.open = opened);
}

// ---- The list ------------------------------------------------------------------------------------------------------

/// The open list of one session's notifications. Drawn from the store every frame, so it follows pushes while it is
/// open; what it keeps is only what its close and its clear need.
pub(crate) struct NotificationList {
    /// The session's key, as the store and the routes name it.
    key: String,
    /// The list's own focus handle, where focus starts.
    container: FocusHandle,
    /// The entries' scroll position, for a list taller than the window allows.
    scroll: ScrollHandle,
    /// The newest sequence number the last drawn frame showed: what the close marks read and Clear all clears through.
    /// `None` until a frame has drawn an entry, and then nothing is marked.
    shown: Option<u64>,
    /// Whether Clear all's request is waiting for its answer; the button is busy meanwhile.
    clearing: bool,
    /// The helm's words for the last refused clear, shown under the button until the next try.
    refusal: Option<HostText>,
    /// The ticker's 30-second clock, so the entries' ages move on while the list is open.
    _clock: ticker::Clock,
    /// Sends the read mark when the view is released, which is when the list closes, whatever closed it.
    _mark_on_close: Subscription,
}

impl NotificationList {
    /// The list of `key`'s notifications, with nothing shown yet.
    fn new(key: &str, cx: &mut Context<Self>) -> Self {
        NotificationList {
            key: key.to_owned(),
            container: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            shown: None,
            clearing: false,
            refusal: None,
            _clock: ticker::clock(cx),
            // Kept in the view itself: GPUI takes an entity's release callbacks before it drops the entity, so the
            // subscription still fires as the view goes.
            _mark_on_close: cx.on_release(|list: &mut NotificationList, cx| list.mark_read(cx)),
        }
    }

    /// Mark read everything the list showed, as it closes: `PUT /api/sessions/{id}/notifications/read` through the
    /// newest entry the last frame drew, unless the store's read mark already covers it (nothing unread was shown, or
    /// the helm moved the mark meanwhile) or no frame drew an entry. A session gone from the store (its row filtered
    /// away while the list was open) is still marked: the list showed its entries.
    ///
    /// Best effort and silent (SPEC.md, "Errors and diagnostics"): a lost mark costs a bell that stays loud until the
    /// list is next closed, which tries again, so a refusal is only logged. On a reset there is no UI session to send
    /// through, and nothing goes.
    fn mark_read(&self, cx: &mut App) {
        let Some(through) = self.shown else {
            return;
        };
        let read_through = session::current_store(cx).and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<SessionRow>(&self.key)
                .map(|row| row.notifications_read_through)
        });
        if read_through.is_some_and(|read_through| through <= read_through) {
            return;
        }
        let Ok(path) = ApiPath::api(&["sessions", &self.key, "notifications", "read"]) else {
            tracing::warn!(
                "a session key that cannot be a path segment had its notifications left unread"
            );
            return;
        };
        let key = self.key.clone();
        operations::send(
            Method::Put,
            path,
            Some(json!({ "through": through })),
            "PUT /api/sessions/[session]/notifications/read",
            cx,
            move |answer, _| {
                if let Err(reason) = answer {
                    tracing::warn!(session = %key, ?reason, "could not mark the session's notifications read");
                }
            },
        );
    }

    /// Clear all was pressed: `PUT /api/sessions/{id}/notifications/cleared` through the newest entry shown. Nothing
    /// changes here on success, since the helm pushes the emptied row before it answers and the bell, and so the list,
    /// goes with it; a refusal is kept to show under the button. While the request waits the button is busy, so a
    /// second press sends nothing.
    fn clear(&mut self, cx: &mut Context<Self>) {
        let Some(through) = self.shown else {
            return;
        };
        if self.clearing {
            return;
        }
        let Ok(path) = ApiPath::api(&["sessions", &self.key, "notifications", "cleared"]) else {
            tracing::warn!(
                "a session key that cannot be a path segment had its notifications left uncleared"
            );
            return;
        };
        let list = cx.weak_entity();
        let sent = operations::send(
            Method::Put,
            path,
            Some(json!({ "through": through })),
            "PUT /api/sessions/[session]/notifications/cleared",
            cx,
            move |answer, cx| {
                // The list may have closed meanwhile (its bell went with a successful clear); then nobody is told.
                let _ = list.update(cx, |list, cx| {
                    list.clearing = false;
                    list.refusal = answer.err();
                    cx.notify();
                });
            },
        );
        self.clearing = sent;
        self.refusal = None;
        cx.notify();
    }
}

impl Render for NotificationList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        // `.host-row-menu-panel`: the floating surface with 4 pixels at top and bottom, never taller than the window
        // allows, its entries scrolling inside it.
        let panel = list_panel(LIST_ID, px(LIST_WIDTH), window, cx);
        let Some(store) = session::current_store(cx) else {
            return panel;
        };
        // A session gone from the store takes its row, and so the list's anchor, with it; until that frame there is
        // nothing to show.
        let Some(row) = store.read(cx).collections().get::<SessionRow>(&self.key) else {
            return panel;
        };
        let title = HostText::from_host(row.info.title.as_str());
        let entries = newest_first(&row.info.notifications);
        if let Some(newest) = entries.first() {
            self.shown = Some(self.shown.map_or(newest.seq, |shown| shown.max(newest.seq)));
        }
        let now = age::seconds(ticker::wall_now(cx));
        let zone = local_time::zone(cx);
        let read_through = row.notifications_read_through;
        let mut body = div()
            .id("notification-entries")
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_y_scroll();
        for (at, notification) in entries.iter().enumerate() {
            if at > 0 {
                body = body.child(hairline(palette));
            }
            body = body.child(entry(
                notification,
                is_unread(notification, read_through),
                now,
                zone,
                palette,
            ));
        }
        let clear = Button::new(
            CLEAR_ID,
            "Clear all",
            HoverHelp::tooltip("Clear: remove these notifications").beside_menu(),
            Role::Button,
        )
        .busy(self.clearing)
        .on_click(cx.listener(|list, _, _, cx| list.clear(cx)));
        panel
            .aria_label(SharedString::from(list_label(&title)))
            .child(MenuHeader::title("Notifications"))
            .child(focus::scroll_region(body, &self.scroll))
            // `.session-bell-actions`: Clear all at the right, under a hairline.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .justify_end()
                    .pt(px(6.))
                    .px(px(11.))
                    .pb(px(2.))
                    .border_t_1()
                    .border_color(theme::color(palette.border_dim))
                    .child(clear),
            )
            .children(self.refusal.as_ref().map(|refusal| {
                // Worded as the old row's error line was ("Clear notifications: <the helm's words>"), in its red.
                div()
                    .flex_none()
                    .px(px(11.))
                    .pt(px(4.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.danger_strong))
                    .child(
                        HostSentence::new(CLEAR_REFUSAL_ID)
                            .text("Clear notifications: ")
                            .host(refusal),
                    )
            }))
    }
}

/// One entry of the list (`.session-bell-entry`): its marks and age on a line, then its text. A new entry has a red
/// edge at its left and the word New, so the distinction does not rest on color alone; a resolved one is greyed and
/// says Resolved, and is never new. `now` is the viewer's clock in seconds, from which the age is counted; with no
/// usable clock or no stamp there is no age, as on a row. `zone` is the viewer's, for the age's full stamp.
fn entry(
    notification: &SessionNotification,
    unread: bool,
    now: Option<i64>,
    zone: Zone,
    palette: &Palette,
) -> impl IntoElement {
    let seq = notification.seq;
    // [`is_unread`] already passes over a resolved entry, so a resolved one is never also new.
    let new = unread;
    let ink = if notification.resolved {
        palette.fg_1
    } else {
        palette.fg_0
    };
    // `.session-bell-entry-meta`: the word, then the age, 6 pixels apart, in the small quiet text.
    let meta = div()
        .flex()
        .flex_row()
        .gap(px(6.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .when(notification.resolved, |meta| {
            meta.child(div().font_weight(FontWeight::SEMIBOLD).child("Resolved"))
        })
        .when(new, |meta| {
            meta.child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.danger_strong))
                    .child("New"),
            )
        })
        .children(age::of(notification.at, now, zone).map(|age| {
            // The row's buckets with "ago" after them, except "now"; the full stamp is the hover help, as the row's
            // age has it.
            let words = if age.short == "now" {
                age.short.clone()
            } else {
                format!("{} ago", age.short)
            };
            StatusMark::new(
                format!("{AGE_ID_PREFIX}{seq}"),
                age.full.clone(),
                HoverHelp::tooltip(age.full),
                Role::Label,
                words,
            )
            .ink(palette.fg_1)
            .monospace()
            .described(false)
        }));
    entry_frame(
        SharedString::from(format!("{ENTRY_ID_PREFIX}{seq}")),
        new,
        palette,
        div()
            .flex()
            .flex_col()
            .child(meta)
            .child(entry_text(notification, ink)),
    )
}

// ---- Pieces the rail's notices panel shares ------------------------------------------------------------------------

/// The floating panel a list of notifications is drawn on, under element id `id` and `width` wide: the row menus'
/// surface with 4 pixels at top and bottom (`.host-row-menu-panel`), a column never taller than the window allows
/// (`max-height: calc(100vh - 16px)`), named as a dialog, which the caller labels. The session bell's list and the
/// rail's notices panel both draw on it, at their old widths.
///
/// The height cap is taken from the window's content area ([`crate::desktop::linux_title_bar::content_area`]), the
/// old page's viewport, not from the whole window: the layer manager keeps every popover inside that area, so on
/// Linux, where the window also holds GPUI Kit's frame and the title bar, a panel capped by the window would be taller
/// than the area and be pushed past its bottom.
pub(crate) fn list_panel(
    id: &'static str,
    width: gpui::Pixels,
    window: &Window,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    layers::surface(cx)
        .id(id)
        .debug_selector(move || id.into())
        .role(Role::Dialog)
        .w(width)
        .max_h(window_cap(window))
        .flex()
        .flex_col()
        .py(px(4.))
}

/// The tallest a floating panel of this area may be in `window`: the height of its content area (inside a drawn Linux
/// frame and below the title bar; the whole window elsewhere) less [`WINDOW_ROOM`]. The session bell's list, the rail's
/// notices panel and the Help menu are capped at it and scroll what does not fit, so none of them runs past the content
/// area's edge at the smallest window size, where the layer manager can keep a popover inside that area only once it is
/// no taller than the area.
pub(crate) fn window_cap(window: &Window) -> gpui::Pixels {
    crate::desktop::linux_title_bar::content_area(window)
        .size
        .height
        - px(WINDOW_ROOM)
}

/// One entry's frame under element id `id` (`.session-bell-entry`): a red edge at its left while it is `new`, so the
/// distinction does not rest on a word alone, and the entry's padding around `content`, which the caller lays out.
pub(crate) fn entry_frame(
    id: SharedString,
    new: bool,
    palette: &Palette,
    content: impl IntoElement,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .flex_none()
        .child(
            div()
                .w(px(NEW_EDGE))
                .flex_none()
                .when(new, |edge| edge.bg(theme::color(palette.danger_strong))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pt(px(6.))
                .pb(px(6.))
                .pl(px(8.))
                .pr(px(11.))
                .child(content),
        )
}

/// The hairline between two entries (`.session-bell-entry + .session-bell-entry`), across the red edge too.
pub(crate) fn hairline(palette: &Palette) -> gpui::Div {
    div()
        .h(px(1.))
        .flex_none()
        .bg(theme::color(palette.border_dim))
}

/// An entry's text in `ink` (`.session-bell-text`), wrapping rather than widening the list. Text without a docs link is
/// one host text element; text with one is cut into runs ([`notification_runs`]), each its own element, the plain ones
/// host text and the links link buttons, wrapping as whole blocks.
fn entry_text(notification: &SessionNotification, ink: Token) -> impl IntoElement {
    let text = div()
        .mt(px(2.))
        .min_w_0()
        .text_size(px(ENTRY_TEXT))
        .text_color(theme::color(ink));
    let runs = notification_runs(&notification.text);
    if !runs.iter().any(|run| matches!(run, TextRun::DocsLink(_))) {
        return text
            .child(host_text::element(&HostText::from_host(notification.text.as_str())).min_w_0());
    }
    let seq = notification.seq;
    text.flex()
        .flex_row()
        .flex_wrap()
        .children(runs.into_iter().enumerate().map(|(at, run)| -> AnyElement {
            match run {
                TextRun::Plain(words) => host_text::element(&HostText::from_host(words))
                    .min_w_0()
                    .into_any_element(),
                TextRun::DocsLink(url) => {
                    docs_link(format!("{LINK_ID_PREFIX}{seq}-{at}"), url).into_any_element()
                }
            }
        }))
}

/// A docs link in an entry's text: a link button whose words and accessible name are `url`, which opens it through
/// `cx.open_url`, as the Help menu's docs links open. The list stays open, as the old one did.
///
/// The button sits in a column of its own that may shrink to the line, and its words may shrink inside it, so a long
/// address wraps within the list instead of running past its edge (the old `overflow-wrap: anywhere`).
fn docs_link(id: String, url: &str) -> impl IntoElement {
    let opened = url.to_owned();
    div().flex().flex_col().min_w_0().child(
        Button::new(
            id,
            url.to_owned(),
            HoverHelp::tooltip("Open this page of Farhelm's documentation in your browser")
                .beside_menu(),
            Role::Link,
        )
        .tier(Tier::Link)
        .content(
            div()
                .min_w_0()
                .text_size(px(ENTRY_TEXT))
                .child(url.to_owned()),
        )
        .on_click(move |_, _, cx| cx.open_url(&opened)),
    )
}
