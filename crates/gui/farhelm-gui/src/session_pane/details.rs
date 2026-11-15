//! The open session's details bar: the slim row under its terminal that names the session, with its status, title,
//! host and age, in that order.
//!
//! SPEC.md, "Terminal experience": "The bottom bar is about one tab-strip row high and shows, in order, status, session
//! name, host, age, the managed-checkout branch mark when present, directory, and command line." Ported from the old
//! app's `.session-details` (`session_view.rs` and `app.css` at the reference commit). This is the bar's first half:
//! the managed-checkout mark, the directory and command copy buttons and their hidden-characters warning are a later
//! piece of this area, and the bar draws no placeholder for them.
//!
//! Here's the TLDR of the rules:
//!
//! - **The list's own words and marks.** The status is the session row's badge ([`status::live`] and
//!   [`status::ended_words`], the plain `Exited` form the quick switcher and the menu header use), without the row's
//!   seen toggle: only a row's own dot is a control. The host mark is the row's ([`row_marks::HostMark::of`]): none when
//!   the session's host cannot be placed, and a hidden `Local` or `Remote` as its name. The age is the row's
//!   ([`age::of`]), with the full stamp as hover help. A second copy of any of these would let the bar describe a
//!   session differently from its row.
//! - **The host is the helm's full name**, never the row's short one, as the session sent it ([`identity::named`]): the
//!   local host under the registry's own name reads `Local (this machine)`, and a session that names no host shows none
//!   rather than an invented one.
//! - **What a status cannot claim, it leaves out.** An unclassified session has no badge but keeps its age (SPEC.md,
//!   "Status": a session nothing has classified "shows no status and still shows its age"). A stale session (its host is
//!   not connected) has neither: its last-known status and age belong in the stale band, a later piece, and SPEC.md says
//!   "a stale session's status appears only in its stale notice, not again in the bottom bar".
//! - **Who gives way.** The bar never wraps. A live mark, the host mark and the age never shrink. The ended words, the
//!   title and the host name shrink as the old bar's flex items did (`flex: 0 1 auto; min-width: 0`, GPUI's default
//!   shrink with the minimum taken off), so each gives up width in proportion to its own: a long title gives up the
//!   most, and in a narrow pane the title and the host are cut together, as the old narrow screens show. Each ends in an
//!   ellipsis when cut, with its whole value as hover help.
//! - **Nothing here takes focus yet.** The bar is drawn after the terminal, so the copy buttons that come with the next
//!   piece will follow the top row and the terminal in Tab's order, as SPEC.md asks; today the bar has no stop at all.
//!
//! What the bar shows is decided by [`Details::of`], a pure function of the session's row, the registry and the clock,
//! so tests read it without a window; [`bar`] only lays it out. The pane reads the row from the store by key, so the bar
//! names the open session also while a filter or the list's cap hides it from the list: the session query names the
//! open session, and the helm keeps it in the pushed collection then (`session_list::filters`, "The query names the
//! open session"). The bar never takes the title from the filtered listing. With no row in the store at all (before the
//! helm answers the query naming a newly opened session, say), the pane draws no bar.

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    App, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement, Pixels,
    SharedString, Stateful, Styled, div, prelude::FluentBuilder as _, px,
};

use crate::components::{HoverHelp, Role, StatusMark};
use crate::host_text::{self, HostSentence, HostText, Line};
use crate::hosts::identity;
use crate::session_list::age::{self, Age};
use crate::session_list::row_marks::{self, HostMark, Locality};
use crate::session_list::status::{self, EndedWords, Live};
use crate::theme::{self, Palette, metrics};

/// The bar's id, by which tests and the bridge find it. The ids of its parts start with it.
pub(crate) const BAR_ID: &str = "session-details";
/// The status badge's id.
pub(crate) const STATUS_ID: &str = "session-details-status";
/// The title's id.
pub(crate) const TITLE_ID: &str = "session-details-title";
/// The host mark's id: [`row_marks::host_mark`]'s for a "row" named [`BAR_ID`], which makes it. Named here for the
/// tests that find the mark.
#[cfg(test)]
pub(crate) const HOST_MARK_ID: &str = "session-details-host";
/// The host name's id.
pub(crate) const HOST_NAME_ID: &str = "session-details-host-name";
/// The age's id.
pub(crate) const AGE_ID: &str = "session-details-age";

/// `.session-details`'s `min-height`, the same as the top row's.
const MIN_HEIGHT: f32 = 30.;
/// `.session-details`'s padding: 3 pixels above and below, 10 at the sides.
const PAD_Y: f32 = 3.;
const PAD_X: f32 = 10.;
/// `.session-details`'s gap between its parts.
const GAP: f32 = 10.;
/// `.session-details-host`'s gap between the host mark and the name.
const HOST_GAP: f32 = 5.;

