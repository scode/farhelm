//! The quick switcher: Cmd+K on a Mac, Ctrl+Shift+K on Linux, a dialog that finds any session by typing part of its
//! title, host or folder and opens the one the user picks.
//!
//! SPEC.md, "Desktop window chrome" (the Cmd+K paragraph). Ported from the old app's `list/quick_switcher.rs` and the
//! `.quick-switcher-*` rules in its stylesheet at reference commit `24bc0580`; the old app's behavior is the reference
//! where SPEC.md says less. Here's the TLDR of its rules:
//!
//! - **Opening.** The shortcut opens it from anywhere on the page, a focused terminal included (the main view handles
//!   the action on its ground, and the chord's `TERMINAL_KEYS` row keeps it from the program). It does nothing, though
//!   the key is still taken, while any dialog or menu is open, while the window is inert, and before the helm's
//!   preferences have arrived with the snapshot, which stands in for SPEC.md's "before preferences finish loading".
//! - **One read per opening.** Opening sends one `GET /api/sessions?sort=activity` with no filter, because the
//!   switcher searches every host whatever the sidebar shows, and matches locally over that snapshot as the user
//!   types. The snapshot does not follow pushed changes while the switcher is open, as the old one did not. Each
//!   opening is a switcher of its own, and its read's reply holds it only weakly, so a reply that arrives after its
//!   switcher closed reaches nobody, and never a switcher opened since. The read is a
//!   plain command, not an operation: nothing waits on it but this dialog. Until it answers the switcher says
//!   "Loading sessions…" and Enter picks nothing; a refusal says "Couldn't load sessions:" and the helm's words.
//! - **Matching.** The old matcher exactly ([`matching_positions`], [`ranked`]): the trimmed query as a
//!   case-insensitive subsequence (lowercase mapping, not full case folding) of the title, the row's `host_name` or
//!   its raw working directory; title matches first, then matches only in the host or the directory, each group in
//!   the helm's activity order. Matching reads the raw values, so how a value is escaped on screen never changes
//!   which sessions match.
//! - **Marks.** A value whose raw form matched has its matched characters drawn in the accent at weight 600 (the old
//!   `mark`; the fill's ink on the selected row), found again on the escaped spelling the user sees
//!   ([`mark_positions`], [`host_text::marked`]). A value that did not match is never marked; one that matched only
//!   through a character shown as an escape marks less or nothing, since no escape is drawn partly marked.
//! - **Choosing.** The first match is selected after every edit; Up and Down move the selection, stopping at both
//!   ends, and scroll it into view. Focus stays in the search field the whole time: the rows are not focusable
//!   ([`OptionRow`]), and the field hands Up and Down over rather than moving its caret ([`TextInput::on_vertical`]).
//!   Hover only paints. Enter picks the selected row and a click picks the clicked one.
//! - **Picking.** A pick closes the switcher, then opens the session through the list's own [`selection::choose`], so
//!   it is selected, remembered with the helm and its terminal takes focus, the already open session's included
//!   (SPEC.md, "Where focus rests": "Choosing a session, including the one already selected, focuses its terminal").
//!   It closes first because a terminal never takes focus from inside a layer ([`crate::focus::choose_terminal`]). A
//!   pick goes ahead while a session action is pending; nothing here asks the operation store.
//! - **Closing.** Escape, Close and a press outside the card close it and return focus to what held it when it opened
//!   ([`ReturnTo::Opener`]). The veil takes that press, so it never also reaches the row or terminal under it.
//!
//! The card is drawn here rather than in the shared dialog frame, because the old switcher had neither the frame's
//! title header nor its answer footer: one padded column on the dialog surface, whose results list is the only part
//! that scrolls. The layer manager places it near the window's top and caps its height
//! ([`DialogPlacement::Top`]); the card keeps the old side margin by capping its own width.
//!
//! NOTE: Not built here, each waiting for the area that brings it: the `New session named "X"` row, the `tl:`
//! templates section and its hint, and the Quick launch row (the launcher area); resetting the host and status filters
//! after a pick they hide (the sidebar's filters, which do not exist yet); and the host's chosen mark beside its name
//! (host marks arrive with the sidebar's row marks). The old layout for windows narrower than 600 pixels is not ported
//! either, because the window's minimum width is 720.

