//! The version readout while the desktop updater runs: the sidebar header's build text turned into the update control,
//! ported from the old app's `app_bar.rs` and `app_updater.rs` views at reference commit `24bc0580` (SPEC.md,
//! "Installation and updates").
//!
//! What it draws follows the updater's latest readout ([`super::readout_control`]):
//!
//! - **A check button** while no update waits: the version in `--fg-1`, then a dim `↻`. Hover and keyboard focus light
//!   both and turn the glyph a quarter; a press checks for updates now. During a check or an install the glyph spins
//!   instead (eight steps a turn, from the ticker) and the words beside it say which. A reinstall, interrupted-install
//!   or withdrawal warning turns the version amber with a `⚠` before it, and the press still retries.
//! - **The update-menu toggle** once an update is installed: the version in red with `↑` before it (and `⚠` before
//!   that when a warning stands). A press opens the menu beside it, with Restart to update and What's new.
//! - **The inline status** after either: Checking… or Installing…, Up to date for about five seconds, or Check failed,
//!   a button that dismisses it. While an update waits, progress gets a ring of its own, since the toggle has no glyph
//!   to spin.
//!
//! Without an updater there is no readout at all: the header keeps its plain build text (`sidebar/header.rs`), so a
//! devbuild, an agent build and the Linux app draw exactly what they drew before.
//!
//! The check button and the toggle share one focus handle, so when an installed update turns one into the other the
//! focus stays on the readout if it was there, and nowhere else is disturbed. The old app needed a script for that
//! hand-off (`CHECK_FOCUS_HANDOFF`); GPUI keeps focus on a handle for as long as some drawn element tracks it.
//!
//! Not ported, each recorded in the parity file: the hover transitions (the change is instant, as every ported hover
//! is); the screen-reader live region and the hidden "Update ready" announcement (a known gap); closing the menu when
//! the status beside it changed width (the old menu was placed from a snapshot; a popover follows its toggle); focus
//! moving to the Settings gear after a dismissal (SPEC.md now sends focus home); opening the menu with an arrow key.

use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Transformation, Window, div, prelude::FluentBuilder as _, px,
    radians, svg,
};

use super::{
    InlineFeedback, InlineStatus, Readout, ReadoutControl, ReadoutMark, WARNING_MARK,
    inline_presentation, readout_control, readout_marks,
};
use crate::assets::{self, Icon};
use crate::components::{
    Button, HoverHelp, MenuHeader, MenuItem, Role, Spinner, StatusMark, Tier, spinner_icon,
};
use crate::desktop::title_band;
use crate::desktop::updates::{self, Updater};
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::theme::{self, metrics};
use crate::{render_count, ticker};

/// How long Up to date stays beside the readout (SPEC.md: "about five seconds").
pub(crate) const UP_TO_DATE_FOR: Duration = Duration::from_secs(5);

/// The release notes What's new opens in the system browser (SPEC.md, "Installation and updates").
pub(crate) const RELEASE_NOTES_URL: &str = "https://farhelm.io/docs/release-notes/";

/// The update menu's width: the old `.bar-menu-panel`'s, which every GPUI menu keeps.
const MENU_WIDTH: f32 = 246.;

/// The gap between the version text and the refresh glyph (`.app-version-check`'s `gap: 5px`).
const GLYPH_GAP: f32 = 5.;

/// The refresh glyph's size: the 12-pixel line it sits on.
const GLYPH_SIZE: f32 = 12.;

/// How the readout's controls look for one state: which inks, and how far the refresh glyph is turned. Pure, so the
/// looks are specified without drawing (`hover_and_keyboard_focus_brighten_the_version_and_turn_the_glyph`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Look {
    /// The version text's ink.
    pub(crate) version: theme::Token,
    /// The refresh glyph's ink.
    pub(crate) glyph: theme::Token,
    /// The refresh glyph's turn, in eighths of a turn.
    pub(crate) eighths: u8,
}

/// The check button's look: at rest the version in `--fg-1` (amber with a warning) and the glyph dim; lit (hover or
/// keyboard focus) both in `--fg-0`, the glyph a quarter turned; spinning the glyph in `--fg-0` at `step`, and lit or
/// not, never the extra quarter (the old sheet's `:not(.app-version-refresh-spinning)`).
pub(crate) fn check_look(
    palette: &theme::Palette,
    lit: bool,
    spinning: Option<u8>,
    warning: bool,
) -> Look {
    let version = match (lit, warning) {
        (true, _) => palette.fg_0,
        (false, true) => palette.warn,
        (false, false) => palette.fg_1,
    };
    let (glyph, eighths) = match spinning {
        Some(step) => (palette.fg_0, step),
        None if lit => (palette.fg_0, 2),
        None => (palette.fg_2, 0),
    };
    Look {
        version,
        glyph,
        eighths,
    }
}

