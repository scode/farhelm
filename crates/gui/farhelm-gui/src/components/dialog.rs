//! The one dialog frame: a pinned header, a scrolling body and pinned answer buttons, as the old app's dialogs had.
//!
//! The old app's dialogs kept their title and their answers on screen while the body scrolled, and when even the title
//! and the answers did not fit a short window, the whole card scrolled instead (`.host-dialog-framed` in the old
//! stylesheet). That guarantee is what kept answers reachable at small window sizes, a bug class the old app fixed one
//! dialog at a time; here every dialog gets it from the frame. The frame only lays out: the layer manager places it,
//! draws the veil behind it and keeps focus inside it. Both scrolling parts, the body and the whole card, are the focus
//! coordinator's scroll regions, so a control that Tab reaches below the fold scrolls into view rather than taking focus
//! out of sight.
//!
//! A dialog may also have the shared header Close control SPEC.md gives editors such as Rename ("a shared title and
//! Close header"): the old `.dialog-close` "×" at the header's top right. Drawn in the header, it comes first in Tab
//! order in ordinary title headers, before the body's fields and the answers (SPEC.md for Rename: "Tab follows Close,
//! the title field, Cancel, Save"). The session launcher has a control before Close in its header
//! ([`DialogFrame::heading_control`]). Each dialog supplies what Close does and its label and help.
//!
//! The old host dialogs and Settings framed their header further (`.host-dialog-framed` and `.host-settings-header` in
//! the old stylesheet): an icon tile left of the title ([`DialogFrame::tile`]), a note at the footer's left beside the
//! answers ([`DialogFrame::note`]), and a width of their own ([`DialogFrame::width`]): Settings, and the host setup
//! confirmation asked through the prompt. Settings alone also keeps a fixed height that does not follow its content
//! ([`DialogFrame::fixed_height`]), so switching sections never moves the frame or its Close.
//! The host settings dialog's header shows more than a title and a one-line summary can: the host's name over its
//! status line, a colored phase dot beside the build. It draws its own heading in their place
//! ([`DialogFrame::heading`]), and its title, "Host settings · <name>", stays the dialog's accessible name.
//! The session launcher's footer gives a press of its main action room for a confirmation or refusal
//! ([`DialogFrame::notices`], placed under the row by [`DialogFrame::notices_below`]). Settings pins
//! the recipe editor's own answers in that region above the row, as a band ruled off from its Close
//! ([`DialogFrame::notices_rule`]).
//! Every body scrolls on Page Up, Page Down, Home and End from a control inside it that has no use for the key
//! ([`scroll_from_controls`]), as the old app's browser scrolled the container around a focused control, so a long
//! body of controls (the Templates editor's) reads to its end from the keyboard without tabbing through all of them.
//! A body whose content may hold nothing focusable can also have a tab stop of its own that the keys scroll
//! ([`DialogFrame::scroll_stop`]; `scroll_box.rs` has the full account). So can the notices region, which also scrolls
//! on those four keys from a control inside it only once given that stop ([`DialogFrame::notices_stop`]), for a
//! question whose text outgrows the region while one of its answers holds focus (the old Trash dialog's global
//! confirmation).
//!
//! Two of the old app's small information dialogs, Announcements and the Welcome card, drew their parts tighter than the
//! ordinary dialog (`.announcements-dialog > .dialog-header` and `.welcome-dialog > .welcome-header` in the old
//! stylesheet): [`DialogFrame::compact`].
//!
//! NOTE: The old Close changed only its color on hover; here it is a ghost icon button, which gets the ghost tier's
//! hover fill. The old one was a 20-pixel "×" character; here it is the close icon at the icon buttons' size.

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Div, ElementId, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, RenderOnce, Role, ScrollHandle, SharedString, Size,
    Stateful, StatefulInteractiveElement, Styled, Window, div, point, prelude::FluentBuilder as _,
    px,
};

use super::{
    Button, HoverHelp, IconButton, Ring, scroll_from_controls, scroll_handle, scroll_stop,
};
use crate::assets::Icon;
use crate::focus::{self, RegionReport};
use crate::host_text::Line;
use crate::theme::{self, metrics};

/// The body's floor before the whole frame starts scrolling instead: about three lines, as the old `min-height: 3em`.
const BODY_MIN_HEIGHT: f32 = 36.;

