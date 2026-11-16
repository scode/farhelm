//! The permission-escalation banner: the red band across the top of the launcher when an agent switch made the shown
//! permission more dangerous by itself, with Undo and Dismiss, and the permission control's red outline beside it.
//!
//! SPEC.md, "Creation": "When an agent switch automatically makes the shown permission more dangerous, a red banner
//! slides down across the top of the launcher, naming both modes and explaining that the new agent does not support the
//! previous mode. Undo restores the draft before the switch; dismiss closes the banner. The banner lasts about eight
//! seconds, paused while hovered or focused, and is announced to screen readers. The permission control gets a thick red
//! outline that fades after about two and a half seconds. Reduced motion replaces slide and fade with a static outline
//! while the banner is shown." Ported from the old launcher's `.permission-escalation-banner` (`list/create_form.rs`
//! and `assets/app.css` at reference commit `24bc0580`). Here's the TLDR:
//!
//! - **What raises it** is the draft's to say ([`Edit::escalation`], and a template batch's, judged once per batch):
//!   only an agent switch the user did not aim at the permission, and only a move to a more dangerous group. The view
//!   shows what it is handed. Any agent switch takes an earlier banner away, since its Undo would restore a draft from
//!   before a switch that is no longer the last one.
//! - **It is its own state**, not the launcher's note: other edits leave it up (a name typed after the switch), so Undo
//!   still restores the whole draft from before the switch, as the old one did. Undo, Dismiss, another agent switch and
//!   its time running out take it away.
//! - **Its time** is eight seconds of being shown, counted by a ticker deadline. Hovering it, or focus on its Undo or
//!   Dismiss, stops the count; the rest of the eight seconds resumes when both end. Unlike the old one it does not stop
//!   while the window is inactive: the ticker's deadlines fire on time whether or not the window is active (SPEC_impl.md,
//!   "Animation time: the ticker"; a recorded deviation).
//! - **Its motion** goes through the component layer's one-shot transitions, keyed on the banner's generation, so a new
//!   banner plays both again: the slide down, and the outline's fade. Under reduced motion there is no slide, and the
//!   outline stays, unfaded, for as long as the banner shows.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Context, FocusHandle, FontWeight, HoverListenerMode, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};

use super::{Launcher, on_launcher};
use crate::components::{Button, HoverHelp, Role, Tier, outline_fade, slide_down};
use crate::focus::{self, Mark};
use crate::launcher::draft::{Edit, Escalation};
use crate::theme::{self, metrics};
use crate::ticker;

/// How long the banner shows, counted only while it is neither hovered nor holding focus: the old
/// `permission-banner-lifetime 8s`.
const SHOWS_FOR: Duration = Duration::from_secs(8);

/// The width of the permission control's red outline, and of the banner's bottom edge: the old `outline: 4px solid` and
/// `border-bottom: 3px solid`.
const OUTLINE: f32 = 4.;
const EDGE: f32 = 3.;

/// The banner while it shows. One at most; a new one replaces it.
pub(super) struct EscalationBanner {
    /// Counts banners in this launcher, so each one's slide and outline fade play afresh (their transition ids carry
    /// it).
    generation: u64,
    escalation: Escalation,
    /// The note beside the agent choices before the switch, which Undo puts back with the draft, as the old Undo did.
    notice_before: Option<String>,
    /// The time left to show, as of the last time the count stopped or started.
    remaining: Duration,
    /// While counting: when the count last started, and the deadline that takes the banner away when it runs out.
    /// `None` while the count stops (hovered or focused).
    counting: Option<(Instant, ticker::Deadline)>,
    /// Whether the pointer is over the banner, as its hover listener last said.
    hovered: bool,
    undo_focus: FocusHandle,
    dismiss_focus: FocusHandle,
}

impl Launcher {
    /// An agent-choice edit happened (a hand switch, a model of another agent, a search row, a recent setup, a template
    /// batch): an agent switch takes the banner away, and an escalation it carries puts up a new one, counting from now.
    /// `notice_before` is the note beside the agent choices before the edit, for Undo to put back.
    pub(super) fn follow_escalation(
        &mut self,
        switched_agent: bool,
        escalation: Option<Escalation>,
        notice_before: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if switched_agent {
            self.escalation = None;
        }
        let Some(escalation) = escalation else {
            return;
        };
        self.escalation_generation += 1;
        self.escalation = Some(EscalationBanner {
            generation: self.escalation_generation,
            escalation,
            notice_before,
            remaining: SHOWS_FOR,
            counting: None,
            hovered: false,
            undo_focus: cx.focus_handle().tab_stop(true),
            dismiss_focus: cx.focus_handle().tab_stop(true),
        });
        self.start_escalation_count(cx);
        cx.notify();
    }

    /// [`Self::follow_escalation`] for a hand edit's result, before its notice replaces the one shown.
    pub(super) fn follow_edit_escalation(&mut self, edit: &Edit, cx: &mut Context<Self>) {
        self.follow_escalation(
            edit.switched_agent,
            edit.escalation.clone(),
            self.notice.clone(),
            cx,
        );
    }

    /// Start counting the banner's remaining time down, if it is not counting already.
    fn start_escalation_count(&mut self, cx: &mut Context<Self>) {
        let launcher = cx.weak_entity();
        let now = cx.background_executor().now();
        let Some(banner) = &mut self.escalation else {
            return;
        };
        if banner.counting.is_some() {
            return;
        }
        let generation = banner.generation;
        let deadline = ticker::after(banner.remaining, cx, move |cx| {
            let _ = launcher.update(cx, |launcher, cx| {
                // Only this banner's own deadline takes it away; a newer banner has its own.
                if launcher
                    .escalation
                    .as_ref()
                    .is_some_and(|banner| banner.generation == generation)
                {
                    launcher.escalation = None;
                    cx.notify();
                }
            });
        });
        banner.counting = Some((now, deadline));
    }

