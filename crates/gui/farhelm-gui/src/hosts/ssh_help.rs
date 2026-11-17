//! SSH help ("SSH needs 2FA?"): the commands that start the helm's two shared SSH connections to a host by hand, for a
//! host that asks for a second factor or other approval on every login.
//!
//! SPEC.md, "Feedback" ("SSH needs 2FA?") and "Topology" (hosts that need an interactive approval are supported on a
//! best-effort basis, once the user starts both connections). Ported from the old app's `ssh_help.rs`
//! (`SshHelpDialog`, `HostCommands`) and the `.ssh-help-*` rules in `app.css`, at reference commit `24bc0580`. Here's
//! the TLDR of its rules:
//!
//! - **Read-only.** The helm composes every command (`POST /api/ssh-connections`), from its own state directory,
//!   connection overrides and socket checks; this dialog shows and copies them and builds nothing itself. The request
//!   changes nothing on the helm, so it is a keyless send ([`operations::send`]), not work on a key: nothing draws it
//!   busy and no refusal of it is kept anywhere but here. The dialog never starts SSH and never registers a host.
//! - **Read once per opening.** The request goes out as the dialog opens, and its reply reaches only that opening (the
//!   pattern the migration settled for reads the helm does not push). Until it arrives the dialog says it is loading; a
//!   refusal shows in its place. Closing never waits for it.
//! - **What it shows.** First, once, each connection whose command the helm cannot compose for any host and why (the
//!   state path too long for its socket, holding `${`, or not UTF-8). Then one section per registered SSH host: its
//!   name, its destination, and the Sessions connection's and the Setup and updates connection's command, each with a
//!   Copy button that copies the command's exact bytes. With no SSH hosts it points at Add host.
//! - **From Add host.** [`open_for_draft`] sends the destination typed there, and the helm's section for it ("The host
//!   you're adding") comes first; an empty draft gets the line asking for a destination instead, and the helm refuses
//!   an unusable destination before composing anything, which shows as the refusal. Add host stays open below, its
//!   draft untouched, and closing SSH help returns focus to the link that opened it.
//! - **Host text.** Every string in the reply is the helm's or a host's (a name, a destination, a command quoting both
//!   and the helm's path, a reason quoting the path), so all of it is drawn as host text.
//!
//! [`open`] is the entry point for the ? menu's "SSH needs 2FA?" (drawn by the area that owns the ? menu).

#[cfg(test)]
mod tests;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ssh_help::{Help, HostCommands};
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, FocusHandle, FontWeight, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledText, WeakEntity, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::json;

use super::identity::host_mark;
use crate::components::{Button, CommandWell, DialogFrame, HoverHelp, Tier};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Dialog, LayerId};
use crate::operations;
use crate::session;
use crate::theme::{self, Palette};

/// `.ssh-help-dialog`'s width.
const WIDTH: f32 = 640.;
/// `.ssh-help-dialog`'s text: 13 pixels at a line height of 1.5.
const TEXT: f32 = 13.;
const LINE: f32 = 19.5;
/// `.ssh-help-body`'s gap between its paragraphs and sections.
const BODY_GAP: f32 = 12.;
/// `.ssh-help-host`'s gap inside a section.
const SECTION_GAP: f32 = 8.;
/// `.host-settings-section + .host-settings-section`: the room above a section under the rule that parts it from the
/// one before.
const SECTION_RULE_PAD: f32 = 20.;
/// An `h3` at the dialog's 13 pixels: the browser's 1.17em, on the dialog's line height of 1.5.
const HEADING: f32 = 15.21;
/// `.ssh-help-command`'s gap between a command's label and its well (the well's own gap to Copy is [`CommandWell`]'s).
const COMMAND_GAP: f32 = 6.;
/// The header tile's mark: `HostMark`'s default 24 pixels, as Add host's.
const TILE_MARK: f32 = 24.;

/// The route, as a refusal without words names it.
const SHOWN: &str = "POST /api/ssh-connections";

/// Where the commands must run: on a Mac the app and the helm run on the user's own Mac; elsewhere the old browser
/// app named the helm's machine and user. The old app said "on this Mac" only in its native Mac build.
const WHERE: &str = if cfg!(target_os = "macos") {
    "on this Mac"
} else {
    "on the machine the helm runs on, as the user it runs as"
};