#[cfg(test)]
mod tests;

use farhelm_helm_client::{ApiPath, CommandError, Method};
use farhelm_proto::ui_channel::{Reply, SessionList, SessionRow};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FontWeight, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window,
    div, px,
};

use super::menu::{self, AgentName};
use super::selection;
use super::status::{self, Badge};
use crate::components::{
    Button, HoverHelp, OptionRow, Role, TextInput, TextInputEvent, TextInputState, Vertical,
    dialog_shadow,
};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Dialog, DialogPlacement, LayerId};
use crate::session::{self, UiSession};
use crate::theme::{self, Palette, metrics};
use crate::{assets, ticker};

// ---- Layout: the old `.quick-switcher-*` rules ------------------------------------------------------------------------

/// The card's widest (`width: min(760px, 100%)`).
const CARD_MAX_WIDTH: Pixels = px(760.);
/// What the card leaves free on each side of a narrower window: the old backdrop's 16-pixel side padding, which the
/// layer manager leaves to the view. (Its height cap, `max-height: 75dvh`, and its place near the top are the layer
/// manager's: `DialogPlacement::Top`.)
const CARD_MARGIN: Pixels = px(16.);
/// The card's padding and the gap between its parts.
const CARD_PAD: Pixels = px(16.);
const CARD_GAP: Pixels = px(10.);
/// A row's fixed columns (`grid-template-columns: 24px minmax(0, 2fr) 70px minmax(0, 1fr) minmax(0, 2fr)`), the gap
/// between them, and the row's own padding and edge, which the flexible columns share what is left after.
const STATUS_COLUMN: Pixels = px(24.);
const AGENT_COLUMN: Pixels = px(70.);
const COLUMN_GAP: Pixels = px(10.);
const ROW_CHROME: Pixels = px(22.);

/// The words of the cap notice, shown when the helm's listing cap cut the snapshot.
pub(crate) const CAP_NOTICE: &str =
    "Only the most recently active sessions were searched; the helm's listing limit was reached.";

// ---- Matching -----------------------------------------------------------------------------------------------------------

/// Where `query` occurs in `text` as a case-insensitive subsequence, as the original characters' indices; `None` when
/// it does not. Ported unchanged from the old matcher, whose rules are the spec.
///
/// Both sides are compared through `char::to_lowercase`, which is lowercase mapping, not full case folding or
/// Unicode normalization. Each query character takes the first matching character after the previous one (leftmost,
/// greedy), so of several alignments the earliest wins. A character that lowercases to several (`İ` becomes `i` and a
/// combining dot) is listed once. An empty query matches everything, at no positions.
pub(crate) fn matching_positions(text: &str, query: &str) -> Option<Vec<usize>> {
    let mut folded = text
        .chars()
        .enumerate()
        .flat_map(|(index, ch)| ch.to_lowercase().map(move |ch| (index, ch)));
    let mut positions = Vec::new();
    for wanted in query.chars().flat_map(char::to_lowercase) {
        let (index, _) = folded.find(|(_, ch)| *ch == wanted)?;
        if positions.last() != Some(&index) {
            positions.push(index);
        }
    }
    Some(positions)
}

