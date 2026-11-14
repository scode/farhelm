//! The terminal's scrollback scrollbar: the old app's slider in the column the terminal's fit leaves free at its right
//! edge (`.grid-terminal-scrollbar` and grid-terminal.js's "Scrollbar" at the reference commit).
//!
//! Here's the TLDR of what it does. The slider's height is the share of the lines that are visible (never under
//! [`SLIDER_MIN_HEIGHT`] pixels) and its place is the view's position between the oldest line and the live screen.
//! It is drawn in the terminal's foreground at 20% opacity, 40% while the pointer is over the slider and 50% while it
//! is dragged. A press on the slider drags it; a press on the track centers the slider there and then drags it, as VS
//! Code's scrollbar does. It says where to scroll through its `on_scroll` handler and keeps no position of its own,
//! so the view's anchor stays the one truth about where the view is.
//!
//! When it shows is its owner's call ([`Scrollbar::shown`]): the old app showed it while the pointer was over the
//! terminal and for a moment after a scroll, and only when there was something to scroll. A drag keeps it shown.
//!
//! NOTE: The old bar faded in over 100 ms and out over 800 ms. This one appears and disappears at once (its owner
//! hides it 800 ms after the last scroll): an animated fade must stop under reduced motion, and the ticker, which owns
//! that setting, gives a control no way to ask for it. Recorded as a deviation in the terminal's parity file.
//!
//! It is not a tab stop, and a press on it leaves focus where it was: SPEC.md ("Reaching everything by keyboard") has
//! the terminal scroll by keyboard with Shift+PageUp and Shift+PageDown, and the old bar's press prevented its default,
//! so the terminal kept focus while the bar was dragged.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, Bounds, DispatchPhase, Element, ElementId, Entity, GlobalElementId, Hitbox,
    HitboxBehavior, Hsla, InspectorElementId, InteractiveElement, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, RenderOnce,
    Role, SharedString, Size, StatefulInteractiveElement, Style, Styled, Window, div, fill, point,
    px, relative, size,
};

use super::HoverHelp;

/// The slider never gets shorter than this, in pixels (`SLIDER_MIN_HEIGHT` in grid-terminal.js).
pub const SLIDER_MIN_HEIGHT: f32 = 20.;

/// The scrollbar's width: the column the terminal's fit leaves free (`SCROLLBAR_WIDTH` in grid-terminal.js).
pub const SCROLLBAR_WIDTH: f32 = 14.;

/// What the bar shows: `visible` lines of `total`, the view's top at `top`, counted from the oldest line (so `top`
/// runs from 0 to `total - visible`, the live screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollbarLines {
    pub visible: usize,
    pub total: usize,
    pub top: usize,
}

impl ScrollbarLines {
    /// Whether there is anything to scroll; a bar with nothing to scroll is not drawn.
    pub fn scrollable(self) -> bool {
        self.total > self.visible && self.visible > 0
    }

    /// The last top position, where the view shows the live screen.
    fn bottom(self) -> usize {
        self.total.saturating_sub(self.visible)
    }
}

/// The slider's place in a track `track` pixels tall: its top offset and height, rounded to whole pixels as the old
/// bar's were.
pub fn slider(lines: ScrollbarLines, track: f32) -> (f32, f32) {
    let total = lines.total.max(1) as f32;
    let height = (track * lines.visible as f32 / total)
        .round()
        .max(SLIDER_MIN_HEIGHT)
        .min(track);
    let bottom = lines.bottom();
    let offset = if bottom > 0 {
        ((track - height) * lines.top as f32 / bottom as f32).round()
    } else {
        0.
    };
    (offset, height)
}

/// The top position a drag puts the view at: the slider's top at `offset` pixels into the track, as a fraction of its
/// travel, times the last top position. Not rounded: the view rounds it to a line.
pub fn top_at(lines: ScrollbarLines, track: f32, offset: f32) -> f32 {
    let (_, height) = slider(lines, track);
    let travel = track - height;
    if travel <= 0. {
        return 0.;
    }
    offset.clamp(0., travel) / travel * lines.bottom() as f32
}

