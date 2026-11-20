//! One paint-only layer for short visual cues. Each request tracks an ordinary element and borrows its clip.
//! Neither the flight nor the flash is a layer children can focus, dismiss, or hit with a pointer.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, Context, Corners, ElementId, EntityId, Hsla, IntoElement as _,
    ParentElement as _, Pixels, Point, Styled as _, Subscription, Window, canvas, div, fill, point,
    px, size,
};

use super::{Anchor, LayerStack, stack};
use crate::components::cue_progress;
use crate::desktop::linux_title_bar::content_area;
use crate::{theme, ticker};

const TOTAL: Duration = Duration::from_millis(1050);
const STATIC_FLASH: Duration = Duration::from_millis(650);
const LANDING: f32 = 0.68;

/// A request owns the animation id, while the target keeps only its own geometry. Releasing a row or changing
/// its reason releases the anchor and retires the cue, even if the window has not become active yet.
struct Cue {
    id: u64,
    target: EntityId,
    anchor: Anchor,
    clipped: Rc<Cell<bool>>,
    motion: Motion,
    _release: Subscription,
}

enum Motion {
    Pending,
    Playing {
        moving: bool,
        _expire: ticker::Deadline,
    },
    #[cfg(test)]
    Still(f32),
}

/// Window-owned display slots. No code that decides input ownership consults these entries.
#[derive(Default)]
pub(super) struct Cues {
    entries: Vec<Cue>,
    next: u64,
}

impl Cues {
    /// Start cues only after the window becomes active. A question that disappeared while the window was away
    /// has already left with its anchor.
    pub(super) fn activate(&mut self, cx: &mut Context<LayerStack>) {
        let moving = !ticker::reduced_motion(cx);
        let mut started = false;
        for cue in &mut self.entries {
            if matches!(cue.motion, Motion::Pending) {
                cue.motion = playing(cue.id, moving, cx);
                started = true;
            }
        }
        if started {
            cx.notify();
        }
    }

    /// A UI session reset takes away its cues and drops their deadlines.
    pub(super) fn clear(&mut self) -> bool {
        let had_cues = !self.entries.is_empty();
        self.entries.clear();
        had_cues
    }

    pub(super) fn draw(&self, cx: &App) -> Option<AnyElement> {
        let color = theme::color(theme::palette(cx).warn_bright);
        let entries: Vec<_> = self
            .entries
            .iter()
            .filter_map(|cue| {
                let (anchor, clipped) = (cue.anchor.clone(), cue.clipped.clone());
                if clipped.get() {
                    return None;
                }
                match cue.motion {
                    Motion::Pending => None,
                    Motion::Playing { moving: true, .. } => Some(cue_progress(
                        ElementId::NamedInteger("attention-cue".into(), cue.id),
                        TOTAL,
                        move |value| {
                            paint(anchor.clone(), clipped.clone(), Frame::at(value), color)
                        },
                    )),
                    Motion::Playing { moving: false, .. } => {
                        Some(paint(anchor, clipped, Frame::Flash(0.24), color))
                    }
                    #[cfg(test)]
                    Motion::Still(value) => Some(paint(anchor, clipped, Frame::at(value), color)),
                }
            })
            .collect();
        (!entries.is_empty()).then(|| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .children(entries)
                .into_any_element()
        })
    }
}

/// Request one cue for a drawn target. The caller identifies the event that warrants attention. The layer stack
/// owns clipping, motion and lifetime, and accepts no input mark or focus handle.
pub(crate) fn request(anchor: &Anchor, window: &mut Window, cx: &mut App) -> bool {
    insert(anchor, window, cx, None)
}

fn insert(anchor: &Anchor, window: &mut Window, cx: &mut App, _still: Option<f32>) -> bool {
    if anchor.visible.get().is_none() {
        return false;
    }
    let Some(state) = anchor.state.upgrade() else {
        return false;
    };
    let Some(stack) = stack(window, cx) else {
        return false;
    };
    let active = ticker::is_active(window, cx);
    let moving = !ticker::reduced_motion(cx);
    let target = state.entity_id();
    stack.update(cx, |stack, cx| {
        if stack.cues.entries.iter().any(|cue| cue.target == target) {
            return false;
        }
        let id = stack.cues.next;
        stack.cues.next += 1;
        let release = cx.observe_release(&state, move |stack, _, cx| retire(stack, id, cx));
        let motion = if active {
            playing(id, moving, cx)
        } else {
            Motion::Pending
        };
        #[cfg(test)]
        let motion = _still.map_or(motion, Motion::Still);
        stack.cues.entries.push(Cue {
            id,
            target,
            anchor: anchor.clone(),
            clipped: Rc::new(Cell::new(false)),
            motion,
            _release: release,
        });
        cx.notify();
        true
    })
}