/// The readout while an updater runs. The sidebar makes one when an updater runs and keeps it for the window's life,
/// so Up to date's timer and a dismissal survive the header moving between the sidebar and the narrow window's top row.
pub struct VersionReadout {
    updater: Entity<Updater>,
    /// Which result the status has hidden (an expired Up to date, a dismissed failure).
    feedback: InlineFeedback,
    /// Up to date's expiry, while one is pending; replacing it cancels the older one.
    expiry: Option<ticker::Deadline>,
    /// The turn of whatever spins, held only while something spins and the window is active.
    spinner: Option<Spinner>,
    /// The one focus handle the check button and the toggle share (see the module docs).
    focus: FocusHandle,
    /// Where the menu opens: beside the toggle, drawn while the toggle is.
    anchor: Option<Anchor>,
    /// The open menu's layer.
    menu: Option<LayerId>,
    _observing: Subscription,
    /// The look the check button was last drawn with, for tests to read (the frame records text, not its ink or a
    /// glyph's turn).
    #[cfg(test)]
    drawn_look: std::cell::Cell<Option<Look>>,
    /// What a screenshot golden holds still on this readout (see [`Pinned`]); nothing outside test builds.
    #[cfg(test)]
    pinned: Pinned,
}

/// What a screenshot golden pins on a readout (`goldens.rs`'s version readout scenes): looks that no input can hold
/// still in a headless capture. A golden has no pointer resting on the check button, so it pins the lit look the
/// pointer would give. The spinners turn with the ticker, which those scenes stop (reduced motion) so that a capture
/// can settle, leaving them at the first step; so it pins a later step, to show that the glyph and the ring do turn
/// (step 0 looks like no turn at all).
///
/// Keyboard focus needs no pin: the scenes give it through the focus coordinator, as a keyboard user would.
#[cfg(test)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Pinned {
    /// Draw the check button lit (as under the pointer) whether or not anything lights it.
    pub(crate) lit: bool,
    /// Draw whatever spins (the check button's glyph, the ring beside the toggle) at this step of its eight instead of
    /// the spinner's. Applies only while something spins, so it never turns a glyph at rest.
    pub(crate) step: Option<u8>,
}

impl VersionReadout {
    /// The readout over `updater`, following its readouts while it lives.
    pub fn new(updater: Entity<Updater>, cx: &mut Context<Self>) -> Self {
        let observing = cx.observe(&updater, |readout, updater, cx| {
            let shown = updater.read(cx).readout().clone();
            readout.observe_feedback(&shown, cx);
            cx.notify();
        });
        let first = updater.read(cx).readout().clone();
        let mut readout = VersionReadout {
            updater,
            feedback: InlineFeedback::default(),
            expiry: None,
            spinner: None,
            focus: cx.focus_handle().tab_stop(true),
            anchor: None,
            menu: None,
            _observing: observing,
            #[cfg(test)]
            drawn_look: std::cell::Cell::new(None),
            #[cfg(test)]
            pinned: Pinned::default(),
        };
        readout.observe_feedback(&first, cx);
        readout
    }

    /// Tell the status about a new readout, and give a new Up to date its five seconds. The feedback ignores a readout
    /// with the same activity and request as before, so an installation-record change neither stretches the five
    /// seconds nor brings back a dismissed failure.
    fn observe_feedback(&mut self, readout: &Readout, cx: &mut Context<Self>) {
        if self.feedback.observe(&readout.activity, readout.notice_id) {
            let (activity, notice_id) = (readout.activity.clone(), readout.notice_id);
            let view = cx.weak_entity();
            self.expiry = Some(ticker::after(UP_TO_DATE_FOR, cx, move |cx| {
                let _ = view.update(cx, |readout, cx| {
                    readout.feedback.expire(&activity, notice_id);
                    cx.notify();
                });
            }));
        }
    }