/// The advance of the bundled interface face's `0`, in ems: 1292 of Inter's 2048 units per em
/// (`assets/fonts/Inter-Regular.ttf`'s `hmtx` entry for the glyph). A CSS `ch` is this width.
const ZERO_ADVANCE_EM: f32 = 0.631;
/// The ended badge's horizontal padding on each side (`.session-details .status-badge`'s `padding: 1px 6px`).
const BADGE_PAD_X: f32 = 6.;

/// The ended badge's widest, padding included: the old `.status-badge`'s `max-width: 32ch` at its 12-pixel text, plus
/// its padding on both sides. The old sheet set no `box-sizing`, so its cap was the content box's, with the padding
/// outside it; GPUI's layout measures the border box, so the padding is added here. Wide enough that an ordinary exit
/// with a short annotation fits whole; a launch failure's detail ends in an ellipsis well before this.
fn badge_max_width() -> Pixels {
    metrics::TEXT_SM * (32. * ZERO_ADVANCE_EM) + px(2. * BADGE_PAD_X)
}

/// What the bar shows for one session, decided from its row, the registry and the viewer's clock. See the module
/// docs for the rules each field follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Details {
    /// The status badge, or `None` for a stale or unclassified session.
    pub(crate) status: Option<Status>,
    pub(crate) title: HostText,
    /// The host, or `None` for a session that names no host.
    pub(crate) host: Option<Host>,
    /// The age, or `None` for a stale session, one with no stamp, or a viewer clock that cannot be used.
    pub(crate) age: Option<Age>,
}

/// The badge: a live mark, or an ended status's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Live(Live),
    Ended(EndedWords),
}

/// The session's host as the bar names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Host {
    /// The row's host mark, `None` when the host cannot be placed.
    pub(crate) mark: Option<HostMark>,
    /// The helm's full name for the host, with the local host's own wording.
    pub(crate) name: HostText,
}

impl Details {
    /// What the bar shows for `row` over `collections`, with the viewer's clock at `now` (seconds since the epoch,
    /// [`age::seconds`]; `None` when it cannot be used, and no age is shown).
    pub(crate) fn of(row: &SessionRow, collections: &Collections, now: Option<i64>) -> Details {
        let status = if row.stale {
            None
        } else {
            status::live(row)
                .map(Status::Live)
                .or_else(|| status::ended_words(row, false).map(Status::Ended))
        };
        let host = (!row.host_name.is_empty()).then(|| {
            let mark = HostMark::of(row, collections, None);
            let local = mark.is_some_and(|mark| mark.locality == Locality::Local);
            Host {
                mark,
                name: identity::named(&row.host_name, local),
            }
        });
        let age = if row.stale {
            None
        } else {
            age::of(row.info.effective_activity(), now)
        };
        Details {
            status,
            title: HostText::from_host(row.info.title.as_str()),
            host,
            age,
        }
    }

    /// Whether the badge pulses: a running session's mark. A stale session has no badge, so this never asks whether
    /// the host is connected.
    pub(crate) fn pulses(&self) -> bool {
        matches!(self.status, Some(Status::Live(live)) if live.pulses(false))
    }
}

/// The bar for `details`, a running mark drawn at pulse step `pulse` ([`crate::ticker::Pulsing::step`]). Drawn by the
/// pane on every frame, uncached, like the top row; the screenshot goldens draw it on its own.
pub(crate) fn bar(details: Details, pulse: u64, cx: &App) -> Stateful<Div> {
    let palette = theme::palette(cx);
    div()
        .id(BAR_ID)
        .debug_selector(|| BAR_ID.into())
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(GAP))
        .min_h(px(MIN_HEIGHT))
        .px(px(PAD_X))
        .py(px(PAD_Y))
        .overflow_hidden()
        .whitespace_nowrap()
        .bg(theme::color(palette.bg_1))
        .text_size(metrics::TEXT_MD)
        .text_color(theme::color(palette.fg_0))
        .children(details.status.map(|status| badge(status, pulse, palette)))
        .child(title(&details.title, palette))
        .children(details.host.map(|host| host_part(host, palette)))
        .children(details.age.map(age_part))
}