/// Capture a particular point of the same flight or flash without leaving a timer to influence the golden.
#[cfg(test)]
pub(crate) fn for_golden(anchor: &Anchor, value: f32, window: &mut Window, cx: &mut App) -> bool {
    insert(anchor, window, cx, Some(value))
}

fn playing(id: u64, moving: bool, cx: &mut Context<LayerStack>) -> Motion {
    let stack = cx.weak_entity();
    let duration = if moving { TOTAL } else { STATIC_FLASH };
    let expire = ticker::after(duration, cx, move |cx| {
        let _ = stack.update(cx, |stack, cx| retire(stack, id, cx));
    });
    Motion::Playing {
        moving,
        _expire: expire,
    }
}

fn retire(stack: &mut LayerStack, id: u64, cx: &mut Context<LayerStack>) {
    let old = stack.cues.entries.len();
    stack.cues.entries.retain(|cue| cue.id != id);
    if old != stack.cues.entries.len() {
        cx.notify();
    }
}

/// A flight leads into a flash with a transparent final frame, even if expiry waits for another app update.
enum Frame {
    Flight(f32),
    Flash(f32),
}

impl Frame {
    fn at(value: f32) -> Self {
        if value < LANDING {
            let travelled = value / LANDING;
            Self::Flight(1. - (1. - travelled).powi(2))
        } else {
            Self::Flash(0.30 * (1. - value) / (1. - LANDING))
        }
    }
}