    /// Open the update menu, or close it when it is open (a second press on the toggle).
    fn toggle_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.menu.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
            cx.notify();
            return;
        }
        let Some(anchor) = self.anchor.clone() else {
            return;
        };
        let mark = Mark::take(cx);
        self.menu = open_menu(anchor, &self.focus, &mark, window, cx);
        cx.notify();
    }

    /// This readout with `pinned` held still on every frame it draws, for a screenshot golden.
    #[cfg(test)]
    pub(crate) fn pinned(self, pinned: Pinned) -> Self {
        VersionReadout { pinned, ..self }
    }

    /// The focus handle the check button and the toggle share, for a golden that gives the readout keyboard focus
    /// through the focus coordinator.
    #[cfg(test)]
    pub(crate) fn shared_focus(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// Press the toggle as a click would, opening the update menu beside it (or closing it), for a golden that shows
    /// the menu open. Does nothing until a frame has drawn the toggle, since the menu opens on its drawn anchor.
    #[cfg(test)]
    pub(crate) fn press_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_menu(window, cx);
    }

    /// The updater's readout now.
    fn readout(&self, cx: &App) -> Readout {
        self.updater.read(cx).readout().clone()
    }

    /// The check button: a press checks for updates now.
    fn check_button(
        &self,
        readout: &Readout,
        spinning: bool,
        hovered: bool,
        window: &Window,
        cx: &App,
    ) -> Button {
        let palette = theme::palette(cx);
        let lit = hovered || focus::shows_ring(&self.focus, window);
        #[cfg(test)]
        let lit = lit || self.pinned.lit;
        let warning = readout.needs_reinstall || readout.withdrawn;
        let step = spinning.then(|| self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx)));
        #[cfg(test)]
        let step = step.map(|step| self.pinned.step.unwrap_or(step));
        let look = check_look(palette, lit, step, warning);
        #[cfg(test)]
        self.drawn_look.set(Some(look));
        let marks = marks(readout_marks(readout), look.version, palette);
        Button::new(
            "app-version-check",
            "Check for updates",
            HoverHelp::tooltip(check_help(&readout.tooltip)),
            Role::Button,
        )
        .tier(Tier::Readout { focus_fill: false })
        .accessible_name(format!(
            "Farhelm {}; check for updates. {}",
            farhelm_proto::BUILD_VERSION,
            readout.tooltip
        ))
        .focus(&self.focus)
        .content(
            div()
                .flex()
                .flex_row()
                .items_center()
                .min_w_0()
                .children(marks)
                .child(version_text(look.version))
                .child(
                    svg()
                        .path(Icon::Refresh.path())
                        .size(px(GLYPH_SIZE))
                        .flex_none()
                        .ml(px(GLYPH_GAP))
                        .text_color(theme::color(look.glyph))
                        .with_transformation(Transformation::rotate(radians(
                            f32::from(look.eighths) * std::f32::consts::TAU / 8.,
                        ))),
                ),
        )
        .on_click(|_, _, cx| updates::check_now(cx))
    }

    /// The update-menu toggle: red, with the update arrow, and the menu beside it.
    fn toggle(&self, readout: &Readout, cx: &mut Context<Self>) -> Button {
        let palette = theme::palette(cx);
        let open = self.menu.is_some();
        let marks = marks(readout_marks(readout), palette.danger, palette);
        let view = cx.weak_entity();
        Button::new(
            "app-version-toggle",
            "Update ready",
            HoverHelp::tooltip(readout.tooltip.clone()),
            Role::Button,
        )
        .tier(Tier::Readout { focus_fill: true })
        .accessible_name(format!(
            "Farhelm {}, update ready: {}",
            farhelm_proto::BUILD_VERSION,
            readout.tooltip
        ))
        .expanded(open)
        .focus(&self.focus)
        .content(
            div()
                .flex()
                .flex_row()
                .items_center()
                .min_w_0()
                .children(marks)
                .child(version_text(palette.danger)),
        )
        .on_click(move |_, window, cx| {
            let _ = view.update(cx, |readout, cx| readout.toggle_menu(window, cx));
        })
    }

    /// The words beside the control, with the ring while an update waits, or the Check failed button; nothing when
    /// nothing shows.
    fn status(&self, readout: &Readout, cx: &mut Context<Self>) -> Option<AnyElement> {
        let presentation = inline_presentation(readout, self.feedback.suppressed.as_ref());
        let shown = presentation.shown?;
        let palette = theme::palette(cx);
        let row = div()
            .flex()
            .flex_row()
            .items_center()
            .ml(px(6.))
            .gap(px(4.))
            .max_w(px(90.))
            // Shrinks with the row (flex items shrink by default), down to nothing before the version does.
            .min_w_0();
        let row = row.when(presentation.separate_progress_spinner, |row| {
            let step = self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx));
            #[cfg(test)]
            let step = self.pinned.step.unwrap_or(step);
            row.child(
                div()
                    .flex_none()
                    // Tests find the ring by this; it records nothing outside test builds.
                    .debug_selector(|| "app-update-ring".into())
                    .child(spinner_icon(step, px(12.), theme::color(palette.accent))),
            )
        });
        let element = if shown == InlineStatus::Failed {
            let (activity, notice_id) = (readout.activity.clone(), readout.notice_id);
            let view = cx.weak_entity();
            row.child(
                Button::new(
                    "app-update-failure",
                    shown.text(),
                    HoverHelp::tooltip(readout.tooltip.clone()),
                    Role::Button,
                )
                .tier(Tier::Readout { focus_fill: false })
                .accessible_name("Check failed; dismiss update check status")
                .content(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(theme::color(palette.warn))
                        .child(shown.text()),
                )
                .on_click(move |_, _, cx| {
                    let _ = view.update(cx, |readout, cx| {
                        readout.feedback.dismiss(&activity, notice_id);
                        cx.notify();
                    });
                }),
            )
        } else {
            // The hover help is the readout's own words, as close as anything to the old status, which had none
            // (StatusMark requires some).
            row.child(
                div().min_w_0().child(
                    StatusMark::new(
                        "app-update-status",
                        shown.text(),
                        HoverHelp::tooltip(readout.tooltip.clone()),
                        Role::Status,
                        shown.text(),
                    )
                    .described(false)
                    .truncate(),
                ),
            )
        };
        Some(element.into_any_element())
    }
}