    /// Stop the count, keeping the time left: dropping the deadline cancels it.
    fn stop_escalation_count(&mut self, cx: &App) {
        let now = cx.background_executor().now();
        if let Some(banner) = &mut self.escalation
            && let Some((started, _deadline)) = banner.counting.take()
        {
            banner.remaining = banner
                .remaining
                .saturating_sub(now.saturating_duration_since(started));
        }
    }

    /// Keep the count in step with what holds the banner: stopped while it is hovered or one of its buttons holds
    /// focus, counting otherwise. Called while drawing, since focus moves without telling the launcher.
    fn follow_escalation_hold(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(banner) = &self.escalation else {
            return;
        };
        let held = banner.hovered
            || banner.undo_focus.is_focused(window)
            || banner.dismiss_focus.is_focused(window);
        match (held, banner.counting.is_some()) {
            (true, true) => self.stop_escalation_count(cx),
            (false, false) => self.start_escalation_count(cx),
            _ => {}
        }
    }

    /// Undo: put back the draft from before the switch, whatever was edited since, and go on in the search box, where
    /// the old launcher put focus.
    fn undo_escalation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(banner) = self.escalation.take() else {
            return;
        };
        let mark = Mark::take(cx);
        self.draft = *banner.escalation.undo;
        self.notice = banner.notice_before;
        self.sync_fields(window, cx);
        self.changed(cx);
        focus::focus_in_layer(&self.search.focus_handle(cx), &mark, window, cx);
    }

    /// Dismiss: take the banner away, keeping the switch, and go on in the search box.
    fn dismiss_escalation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.escalation.take().is_none() {
            return;
        }
        let mark = Mark::take(cx);
        focus::focus_in_layer(&self.search.focus_handle(cx), &mark, window, cx);
        cx.notify();
    }

    /// The banner's band, for the dialog frame's top band, while it shows: the warning glyph, the message, then Undo
    /// and Dismiss with the old tooltips, announced as an alert. `holding` is the YOLO question's host write being out,
    /// while which Undo, like every edit, waits.
    pub(super) fn escalation_band(
        &mut self,
        holding: bool,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.follow_escalation_hold(window, cx);
        let banner = self.escalation.as_ref()?;
        let message = SharedString::from(banner.escalation.message());
        let generation = banner.generation;
        let hover = cx.weak_entity();
        let band = div()
            // A new banner is a new element, so assistive technology announces it afresh even where it replaces one in
            // place (under reduced motion nothing else marks the change).
            .id(SharedString::from(format!(
                "launcher-escalation-{generation}"
            )))
            .debug_selector(|| "launcher-escalation".into())
            .role(Role::Alert)
            .aria_label(message.clone())
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .p(px(12.))
            .border_b(px(EDGE))
            .border_color(theme::color(palette.danger))
            .bg(theme::color(palette.danger_tint))
            .text_size(metrics::TEXT_MD)
            .text_color(theme::color(palette.fg_0))
            // Hover is followed whatever the last input was: by default GPUI takes a key press for the pointer leaving,
            // which would let the banner run out under a resting pointer while the user types in the search box.
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(move |over, _, cx| {
                let over = *over;
                let _ = hover.update(cx, |launcher, cx| {
                    if let Some(banner) = &mut launcher.escalation
                        && banner.hovered != over
                    {
                        banner.hovered = over;
                        cx.notify();
                    }
                });
            })
            .child(
                div()
                    .flex_none()
                    .self_start()
                    .text_color(theme::color(palette.danger))
                    .child("⚠"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(240.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(message),
            )
            .child(
                Button::new(
                    "launcher-escalation-undo",
                    "Undo",
                    HoverHelp::tooltip("Undo: restore the draft from before this agent switch"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .focus(&banner.undo_focus)
                .busy(holding)
                .on_click(on_launcher(cx.weak_entity(), Launcher::undo_escalation)),
            )
            .child(
                Button::new(
                    "launcher-escalation-dismiss",
                    "Dismiss",
                    HoverHelp::tooltip("Dismiss this permission warning"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .focus(&banner.dismiss_focus)
                .on_click(on_launcher(cx.weak_entity(), Launcher::dismiss_escalation)),
            );
        Some(slide_down(
            SharedString::from(format!("launcher-escalation-slide-{generation}")),
            band,
            cx,
        ))
    }

    /// `control`, the permission control, with the banner's thick red outline around it while a banner shows: fading
    /// from the banner's start, or static under reduced motion (the transition's own rule). The outline is drawn
    /// outside the control's edge, as the old `outline-offset: 0` drew it, by a border on a wrapper that takes as much
    /// room as the outline, so the control does not move when it comes or goes.
    pub(super) fn with_escalation_outline(
        &self,
        control: impl IntoElement,
        palette: &theme::Palette,
        cx: &App,
    ) -> AnyElement {
        let wrapper = div().p(px(OUTLINE)).m(px(-OUTLINE)).child(control);
        let Some(banner) = &self.escalation else {
            return wrapper.into_any_element();
        };
        let danger = theme::color(palette.danger);
        outline_fade(
            SharedString::from(format!("launcher-escalation-outline-{}", banner.generation)),
            wrapper
                .p(px(0.))
                .border(px(OUTLINE))
                .border_color(danger)
                // Tests find the outline by where it is drawn; it is drawn only while a banner shows.
                .debug_selector(|| "launcher-escalation-outline".into()),
            danger,
            cx,
        )
    }
}