/// The characters to mark in `shown`, the escaped spelling the user sees of the value `raw`: none unless `query` matches
/// `raw` itself (the value took part in the match), and then where `query` occurs in `shown`, which may be nowhere. A
/// value is matched on its raw form ([`ranked`]) but marked on what is shown, so a title found by typing an invisible
/// character, which shows as its `<U+…>` escape, is listed with nothing marked, and a value that did not match is not
/// marked because its escapes happen to spell the query, as in the old switcher.
pub(crate) fn mark_positions(raw: &str, shown: &str, query: &str) -> Vec<usize> {
    if matching_positions(raw, query).is_none() {
        return Vec::new();
    }
    matching_positions(shown, query).unwrap_or_default()
}

/// The sessions of `sessions` (the helm's activity order) that `query` finds, as indices into it, in the order the
/// switcher lists them: every title match first, then every session that matches only in its host's name or its
/// working directory, each group keeping the helm's order. No score and no other sort: a scattered title match still
/// comes before a contiguous match in a folder, and among title matches the most recently active comes first.
///
/// The query is trimmed first. Matching reads the raw values (the title, `host_name` and `cwd` as the host wrote
/// them), so a hidden character in a title is found by typing it, and typing its escaped spelling (`<U+200B>`) finds
/// nothing.
pub(crate) fn ranked(sessions: &[SessionRow], query: &str) -> Vec<usize> {
    let query = query.trim();
    let mut titles = Vec::new();
    let mut elsewhere = Vec::new();
    for (index, row) in sessions.iter().enumerate() {
        if matching_positions(&row.info.title, query).is_some() {
            titles.push(index);
        } else if matching_positions(&row.host_name, query).is_some()
            || matching_positions(&row.info.cwd, query).is_some()
        {
            elsewhere.push(index);
        }
    }
    titles.extend(elsewhere);
    titles
}

// ---- Opening ------------------------------------------------------------------------------------------------------------

/// The shortcut ([`crate::actions::QuickSwitcher`]): open the switcher on top of the window's page. The main view calls
/// it for the action wherever focus is, and the action is taken whatever this does, as `ONCE_PER_PRESS` requires.
///
/// Does nothing while any dialog or menu is open (SPEC.md: "It does nothing while another modal is open"), while the
/// window is inert, or before the snapshot has brought the helm's preferences.
pub(crate) fn open(window: &mut Window, cx: &mut App) {
    // A key pressed inside a layer reaches the main view's ground too, since the overlay is drawn inside it; this is
    // what keeps a second switcher from opening over the first, or over any other dialog or menu.
    if layers::any_open(window, cx) {
        return;
    }
    let Some(current) = session::connection(cx).and_then(|c| c.read(cx).session().cloned()) else {
        return;
    };
    if selection::preferences(current.read(cx).store().read(cx).collections()).is_none() {
        return;
    }
    let mark = Mark::take(cx);
    let switcher = cx.new(|cx| QuickSwitcher::new(window, cx));
    let start = switcher.read(cx).input.focus_handle(cx);
    // Near the window's top, and closed by a press outside the card, as the old switcher was: the layer manager's
    // placement for exactly this dialog. The press goes the way Escape goes, so focus returns to the opener.
    let Some(layer) = layers::open_dialog(
        Dialog {
            view: switcher.clone().into(),
            container: cx.focus_handle(),
            start,
            // Nothing it shows can go away under it: it searches a snapshot of its own.
            subject: None,
            // None, because the card itself never takes focus (below), so Enter on it could never be pressed.
            main_action: None,
            // The old switcher kept focus in its field whatever was clicked inside the card, rows included.
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Opener,
            on_dismiss: None,
            placement: DialogPlacement::Top,
            dismiss_on_outside_press: true,
        },
        &mark,
        window,
        cx,
    ) else {
        return;
    };
    switcher.update(cx, |switcher, _| switcher.layer = Some(layer));
    #[cfg(test)]
    cx.set_global(tests::Opened(switcher.downgrade()));
    read_sessions(&current, switcher.downgrade(), cx);
}