/// `.host-settings-mark`: the header tile's square side.
const TILE: f32 = 44.;

/// `.host-settings-footer > span`: the footer note's text size.
const NOTE_TEXT: f32 = 11.;

/// `.launch-composer-notices`: the footer's notices give up height before the actions do, at most about a quarter of
/// the window (`max-height: 24dvh`).
const NOTICES_MAX_SHARE: f32 = 0.24;

/// `.recipe-editor-sticky-actions`' `padding: 12px 0`: the room between ruled-off notices and their rule
/// ([`DialogFrame::notices_rule`]).
const NOTICES_RULE_PAD: f32 = 12.;

/// `.launch-composer-footer-summary`: a footer lead's basis (`flex: 1 1 220px`) before the row wraps the answers.
const FOOTER_LEAD_BASIS: f32 = 220.;

/// The window width from which a frame asking for it keeps a flat vertical margin
/// ([`DialogFrame::flat_margin_when_wide`]): the old launcher backdrop's `@media (min-width: 1000px)`.
const FLAT_MARGIN_MIN_WIDTH: Pixels = px(1000.);

/// A compact frame's header padding, vertical then horizontal: the old Announcements and Welcome dialogs' `10px 14px`.
const COMPACT_HEADER_PAD: (Pixels, Pixels) = (px(10.), px(14.));
/// A compact frame's body padding: the old `12px 14px`.
const COMPACT_BODY_PAD: (Pixels, Pixels) = (px(12.), px(14.));
/// A compact frame's footer padding: the old `8px 14px`.
const COMPACT_FOOTER_PAD: (Pixels, Pixels) = (px(8.), px(14.));

/// A dialog's frame. The title is the accessible name. The title and the summary are each a [`Line`]: the app's own
/// words, or a [`HostSentence`](crate::host_text::HostSentence) when they name a host value, which then is a labelled
/// node of its own inside the dialog. The answers are drawn right-aligned in the order given, the old footer's order
/// (Cancel first, the main action last). A frame with no answers, note, lead or notices draws no footer at all, and one
/// with notices alone draws them without an answers row.
#[derive(IntoElement)]
pub struct DialogFrame {
    id: SharedString,
    title: Line,
    summary: Option<Line>,
    heading: Option<Heading>,
    body: Option<AnyElement>,
    answers: Vec<Button>,
    close: Option<IconButton>,
    close_first: bool,
    tile: Option<AnyElement>,
    note: Option<SharedString>,
    width: Option<Pixels>,
    fixed_height: Option<Pixels>,
    split_body: bool,
    footer_lead: Option<AnyElement>,
    footer_actions: Option<AnyElement>,
    separate_close: bool,
    notices: Option<AnyElement>,
    top_band: Option<AnyElement>,
    notices_rule: bool,
    notices_below: bool,
    compact: bool,
    scroll_stop: Option<SharedString>,
    notices_stop: Option<SharedString>,
    flat_margin_when_wide: bool,
}

/// A custom title occupies the normal title slot. A control keeps its own line with Close and can give its
/// supporting text the whole header width underneath.
enum Heading {
    Title(AnyElement),
    Control {
        field: AnyElement,
        detail: Option<AnyElement>,
        tracker: Option<AnyElement>,
    },
}