/// Open SSH help on top of everything else in `window`, listing the registered SSH hosts' commands: the entry point for
/// the ? menu's "SSH needs 2FA?".
///
/// `opener` is the focus handle of the ? button, the control that opened the menu, and not the menu item: the item goes
/// with its popover as the menu closes, and a return target that is no longer drawn falls back to whatever held focus
/// before the ? click. The old app returned focus to that button (its Help toggle). `mark` is the [`Mark`] taken in the
/// item's click handler. Closing returns focus to `opener` ([`ReturnTo::Control`]), also where a click leaves a button
/// unfocused (macOS), as Add host does for its opener. Nothing opens when the user has moved on since `mark` or the
/// window is inert. Returns the dialog's layer, or `None` when nothing opened.
pub fn open(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_with(None, opener, mark, window, cx)
}

/// [`open`] from Add host's "Start its connections yourself first.": the commands for `destination`, the draft as typed
/// (empty included, which the dialog answers by asking for a destination), above the registered hosts'. `opener` is
/// the link, inside Add host, which keeps its draft while SSH help is open over it.
pub(crate) fn open_for_draft(
    destination: String,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_with(Some(destination), opener, mark, window, cx)
}

/// Open the dialog and send its one read: for `draft`'s destination as well when it came from Add host.
fn open_with(
    draft: Option<String>,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    // An inert window has nothing to send the read through.
    session::current_store(cx)?;
    let from_draft = draft.is_some();
    let dialog = cx.new(|cx| SshHelpDialog {
        from_draft,
        reply: None,
        close: cx.focus_handle().tab_stop(true),
        layer: None,
    });
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: cx.focus_handle(),
            // The footer's Close, as the old dialog started there.
            start: dialog.read(cx).close.clone(),
            subject: None,
            // None: the dialog has no main action for Enter on its empty space.
            main_action: None,
            // SPEC.md, "Opening and closing": the dialogs that share the host and Settings installer keep focus on the
            // control in use when their empty space is clicked, and the old SSH help was one of them.
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Control(opener.clone()),
            on_dismiss: None,
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    request(draft, dialog.downgrade(), cx);
    Some(layer)
}

/// Send the read for `dialog`, with `draft`'s destination when there is one (`null` otherwise, which the helm reads as
/// none), and put the reply in the dialog once it arrives, if it is still open. A path that cannot be built is shown as
/// the reply's refusal, so the dialog never waits on a read that was not sent.
fn request(draft: Option<String>, dialog: WeakEntity<SshHelpDialog>, cx: &mut App) {
    let path = match ApiPath::api(&["ssh-connections"]) {
        Ok(path) => path,
        Err(error) => {
            let reason = HostText::from_host(error.to_string());
            let _ = dialog.update(cx, |dialog, cx| dialog.settle(Err(reason), cx));
            return;
        }
    };
    let body = json!({ "destination": draft });
    operations::send(
        Method::Post,
        path,
        Some(body),
        SHOWN,
        cx,
        move |answer, cx| {
            let reply = operations::decode::<Help>(answer, SHOWN).map(Shown::from);
            let _ = dialog.update(cx, |dialog, cx| dialog.settle(reply, cx));
        },
    );
}

/// One destination's commands as the dialog shows them: every part host text from the moment it is decoded.
struct Commands {
    name: HostText,
    destination: HostText,
    sessions: Option<HostText>,
    setup: Option<HostText>,
}

impl From<HostCommands> for Commands {
    fn from(host: HostCommands) -> Self {
        let text = |value: String| HostText::from_host(value);
        Commands {
            name: text(host.name),
            destination: text(host.destination),
            sessions: host.sessions.map(text),
            setup: host.setup.map(text),
        }
    }
}

/// The helm's reply as the dialog shows it ([`Help`], in host text).
struct Shown {
    unavailable: Vec<HostText>,
    hosts: Vec<Commands>,
    extra: Option<Commands>,
}

impl From<Help> for Shown {
    fn from(help: Help) -> Self {
        Shown {
            unavailable: help
                .unavailable
                .into_iter()
                .map(HostText::from_host)
                .collect(),
            hosts: help.hosts.into_iter().map(Commands::from).collect(),
            extra: help.extra.map(Commands::from),
        }
    }
}