/// Send the switcher's one read, `GET /api/sessions?sort=activity`, through the UI session `current`, and hand the
/// answer to `switcher` if it is still open. The reply holds the switcher weakly and each opening makes a switcher of
/// its own, so a reply after its switcher closed reaches nobody, and never a switcher opened since. A reset retires the
/// UI session, which drops the wait, and closes every dialog, the switcher included.
fn read_sessions(current: &Entity<UiSession>, switcher: WeakEntity<QuickSwitcher>, cx: &mut App) {
    let path =
        ApiPath::api(&["sessions"]).and_then(|path| path.with_query(&[("sort", "activity")]));
    let path = match path {
        Ok(path) => path,
        // The path is fixed, so this cannot happen; were it to, the switcher says so rather than loading forever.
        Err(error) => {
            let _ = switcher.update(cx, |switcher, cx| {
                switcher.answered(Read::Failed(HostText::from_host(error.to_string())), cx)
            });
            return;
        }
    };
    current.update(cx, |current, cx| {
        current.command(Method::Get, path, None, cx, move |_, result, cx| {
            let read = Read::from_reply(result);
            // A switcher that closed is gone, so the update fails and the reply is dropped.
            let _ = switcher.update(cx, |switcher, cx| switcher.answered(read, cx));
        });
    });
}

// ---- The view -----------------------------------------------------------------------------------------------------------

/// Where the switcher's read stands.
enum Read {
    /// No reply yet: the switcher lists nothing and Enter picks nothing.
    Loading,
    /// The helm refused or the reply could not be read; the reason may quote a host, so it is host text.
    Failed(HostText),
    /// The listing the switcher searches until it closes.
    Loaded(SessionList),
}

impl Read {
    /// What the reply to the read says. A refusal's words are the helm's, or the status when it sent none
    /// ([`super::refusal_words`]).
    fn from_reply(result: Result<Reply, CommandError>) -> Read {
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => return Read::Failed(HostText::from_host(error.to_string())),
        };
        if !(200..300).contains(&reply.status) {
            return Read::Failed(super::refusal_words(
                reply.status,
                reply.body,
                "GET /api/sessions",
            ));
        }
        match reply.body.map(serde_json::from_value::<SessionList>) {
            Some(Ok(list)) => Read::Loaded(list),
            Some(Err(error)) => Read::Failed(HostText::from_host(format!(
                "the helm's listing could not be read: {error}"
            ))),
            None => Read::Failed(HostText::from_host("the helm sent no listing")),
        }
    }
}

/// The open switcher: its field, its read, which row is selected, and its layer.
pub(crate) struct QuickSwitcher {
    /// The search field. Its text is the query; the switcher keeps no copy of it.
    input: TextInputState,
    read: Read,
    /// The selected row's position among the matches, kept within them wherever it is used, since the matches change
    /// with every edit and when the read arrives.
    selected: usize,
    /// The results list's scroll position, which follows the selected row.
    scroll: ScrollHandle,
    /// The running pulse's ticks, held exactly while the window is active and a listed row pulses, and worked out at
    /// the top of each frame, as the session list does.
    pulse: Option<ticker::Pulsing>,
    /// Set right after the layer opens; taken when the switcher closes itself, so it closes once.
    layer: Option<LayerId>,
    /// The field's events.
    _events: Subscription,
}

