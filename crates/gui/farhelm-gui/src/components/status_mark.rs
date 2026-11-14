//! A status mark: a short piece of text that says something about the state of things (the host rail's "?" when the
//! host list could not be loaded), with words for assistive technology and hover help, and nothing to press.
//!
//! The old app drew these as a `span` with `role="status"`, an `aria-label` and a `data-tooltip`: not a control, so not
//! a tab stop, and a click on it does nothing. This is that span. Its constructor takes the label, hover help and role
//! every control takes, so a status the user can only see a symbol of still has its words for hover and for a screen
//! reader; the shown text is usually a symbol and is no substitute for the label. Its ink is a theme token, the quiet
//! foreground unless the caller names another ([`StatusMark::ink`]).
//!
//! A status that is more than a word (the Hosts panel's phase dot with its words, a host's identity mark) draws what
//! the caller built in place of the text ([`StatusMark::content`]), the way [`super::Button::content`] does, and keeps
//! the label, help and role. The help is also the mark's accessible description unless the caller says it only
//! repeats the name ([`StatusMark::described`]).

use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div,
};

use super::HoverHelp;
use crate::theme::{self, Token, metrics};

/// A non-interactive status with its label, hover help and role. See the module docs.
#[derive(IntoElement)]
pub struct StatusMark {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    text: SharedString,
    ink: Option<Token>,
    content: Option<AnyElement>,
    described: bool,
}

impl StatusMark {
    /// A status mark. `id` must be stable and unique among its siblings (it keys the tooltip owner, and tests and the
    /// bridge find the mark by it); `label` is its accessible name; `role` is what assistive technology and the test
    /// bridge see (usually [`Role::Status`]); `text` is what it shows.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
        text: impl Into<SharedString>,
    ) -> Self {
        StatusMark {
            id: id.into(),
            label: label.into(),
            help,
            role,
            text: text.into(),
            ink: None,
            content: None,
            described: true,
        }
    }

    /// Draw the text in `ink` instead of the quiet foreground (`--fg-1`), as a warning's amber.
    pub fn ink(mut self, ink: Token) -> Self {
        self.ink = Some(ink);
        self
    }

    /// Draw `content` in place of the text: a mark, or a dot beside words. It inherits the mark's text size and ink;
    /// the label stays the mark's accessible name, since what it shows may say nothing to a screen reader.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Whether the hover help is also the mark's accessible description (it is unless told otherwise). Off for a status
    /// whose help is its name again, which a screen reader would otherwise read twice; the help still shows on hover.
    pub fn described(mut self, described: bool) -> Self {
        self.described = described;
        self
    }
}

impl RenderOnce for StatusMark {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let ink = self.ink.unwrap_or(theme::palette(cx).fg_1);
        let mark = div()
            .id(ElementId::Name(self.id.clone()))
            // Tests find a mark by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(ink));
        let mark = match self.content {
            Some(content) => mark.child(content),
            None => mark.child(self.text),
        };
        super::with_hover_help(mark, &self.id, &self.help, self.described, window, cx)
    }
}
