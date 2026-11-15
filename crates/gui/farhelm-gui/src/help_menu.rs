//! The host rail foot's `?` button and the Help menu it opens beside itself.
//!
//! SPEC.md, "Feedback": "The host rail's foot has a **?** button below the settings gear. It opens a small menu", with
//! its items in a fixed order. Ported from the old app's `app_bar.rs` at reference commit `24bc0580`: `BarMenuItem` for
//! each item's label, description and hover help, `help_menu_items` for the order, and `BarMenuToggle` and
//! `BarMenuFlyout` for the toggle and the flyout beside the rail. Here's the TLDR of how it behaves:
//!
//! - **A view of its own, placed by the rail.** [`HelpButton`] keeps the button's focus handle and the menu it opened,
//!   so the session list only creates it and draws it in the rail's foot. The button wears the expanded look while its
//!   menu is open (the old `.app-help-toggle[aria-expanded="true"]`), and a second press on it closes the menu.
//! - **The menu is a popover beside the button.** It is drawn as the session row's menu is (the same panel, header and
//!   items, 246 pixels wide, `Anchor::beside`), and names the `?` button as where closing returns focus, so Escape and
//!   every item leave focus there, also on macOS, where the click that opened the menu left the button unfocused.
//! - **An item closes the menu first, then acts.** A link item opens its page in the system browser through
//!   `cx.open_url`, the opener the app's other docs links use; Send feedback and SSH needs 2FA? open their dialogs
//!   ([`crate::feedback::open`], [`crate::hosts::ssh_help::open`]) with the `?` button as their opener, so closing
//!   either returns focus to the button, as the old app returned focus to its Help toggle; Check for updates asks the
//!   updater ([`crate::desktop::updates::check_now`]).
//! - **Conditional items.** Check for updates is drawn only while an updater runs ([`crate::desktop::updates::runs`]),
//!   last, as SPEC.md says. On Linux, which has no menu bar, the menu also offers Open-Source Notices, which dispatches
//!   the same action as the Mac app menu's item (the maintainer decided on 2026-11-13 that the notices live in that
//!   window, and the GPUI migration's orchestrator put the Linux item in this menu). SPEC.md does not place it, so it
//!   comes last.
//! - **Announcements** appears directly after Release notes once this helm has shown an announcement (the pushed
//!   queue's history is not empty, read when the menu draws) and opens that history
//!   ([`crate::announcements::open_history`]) with the `?` button as its opener.
//!
//! - **Welcome** reopens the Welcome card ([`crate::welcome::open`]) with the `?` button as its opener; a reopened card
//!   writes nothing.
//!
//! NOTE: Not drawn yet: Keyboard shortcuts, waiting for its dialog. It joins [`items`] in the old order (after
//! Documentation) when its dialog lands; a menu item whose dialog does not exist would open nothing.
//!
//! NOTE: Deviations from the old flyout, both from the layer manager's one beside placement (its `Placement::Beside`
//! docs record the shared clamp): the old Help menu hung from its toggle's bottom edge and kept at least 8 pixels from
//! the window's edges, where this one is placed from the toggle's top and kept flush inside the window, so at the
//! rail's foot it sits on the window's bottom edge; and the old flyout's pointer notch toward the toggle is not drawn
//! (the session row's menu has the same gap). The old toggle's `aria-haspopup` is not ported either, as for every menu
//! button so far.

use gpui::{
    AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    WeakEntity, Window, div, px, svg,
};

use crate::actions::OpenSourceNotices;
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, MenuHeader, MenuItem, Role, Tier};
use crate::focus::{self, Mark};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::theme;

#[cfg(test)]
mod tests;

/// The `?` button's element id, the old `.app-help-toggle`.
const BUTTON_ID: &str = "app-help-toggle";

/// The menu panel's width, the old `.host-row-menu-flyout`'s, which the bar menus shared with the row menus.
const PANEL_WIDTH: f32 = 246.;

/// The help glyph's size in the button: the Settings gear's 16-pixel box, so the two read as one pair of controls (the
/// old `HelpIcon`'s comment).
const GLYPH: f32 = 16.;

/// The docs site Documentation opens (SPEC.md, "Feedback").
const DOCS_URL: &str = "https://farhelm.io/docs/";