/// Paint reads this frame's anchor after normal page prepaint. A row scrolled out of view ends its animation
/// instead of letting an old destination flash through the sidebar header.
///
/// On the frame a target leaves the element tree, GPUI can still hold its last bounds until releasing the keyed
/// state after paint. This is the same lifetime that [`Anchor::drawn_bounds`] gives the other anchored overlays.
fn paint(anchor: Anchor, clipped: Rc<Cell<bool>>, frame: Frame, color: Hsla) -> AnyElement {
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            if clipped.get() || anchor.drawn_bounds().is_none() {
                return;
            }
            let Some(target) = anchor.visible.get() else {
                clipped.set(true);
                return;
            };
            match frame {
                Frame::Flight(progress) => {
                    let start = content_area(window).center();
                    let end = point(target.left() + px(2.), target.center().y);
                    comet(start, end, progress, color, window);
                }
                Frame::Flash(opacity) => window.paint_quad(fill(target, color.opacity(opacity))),
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// A round light with a soft tail. The whole flight is decorative paint, including when it crosses controls.
fn comet(
    start: Point<Pixels>,
    end: Point<Pixels>,
    progress: f32,
    color: Hsla,
    window: &mut Window,
) {
    for i in (0..7).rev() {
        let i = i as f32;
        let at = (progress - i * 0.019).max(0.);
        let center = start + (end - start) * at;
        let radius = px(5. - i * 0.46);
        let dot = |r| Bounds::new(center - point(r, r), size(r * 2., r * 2.));
        let halo = radius * 1.85;
        window.paint_quad(
            fill(dot(halo), color.opacity(0.10 - i * 0.011)).corner_radii(Corners::all(halo)),
        );
        window.paint_quad(
            fill(dot(radius), color.opacity(1. - i * 0.135)).corner_radii(Corners::all(radius)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Role, TestAppContext};
    use std::cell::RefCell;

    use crate::components::{Button, HoverHelp};
    use crate::test_support::{bounds, click, focused_id, open, redraw, settle};

    /// The same anchored button receives clicks and keeps its focus while the cue paints above it. Hiding
    /// that target retires the slot before any subsequent window can show it.
    #[gpui::test]
    fn a_cue_leaves_input_and_focus_on_its_anchor(cx: &mut TestAppContext) {
        let anchor = Rc::new(RefCell::new(None));
        let shown = Rc::new(Cell::new(true));
        let clicks = Rc::new(Cell::new(0));
        let (record, show, counted) = (anchor.clone(), shown.clone(), clicks.clone());
        let cx = open(cx, move |window, cx| {
            if !show.get() {
                return div().into_any_element();
            }
            let anchored = Anchor::for_element("cue-test-anchor", window, cx);
            record.replace(Some(anchored.clone()));
            let counted = counted.clone();
            div()
                .relative()
                .w(px(180.))
                .child(
                    Button::new(
                        "cue-test",
                        "Answer",
                        HoverHelp::tooltip("Answer"),
                        Role::Button,
                    )
                    .on_click(move |_, _, _| counted.set(counted.get() + 1)),
                )
                .child(anchored.track())
                .into_any_element()
        });
        click(cx, "cue-test");
        assert_eq!(clicks.get(), 1);
        let before = focused_id(cx);
        assert_eq!(before.as_deref(), Some("cue-test"));
        let control = bounds(cx, "cue-test");
        let anchor = anchor.borrow().clone().expect("the target was drawn");
        cx.update(|window, cx| {
            assert!(anchor.drawn_bounds().is_some());
            assert!(for_golden(&anchor, 0.74, window, cx));
            assert!(
                !request(&anchor, window, cx),
                "one cue for the drawn target"
            );
            assert!(!super::super::focus_layer_open(window, cx));
            assert!(
                !super::super::dismiss_top(window, cx),
                "the cue takes no Escape"
            );
        });
        settle(cx);
        cx.update(|window, cx| {
            let Frame::Flash(opacity) = Frame::at(0.74) else {
                panic!("the chosen frame is the landing flash");
            };
            let flash = gpui::Background::from(
                theme::color(theme::palette(cx).warn_bright).opacity(opacity),
            );
            let center = control.scale(window.scale_factor()).center();
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .any(|quad| quad.background == flash && quad.bounds.contains(&center))
            );
        });
        assert_eq!(focused_id(cx), before);
        click(cx, "cue-test");
        assert_eq!(clicks.get(), 2, "paint did not intercept the button");
        shown.set(false);
        redraw(cx);
        settle(cx);
        cx.update(|window, cx| {
            let owner = stack(window, cx).expect("the window owns its layer stack");
            assert!(owner.read(cx).cues.entries.is_empty());
            assert!(!request(&anchor, window, cx), "the target no longer exists");
        });
    }

    /// Reduced motion paints a stationary flash, expires by the scheduler even without drawing, and leaves
    /// the rest of the layer stack empty.
    #[gpui::test]
    fn reduced_motion_expires_the_stationary_flash(cx: &mut TestAppContext) {
        let anchor = Rc::new(RefCell::new(None));
        let record = anchor.clone();
        let cx = open(cx, move |window, cx| {
            crate::test_support::anchor_element("cue-static-target", &record, window, cx)
                .into_any_element()
        });
        cx.update(|window, cx| {
            ticker::set_reduced_motion(true, cx);
            assert!(request(
                anchor.borrow().as_ref().expect("target"),
                window,
                cx
            ));
            let owner = stack(window, cx).expect("layer stack");
            assert!(matches!(
                owner.read(cx).cues.entries[0].motion,
                Motion::Playing { moving: false, .. }
            ));
        });
        settle(cx);
        cx.executor().advance_clock(STATIC_FLASH);
        settle(cx);
        cx.update(|window, cx| {
            let owner = stack(window, cx).expect("layer stack");
            assert!(owner.read(cx).cues.entries.is_empty());
        });
    }

    /// A pending cue waits while another app has focus. On activation a clipped target consumes itself without
    /// showing up against an old rectangle or waiting to surprise someone who scrolls back.
    #[gpui::test]
    fn an_inactive_cue_waits_and_a_clipped_target_does_not_flash(cx: &mut TestAppContext) {
        let anchor = Rc::new(RefCell::new(None));
        let record = anchor.clone();
        let clipped = Rc::new(Cell::new(false));
        let clipping = clipped.clone();
        let cx = open(cx, move |window, cx| {
            div()
                .h(px(if clipping.get() { 12. } else { 66. }))
                .overflow_hidden()
                .child(
                    crate::test_support::anchor_element("cue-clipped", &record, window, cx)
                        .mt(px(27.)),
                )
                .into_any_element()
        });
        cx.update(|window, cx| {
            let anchor = anchor.borrow();
            let anchor = anchor.as_ref().expect("target");
            assert!(anchor.drawn_bounds().is_some(), "the tracker was laid out");
            assert!(
                anchor.visible.get().is_some(),
                "the target starts inside the clip"
            );
            ticker::set_active_override(Some(false), cx);
            assert!(request(anchor, window, cx));
        });
        cx.executor().advance_clock(TOTAL * 2);
        settle(cx);
        clipped.set(true);
        redraw(cx);
        cx.update(|window, cx| {
            let owner = stack(window, cx).expect("layer stack");
            assert!(matches!(
                owner.read(cx).cues.entries[0].motion,
                Motion::Pending
            ));
            assert!(owner.read(cx).cues.draw(cx).is_none());
            let anchor = anchor.borrow();
            let anchor = anchor.as_ref().expect("target");
            assert!(
                anchor.visible.get().is_none(),
                "the target moved outside the clip"
            );
            assert!(!request(anchor, window, cx));
            ticker::set_active_override(Some(true), cx);
            owner.update(cx, |owner, cx| owner.cues.activate(cx));
        });
        redraw(cx);
        cx.update(|window, cx| {
            let owner = stack(window, cx).expect("layer stack");
            assert!(owner.read(cx).cues.entries[0].clipped.get());
            assert!(owner.read(cx).cues.draw(cx).is_none());
        });
    }
}