/// The open dialog: whether it asked for a typed destination, the reply once it is in, and the footer Close where focus
/// starts.
struct SshHelpDialog {
    /// Opened from Add host: the typed destination's section, or the line asking for one, comes first.
    from_draft: bool,
    /// `None` while the read is out; then the reply, or why there is none.
    reply: Option<Result<Shown, HostText>>,
    /// The footer's Close, where focus starts.
    close: FocusHandle,
    /// Set right after the layer opens.
    layer: Option<LayerId>,
}

impl SshHelpDialog {
    /// The read answered: show what it said.
    fn settle(&mut self, reply: Result<Shown, HostText>, cx: &mut Context<Self>) {
        self.reply = Some(reply);
        cx.notify();
    }

    /// What follows the dialog's fixed paragraphs: the loading line, the refusal, or the reply's lines and sections.
    fn results(&self, palette: &Palette) -> Vec<AnyElement> {
        let shown = match &self.reply {
            None => {
                return vec![
                    line(
                        "ssh-help-loading",
                        Role::Status,
                        "Loading connection commands…",
                    )
                    .into_any_element(),
                ];
            }
            Some(Err(reason)) => return vec![error("ssh-help-error", reason, palette)],
            Some(Ok(shown)) => shown,
        };
        let mut items: Vec<AnyElement> = shown
            .unavailable
            .iter()
            .enumerate()
            .map(|(index, reason)| error(format!("ssh-help-unavailable-{index}"), reason, palette))
            .collect();
        // `.host-settings-section + .host-settings-section`: a rule above a commands section only when one is right
        // before it. The line asking for a destination is a plain section, so the first host after it has no rule.
        let mut after_commands = false;
        if self.from_draft {
            match &shown.extra {
                Some(extra) => {
                    items.push(section("ssh-help-extra", extra, false, palette));
                    after_commands = true;
                }
                // A plain `section`, outside `.ssh-help-host`, so its `h3` keeps the browser's 1em margins.
                None => items.push(
                    div()
                        .flex()
                        .flex_col()
                        .child(heading(div().child("The host you're adding")).my(px(HEADING)))
                        .child(line(
                            "ssh-help-no-destination",
                            Role::Label,
                            "Enter an SSH destination in Add host first.",
                        ))
                        .into_any_element(),
                ),
            }
        }
        for (index, host) in shown.hosts.iter().enumerate() {
            let id = format!("ssh-help-host-{index}");
            items.push(section(&id, host, after_commands, palette));
            after_commands = true;
        }
        if shown.hosts.is_empty() {
            items.push(
                line(
                    "ssh-help-no-hosts",
                    Role::Label,
                    "No SSH hosts are registered. Use Add host in the hosts panel to register one.",
                )
                .into_any_element(),
            );
        }
        items
    }
}

/// A click handler that closes the dialog through the layer manager, as Escape does: both Close controls.
fn dismiss(dialog: WeakEntity<SshHelpDialog>) -> impl Fn(&ClickEvent, &mut Window, &mut App) {
    move |_, window, cx| {
        let layer = dialog.upgrade().and_then(|dialog| dialog.read(cx).layer);
        if let Some(layer) = layer {
            layers::dismiss(layer, window, cx);
        }
    }
}

/// A line of the app's own words that tests and assistive technology find by `id`, in `role`.
fn line(id: &'static str, role: Role, words: &'static str) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .role(role)
        .aria_label(words)
        .child(words)
}

/// One of the helm's lines, which quote its state path or the destination, as host text on the hosts' error line
/// (`.create-session-error`), found by `id`.
fn error(id: impl Into<SharedString>, reason: &HostText, palette: &Palette) -> AnyElement {
    super::problem_line(HostSentence::new(id.into()).host(reason), palette)
}

/// A section's heading: an `h3` at the dialog's size, bold.
fn heading(content: gpui::Div) -> gpui::Div {
    content
        .text_size(px(HEADING))
        .line_height(px(HEADING * 1.5))
        .font_weight(FontWeight::BOLD)
}