/// The project's Discord invite, shared with the README and the website's header (SPEC.md, "Feedback"), and with the
/// feedback dialog's own Chat on Discord link, which SPEC.md has open the same invite.
pub(crate) const DISCORD_URL: &str = "https://discord.gg/G3bukxTjXR";

/// The general release notes page Release notes opens, whether or not an update is waiting (SPEC.md, "Feedback").
const RELEASE_NOTES_URL: &str = "https://farhelm.io/docs/release-notes/";

/// One item the Help menu can offer. The order of [`items`] is the menu's; the variants are listed in it too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    /// The Send feedback dialog, a private message to the maintainer.
    SendFeedback,
    ChatOnDiscord,
    Documentation,
    /// Always offered, even when no installed update waits for a restart.
    ReleaseNotes,
    /// The history of announcements this helm has shown; offered only once it has shown one.
    Announcements,
    /// Commands to start a host's SSH connections by hand, for a host that asks for approval on every login.
    SshHelp,
    /// The Welcome card again, writing nothing.
    Welcome,
    /// Offered only while an updater runs (SPEC.md, "Installation and updates").
    CheckForUpdates,
    /// Linux only: the app menu's notices window, which Linux has no menu bar to reach.
    OpenSourceNotices,
}

impl Item {
    /// The item's label, which is also its accessible name.
    fn label(self) -> &'static str {
        match self {
            Item::SendFeedback => "Send feedback",
            Item::ChatOnDiscord => "Chat on Discord",
            Item::Documentation => "Documentation",
            Item::ReleaseNotes => "Release notes",
            Item::Announcements => "Announcements",
            Item::SshHelp => "SSH needs 2FA?",
            Item::Welcome => "Welcome",
            Item::CheckForUpdates => "Check for updates",
            Item::OpenSourceNotices => crate::desktop::licenses::TITLE,
        }
    }

    /// The description line under the label, the old `BarMenuItem::description`.
    fn description(self) -> &'static str {
        match self {
            Item::SendFeedback => "Privately, to Farhelm's maintainer",
            Item::ChatOnDiscord => "Opens Farhelm's Discord in your browser",
            Item::Documentation => "Opens farhelm.io/docs in your browser",
            Item::ReleaseNotes => "Opens the release notes in your browser",
            Item::Announcements => "Shows previously shown announcements again",
            Item::SshHelp => "Start a host's SSH connections yourself",
            Item::Welcome => "Shows the welcome card again",
            Item::CheckForUpdates => "Installs a newer Farhelm if there is one",
            Item::OpenSourceNotices => "Licenses of the code and fonts in this app",
        }
    }

    /// The item's hover help, the old `BarMenuItem::tooltip`: a little more than its description line. The old app gave
    /// every bar menu item one, although a described item may go without (see `MenuItem`'s module docs).
    fn help(self) -> &'static str {
        match self {
            Item::SendFeedback => {
                "Send feedback: a private message to Farhelm's maintainer, not a public issue"
            }
            Item::ChatOnDiscord => {
                "Chat on Discord: open Farhelm's Discord in your browser, to talk with the maintainer and other users in the open"
            }
            Item::Documentation => {
                "Documentation: open the Farhelm documentation at farhelm.io/docs in your browser"
            }
            Item::ReleaseNotes => {
                "Release notes: open Farhelm's release notes at farhelm.io/docs in your browser, each release with what changed"
            }
            Item::Announcements => {
                "Announcements: show the announcements Farhelm has shown you before, newest first, such as serious changes you needed to act on"
            }
            Item::SshHelp => {
                "SSH needs 2FA?: commands to start Farhelm's connections by hand when a host asks for approval on every SSH login"
            }
            Item::Welcome => {
                "Welcome: show the welcome card with Get started and how to send feedback"
            }
            Item::CheckForUpdates => {
                "Check for updates: look for a newer Farhelm release now and install it in the background; the version readout shows how it went"
            }
            Item::OpenSourceNotices => {
                "Open-Source Notices: show the licenses of the open-source code and fonts this app includes"
            }
        }
    }

    /// The item's decorative glyph, the old `BarMenuItem::glyph`.
    fn icon(self) -> Icon {
        match self {
            Item::SendFeedback => Icon::Feedback,
            Item::ChatOnDiscord => Icon::External,
            Item::Documentation | Item::ReleaseNotes | Item::OpenSourceNotices => Icon::Document,
            Item::Announcements | Item::Welcome => Icon::Info,
            Item::SshHelp => Icon::Terminal,
            Item::CheckForUpdates => Icon::Update,
        }
    }

    /// The item's element id: `help-menu-` and its label's words.
    fn id(self) -> &'static str {
        match self {
            Item::SendFeedback => "help-menu-send-feedback",
            Item::ChatOnDiscord => "help-menu-discord",
            Item::Documentation => "help-menu-documentation",
            Item::ReleaseNotes => "help-menu-release-notes",
            Item::Announcements => "help-menu-announcements",
            Item::SshHelp => "help-menu-ssh-help",
            Item::Welcome => "help-menu-welcome",
            Item::CheckForUpdates => "help-menu-check-for-updates",
            Item::OpenSourceNotices => "help-menu-open-source-notices",
        }
    }
}