impl Render for VersionReadout {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("version-readout", cx);
        #[cfg(test)]
        self.drawn_look.set(None);
        let readout = self.readout(cx);
        let control = readout_control(Some(&readout));
        let presentation = inline_presentation(&readout, self.feedback.suppressed.as_ref());
        // Something spins: the check glyph during visible work, or the ring while an update waits.
        let spins = matches!(control, ReadoutControl::Check { spinning: true })
            || presentation.separate_progress_spinner;
        let spins = spins && ticker::is_active(window, cx);
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| Spinner::start(window, cx));
        }
        // The menu belongs to the toggle: once no update waits, its anchor is no longer drawn and the layer manager
        // closes it, returning focus to the shared handle, now the check button's.
        if self
            .menu
            .is_some_and(|layer| !layers::is_open(layer, window, cx))
        {
            self.menu = None;
        }
        // Whether the pointer is over the control, kept in the element's own state rather than the view's: when the
        // control is not drawn for a frame (an inert window, the header moving into the narrow window's top row) it
        // starts over unhovered, as GPUI's own hover does, instead of staying lit with no pointer over it.
        let hovered = window.use_keyed_state("app-version-hovered", cx, |_, _| false);
        let is_hovered = *hovered.read(cx);
        let view = cx.weak_entity();
        let control_element = match control {
            ReadoutControl::UpdateMenu => {
                let anchor = Anchor::for_element("app-version-menu-anchor", window, cx).beside();
                self.anchor = Some(anchor.clone());
                div()
                    .relative()
                    .min_w_0()
                    .child(self.toggle(&readout, cx))
                    .child(anchor.track())
                    .into_any_element()
            }
            ReadoutControl::Check { spinning } => {
                self.anchor = None;
                self.check_button(&readout, spinning, is_hovered, window, cx)
                    .into_any_element()
            }
            // Never reached: the readout exists only while an updater runs, which always publishes a readout.
            ReadoutControl::Plain => div().into_any_element(),
        };
        // The whole readout keeps its presses in the macOS title band, the status, its ring and the gaps between them
        // included: it is the readout's, not empty band background (`title_band`).
        title_band::claims_presses(div().id("version-readout"))
            .flex()
            .flex_row()
            .items_center()
            .min_w_0()
            .child(
                div()
                    .id("app-version-hover")
                    .min_w_0()
                    .on_hover(move |over, _, cx| {
                        hovered.update(cx, |hovered, _| *hovered = *over);
                        let _ = view.update(cx, |_, cx| cx.notify());
                    })
                    .child(control_element),
            )
            .children(self.status(&readout, cx))
    }
}