impl DialogFrame {
    /// A frame titled `title` around `body`, with `answers` pinned at the bottom.
    ///
    /// `title` is app wording as a string, or a [`HostSentence`](crate::host_text::HostSentence) when it names a host
    /// value (the host setup confirmation's "Set up host · <name>"). A sentence is drawn through the host-text element,
    /// and the dialog's accessible name is its [`Line::accessible_text`], the host value escaped as on screen, so a
    /// title never hands a screen reader the host's raw characters.
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<Line>,
        body: impl IntoElement,
        answers: Vec<Button>,
    ) -> Self {
        DialogFrame {
            id: id.into(),
            title: title.into(),
            summary: None,
            heading: None,
            body: Some(body.into_any_element()),
            answers,
            close: None,
            close_first: false,
            tile: None,
            note: None,
            width: None,
            fixed_height: None,
            split_body: false,
            footer_lead: None,
            footer_actions: None,
            separate_close: false,
            notices: None,
            top_band: None,
            notices_rule: false,
            notices_below: false,
            compact: false,
            scroll_stop: None,
            notices_stop: None,
            flat_margin_when_wide: false,
        }
    }

    /// Give the frame the header Close control: `label` is its accessible name ("Close rename"), `help` its hover help,
    /// and `on_close` what it does, which is the dialog's own business (closing its layer, usually, with whatever its
    /// Cancel does to a draft). It is an ordinary button: a click or Enter or Space while it has focus runs `on_close`.
    ///
    /// `disabled` draws it unavailable, ignoring activation and out of Tab's reach, as a disabled button is. A dialog
    /// passes the same value it gives its Cancel: SPEC.md has Rename's "Close uses the same cancellation and busy-state
    /// rules as Cancel", and the old app disabled both while a rename was saving (`rename.rs` at reference commit
    /// `24bc0580`).
    pub fn close(
        mut self,
        label: impl Into<SharedString>,
        help: HoverHelp,
        disabled: bool,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let id = format!("{}-close", self.id);
        self.close = Some(
            IconButton::new(id, Icon::Close, label, help, Role::Button)
                .disabled(disabled)
                .on_click(on_close),
        );
        self
    }

    /// A one-line summary under the title (the old `.dialog-summary`): app wording as a string, or a
    /// [`HostSentence`](crate::host_text::HostSentence) when it names a host value (the session a delete removes).
    pub fn summary(mut self, summary: impl Into<Line>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Draw `heading` in place of the title, the title staying the dialog's accessible name: for a header that shows
    /// more than a title line and a one-line summary can (the host settings dialog's, whose status line has the phase's
    /// colored dot), so the heading draws its own summary too. A [`DialogFrame::summary`] set beside it is still drawn
    /// under it; no caller sets both. The heading draws any host value through the host-text element, as everywhere.
    pub fn heading(mut self, heading: impl IntoElement) -> Self {
        self.heading = Some(Heading::Title(heading.into_any_element()));
        self
    }

    /// Put the primary control in the heading and align Close to it. The detail occupies a second line across
    /// the header, so hint chips or an explanation can wrap without shifting Close out of the control's row.
    /// The optional tracker covers the whole control row, so a popover can share the form's edges. The frame title
    /// is still its accessible name. Tab visits the control before Close.
    pub(crate) fn heading_control(
        mut self,
        field: impl IntoElement,
        detail: Option<AnyElement>,
        tracker: Option<AnyElement>,
    ) -> Self {
        self.heading = Some(Heading::Control {
            field: field.into_any_element(),
            detail,
            tracker,
        });
        self
    }

    /// Preserve Close-first Tab order for a control header with Close at its visual right edge.
    /// Rename starts in the field but Shift+Tab must still reach Close.
    pub(crate) fn close_first_in_control_header(mut self) -> Self {
        self.close_first = true;
        self
    }

    /// Omit the body when a control header and footer carry the whole dialog. An empty scrolling region
    /// would otherwise reserve the same padding and minimum height as one with content.
    pub(crate) fn without_body(mut self) -> Self {
        self.body = None;
        self
    }

    /// An icon tile left of the title (the old host dialogs' `.host-settings-mark`): `tile` drawn centered in a
    /// 44-pixel square of the raised fill with the medium radius, in the primary text color, so an SVG icon that takes
    /// the text color draws in it. The tile is decoration: the title stays the dialog's accessible name.
    pub fn tile(mut self, tile: impl IntoElement) -> Self {
        self.tile = Some(tile.into_any_element());
        self
    }

    /// A note at the footer's left, the answers staying at its right (the old `.host-settings-footer`'s leading span,
    /// such as Settings' "Most choices save immediately"). It has its own accessible text so it can carry
    /// information independently of the answers. A frame takes this note or a [`Self::footer_lead`].
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// The frame's width in place of the ordinary dialog width. Like the ordinary width, it is capped to the window less
    /// the old backdrop's side margins ([`metrics::DIALOG_SIDE_MARGIN`] each side), so a wide dialog narrows in a narrow
    /// window rather than running off its edges.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    /// A height the frame keeps whatever its body holds, capped by the window as every frame's height is: the body
    /// takes the room the header and footer leave, leaving space below short content and scrolling long content. For a
    /// dialog whose body changes under the user, as Settings' sections do, so the frame and its controls stay put.
    pub fn fixed_height(mut self, height: Pixels) -> Self {
        self.fixed_height = Some(height);
        self
    }

    /// Give a layout with independent scrolling the whole space between the header and footer.
    /// The caller provides its own padding and decides which part of its body scrolls.
    pub(crate) fn split_body(mut self) -> Self {
        self.split_body = true;
        self
    }

    /// Place an editor's own controls in the footer's answer row. Its work status belongs in the notices above it.
    pub(crate) fn footer_actions(mut self, actions: impl IntoElement) -> Self {
        self.footer_actions = Some(actions.into_any_element());
        self
    }

    /// Separate a trailing Close from the editor or save controls that precede it.
    pub(crate) fn separate_close(mut self) -> Self {
        self.separate_close = true;
        self
    }

    /// Something drawn at the start of the answers' row, taking the room the answers leave (the launcher's summary of
    /// what Launch would start, `.launch-composer-footer-summary`). Unlike [`Self::note`], a short line of the
    /// dialog's own words, it is any element; a frame has one or the other.
    pub fn footer_lead(mut self, lead: impl IntoElement) -> Self {
        self.footer_lead = Some(lead.into_any_element());
        self
    }

    /// A region above the answers' row (under it with [`Self::notices_below`]) for what pressing an answer has to say
    /// (the launcher's refusal and YOLO question, under its row). It is pinned with the answers rather than scrolling
    /// with the body, so a refusal is always seen beside the action that caused it, and it scrolls on its own once
    /// taller than about a quarter of the window (`.launch-composer-notices`), so long text never pushes the answers
    /// away. The pointer scrolls it; the keyboard does too once [`Self::notices_stop`] is set.
    pub fn notices(mut self, notices: impl IntoElement) -> Self {
        self.notices = Some(notices.into_any_element());
        self
    }

    /// Rule the notices off from the answers' row below them: a line in the dim border color under the notices, 12
    /// pixels below their content. For notices that are a surface of their own rather than a word about the frame's
    /// answers: Settings pins the recipe editor's Cancel and Save there, which belong to the editor and not to the
    /// dialog's Close, and the old app drew them as a band with its own rule (`.recipe-editor-sticky-actions`,
    /// `border-top: 1px solid var(--border-dim)`) above the footer's. Without notices it draws nothing.
    pub fn notices_rule(mut self) -> Self {
        self.notices_rule = true;
        self
    }

    /// Draw the [`Self::notices`] region under the answers' row instead of above it, where the old launcher kept its
    /// refusal, its Replace and Restart warnings and its YOLO question (`.launch-composer-notices` followed
    /// `.launch-composer-footer-row` in `.launch-composer-footer`'s column at reference commit `24bc0580`). The region
    /// still scrolls on its own and is still pinned; only its side of the row changes. Not for ruled-off notices
    /// ([`Self::notices_rule`]), whose rule separates them from a row below.
    pub fn notices_below(mut self) -> Self {
        self.notices_below = true;
        self
    }

    /// Draw the header, body and footer with the tighter padding the old Announcements and Welcome dialogs had (header
    /// 10 by 14 pixels, body 12 by 14, footer 8 by 14) in place of the ordinary dialog's. Only the padding changes.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Keep a flat [`metrics::DIALOG_VERTICAL_MARGIN`] above and below the frame in a window at least
    /// [`FLAT_MARGIN_MIN_WIDTH`] wide, in place of 5% of the window's height: the old launcher's backdrop rule
    /// (`.launch-composer-backdrop { --dialog-vertical-margin: 24px; }` from 1000 pixels wide, `app.css` at the reference
    /// commit, line 6629), which spends the room on the launcher's rows rather than on empty space above and below it.
    /// Narrower windows keep the ordinary margin.
    pub fn flat_margin_when_wide(mut self) -> Self {
        self.flat_margin_when_wide = true;
        self
    }

    /// Give the body a tab stop named `label` ("Announcement content") whenever it holds no tab stop, as a long
    /// announcement without a link or a list whose every action is disabled holds none (a button counts, whatever Tab
    /// reaches on macOS; a disabled control does not). The keys scroll it as they do the scroll box; `scroll_box.rs`'s
    /// module docs have the full account. The stop follows the body one frame behind ([`focus::ScrollRegion::report`]),
    /// and one taken away while it has focus hands focus to the closest control left, as any control that goes away
    /// does. It has no hover help: a tooltip over the whole body would cover what the user reads. Without this the body
    /// is drawn exactly as before.
    pub fn scroll_stop(mut self, label: impl Into<SharedString>) -> Self {
        self.scroll_stop = Some(label.into());
        self
    }

    /// A band pinned across the top of the body, under the header, that does not scroll with the body: the launcher's
    /// permission-escalation banner (the old `.permission-escalation-banner`, sticky at the top of the launcher's
    /// body). The band brings its own look; the frame only places it.
    pub fn top_band(mut self, band: impl IntoElement) -> Self {
        self.top_band = Some(band.into_any_element());
        self
    }

    /// Make the [`Self::notices`] region scrollable from the keyboard, for a question there whose text can outgrow it
    /// (the Trash dialog's global Delete all question, whose heading and the line naming what it deletes would
    /// otherwise stay above view while an answer holds focus). Two parts, both the old region's:
    ///
    /// - Page Up, Page Down, Home and End scroll the region while a control inside it holds focus
    ///   ([`scroll_from_controls`]), as the browser scrolled the region around a focused answer. The arrows and Space
    ///   stay the control's.
    /// - Whenever the region holds no tab stop (every answer disabled), it is a stop of its own named `label`, under
    ///   [`Self::scroll_stop`]'s rule and with its keys, in Tab order after the body and before the answers' row. Its
    ///   keys act only while the stop itself holds focus. Its ring reaches out to the card's sides, as the old
    ///   full-width region's did, and two pixels above and below the notices.
    ///
    /// The old Trash dialog drew its question in the shared `ScrollRegion` named "Global checkout deletion
    /// confirmation" (`crates/farhelm-ui/src/list/trash.rs` lines 1474 to 1479 at reference commit `24bc0580`), the
    /// same component as a dialog's body: a tab stop with the region role and that name only while no control inside
    /// it was in Tab order (`crates/farhelm-ui/src/modal_dialog.rs` lines 599 to 604), with the ring two pixels inside
    /// its edge (`crates/farhelm-ui/assets/app.css` lines 9498 to 9500). With its answers usable it was no stop, and
    /// the warning stayed "reachable by scrolling upwards" (`list/trash.rs` lines 658 to 661) through WebKit, which
    /// scrolls the focused element's scrolling container on those keys. Without this, the region is drawn exactly as
    /// before.
    pub fn notices_stop(mut self, label: impl Into<SharedString>) -> Self {
        self.notices_stop = Some(label.into());
        self
    }
}