/// The items the menu offers, in order: the fixed ones in SPEC.md's order, Send feedback first and Welcome last among
/// them, with Announcements directly after Release notes once this helm has shown one (`announced`), then Check for
/// updates when an updater runs (`updater`), then Open-Source Notices on Linux (`linux`). Parameters rather than
/// lookups, so the order is tested for every combination on any platform.
fn items(updater: bool, announced: bool, linux: bool) -> Vec<Item> {
    let mut items = vec![
        Item::SendFeedback,
        Item::ChatOnDiscord,
        Item::Documentation,
        Item::ReleaseNotes,
    ];
    if announced {
        items.push(Item::Announcements);
    }
    items.push(Item::SshHelp);
    items.push(Item::Welcome);
    if updater {
        items.push(Item::CheckForUpdates);
    }
    if linux {
        items.push(Item::OpenSourceNotices);
    }
    items
}

/// The `?` button's hover help, naming what the menu offers, as the old toggle's did.
fn button_help(updater: bool) -> &'static str {
    if updater {
        "Help: send feedback, open the documentation or release notes, chat on Discord, get SSH 2FA help, show Welcome, or check for updates"
    } else {
        "Help: send feedback, open the documentation or release notes, chat on Discord, get SSH 2FA help, or show Welcome"
    }
}

/// The `?` button at the host rail's foot, with the Help menu it opens (see the module docs). Created once by the
/// session list, which draws it in the foot for the window's life.
pub struct HelpButton {
    /// The button's focus handle, a tab stop, which the menu and the dialogs its items open name as where closing
    /// returns focus.
    focus: FocusHandle,
    /// Where the button was last drawn, for the menu to open beside: element state, so it is taken while rendering.
    anchor: Option<Anchor>,
    /// The menu this button opened last; checked with the layer manager whenever it is read, so a menu closed some
    /// other way (Escape, a press outside, a reset) needs no bookkeeping here.
    menu: Option<LayerId>,
}

impl HelpButton {
    /// A button with no menu open.
    pub fn new(cx: &mut Context<Self>) -> Self {
        HelpButton {
            focus: cx.focus_handle().tab_stop(true),
            anchor: None,
            menu: None,
        }
    }

    /// The menu this button opened, if it is still open.
    fn open_menu(&self, window: &Window, cx: &gpui::App) -> Option<LayerId> {
        self.menu.filter(|menu| layers::is_open(*menu, window, cx))
    }

    /// The button was pressed: close its menu if it is open, otherwise open it beside the button, starting on its first
    /// item. The mark is taken here, in the press's handler, so the menu opens for this press and not for one the user
    /// has since moved on from. Visible to the crate for the screenshot goldens, which open the menu without a click.
    pub(crate) fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_menu(window, cx).is_some() {
            self.close_menu(window, cx);
            return;
        }
        let Some(anchor) = self.anchor.clone() else {
            return;
        };
        let mark = Mark::take(cx);
        let first = cx.focus_handle().tab_stop(true);
        let button = cx.weak_entity();
        let opener = self.focus.clone();
        let start = first.clone();
        let view = cx.new(|_| HelpMenu {
            button,
            opener,
            first,
        });
        self.menu = layers::open_popover(
            // Tab stays inside the open menu, as the old bar menus kept it (`handle_menu_key`), like the row menus.
            Popover::menu(view, cx.focus_handle(), anchor.beside(), start)
                // The button, not what held focus before: on macOS the click that opened the menu left the button
                // unfocused, and the old menu returned focus to its toggle.
                .returns_to(focus::ReturnTo::Control(self.focus.clone())),
            &mark,
            window,
            cx,
        );
        cx.notify();
    }

    /// Close the menu if it is still open, which returns focus to the button: what every item does before it acts.
    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.open_menu(window, cx) {
            layers::close(menu, window, cx);
        }
        self.menu = None;
        cx.notify();
    }
}