/// What the bar asks of its owner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollbarEvent {
    /// Move the view's top to this position (see [`ScrollbarLines`]), from a press or a drag.
    Scroll(f32),
    /// The drag ended: the owner starts the bar's hide delay. A drag the bar's own going away cut short (nothing
    /// left to scroll) sends none; it simply ends.
    Released,
}

type OnScroll = Rc<dyn Fn(ScrollbarEvent, &mut Window, &mut App)>;

/// The scrollbar control. See the module docs.
#[derive(IntoElement)]
pub struct Scrollbar {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    role: Role,
    lines: ScrollbarLines,
    color: Hsla,
    shown: bool,
    on_scroll: Option<OnScroll>,
}

impl Scrollbar {
    /// A scrollbar over `lines`, drawn in `color`, which it dims itself. `label` is its accessible name and `role` what
    /// assistive technology and the test bridge see (usually [`Role::ScrollBar`]).
    ///
    /// The color is the terminal's default foreground, from the terminal's palette (or the program's own default),
    /// not a theme token: the old bar drew in `currentColor`, so it reads as part of the terminal whatever its colors.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        role: Role,
        lines: ScrollbarLines,
        color: Hsla,
    ) -> Self {
        Scrollbar {
            id: id.into(),
            label: label.into(),
            help,
            role,
            lines,
            color,
            shown: false,
            on_scroll: None,
        }
    }

    /// Whether the slider is drawn now (its owner's showing rule; a drag in progress shows it regardless).
    pub fn shown(mut self, shown: bool) -> Self {
        self.shown = shown;
        self
    }

    /// Where presses and drags on the bar go.
    pub fn on_scroll(
        mut self,
        handler: impl Fn(ScrollbarEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_scroll = Some(Rc::new(handler));
        self
    }
}

/// A drag in progress, kept across frames in the bar's own keyed state: the pointer's offset from the slider's top
/// when the press started, or `None` while nothing is dragged.
#[derive(Default)]
struct Drag {
    grab: Option<f32>,
}

/// Where the last frame drew the bar, for the press listener, which runs between frames: the track and the slider's
/// place in it.
#[derive(Clone, Copy)]
struct Painted {
    lines: ScrollbarLines,
    bounds: Bounds<Pixels>,
    slider: Bounds<Pixels>,
}

impl Painted {
    /// The view's top position for the slider's top at `y` (window coordinates) less `grab`.
    fn top_at(&self, y: Pixels, grab: f32) -> f32 {
        top_at(
            self.lines,
            f32::from(self.bounds.size.height),
            f32::from(y - self.bounds.origin.y) - grab,
        )
    }
}

impl RenderOnce for Scrollbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // With nothing to scroll there is no bar at all, as the old one was `display: none`: no hover help over an
        // empty column, no scrollbar for assistive technology, and the press goes on to what is under it. Its drag
        // state goes with it, so a drag cut short when the history went away (the alternate screen, a reset) does
        // not come back stuck.
        if !self.lines.scrollable() {
            return div().into_any_element();
        }
        let drag = window.use_keyed_state(
            ElementId::Name(format!("{}-drag", self.id).into()),
            cx,
            |_, _| Drag::default(),
        );
        let painted: Rc<Cell<Option<Painted>>> = Rc::default();
        // A press on the bar is the bar's alone, whichever button: it neither reaches the program nor moves focus (the
        // old bar's `preventDefault` and `stopPropagation`). Only the primary button scrolls. On the slider the press
        // grabs it where it landed and moves nothing yet; on the track it centers the slider on the pointer, which
        // scrolls there at once, and then drags. The press is the element's own listener rather than the window's,
        // which is also what makes assistive technology and the test bridge see the bar as a control.
        let on_press = {
            let painted = painted.clone();
            let drag = drag.clone();
            let on_scroll = self.on_scroll.clone();
            move |event: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                let Some(painted) = painted.get() else {
                    return;
                };
                window.prevent_default();
                cx.stop_propagation();
                if event.button != MouseButton::Left {
                    return;
                }
                let on_slider = painted.slider.contains(&event.position);
                let grab = if on_slider {
                    f32::from(event.position.y - painted.slider.origin.y)
                } else {
                    f32::from(painted.slider.size.height) / 2.
                };
                drag.update(cx, |drag, _| drag.grab = Some(grab));
                if !on_slider && let Some(on_scroll) = &on_scroll {
                    on_scroll(
                        ScrollbarEvent::Scroll(painted.top_at(event.position.y, grab)),
                        window,
                        cx,
                    );
                }
                window.refresh();
            }
        };
        let bar = div()
            .id(ElementId::Name(self.id.clone()))
            .debug_selector(|| self.id.to_string())
            .role(self.role)
            .aria_label(self.label.clone())
            .size_full()
            .on_any_mouse_down(on_press)
            .child(SliderElement {
                lines: self.lines,
                color: self.color,
                shown: self.shown,
                on_scroll: self.on_scroll,
                drag,
                painted,
            });
        super::with_hover_help(bar, &self.id, &self.help, true, window, cx).into_any_element()
    }
}

