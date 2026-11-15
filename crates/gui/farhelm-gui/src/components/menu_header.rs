//! A menu's header: what the menu is about, named above its commands, which no arrow key or Tab ever lands on.
//!
//! Built as a component because the migration's orchestrator decided on 2026-11-14 that the host menu and the session
//! menu share one header rather than each drawing its own.
//!
//! Ported from the old app's `menu_panel::MenuHeader` and `clamp_title` (`menu_panel.rs` at reference commit `24bc0580`)
//! and the stylesheet's `.menu-header`, `.menu-title` and `.menu-summary` rules. Both old row menus, a session's and a
//! host's, drew their subject this way: a title (the session's or host's name) and a one-line summary under it (the
//! agent and status, or the destination and build), above a hairline. A menu draws it before the element with the menu
//! role, so the header is read as the menu's subject, not as one of its commands.
//!
//! It draws exactly what the session row menu's own header draws today (`session_list::menu`'s `header`), so that menu
//! can move onto this component with no change on screen; the host menu uses it already. [`MenuHeader::clamp_name`] is
//! the old `clamp_title`, for the accessible name of a menu and of the button that opens it, which the session menu's
//! `clamp_name` also is.
//!
//! The Help menu's header is the app's own words ("Help" over "Farhelm guides and feedback", the old `BarMenuFlyout`'s),
//! so [`MenuHeader::app`] draws a title that is not host text; everything else about the look is shared.
//!
//! NOTE: The old header carried hover help naming the full title and summary, for a header cut short by its line
//! clamps. Neither row menu ported so far draws that help, and this component does not either.

use gpui::{
    App, FontWeight, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _,
    Window, div, px,
};

use crate::host_text::{self, HostSentence, HostText};
use crate::theme::{self, metrics};

/// How many characters of a name an accessible name keeps: a title or destination has no length bound, and an
/// accessible name is read aloud in full. 64 characters tell rows apart, and the ellipsis says something was cut (the
/// old `clamp_title`'s `MAX_CHARS`).
const NAME_CHARS: usize = 64;

/// `.menu-summary`'s size.
const SUMMARY_TEXT: f32 = 11.;

/// A menu's header: `title` over `summary`, both host text, or both the app's words (see the module docs).
#[derive(IntoElement)]
pub struct MenuHeader {
    title: Title,
    summary: HostSentence,
}

/// What a header's title is: a host value, drawn through the escaping element, or the app's own words.
enum Title {
    Host(HostText),
    App(SharedString),
}

impl MenuHeader {
    /// A header naming `title` (a host value, drawn escaped and cut after three lines) over `summary` (one sentence
    /// mixing the app's words with host values, cut after four lines).
    pub fn new(title: &HostText, summary: HostSentence) -> Self {
        MenuHeader {
            title: Title::Host(title.clone()),
            summary,
        }
    }

    /// A header whose `title` and `summary` are both the app's own words, for a menu about the app rather than a host
    /// value (the Help menu). Drawn exactly as [`MenuHeader::new`] draws, with nothing to escape.
    pub fn app(title: impl Into<SharedString>, summary: impl Into<SharedString>) -> Self {
        MenuHeader {
            title: Title::App(title.into()),
            summary: HostSentence::new("menu-header-summary").text(summary),
        }
    }

    /// `name` escaped as the screen shows it and cut to 64 characters with an ellipsis, for an accessible name ("Host
    /// actions for …"). Never cut inside an escape (`<U+202E>`), which would otherwise read as different characters: a cut
    /// that would split one drops the whole escape instead, as the old `clamp_title` backed off from it.
    pub fn clamp_name(name: &HostText) -> String {
        let name = HostSentence::new("menu-header-name")
            .host(name)
            .accessible_name();
        let Some((cut, _)) = name.char_indices().nth(NAME_CHARS) else {
            return name.to_string();
        };
        let mut kept = &name[..cut];
        if let Some(open) = kept.rfind("<U+")
            && !kept[open..].contains('>')
        {
            kept = &kept[..open];
        }
        format!("{kept}…")
    }
}

impl RenderOnce for MenuHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        // `.menu-header`: the title and the summary 2 pixels apart, inside the old padding, above a hairline that sets
        // the commands off 4 pixels below.
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .min_w_0()
            .pt(px(6.))
            .pb(px(8.))
            .px(px(11.))
            .mb(px(4.))
            .border_b_1()
            .border_color(theme::color(palette.border_dim))
            .child(
                match self.title {
                    Title::Host(title) => host_text::element(&title),
                    Title::App(title) => div().child(title),
                }
                .text_size(metrics::TEXT_SM)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::color(palette.fg_0))
                .line_clamp(3),
            )
            .child(
                div()
                    .text_size(px(SUMMARY_TEXT))
                    .text_color(theme::color(palette.fg_1))
                    .line_clamp(4)
                    .child(self.summary),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A long name must not make an accessible name run on, and cutting it must not leave half an escape that reads as
    /// some other character. Specified: a name within the limit is kept whole; a longer one is cut at the limit with an
    /// ellipsis; a cut that would split a `<U+XXXX>` escape drops the whole escape instead; a hidden character is named
    /// by its escape, as the screen shows it.
    #[test]
    fn a_long_name_is_cut_with_an_ellipsis_and_never_inside_an_escape() {
        let clamp = |raw: &str| MenuHeader::clamp_name(&HostText::from_host(raw));
        assert_eq!(clamp("short"), "short");
        assert_eq!(clamp(&"a".repeat(80)), format!("{}…", "a".repeat(64)));
        let split = format!("{}\u{202E}tail", "b".repeat(60));
        assert_eq!(clamp(&split), format!("{}…", "b".repeat(60)));
        assert_eq!(clamp("x\u{202E}y"), "x<U+202E>y");
    }
}