impl RenderOnce for DialogFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let name = self.title.accessible_text();
        let has_body = self.body.is_some();
        let ((head_y, head_x), (pad_y, pad_x), (foot_y, foot_x)) = if self.compact {
            (COMPACT_HEADER_PAD, COMPACT_BODY_PAD, COMPACT_FOOTER_PAD)
        } else {
            (
                metrics::DIALOG_PAD,
                metrics::DIALOG_PAD,
                metrics::DIALOG_FOOTER_PAD,
            )
        };
        let area = crate::desktop::linux_title_bar::content_area(window).size;
        let limits = frame_limits(area, self.width, self.flat_margin_when_wide);
        let (width, max_height) = (limits.width, limits.height);

        let control_header = matches!(&self.heading, Some(Heading::Control { .. }));
        let mut heading = div().flex_1().min_w_0();
        let (detail, tracker) = match self.heading {
            Some(Heading::Title(drawn)) => {
                heading = heading.child(drawn);
                (None, None)
            }
            Some(Heading::Control {
                field,
                detail,
                tracker,
            }) => {
                heading = heading.child(field);
                (detail, tracker)
            }
            None => {
                heading = heading.child(
                    div()
                        // The id lets a capture name a spot to park the pointer on. Not `-title`: a question's
                        // host-value title already draws under `prompt-title`, and `settings-heading` is taken.
                        .id(ElementId::Name(format!("{}-frame-title", self.id).into()))
                        .text_size(metrics::TEXT_TITLE)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::color(palette.fg_0))
                        .child(self.title),
                );
                (None, None)
            }
        };
        if let Some(summary) = self.summary {
            heading = heading.child(
                div()
                    .mt(px(6.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1))
                    .child(summary),
            );
        }
        let tile = self.tile.map(|tile| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(TILE))
                .rounded(metrics::RADIUS_MD)
                .bg(theme::color(palette.bg_1))
                .text_color(theme::color(palette.fg_0))
                .child(tile)
        });
        let header_row = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(if control_header { 8. } else { 14. }));
        // Keep title-header Close level with the title, even if the summary wraps.
        let close = self.close.map(|close| {
            div()
                .flex_shrink_0()
                .when(!control_header, Styled::self_start)
                .child(close)
        });
        let header_row = if control_header && self.close_first {
            // The frame records Tab stops in child order. Reversing the layout keeps the field visually first.
            header_row
                .flex_row_reverse()
                .children(close)
                .child(heading)
                .children(tile)
        } else {
            header_row.children(tile).child(heading).children(close)
        }
        .children(tracker);
        let header = if control_header {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(header_row)
                .children(detail)
        } else {
            header_row
        }
        .flex_shrink_0()
        .py(if control_header { px(12.) } else { head_y })
        .px(head_x)
        .bg(theme::color(palette.control_hover))
        .border_b_1()
        .border_color(theme::color(palette.border_dim));
        // Both scrolling parts scroll a control Tab or the app focuses into view (the focus coordinator's
        // `scroll_region`): the body for its fields, the whole card for the header's Close and the answers when even
        // they do not fit. Their scroll handles live in keyed element state, since a frame is rebuilt on every draw.
        let body_scroll = scroll_handle(format!("{}-body-scroll", self.id), window, cx);
        let card_scroll = scroll_handle(format!("{}-card-scroll", self.id), window, cx);
        let body_slot = if self.split_body { "panes" } else { "body" };
        let body = div()
            .id(ElementId::Name(format!("{}-{body_slot}", self.id).into()))
            .debug_selector(|| format!("{}-{body_slot}", self.id))
            .flex_shrink_1()
            .when(self.split_body, |body| body.flex().flex_col())
            .when(!self.split_body, |body| {
                body.overflow_y_scroll().py(pad_y).px(pad_x)
            })
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_0))
            .children(self.body);
        // Page Up, Page Down, Home and End from a control in the body scroll it, as WebKit scrolled the container
        // around a focused control (SPEC_impl.md, "The component layer and theme"). With a scroll stop drawn, the stop
        // holds focus around the body rather than in it, so this never sees the stop's keys.
        let body = if self.split_body {
            body
        } else {
            scroll_from_controls(body, &body_scroll)
        };
        // How the body sits in the card: it gives up height down to its floor, and a fixed-height frame's body takes
        // whatever the header and footer leave. With a scroll stop, that is the stop's place, and the body fills it.
        let fixed = self.fixed_height.is_some();
        let place = |element: Stateful<Div>| {
            element
                .flex_shrink_1()
                .when(fixed, |element| element.flex_grow(1.))
                .min_h(px(BODY_MIN_HEIGHT))
        };
        let body = match self.scroll_stop {
            None if self.split_body => place(body).into_any_element(),
            None => focus::scroll_region(place(body), &body_scroll).into_any_element(),
            // The body has no border: the ring lies along the stop's edge, inside the card, which would clip anything
            // outside.
            Some(label) => place(region_with_stop(
                &format!("{}-body", self.id),
                label,
                Ring {
                    inset_x: px(0.),
                    inset_y: px(0.),
                    radius: px(0.),
                },
                body.flex_grow(1.).min_h_0(),
                &body_scroll,
                window,
                cx,
            ))
            .into_any_element(),
        };
        // A frame with nothing for its footer to hold draws none: the Templates dialog, whose editor carries its own
        // action row inside the body (SPEC.md: "the complete editor keeps scrolling within the dialog, including its
        // editor-local action footer").
        let has_row = !self.answers.is_empty()
            || self.note.is_some()
            || self.footer_lead.is_some()
            || self.footer_actions.is_some();
        let has_footer = has_row || self.notices.is_some();
        let mut buttons = self.answers;
        let close = self.separate_close.then(|| buttons.pop()).flatten();
        let answers = div()
            .flex()
            .flex_wrap()
            .justify_end()
            .items_center()
            .gap(px(8.))
            .children(self.footer_actions)
            .children(buttons.into_iter().map(Button::help_below))
            .children(close.map(|close| {
                div()
                    .flex_none()
                    .border_l_1()
                    .border_color(theme::color(palette.border_dim))
                    .pl(px(8.))
                    .child(close.help_below())
            }));
        // The answers' row. Every shape lets the answers wrap at the footer's width: a row item never shrinks below its
        // content unless told to (`min_w_0`), and answers that cannot wrap are cut at the frame's start edge, Cancel
        // first.
        debug_assert!(
            self.note.is_none() || self.footer_lead.is_none(),
            "a dialog footer has a note or a lead, not both"
        );
        let row = match (self.note, self.footer_lead) {
            // `.host-settings-footer`: the note and the answers at either end, 12 pixels apart at least, the answers
            // wrapping below the note when the row is too narrow for both. Wrapped, they stay right-aligned on their
            // own line (the auto left margin), as the old host setup confirmation's did under its long note.
            (Some(note), _) => div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .id(ElementId::Name(format!("{}-footer-note", self.id).into()))
                        .role(Role::Label)
                        .aria_label(note.clone())
                        .min_w_0()
                        .text_size(px(NOTE_TEXT))
                        .text_color(theme::color(palette.fg_1))
                        .child(note),
                )
                .child(answers.ml_auto().min_w_0()),
            // `.launch-composer-footer-row`: the lead takes the room the answers leave and is cut rather than wrapped,
            // 16 pixels from them; the answers wrap onto a line of their own, 8 pixels below, when both do not fit.
            (None, Some(lead)) => div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_end()
                .gap_x(px(16.))
                .gap_y(px(8.))
                .child(
                    div()
                        .flex_1()
                        .flex_basis(px(FOOTER_LEAD_BASIS))
                        .min_w_0()
                        .child(lead),
                )
                .child(answers.min_w_0()),
            // The answers alone, stretched across the footer, right-aligned.
            (None, None) => answers,
        };
        // A scroll region like the body, so a control inside (a question's answers) is scrolled into view when focus
        // reaches it.
        let notices_stop = self.notices_stop;
        let notices = self.notices.map(|notices| {
            let scroll = scroll_handle(format!("{}-notices-scroll", self.id), window, cx);
            let region = div()
                .id(ElementId::Name(format!("{}-notices", self.id).into()))
                .debug_selector(|| format!("{}-notices", self.id))
                .max_h(area.height * NOTICES_MAX_SHARE)
                .overflow_y_scroll()
                .when(self.notices_rule, |region| {
                    region
                        .pb(px(NOTICES_RULE_PAD))
                        .border_b_1()
                        .border_color(theme::color(palette.border_dim))
                })
                .child(notices);
            let Some(label) = notices_stop else {
                return focus::scroll_region(region, &scroll).into_any_element();
            };
            // The old region spanned the dialog's width with its text 24 pixels in (`app.css` line 9525 at the
            // reference commit), and its ring lay two pixels inside that edge. The notices sit inside the footer's
            // padding instead, so the ring's sides reach out across it to the card's inner edge, where the body's ring
            // lies too; its top and bottom sit two pixels outside the notices, inside the footer's top padding and the
            // gap above the answers' row (under the row, [`DialogFrame::notices_below`]: the gap below it and the
            // footer's bottom padding), rather than over the text.
            let ring = Ring {
                inset_x: -foot_x,
                inset_y: px(-2.),
                radius: px(0.),
            };
            let region = scroll_from_controls(region, &scroll);
            let notices = format!("{}-notices", self.id);
            region_with_stop(&notices, label, ring, region, &scroll, window, cx).into_any_element()
        });
        let footer = div()
            .debug_selector(|| format!("{}-answers", self.id))
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap(px(8.))
            .py(foot_y)
            .px(foot_x)
            .bg(theme::color(palette.bg_1))
            .border_t_1()
            .border_color(theme::color(palette.border_dim));
        // A footer of notices alone (Add host's recipe editor, whose answers are its notices) draws no empty row, which
        // would leave the footer's gap below them.
        let row = has_row.then_some(row);
        let footer = if self.notices_below {
            footer.children(row).children(notices)
        } else {
            footer.children(notices).children(row)
        };

        let card = div()
            .id(ElementId::Name(self.id.clone()))
            .debug_selector(|| self.id.to_string())
            .role(Role::Dialog)
            .aria_label(name)
            .flex()
            .flex_col()
            .w(width)
            .when_some(self.fixed_height, |frame, height| frame.h(height))
            .max_h(max_height)
            .overflow_y_scroll()
            .bg(theme::color(palette.bg_2))
            .border_1()
            .border_color(theme::color(palette.border_dialog))
            .rounded(metrics::RADIUS_MD)
            .shadow(dialog_shadow(palette))
            .child(header)
            // Pinned like the header: it neither scrolls with the body nor shrinks. Clipped, so a band that moves into
            // place (the escalation banner's slide) never draws over the header.
            .children(
                self.top_band
                    .map(|band| div().flex_shrink_0().overflow_hidden().child(band)),
            )
            .children(has_body.then_some(body))
            .when(has_footer, |card| card.child(footer));
        focus::scroll_region(card, &card_scroll)
    }
}