/// The track and slider, painted directly, with the drag's move and release. The press is the bar's element's own
/// listener (see `render`), which takes presses only where the bar is hit, so a dialog or menu drawn over the terminal
/// keeps its clicks. The drag's move and release are the window's, so a drag keeps following the pointer once that
/// leaves the bar's few pixels, as the old bar's document-level listeners did.
struct SliderElement {
    lines: ScrollbarLines,
    color: Hsla,
    shown: bool,
    on_scroll: Option<OnScroll>,
    drag: Entity<Drag>,
    /// Where this frame draws the bar, for the press listener.
    painted: Rc<Cell<Option<Painted>>>,
}

impl IntoElement for SliderElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for SliderElement {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            size: Size {
                width: relative(1.).into(),
                height: relative(1.).into(),
            },
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) -> Hitbox {
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        // While there is something to scroll the bar takes presses even when it is not drawn, as the old transparent
        // bar did; with nothing to scroll it is not here at all (see `render`).
        let lines = self.lines;
        let track = f32::from(bounds.size.height);
        let (offset, height) = slider(lines, track);
        let slider_bounds = Bounds::new(
            bounds.origin + point(px(0.), px(offset)),
            size(bounds.size.width, px(height)),
        );
        self.painted.set(Some(Painted {
            lines,
            bounds,
            slider: slider_bounds,
        }));
        let dragging = self.drag.read(cx).grab.is_some();
        let hovered = hitbox.is_hovered(window) && slider_bounds.contains(&window.mouse_position());
        if self.shown || dragging {
            let opacity = if dragging {
                0.5
            } else if hovered {
                0.4
            } else {
                0.2
            };
            window.paint_quad(fill(slider_bounds, self.color.opacity(opacity)));
        }
        let top = bounds.origin.y;
        // The drag's moves, window-wide, so they arrive wherever the pointer goes.
        let scroll_to = {
            let on_scroll = self.on_scroll.clone();
            move |grab: f32, y: Pixels, window: &mut Window, cx: &mut App| {
                if let Some(on_scroll) = &on_scroll {
                    let at = top_at(lines, track, f32::from(y - top) - grab);
                    on_scroll(ScrollbarEvent::Scroll(at), window, cx);
                }
            }
        };
        window.on_mouse_event({
            let drag = self.drag.clone();
            let released = self.on_scroll.clone();
            let hitbox = hitbox.clone();
            move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                if let Some(grab) = drag.read(cx).grab {
                    if event.pressed_button == Some(MouseButton::Left) {
                        scroll_to(grab, event.position.y, window, cx);
                    } else {
                        // The release happened where the window never saw it (outside it, say): a move with the
                        // button up ends the drag, so the bar does not stay grabbed.
                        drag.update(cx, |drag, _| drag.grab = None);
                        if let Some(on_scroll) = &released {
                            on_scroll(ScrollbarEvent::Released, window, cx);
                        }
                        window.refresh();
                    }
                } else if (hitbox.is_hovered(window) && slider_bounds.contains(&event.position))
                    != hovered
                {
                    // The slider's hover brightness is painted from where the pointer was at this frame.
                    window.refresh();
                }
            }
        });
        let drag = self.drag.clone();
        let on_scroll = self.on_scroll.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble
                || event.button != MouseButton::Left
                || drag.read(cx).grab.is_none()
            {
                return;
            }
            drag.update(cx, |drag, _| drag.grab = None);
            if let Some(on_scroll) = &on_scroll {
                on_scroll(ScrollbarEvent::Released, window, cx);
            }
            window.refresh();
        });
    }
}
