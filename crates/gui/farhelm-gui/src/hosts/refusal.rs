//! The host refusal dialog: the helm's reason for refusing an action on a host, in a dialog keyed to that host, with
//! Close, and Remove when the helm says removing the host is the remedy.
//!
//! Ported from the old app's `HostRefusalDialog` (`hosts.rs`, reference commit `24bc0580`): "<action> · <name>" with
//! the host's mark, "Farhelm refused this action" in red under it, then the host's full destination in the monospace
//! face and the helm's reason, both at the old dialog's 16 pixels, and Close, where focus starts and which Enter on the
//! dialog presses. When the helm marked the refusal "remove instead" ([`HelmMarker::RemoveHostInstead`]), Remove
//! follows Close: it closes the dialog and starts Remove host's flow ([`super::remove`]), its confirmation or, with
//! "Remove hosts without asking" on, the removal itself. The helm sets that marker only on an uninstall plan's
//! refusals, so an Update's refusal shows Close alone.
//!
//! So far an Update's refused plan ([`super::update`]), an uninstall's refused plan or confirmation
//! ([`super::uninstall`], "Uninstall Farhelm"), a refused Retry or Adopt ([`super::remedies`]) and a refused Suspend or
//! Resume ([`super::lifecycle`]) open it, all but the uninstall's through [`show_and_forget`]; the old app also opened
//! it for a refused setup and, with "Remove hosts without asking" on, Remove, which take it as their pieces of the port
//! arrive ([`Refusal::of`] takes the action's words).
//!
//! Close in the footer and the header's × dismiss the refusal. Remove takes its confirmation choice from the
//! presented footer. The refusal keeps the host snapshot from when it opened, so it stays readable if the registry
//! stops listing the host meanwhile.

use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{HelmMarker, HostView};
use gpui::{
    AnyWindowHandle, App, AppContext as _, ClickEvent, Context, FocusHandle, FontWeight,
    IntoElement, ParentElement as _, Render, Styled as _, WeakEntity, Window, div, px,
};

use super::identity::{display_name, host_mark};
use crate::assets;
use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Dialog, LayerId};
use crate::operations::OpKey;
use crate::prompt::ActionFrame;
use crate::theme::{self, metrics};

/// The host's mark in the header's tile, as the other host dialogs draw it.
const TILE_MARK: f32 = 24.;

/// The body's text size: the old dialog's body inherited the page's 16 pixels, larger than the dialog frame's 12.
const BODY_TEXT: f32 = 16.;

/// The reason's line height: the old `normal` at 16 pixels, 20 pixels from line to line in the reference screens.
const REASON_LINE: f32 = 20.;

/// The destination line's margin above and below: the old `<p>`'s browser default of `1em`.
const DESTINATION_MARGIN: f32 = 16.;

/// `.host-settings-body`'s `gap: 20px` between the destination and the reason.
const BODY_GAP: f32 = 20.;

/// The help of both Close controls, the old dialog's.
const CLOSE_HELP: &str = "Close this refusal";

/// Remove's help, the old dialog's.
const REMOVE_HELP: &str =
    "Remove this host from Farhelm's list; its supervisor and sessions keep running";

/// What the dialog shows, frozen when it opens.
#[derive(Clone)]
pub(crate) struct Refusal {
    /// The action the helm refused, as the title names it ("Update host").
    action: &'static str,
    /// The registry entry whose identity the dialog shows and whose removal it offers.
    host: HostView,
    /// The host's name as its row shows it.
    name: HostText,
    /// The host's full destination (its name for a host without one), which the body shows whole however the title is
    /// cut (SPEC.md: "A shortened heading retains the full destination in the body").
    destination: HostText,
    icon: HostIcon,
    color: HostColor,
    /// The helm's words.
    reason: HostText,
    /// Whether the helm marked the refusal "remove instead", which draws Remove.
    remove_instead: bool,
}

impl Refusal {
    /// The helm's `refusal` of `action` on `view`: its words, and whether it carries the "remove instead" marker.
    pub(crate) fn of(
        action: &'static str,
        view: &HostView,
        refusal: &crate::operations::Refusal,
    ) -> Refusal {
        Refusal {
            action,
            host: view.clone(),
            name: display_name(view),
            destination: HostText::from_host(view.destination.as_deref().unwrap_or(&view.name)),
            icon: view.icon,
            color: view.color,
            reason: refusal.words.clone(),
            remove_instead: refusal.has(HelmMarker::RemoveHostInstead),
        }
    }
}

/// Open `refusal` over everything else in `window`, unless the user has moved on since `mark`, the press that asked for
/// the refused action. Closing it returns focus to what held it when it opened. Returns the dialog's layer.
pub(crate) fn open(
    refusal: Refusal,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let dialog = cx.new(|cx| RefusalDialog {
        refusal,
        close: cx.focus_handle().tab_stop(true),
        layer: None,
        remove_frame: ActionFrame::default(),
    });
    // Close is the dialog's main action, which Enter presses while the dialog itself holds focus (SPEC.md: "If a dialog
    // container itself has focus, Enter presses its enabled main action"), as the old dialog's Enter closed it. A click
    // on its empty space keeps focus on Close, so in practice that is Close's own Enter.
    let weak = dialog.downgrade();
    let main_action: layers::MainAction = std::rc::Rc::new(move |window, cx| {
        let layer = weak.upgrade().and_then(|dialog| dialog.read(cx).layer);
        if let Some(layer) = layer {
            layers::dismiss(layer, window, cx);
        }
    });
    let container = cx.focus_handle();
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container,
            // Close, as the old dialog's focus started there.
            start: dialog.read(cx).close.clone(),
            subject: None,
            main_action: Some(main_action),
            // SPEC.md, "Opening and closing": the dialogs that share the host and Settings installer keep focus on the
            // control in use when their empty space is clicked.
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Opener,
            on_dismiss: None,
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    Some(layer)
}