impl QuickSwitcher {
    /// A switcher, loading, with an empty field and the first row selected.
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = TextInputState::new(window, cx);
        // The old placeholder went on with "· tl: for templates", which belongs to the launcher area's templates part.
        input.set_placeholder("Jump to a session", window, cx);
        let events = input.subscribe(window, cx, |switcher, event, window, cx| match event {
            TextInputEvent::Changed => switcher.edited(cx),
            TextInputEvent::Submitted => switcher.pick_selected(window, cx),
        });
        QuickSwitcher {
            input,
            read: Read::Loading,
            selected: 0,
            scroll: ScrollHandle::new(),
            pulse: None,
            layer: None,
            _events: events,
        }
    }

    /// The read answered.
    fn answered(&mut self, read: Read, cx: &mut Context<Self>) {
        self.read = read;
        cx.notify();
    }

    /// The matches for the field's text now, as indices into the listing; none until it has loaded.
    fn matches(&self, cx: &App) -> Vec<usize> {
        match &self.read {
            Read::Loaded(list) => ranked(&list.sessions, &self.input.text(cx)),
            Read::Loading | Read::Failed(_) => Vec::new(),
        }
    }

    /// The user edited the query: the first match is selected again, and shown.
    fn edited(&mut self, cx: &mut Context<Self>) {
        self.selected = 0;
        self.scroll.scroll_to_item(0);
        cx.notify();
    }

    /// Up or Down in the field: move the selection one row, stopping at either end, and scroll it into view. With
    /// nothing listed there is nothing to move.
    fn step(&mut self, direction: Vertical, cx: &mut Context<Self>) {
        let count = self.matches(cx).len();
        if count == 0 {
            return;
        }
        let current = self.selected.min(count - 1);
        self.selected = match direction {
            Vertical::Up => current.saturating_sub(1),
            Vertical::Down => (current + 1).min(count - 1),
        };
        self.scroll.scroll_to_item(self.selected);
        cx.notify();
    }

    /// Enter: pick the selected row, if there is one.
    fn pick_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Read::Loaded(list) = &self.read else {
            return;
        };
        let matches = ranked(&list.sessions, &self.input.text(cx));
        let Some(&index) = matches.get(self.selected.min(matches.len().saturating_sub(1))) else {
            return;
        };
        let key = list.sessions[index].info.id.clone();
        self.pick(&key, window, cx);
    }

    /// Open the session filed under `key` as the user's choice, after closing the switcher (the module docs say why
    /// in that order). The key is the row's served id, which is the key the store files the session under.
    fn pick(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        selection::choose(key, cx);
    }

    /// Close: close the switcher, picking nothing.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
    }
}