/// The version text: this window's build, in the monospace face, cut short with an ellipsis.
fn version_text(ink: theme::Token) -> impl IntoElement {
    div()
        .min_w_0()
        .truncate()
        .font_family(assets::TERMINAL_FONT)
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(ink))
        .child(farhelm_proto::BUILD_VERSION)
}

/// The marks before the version: the warning in `--warn` whatever surrounds it, the update arrow in `ink`.
fn marks(marks: Vec<ReadoutMark>, ink: theme::Token, palette: &theme::Palette) -> Vec<AnyElement> {
    marks
        .into_iter()
        .map(|mark| {
            let mark_ink = if mark == WARNING_MARK {
                palette.warn
            } else {
                ink
            };
            div()
                .flex_none()
                // `margin-right: 0.25em` at 12 pixels.
                .mr(px(3.))
                .font_family(assets::TERMINAL_FONT)
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(mark_ink))
                .child(SharedString::from(mark.glyph))
                .into_any_element()
        })
        .collect()
}

/// Open the update menu beside `anchor`, focus on its first item, closing back to `opener` (the readout). `None` when
/// the user has moved on since `mark` or the anchor is not drawn.
fn open_menu(
    anchor: Anchor,
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let restart_focus = cx.focus_handle().tab_stop(true);
    let whats_new_focus = cx.focus_handle().tab_stop(true);
    let menu = cx.new(|_| UpdateMenu {
        restart_focus: restart_focus.clone(),
        whats_new_focus,
        layer: None,
    });
    let layer = layers::open_popover(
        Popover::menu(menu.clone(), cx.focus_handle(), anchor, restart_focus)
            .returns_to(focus::ReturnTo::Control(opener.clone())),
        mark,
        window,
        cx,
    )?;
    menu.update(cx, |menu, _| menu.layer = Some(layer));
    Some(layer)
}

/// The open update menu: Restart to update and What's new. Each closes the menu, then acts.
struct UpdateMenu {
    restart_focus: FocusHandle,
    whats_new_focus: FocusHandle,
    /// Set right after the layer opens; what a chosen item closes.
    layer: Option<LayerId>,
}

impl UpdateMenu {
    /// Close this menu (before an item acts).
    fn close(menu: &gpui::WeakEntity<UpdateMenu>, window: &mut Window, cx: &mut App) {
        if let Some(layer) = menu.upgrade().and_then(|menu| menu.read(cx).layer) {
            layers::close(layer, window, cx);
        }
    }
}

impl Render for UpdateMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (restart_menu, notes_menu) = (cx.weak_entity(), cx.weak_entity());
        let restart = MenuItem::new(
            "update-menu-restart",
            Icon::Restart,
            "Restart to update",
            HoverHelp::tooltip(
                "Restart to update: quit Farhelm and open it again on the version that is installed; your sessions \
                 keep running",
            ),
            Role::MenuItem,
        )
        .description("Sessions keep running")
        .focus(&self.restart_focus)
        .on_click(move |_, window, cx| {
            UpdateMenu::close(&restart_menu, window, cx);
            updates::restart_to_update(cx);
        });
        let whats_new = MenuItem::new(
            "update-menu-whats-new",
            Icon::Document,
            "What's new",
            HoverHelp::tooltip(
                "What's new: open Farhelm's release notes at farhelm.io/docs in your browser, each release with what \
                 changed",
            ),
            Role::MenuItem,
        )
        .description("Opens the release notes in your browser")
        .focus(&self.whats_new_focus)
        .on_click(move |_, window, cx| {
            UpdateMenu::close(&notes_menu, window, cx);
            cx.open_url(RELEASE_NOTES_URL);
        });
        layers::panel(cx)
            .id("update-menu")
            .debug_selector(|| "update-menu".into())
            .w(px(MENU_WIDTH))
            .child(MenuHeader::new(
                &HostText::from_host("Farhelm update"),
                HostSentence::new("update-menu-summary").text("An installed update is ready"),
            ))
            .child(
                div()
                    .id("update-menu-items")
                    .role(Role::Menu)
                    .aria_label("Update")
                    .flex()
                    .flex_col()
                    .child(restart)
                    .child(whats_new),
            )
    }
}

/// The check button's hover help: what a press does, then the readout's own hover words. One function so the
/// version readout's tooltip golden draws exactly the words a real hover shows.
pub(crate) fn check_help(tooltip: &str) -> String {
    format!("Select this to check for updates. {tooltip}")
}

#[cfg(test)]
mod tests;
