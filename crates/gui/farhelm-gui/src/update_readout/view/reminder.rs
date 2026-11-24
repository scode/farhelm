//! A pending update earns one reminder per interval. A missed reminder waits for the user to return, with no
//! animation subscription or retry timer while it waits.

use std::time::Duration;

use gpui::{
    AnyElement, App, AvailableSpace, Context, Entity, FontWeight, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Pixels, Styled as _, Subscription, Window, canvas, div,
    point, px, size,
};

use super::{VersionReadout, inline_presentation};
use crate::{assets, focus, theme, ticker};

pub(super) const INTERVAL: Duration = Duration::from_secs(15 * 60);
pub(super) const RECENT: Duration = Duration::from_secs(5 * 60);
pub(super) const BURST_TICKS: u64 = 25;

/// A spare 24 pixels fits the glyph through its full bounce and leaves room for the pill's outside focus ring.
pub(super) const ARROW_ROOM: Pixels = px(24.);

#[derive(Default)]
pub(super) struct Reminder {
    ready: bool,
    visible: bool,
    activation: Option<Subscription>,
    due: Option<Due>,
    pub(super) deadline: Option<ticker::Deadline>,
    pub(super) burst: Option<ticker::AnimationTicks>,
    /// Screenshot captures hold a single frame with the scheduler and input wakes disabled by reduced motion.
    #[cfg(test)]
    pub(super) held_frame: Option<u64>,
}

/// An arrival can play as soon as the header is shown and active. A periodic reminder also checks recency when
/// the header is first shown again after a disconnect.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Due {
    Arrival,
    Periodic,
}

/// The advancing arrow and tint are sampled together. The tint survives when layout has no room for an arrow.
#[derive(Clone, Copy)]
pub(super) struct Frame {
    offset: Pixels,
    opacity: f32,
    pub(super) pulsing: bool,
}

/// At 10 Hz, eight steps make one bounce. After three bounces, the final resting frame ends the 2.5-second burst.
fn frame(tick: u64) -> Option<Frame> {
    if tick >= BURST_TICKS {
        return None;
    }
    let distance = [0., 1., 3., 5., 5., 3., 1., 0.][(tick % 8) as usize];
    Some(Frame {
        offset: px(distance),
        opacity: 0.65 + distance * 0.07,
        pulsing: distance >= 3.,
    })
}

impl Reminder {
    /// An owed reminder can wait without either kind of ticker handle.
    pub(super) fn is_due(&self) -> bool {
        self.due.is_some()
    }

    /// Release the live work when the pill disappears, becomes steady, or is dismissed.
    fn stop_work(&mut self) {
        self.burst = None;
        self.deadline = None;
    }

    /// Read the same frame for the pill and its neighboring arrow. A ready row may supply a fixed screenshot frame.
    pub(super) fn frame(&self, cx: &App) -> Option<Frame> {
        if !self.ready {
            return None;
        }
        #[cfg(test)]
        if let Some(tick) = self.held_frame {
            return frame(tick);
        }
        self.burst.as_ref().and_then(|burst| frame(burst.count(cx)))
    }
}

impl VersionReadout {
    /// The disconnected view retains the readout without drawing it. Remember anything already due, but release
    /// the timer and animation and defer any wake until the readout is visible again.
    pub(crate) fn suspend_reminder(&mut self) {
        self.reminder.visible = false;
        self.reminder.stop_work();
    }

    /// An arrival may animate immediately. Ordinary renders only retire a spent burst and ensure a quiet deadline
    /// exists. A deferred reminder stays due until an activation or an actual input wake consumes it.
    pub(super) fn update_reminder(
        &mut self,
        ready: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let arrived = ready && !self.reminder.ready;
        let shown_again = !std::mem::replace(&mut self.reminder.visible, true);
        self.reminder.ready = ready;
        if self.reminder.activation.is_none() {
            self.reminder.activation =
                Some(cx.observe_window_activation(window, |view, window, cx| {
                    view.start_due_reminder(window, cx);
                    cx.notify();
                }));
        }
        if !ready || ticker::reduced_motion(cx) {
            self.reminder.stop_work();
            self.reminder.due = None;
            return;
        }
        let active = ticker::is_active(window, cx);
        if !active
            || self
                .reminder
                .burst
                .as_ref()
                .is_some_and(|burst| burst.count(cx) >= BURST_TICKS)
        {
            self.reminder.burst = None;
        }
        if arrived {
            self.reminder.due = Some(Due::Arrival);
            self.start_due_reminder(window, cx);
        } else if shown_again {
            match self.reminder.due {
                Some(Due::Arrival) => self.start_due_reminder(window, cx),
                Some(Due::Periodic) => self.wake_periodic_reminder(window, cx),
                None => {}
            }
        }
        if !self.reminder.is_due() && self.reminder.deadline.is_none() {
            self.schedule_reminder(window, cx);
        }
    }