impl Render for QuickSwitcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let viewport = window.viewport_size();
        let width = CARD_MAX_WIDTH.min(viewport.width - CARD_MARGIN * 2.);
        let query = self.input.text(cx);
        let matches = self.matches(cx);
        let selected = self.selected.min(matches.len().saturating_sub(1));

        let pulses = ticker::is_active(window, cx)
            && match &self.read {
                Read::Loaded(list) => matches
                    .iter()
                    .any(|&index| status::pulses(&list.sessions[index])),
                Read::Loading | Read::Failed(_) => false,
            };
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let step = self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx));

        let weak = cx.weak_entity();
        let field = {
            let weak = weak.clone();
            div()
                // The selector is for tests, which find the field by where it is drawn.
                .debug_selector(|| "quick-switcher-field".into())
                // Only the results list gives up height when the card is capped.
                .flex_none()
                .child(
                    TextInput::new(
                        "quick-switcher-search",
                        &self.input,
                        "Search sessions",
                        HoverHelp::tooltip("Search sessions across every host"),
                    )
                    .on_vertical(move |direction, _, cx| {
                        let _ = weak.update(cx, |switcher, cx| switcher.step(direction, cx));
                    }),
                )
        };
        let heading = quiet(
            if query.trim().is_empty() {
                "Recent sessions"
            } else {
                "Sessions"
            },
            palette,
        );
        let state: Option<AnyElement> = match &self.read {
            Read::Loading => Some(announced(
                "quick-switcher-loading",
                Role::Status,
                "Loading sessions…",
            )),
            Read::Failed(reason) => {
                let sentence = HostSentence::new("quick-switcher-failure-text")
                    .text("Couldn't load sessions: ")
                    .host(reason);
                Some(
                    div()
                        .id("quick-switcher-failure")
                        .role(Role::Alert)
                        .aria_label(sentence.accessible_name())
                        .child(sentence)
                        .into_any_element(),
                )
            }
            Read::Loaded(_) if matches.is_empty() => Some(announced(
                "quick-switcher-empty",
                Role::Status,
                "No sessions match",
            )),
            Read::Loaded(_) => None,
        };
        let columns = Columns::for_card(width);
        let rows: Vec<AnyElement> = match &self.read {
            Read::Loaded(list) => matches
                .iter()
                .enumerate()
                .map(|(position, &index)| {
                    let row = &list.sessions[index];
                    let key = row.info.id.clone();
                    let weak = weak.clone();
                    result_row(
                        row,
                        query.trim(),
                        position == selected,
                        step,
                        columns,
                        palette,
                    )
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |switcher, cx| switcher.pick(&key, window, cx));
                    })
                    .into_any_element()
                })
                .collect(),
            Read::Loading | Read::Failed(_) => Vec::new(),
        };
        let results = div()
            .id("quick-switcher-results")
            .role(Role::ListBox)
            .aria_label("Sessions")
            .track_scroll(&self.scroll)
            .flex()
            .flex_col()
            .flex_shrink_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .children(rows);
        let cap = match &self.read {
            Read::Loaded(list) if list.truncated => Some(quiet(CAP_NOTICE, palette)),
            Read::Loaded(_) | Read::Loading | Read::Failed(_) => None,
        };
        let footer = div()
            .flex()
            .flex_none()
            .flex_row()
            .justify_between()
            .items_center()
            .pt(CARD_GAP)
            .border_t_1()
            .border_color(theme::color(palette.border_dialog))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_1))
            .child("↑↓ choose · Enter pick · Escape close")
            .child(
                Button::new(
                    "quick-switcher-close",
                    "Close",
                    HoverHelp::tooltip("Close the quick switcher"),
                    Role::Button,
                )
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |switcher, cx| switcher.close(window, cx));
                }),
            );

        div()
            .id("quick-switcher")
            .debug_selector(|| "quick-switcher".into())
            .role(Role::Dialog)
            .aria_label("Quick switcher")
            .flex()
            .flex_col()
            .gap(CARD_GAP)
            .p(CARD_PAD)
            .w(width)
            // The layer manager caps the top-placed dialog at 75% of the window's height; shrinking to that cap is
            // this card's part, so only the results list, which scrolls, gives up the height.
            .flex_1()
            .min_h(px(0.))
            .bg(theme::color(palette.bg_2))
            .text_color(theme::color(palette.fg_0))
            .border_1()
            .border_color(theme::color(palette.border_dialog))
            .rounded(metrics::RADIUS_MD)
            .shadow(dialog_shadow(palette))
            .child(field)
            .child(heading)
            .children(state)
            .child(results)
            .children(cap)
            .child(footer)
    }
}

/// The 12-pixel quiet line the old heading, footer and cap notice shared.
fn quiet(text: &'static str, palette: &Palette) -> AnyElement {
    div()
        .flex_none()
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(text)
        .into_any_element()
}

/// One of the switcher's one-line states, announced to assistive technology under `role` as the old live regions
/// were.
fn announced(id: &'static str, role: Role, text: &'static str) -> AnyElement {
    div()
        .id(id)
        .flex_none()
        .role(role)
        .aria_label(text)
        .child(text)
        .into_any_element()
}

/// The widths of a result row's flexible columns in a card `card` wide: what the fixed columns, the gaps and the
/// padding leave, shared two to one to two between title, host and directory, as the old grid's `fr` tracks did.
/// Computed rather than left to flex, because a flexible item with text makes layout shape it on every sizing probe,
/// which shows at the listing cap (SPEC_impl.md, "GPUI cookbook: traps", "Layout costs show only at scale").
#[derive(Clone, Copy)]
struct Columns {
    title: Pixels,
    host: Pixels,
    directory: Pixels,
}