/// The status badge. A live status is the row's mark alone, which never shrinks, named by its word and helped by it
/// (the old dot's tooltip was the badge's text). An ended status is its words in the status's color, which may shrink
/// to an ellipsis, with the whole wording as hover help and name (`.session-details .status-badge`).
fn badge(status: Status, pulse: u64, palette: &Palette) -> Div {
    match status {
        Status::Live(live) => div().flex_none().child(
            StatusMark::new(
                STATUS_ID,
                live.word(),
                HoverHelp::tooltip(live.word()),
                Role::Label,
                live.word(),
            )
            .content(
                div()
                    .flex()
                    .items_center()
                    .child(live.mark(palette, false, false, pulse)),
            )
            .described(false),
        ),
        Status::Ended(ended) => {
            let label = ended.line(STATUS_ID.into()).accessible_text();
            let help = match ended.line(format!("{STATUS_ID}-help").into()) {
                Line::App(words) => HoverHelp::tooltip(words),
                Line::Sentence(sentence) => HoverHelp::sentence(sentence),
            };
            let quotes_host = ended.host.is_some();
            // The words and the host value side by side on one line, each its own text element, as the host-text rule
            // requires (one run could not keep the value's direction to itself). With a host value it is the part that
            // gives way, since the words carry the status itself and the exit code; with none the words do. The old
            // badge was one text run that ellipsized at its end; here, once the host value is cut to nothing, the words
            // never shrink, so a bar too narrow for them clips them at the badge's edge with no ellipsis (and perhaps a
            // dangling dash). A shrink factor far below the value's would not help: any shrink at all ellipsizes the
            // words while the value still shows. That takes a pane far narrower than the 320-pixel floor leaves for
            // the badge beside a title and a host, and the whole wording stays the hover help.
            let wording = div()
                .flex()
                .flex_row()
                .min_w_0()
                .overflow_hidden()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .when(quotes_host, |words| words.flex_none())
                        .child(ended.words),
                )
                .children(
                    ended
                        .host
                        .map(|host| host_text::element(&host).min_w_0().truncate()),
                );
            div()
                .min_w_0()
                .max_w(badge_max_width())
                .px(px(BADGE_PAD_X))
                .py(px(1.))
                .overflow_hidden()
                .child(
                    StatusMark::new(STATUS_ID, label, help, Role::Label, "")
                        .ink(ended.kind.color(palette, false))
                        .truncate()
                        .content(wording)
                        .described(false),
                )
        }
    }
}

/// The title, semibold, cut with an ellipsis when the bar runs out of room, with the whole title as hover help through
/// the host-text element (`.session-details .title`).
fn title(title: &HostText, palette: &Palette) -> impl IntoElement {
    StatusMark::new(
        TITLE_ID,
        HostSentence::new(TITLE_ID).host(title).accessible_name(),
        HoverHelp::sentence(HostSentence::new(format!("{TITLE_ID}-help")).host(title)),
        Role::Label,
        "",
    )
    .ink(palette.fg_0)
    .truncate()
    .content(value(title))
    .described(false)
}

/// The host: its mark, which never shrinks, then its name in the bright foreground, semibold, cut with an ellipsis when
/// the bar runs out of room, with the shown name as hover help (`.session-details-host`).
fn host_part(host: Host, palette: &Palette) -> Div {
    let name = &host.name;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(HOST_GAP))
        .min_w_0()
        .overflow_hidden()
        .children(host.mark.map(|mark| {
            div()
                .flex_none()
                .flex()
                .items_center()
                .child(row_marks::host_mark(BAR_ID, mark, palette))
        }))
        .child(
            StatusMark::new(
                HOST_NAME_ID,
                HostSentence::new(HOST_NAME_ID).host(name).accessible_name(),
                HoverHelp::sentence(
                    HostSentence::new(SharedString::from(format!("{HOST_NAME_ID}-help")))
                        .host(name),
                ),
                Role::Label,
                "",
            )
            .ink(palette.fg_bright)
            .truncate()
            .content(value(name))
            .described(false),
        )
}

/// A host value at the bar's text size, semibold, on one line ending in an ellipsis when it does not fit: the title and
/// the host name share this look.
fn value(text: &HostText) -> Div {
    host_text::element(text)
        .min_w_0()
        .truncate()
        .text_size(metrics::TEXT_MD)
        .font_weight(FontWeight::SEMIBOLD)
}

/// The age: the row's relative age in the quiet foreground and the monospace face, which never shrinks, named and
/// helped by the full stamp (`.status-time`).
fn age_part(age: Age) -> Div {
    div().flex_none().child(
        StatusMark::new(
            AGE_ID,
            age.full.clone(),
            HoverHelp::tooltip(age.full),
            Role::Label,
            age.short,
        )
        .monospace()
        .described(false),
    )
}