impl Render for HelpButton {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let anchor = Anchor::for_element("app-help-anchor", window, cx);
        self.anchor = Some(anchor.clone());
        let open = self.open_menu(window, cx).is_some();
        let button = Button::new(
            BUTTON_ID,
            "Help",
            HoverHelp::tooltip(button_help(crate::desktop::updates::runs(cx))),
            Role::Button,
        )
        .tier(Tier::Rail)
        .expanded(open)
        .focus(&self.focus)
        // Drawn like the gear above it, in the rail's quiet ink, which the expanded look's fill turns to the inverse.
        .content(
            svg()
                .path(Icon::Help.path())
                .size(px(GLYPH))
                .flex_none()
                .text_color(theme::color(if open {
                    palette.on_fill
                } else {
                    palette.fg_1
                })),
        )
        .on_click(cx.listener(|button, _, window, cx| button.toggle(window, cx)));
        div().relative().child(button).child(anchor.track())
    }
}

/// The open Help menu: its header and items, drawn fresh each frame so Check for updates follows the updater.
struct HelpMenu {
    /// The button that opened it, which closes it before an item acts.
    button: WeakEntity<HelpButton>,
    /// The button's focus handle, the opener every dialog an item opens returns focus to.
    opener: FocusHandle,
    /// The first item's focus handle, where the menu starts.
    first: FocusHandle,
}

impl HelpMenu {
    /// `item` was chosen at `mark`: close the menu, which returns focus to the `?` button, then do what the item does.
    fn choose(
        button: &WeakEntity<HelpButton>,
        opener: &FocusHandle,
        item: Item,
        mark: &Mark,
        window: &mut Window,
        cx: &mut gpui::App,
    ) {
        // The button may be gone with its window; then there is no menu to close.
        let _ = button.update(cx, |button, cx| button.close_menu(window, cx));
        match item {
            Item::SendFeedback => {
                let _ = crate::feedback::open(opener, mark, window, cx);
            }
            Item::ChatOnDiscord => cx.open_url(DISCORD_URL),
            Item::Documentation => cx.open_url(DOCS_URL),
            Item::ReleaseNotes => cx.open_url(RELEASE_NOTES_URL),
            Item::Announcements => {
                let _ = crate::announcements::open_history(opener, mark, window, cx);
            }
            Item::SshHelp => {
                let _ = crate::hosts::ssh_help::open(opener, mark, window, cx);
            }
            Item::Welcome => {
                let _ = crate::welcome::open(opener, mark, window, cx);
            }
            Item::CheckForUpdates => crate::desktop::updates::check_now(cx),
            Item::OpenSourceNotices => window.dispatch_action(Box::new(OpenSourceNotices), cx),
        }
    }
}

impl Render for HelpMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = items(
            crate::desktop::updates::runs(cx),
            crate::announcements::has_history(cx),
            cfg!(target_os = "linux"),
        );
        let entries = shown.into_iter().enumerate().map(|(at, item)| {
            let (button, opener) = (self.button.clone(), self.opener.clone());
            let entry = MenuItem::new(
                item.id(),
                item.icon(),
                item.label(),
                HoverHelp::tooltip(item.help()),
                Role::MenuItem,
            )
            .description(item.description())
            .on_click(move |_, window, cx| {
                // Taken in the handler, after the coordinator counted this press or key.
                let mark = Mark::take(cx);
                HelpMenu::choose(&button, &opener, item, &mark, window, cx);
            });
            if at == 0 {
                entry.focus(&self.first)
            } else {
                entry
            }
        });
        layers::panel(cx)
            .id("help-menu")
            .debug_selector(|| "help-menu".into())
            .w(px(PANEL_WIDTH))
            .child(MenuHeader::app("Help", "Farhelm guides and feedback"))
            .child(
                div()
                    .id("help-menu-items")
                    .role(Role::Menu)
                    .aria_label(SharedString::from("Help"))
                    .flex()
                    .flex_col()
                    .children(entries),
            )
    }
}