impl Columns {
    fn for_card(card: Pixels) -> Columns {
        let inner = card - CARD_PAD * 2. - px(2.) - ROW_CHROME;
        let shared = (inner - STATUS_COLUMN - AGENT_COLUMN - COLUMN_GAP * 4.).max(px(0.));
        Columns {
            title: shared * 0.4,
            host: shared * 0.2,
            directory: shared * 0.4,
        }
    }
}

/// One session's result row: its status mark, title, agent, host and working directory on one line, and an ended
/// status's words on a second line under the title, since they do not fit the mark's column. Everything the host wrote
/// is host text, each value its own element, with the characters `query` (already trimmed) matches in it marked. On the
/// selected row the title and marks take the inverse ink and the quieter columns the dimmer one, as the old sheet's
/// selected row did.
fn result_row(
    row: &SessionRow,
    query: &str,
    selected: bool,
    step: u64,
    columns: Columns,
    palette: &Palette,
) -> OptionRow {
    let key = &row.info.id;
    let id: SharedString = format!("quick-switcher-row-{key}").into();
    let title = HostText::from_host(row.info.title.as_str());
    let label = HostSentence::new(id.clone()).host(&title).accessible_name();
    let badge = status::badge(row, format!("quick-switcher-status-{key}").into());
    // The sidebar row's form: the status word joins the help, which is also the row's accessible description, so a
    // live status shown only as a mark still has words for hover and assistive technology.
    let help = match Badge::word(badge.as_ref()) {
        Some(word) => format!("{word} — open this session"),
        None => "Open this session".to_owned(),
    };
    let (live, ended) = match badge {
        Some(Badge::Live(live)) => (Some(live), None),
        Some(Badge::Ended { kind, line }) => (None, Some((kind, line))),
        None => (None, None),
    };
    let (ink, quiet) = if selected {
        (palette.on_fill, palette.on_accent_dim)
    } else {
        (palette.fg_0, palette.fg_1)
    };
    let small = |content: AnyElement, width: Pixels| {
        div()
            .w(width)
            .flex_none()
            .min_w_0()
            .overflow_hidden()
            .truncate()
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(quiet))
            .child(content)
    };
    let agent = match menu::agent_name(row) {
        AgentName::Known(agent) => div().child(agent).into_any_element(),
        AgentName::Program(program) => host_text::element(&program).into_any_element(),
    };
    // The old `mark`: the accent at weight 600, or the fill's ink on the selected row's accent fill.
    let style = HighlightStyle {
        color: Some(theme::color(if selected {
            palette.on_fill
        } else {
            palette.accent
        })),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    let marked = |raw: &str, value: &HostText| {
        host_text::marked(value, |shown| mark_positions(raw, shown, query), style)
    };
    let host_name = row.host_name.as_str();
    let host = marked(host_name, &HostText::from_host(host_name)).into_any_element();
    let cwd = row.info.cwd.as_str();
    let directory = marked(cwd, &HostText::from_host(cwd))
        .font_family(assets::TERMINAL_FONT)
        .into_any_element();
    let line = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(COLUMN_GAP)
        .min_w_0()
        .child(
            div()
                .w(STATUS_COLUMN)
                .flex_none()
                .flex()
                .items_center()
                .children(live.map(|live| live.mark(palette, selected, row.stale, step))),
        )
        .child(
            marked(row.info.title.as_str(), &title)
                .w(columns.title)
                .flex_none()
                .min_w_0()
                .truncate()
                .text_color(theme::color(ink)),
        )
        .child(small(agent, AGENT_COLUMN))
        .child(small(host, columns.host))
        .child(small(directory, columns.directory));
    let mut option = OptionRow::new(id, label, HoverHelp::tooltip(help), Role::ListBoxOption)
        .selected(selected)
        .child(line);
    if let Some((kind, line)) = ended {
        option = option.child(
            div()
                .pl(STATUS_COLUMN + COLUMN_GAP)
                .child(status::ended_line(kind, line, palette, selected)),
        );
    }
    option
}