/// Show `refusal`, the helm's refusal of `action` on `host`, in the dialog through `mark` in `window`, with the host as
/// the registry lists it now, then drop the store's record of it under `key`, so the row neither shows it again nor
/// opens for it, as the old app kept a refusal in its dialog alone. Nothing opens once the user has moved on since
/// `mark`, the window is gone, or the registry no longer lists the host, and the record then stays under the row, the
/// one place left to show it. How the Hosts panel's commands show a refusal (`super::lifecycle`,
/// `super::remedies`, `super::update`).
pub(super) fn show_and_forget(
    host: i64,
    action: &'static str,
    refusal: &crate::operations::Refusal,
    key: &OpKey,
    mark: &Mark,
    window: AnyWindowHandle,
    cx: &mut App,
) {
    let Some(view) = super::listed(host, cx) else {
        return;
    };
    let shown = window
        .update(cx, |_, window, cx| {
            open(Refusal::of(action, &view, refusal), mark, window, cx)
        })
        .ok()
        .flatten()
        .is_some();
    if shown && let Some(operations) = crate::session::current_operations(cx) {
        operations.update(cx, |operations, cx| operations.forget(key, cx));
    }
}

/// The open dialog.
struct RefusalDialog {
    refusal: Refusal,
    /// The footer's Close, where focus starts.
    close: FocusHandle,
    /// Set right after the layer opens; what both Close controls close.
    layer: Option<LayerId>,
    /// Remove and its confirmation policy become eligible together when the footer frame is presented.
    remove_frame: ActionFrame<(HostView, bool)>,
}

/// A click handler that closes the dialog through the layer manager, as Escape does.
fn dismiss(dialog: WeakEntity<RefusalDialog>) -> impl Fn(&ClickEvent, &mut Window, &mut App) {
    move |_, window, cx| {
        let layer = dialog.upgrade().and_then(|dialog| dialog.read(cx).layer);
        if let Some(layer) = layer {
            layers::dismiss(layer, window, cx);
        }
    }
}

impl Render for RefusalDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let refusal = &self.refusal;
        let title = HostSentence::new("host-refusal-title")
            .text(format!("{} · ", refusal.action))
            .host(&refusal.name);
        // The title as the frame draws one, and under it the old red summary, which the frame's own summary (in the quiet
        // ink) cannot draw.
        let heading = div()
            .child(
                div()
                    .text_size(metrics::TEXT_TITLE)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(title.clone()),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.danger))
                    .child("Farhelm refused this action"),
            );
        let body = div()
            .flex()
            .flex_col()
            .gap(px(BODY_GAP))
            .text_size(px(BODY_TEXT))
            .text_color(theme::color(palette.fg_0))
            .child(
                host_text::element(&refusal.destination)
                    .my(px(DESTINATION_MARGIN))
                    .font_family(assets::TERMINAL_FONT),
            )
            .child(
                div()
                    .line_height(px(REASON_LINE))
                    .child(HostSentence::new("host-refusal-reason").host(&refusal.reason)),
            );
        let close = Button::new(
            "host-refusal-done",
            "Close",
            HoverHelp::tooltip(CLOSE_HELP),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .focus(&self.close)
        .on_click(dismiss(weak.clone()));
        // `.host-refusal-remove`: after Close, as the old footer drew it. The mark is this click's, taken before the
        // dialog closes, so Remove host's question opens through it.
        let remove = refusal.remove_instead.then(|| {
            let frame = self.remove_frame.capture(
                (refusal.host.clone(), super::remove::asks_first(cx)),
                (),
                window,
            );
            let close = dismiss(weak.clone());
            Button::new(
                "host-refusal-remove",
                "Remove",
                HoverHelp::tooltip(REMOVE_HELP),
                Role::Button,
            )
            .tier(Tier::Danger)
            .on_click(move |event, window, cx| {
                let Some((drawn, asks)) = frame.shown() else {
                    return;
                };
                let mark = Mark::take(cx);
                close(event, window, cx);
                super::remove::start_shown(drawn, *asks, &mark, window, cx);
            })
        });
        DialogFrame::new(
            "host-refusal",
            title,
            body,
            std::iter::once(close).chain(remove).collect(),
        )
        .heading(heading)
        .tile(host_mark(
            refusal.icon,
            refusal.color,
            px(TILE_MARK),
            palette,
        ))
        .close(
            "Close host refusal",
            HoverHelp::tooltip(CLOSE_HELP),
            false,
            dismiss(weak),
        )
    }
}