/// One destination's section (`.ssh-help-host`): its name, its destination in the monospace face, and each command
/// the helm could compose. `ruled` draws the rule and room above it that part it from the section before.
fn section(id: &str, host: &Commands, ruled: bool, palette: &Palette) -> AnyElement {
    let commands = [
        (
            "sessions",
            "Sessions connection",
            "sessions connection",
            &host.sessions,
        ),
        (
            "setup",
            "Setup and updates connection",
            "setup and updates connection",
            &host.setup,
        ),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(SECTION_GAP))
        .when(ruled, |section| {
            section
                .border_t_1()
                .border_color(theme::color(palette.border_dim))
                .pt(px(SECTION_RULE_PAD))
        })
        .child(heading(div().child(
            HostSentence::new(format!("{id}-name")).host(&host.name),
        )))
        .child(HostSentence::new(format!("{id}-destination")).mono(&host.destination))
        .children(
            commands
                .into_iter()
                .filter_map(|(kind, label, copy_label, command)| {
                    let command = command.as_ref()?;
                    Some(command_row(
                        &format!("{id}-{kind}"),
                        label,
                        copy_label,
                        command,
                    ))
                }),
        )
        .into_any_element()
}

/// One command (`.ssh-help-command`): its label over the command in a [`CommandWell`], whose Copy takes the command's
/// raw bytes, so a pasted command addresses the same destination and path even where the screen shows an escape; the
/// button's name finishes the label's sentence ("Copy sessions connection"), as the old one did.
fn command_row(
    id: &str,
    label: &'static str,
    copy_label: &'static str,
    command: &HostText,
) -> AnyElement {
    let well = CommandWell::new(
        id.to_owned(),
        command.clone(),
        "Copy",
        HoverHelp::tooltip("Copy: the exact SSH command to run on the helm's machine"),
    )
    .accessible_name(format!("Copy {copy_label}"));
    div()
        .flex()
        .flex_col()
        .gap(px(COMMAND_GAP))
        .child(div().font_weight(FontWeight::BOLD).child(label))
        .child(well)
        .into_any_element()
}

/// The first paragraph: why the commands exist and where to run them, the place in bold (the old `strong`).
fn intro() -> StyledText {
    let before = "Farhelm connects without prompting, so it cannot answer a second factor or other approval on every SSH \
                  login. Run both commands in a terminal ";
    let after = ". Each asks for approval once, then keeps a connection open in the background for Farhelm. When a \
                 connection drops, run its command again.";
    let bold = HighlightStyle {
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    };
    StyledText::new(format!("{before}{WHERE}{after}"))
        .with_highlights([(before.len()..before.len() + WHERE.len(), bold)])
}

impl Render for SshHelpDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let body = div()
            .flex()
            .flex_col()
            .gap(px(BODY_GAP))
            .text_size(px(TEXT))
            .line_height(px(LINE))
            .child(div().child(intro()))
            .child(div().child(
                "These commands disable agent, X11 and port forwarding, as Farhelm does. Port forwards belong to the \
                 master connection and cannot be cancelled by clients that reuse it.",
            ))
            .child(div().child(
                "We recommend taking a screenshot of this dialog and asking an agent to set up a script that \
                 automates these commands for you.",
            ))
            .children(self.results(palette));
        let close = Button::new(
            "ssh-help-done",
            "Close",
            HoverHelp::tooltip("Close: return without starting SSH"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .focus(&self.close)
        .on_click(dismiss(weak.clone()));
        DialogFrame::new("ssh-help", "SSH needs 2FA?", body, vec![close])
            .summary("Start a host's SSH connections yourself")
            .tile(host_mark(
                HostIcon::Cloud,
                HostColor::Default,
                px(TILE_MARK),
                palette,
            ))
            .width(px(WIDTH))
            // SPEC.md, "Reaching everything by keyboard": while the read is out, after a refusal, and with no host or
            // no command to copy, the body holds no Copy, so it needs a stop of its own to be scrolled from the
            // keyboard. The frame draws it only then; once a Copy arrives the stop goes, and focus on it moves to the
            // closest control left. The name is the old app's for the same region.
            .scroll_stop("SSH connection help")
            .close(
                "Close SSH help",
                HoverHelp::tooltip("Close: return without adding a host or starting SSH"),
                false,
                dismiss(weak),
            )
    }
}