    /// Recheck the updater and window at each wake. An input wake can be deferred past a readout or motion change.
    fn start_due_reminder(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.reminder.visible
            && self.reminder.is_due()
            && self.readout(cx).update_ready
            && ticker::is_active(window, cx)
            && !ticker::reduced_motion(cx)
        {
            self.reminder.due = None;
            self.reminder.burst = Some(ticker::AnimationTicks::start(window, cx));
        }
    }

    /// Schedule on the shared clock. Only a focused window with recent input plays on the deadline itself.
    /// An inactive window wakes on activation, while an active but idle window waits for a press.
    fn schedule_reminder(&mut self, window: &Window, cx: &mut Context<Self>) {
        let handle = window.window_handle();
        let view = cx.weak_entity();
        self.reminder.deadline = Some(ticker::after(INTERVAL, cx, move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |readout, cx| {
                    readout.reminder.deadline = None;
                    readout.reminder.due = Some(Due::Periodic);
                    readout.wake_periodic_reminder(window, cx);
                    cx.notify();
                });
            });
        }));
    }

    /// An active window still needs recent input for a periodic reminder. If the user has gone idle, actual input
    /// wakes the owed reminder without having to run another timer.
    fn wake_periodic_reminder(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.reminder.visible
            || !ticker::is_active(window, cx)
            || !self.readout(cx).update_ready
            || ticker::reduced_motion(cx)
        {
            return;
        }
        let now = cx.background_executor().now();
        if focus::last_activity(window, cx)
            .is_some_and(|last| now.saturating_duration_since(last) <= RECENT)
        {
            self.start_due_reminder(window, cx);
        } else {
            let view = cx.weak_entity();
            focus::on_next_activity(window, cx, move |window, cx| {
                let _ = view.update(cx, |readout, cx| {
                    readout.start_due_reminder(window, cx);
                    cx.notify();
                });
            });
        }
    }

    /// Fill the actual gap between the wordmark and the readout. In prepaint the gap has its final width and the
    /// readout has sampled its frame. The arrow is positioned within that gap, independent of pill and menu layout.
    /// Keep the gap passive so it remains available for title-band dragging, even with the arrow visible.
    pub(crate) fn reminder_gap(view: Entity<Self>) -> impl gpui::IntoElement {
        canvas(
            move |bounds, window, cx| -> Option<AnyElement> {
                if bounds.size.width < ARROW_ROOM {
                    return None;
                }
                let view = view.read(cx);
                let frame = view.reminder.frame(cx)?;
                let status =
                    inline_presentation(&view.readout(cx), view.feedback.suppressed.as_ref());
                if status.shown.is_some() {
                    return None;
                }
                let mut arrow = div()
                    .id("upgrade-reminder-arrow")
                    .debug_selector(|| "upgrade-reminder-arrow".into())
                    .font_family(assets::TERMINAL_FONT)
                    .text_size(theme::metrics::TEXT_MD)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::color(theme::palette(cx).danger_strong))
                    .opacity(frame.opacity)
                    .child("→")
                    .into_any_element();
                let size = arrow.layout_as_root(
                    size(AvailableSpace::MinContent, AvailableSpace::MinContent),
                    window,
                    cx,
                );
                let origin = point(
                    bounds.right() - ARROW_ROOM + frame.offset,
                    bounds.center().y - size.height / 2.,
                );
                arrow.prepaint_at(origin, window, cx);
                Some(arrow)
            },
            |_, arrow, window, cx| {
                if let Some(mut arrow) = arrow {
                    arrow.paint(window, cx);
                }
            },
        )
        .flex_1()
        .min_w_0()
        .h_full()
    }
}