/// The scrolling `region` (scroll state `scroll`) of one of the frame's parts, drawn under `part` (`<frame id>-body`
/// or `<frame id>-notices`), inside the box that is its scroll stop named `label` ([`DialogFrame::scroll_stop`],
/// [`DialogFrame::notices_stop`]), with its ring at `ring`. The box is drawn in every frame, so the region's layout
/// never changes with the stop; only whether the box is a stop does, read from what the region reported the last time
/// it was prepared. The caller places the box where the region would otherwise sit.
fn region_with_stop(
    part: &str,
    label: SharedString,
    ring: Ring,
    region: Stateful<Div>,
    scroll: &ScrollHandle,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let report: RegionReport = window
        .use_keyed_state(
            ElementId::Name(format!("{part}-report").into()),
            cx,
            |_, _| RegionReport::default(),
        )
        .read(cx)
        .clone();
    let holds_stops = report.get();
    let region = focus::scroll_region(region, scroll).report(&report);
    let stop_id: SharedString = format!("{part}-stop").into();
    let stop = div()
        .id(ElementId::Name(stop_id.clone()))
        .debug_selector(|| stop_id.to_string())
        .flex()
        .flex_col()
        .child(region);
    if holds_stops {
        return stop;
    }
    scroll_stop(
        stop,
        &stop_id,
        scroll,
        Role::Region,
        label,
        ring,
        window,
        cx,
    )
}

/// `--shadow-dialog`: `0 10px 24px` in the token's color, under the dialog frame and under the one modal panel the old
/// app drew without that frame (the quick switcher's card), so the two cannot drift apart.
pub(crate) fn dialog_shadow(palette: &theme::Palette) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: theme::color(palette.shadow_dialog),
        offset: point(px(0.), px(10.)),
        blur_radius: px(24.),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// The widest and tallest a frame may be in a window whose content area is `area`
/// ([`crate::desktop::linux_title_bar::content_area`]): the frame's own `width` (or the ordinary dialog width) kept 20
/// pixels inside each side, and the old height cap. The old app measured both against its page, which is the content
/// area here.
///
/// The height cap is `max-height: calc(100dvh - 2 * var(--dialog-vertical-margin))`, with the margin
/// `max(24px, 5dvh)`: on a tall window a dialog that fills the cap (Settings, say) keeps 5% of the height above and
/// below it, not a flat 24 pixels, except in a frame that asks for the old launcher's flat margin in a wide window
/// (`flat_margin_when_wide`, [`DialogFrame::flat_margin_when_wide`]). (The old `max-width: 620px` rule, which shrank
/// the margin to `max(12px, 3dvh)`, never applies: the window is at least 720 pixels wide.)
///
/// The width cap is the old `width: min(Npx, 100%)` inside a backdrop padded 20 pixels each side. The layer manager
/// centers a dialog without constraining it, so `max_w_full` in the frame would be relative to a parent as wide as the
/// card itself and cap nothing; the frame caps its own width instead.
pub(super) fn frame_limits(
    area: Size<Pixels>,
    width: Option<Pixels>,
    flat_margin_when_wide: bool,
) -> Size<Pixels> {
    let vertical_margin = if flat_margin_when_wide && area.width >= FLAT_MARGIN_MIN_WIDTH {
        metrics::DIALOG_VERTICAL_MARGIN
    } else {
        metrics::DIALOG_VERTICAL_MARGIN.max(area.height * 0.05)
    };
    Size {
        width: width
            .unwrap_or(metrics::DIALOG_WIDTH)
            .min(area.width - metrics::DIALOG_SIDE_MARGIN * 2.),
        height: area.height - vertical_margin * 2.,
    }
}
