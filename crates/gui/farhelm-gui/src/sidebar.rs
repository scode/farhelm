//! The sidebar: the app header above the session list, at the width the user chose, with the divider that changes it.
//!
//! SPEC.md, "Session list" (the resize paragraph) and "Desktop window chrome" (the app bar). Ported from the old app's
//! `.app-sidebar` column, its `app_bar.rs` and the `sidebar-width.js` script with its `.sidebar-resize-handle`, at the
//! reference commit. The column is, top to bottom, the app header ([`header`]: the wordmark and the version readout,
//! across the whole sidebar, the host rail's column included) and the session list, whose own row holds the host rail
//! and the list beside it.
//!
//! - **The width is a helm preference.** It is [`Preferences::sidebar_width`], the whole sidebar with the 56-pixel
//!   collapsed rail included, shared by every client of the helm (the maintainer's decision of 2026-11-13; the old app
//!   kept it per device and nothing carries over), and [`default_width`] while it is unset. The view keeps no copy of
//!   it beyond the hold below: it reads the store as it draws, so a change the helm pushes moves the sidebar too.
//! - **A drag follows every move and writes once.** While the primary button is held on the divider, the sidebar takes
//!   the pointer's width on every move (clamped to [`SIDEBAR_WIDTH_RANGE`]), and the helm hears only the width the drag
//!   ended at, as the old script persisted "completed gestures, never every pointer sample". A drag that ends without
//!   its release being seen (the window never got it) ends the same way, at the last width it showed. A press and
//!   release that moved nothing writes nothing.
//! - **Double-click and the arrows write at once.** A double-click on the divider resets to the default and writes it;
//!   Left and Right on the focused divider step by [`STEP`], clamped, and write on every press, as the old handle did.
//!   Focus stays on the divider.
//! - **What it set, it shows until the helm answers.** A write takes a moment to come back as a pushed preference, and
//!   in that moment the store still holds the width from before. So the width this window last sent is held and shown
//!   until the helm replies to it, by which time the store holds it (the helm pushes a command's changes before its
//!   reply); a second press meanwhile steps from the held width, not the stale one. Only the latest write's reply drops
//!   the hold, so an earlier reply cannot pull the sidebar back. A refused write drops it too, and the sidebar shows
//!   what the helm stores. This hold is one of the two exceptions to the store rule that views keep no copy of shared
//!   state (SPEC_impl.md, "The store, and how views redraw", decided by the migration's orchestrator on 2026-11-14; the
//!   terminal text size's step is the other, held the same way), and it must stay that narrow: only the width the user
//!   is moving, only until its write's reply. The helm refuses a width outside the range rather than clamping it, which
//!   is why every width is clamped before it is sent.
//!
//! - **Expanding the rail widens what is shown, never what is stored.** While the host rail is expanded
//!   ([`SessionList::rail_expanded`]), the sidebar shows its width plus the expanded rail's extra 164 pixels, taken from
//!   the main pane, so the session list beside the rail keeps its width ([`shown_width`], the old `shownWidth`). In a
//!   window too narrow for that, the sidebar grows only as far as leaves the pane its floor, and the session list gives
//!   up the rest; collapsing gives the width back. The width preference, the divider's value, a drag, a step and a
//!   double-click all keep working on the stored width, whatever the rail does (SPEC.md: "The stored resize width and its
//!   range keep their meaning, independent of rail expansion"), so a drag moves the shown edge by as much as the
//!   pointer moves while there is room.
//!
//! The drag and the hold belong to the UI session ([`PerSession`]): a reset forgets both, and the sidebar draws what
//! the new snapshot holds.
//!
//! The divider sits between the sidebar's column and the pane, over the column's 1-pixel edge and 4 pixels of the pane
//! ([`DIVIDER_REACH`]), as the old handle did. It cannot stay inside the sidebar: each row's actions-menu button sits 2
//! pixels inside the column's inner edge, so a 5-pixel strip there would cover part of every button, and a strip that
//! missed them would be narrower than the old 5-pixel hit area. The 4 pixels it reaches into the pane land in the
//! terminal's 8-pixel padding (`PANE_PADDING` in `terminal/attached.rs`). It is drawn in two parts
//! ([`crate::components::Divider`] has the whole reason): the strip, the tab stop drawn here after the column, so Tab
//! and assistive technology meet it between the sidebar and the pane; and its grip, which takes the pointer, drawn by
//! the main view after the pane ([`Sidebar::grip`]), since GPUI gives the pointer to whatever is drawn last and the
//! pane would otherwise take the 4 pixels the divider reaches over.
//!
//! The sidebar also draws the edge that hover help placed beside it sits against ([`layers::beside_edge`]: SPEC.md's
//! session-row hovers and host progress card). It is drawn once per window, on a borderless box around the bordered
//! column, the host rail and the list included: a tracker covers the box inside its element's border, and the old app
//! measured the help's 6-pixel gap from outside the column's 1-pixel right border, so drawing it on the column itself
//! would put help a pixel nearer than the old app did. Without it, help that asks to sit beside the sidebar shows at
//! the pointer.
//!
//! NOTE: Not built here, each with its own area: the macOS title band (the 48-pixel app bar with room for the window
//! buttons and the window drag on its empty background, the desktop-integration area's) and the updater's states and
//! actions in the version readout (the same area's). The version readout shows this window's build: the GUI does not
//! learn the helm's build, so it cannot yet say when the two differ (the parity file records the wording that waits).

mod header;

#[cfg(test)]
mod tests;

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences, SIDEBAR_WIDTH_RANGE};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement, Pixels, Render, Styled, Window, div, prelude::FluentBuilder as _,
    px,
};

use crate::components::{Divider, DividerDrag, DividerStep, HoverHelp, Role};
use crate::desktop::{linux_title_bar, title_band, updates};
use crate::session::{self, PerSession};
use crate::session_list::SessionList;
use crate::session_list::host_rail::expansion;
use crate::theme::{self, metrics};
use crate::update_readout::view::VersionReadout;
use crate::{layers, operations, render_count};

/// The sidebar's width while the helm stores none: `--sidebar-width`'s default, the rail included, which the theme
/// keeps as [`metrics::SIDEBAR_WIDTH`] (the screenshot goldens draw the session list at it too).
pub fn default_width() -> u32 {
    f32::from(metrics::SIDEBAR_WIDTH).round() as u32
}

/// How far Left or Right on the focused divider moves the width.
pub const STEP: u32 = 10;

/// The main pane's floor (`.app-main`'s `min-width`): a window too narrow for the sidebar and this much scrolls
/// sideways instead of crushing the pane (SPEC.md: "The main pane keeps its 320px floor; a narrower window scrolls
/// horizontally").
pub const MAIN_PANE_MIN_WIDTH: Pixels = px(320.);

/// How far the divider reaches over the pane beside the sidebar (the old handle's `margin-right: -4px`).
const DIVIDER_REACH: Pixels = px(4.);

/// The width the sidebar shows for a stored `width`, with the host rail `expanded` or not, in a window whose content is
/// `viewport` wide ([`linux_title_bar::content_width`]): the old `shownWidth`. Collapsed, the stored width. Expanded, the stored width
/// plus the expanded rail's extra pixels ([`expansion::EXTRA`]) when the window has room, else as much of them as
/// leaves the main pane its floor and the sidebar's edge a pixel more, but never less than the stored width: a window
/// already too narrow for the stored width keeps it and scrolls sideways, as it would collapsed.
///
/// The extra pixel keeps the shown width below the narrow-window cutoff ([`header_floats`]: the pane's floor and the
/// edge) whenever the stored width alone is below it, so expanding never by itself moves the macOS app header into the
/// window's top row, which the old script promised too ("Expansion alone must not turn on either narrow-window class").
pub(crate) fn shown_width(width: u32, expanded: bool, viewport: Pixels) -> u32 {
    if !expanded {
        return width;
    }
    let room = (f32::from(viewport - MAIN_PANE_MIN_WIDTH) - 2.)
        .floor()
        .max(0.) as u32;
    width.max((width + expansion::EXTRA).min(room))
}

/// The divider's width: the old handle's 5-pixel strip, one pixel over the sidebar's edge and the rest over the pane.
const DIVIDER_WIDTH: Pixels = px(5.);

/// The divider's hover help, the old handle's tooltip.
const DIVIDER_HELP: &str = "Drag to resize the sidebar; double-click to reset";

/// The sidebar view. The main view holds one for the window's life, around the session list it also holds.
pub struct Sidebar {
    /// The list below the header, the same entity the main view hands its shortcuts to.
    session_list: Entity<SessionList>,
    /// What belongs to the current UI session alone: the drag in progress and the width held until the helm answers.
    current: PerSession<Current>,
    /// The divider's focus handle, a tab stop. Kept here rather than in the divider's element state, so it survives the
    /// frames in which the window is inert and draws no sidebar.
    divider_focus: FocusHandle,
    /// The version readout, while the desktop updater runs (`update_readout::view`); without one the header draws the
    /// plain build text. Kept here, for the window's life, rather than in the header's element state, so Up to date's
    /// five seconds and a dismissal survive the header moving into the narrow window's top row and back.
    readout: Option<Entity<VersionReadout>>,
}

/// The sidebar's state in one UI session.
#[derive(Default)]
struct Current {
    /// The drag in progress, if one is.
    drag: Option<Drag>,
    /// The width this window last sent and the number it was sent under, shown until the helm replies to it: the store
    /// rule's one exception (SPEC_impl.md, "The store, and how views redraw").
    held: Option<(u64, u32)>,
    /// How many widths this UI session sent; the last one's number.
    sent: u64,
}

/// A drag on the divider: where it started and what it shows now.
#[derive(Clone, Copy, Debug)]
struct Drag {
    /// The pointer's horizontal position when the button went down.
    start_x: Pixels,
    /// The width the sidebar had then.
    start_width: u32,
    /// The width the latest move gave.
    width: u32,
}

impl Sidebar {
    /// The sidebar around `session_list`.
    pub fn new(session_list: Entity<SessionList>, cx: &mut Context<Self>) -> Self {
        Sidebar {
            session_list,
            current: PerSession::default(),
            divider_focus: cx.focus_handle().tab_stop(true),
            readout: updates::entity(cx)
                .map(|updater| cx.new(|cx| VersionReadout::new(updater, cx))),
        }
    }

    /// The width the sidebar is set to now, which the divider reports and a drag or step starts from: the drag's, else
    /// the width held until the helm answers, else the stored one. The rail's expansion is not in it ([`Self::shown`]).
    fn width(&mut self, cx: &App) -> u32 {
        let current = self.current.current(cx);
        current
            .drag
            .map(|drag| drag.width)
            .or(current.held.map(|(_, width)| width))
            .unwrap_or_else(|| stored(cx))
    }

    /// The width the sidebar's column is drawn at now in `window`: [`Self::width`] widened for an expanded rail
    /// ([`shown_width`]).
    fn shown(&mut self, window: &Window, cx: &App) -> u32 {
        let expanded = self.session_list.read(cx).rail_expanded(cx);
        shown_width(
            self.width(cx),
            expanded,
            linux_title_bar::content_width(window),
        )
    }

    /// What a drag on the divider did.
    fn drag(&mut self, event: DividerDrag, cx: &mut Context<Self>) {
        match event {
            DividerDrag::Start(x) => {
                let width = self.width(cx);
                self.current.current(cx).drag = Some(Drag {
                    start_x: x,
                    start_width: width,
                    width,
                });
                cx.notify();
            }
            DividerDrag::Move(x) => {
                let current = self.current.current(cx);
                let Some(drag) = current.drag.as_mut() else {
                    return;
                };
                let width =
                    clamp(f64::from(drag.start_width) + f64::from(f32::from(x - drag.start_x)));
                if width != drag.width {
                    drag.width = width;
                    cx.notify();
                }
            }
            DividerDrag::End => {
                let Some(drag) = self.current.current(cx).drag.take() else {
                    return;
                };
                if drag.width != drag.start_width {
                    self.write(drag.width, cx);
                }
                cx.notify();
            }
        }
    }

    /// Left or Right on the focused divider: one step, clamped, written at once.
    fn step(&mut self, step: DividerStep, cx: &mut Context<Self>) {
        let width = self.width(cx);
        let next = match step {
            DividerStep::Back => width.saturating_sub(STEP),
            DividerStep::Forward => width + STEP,
        };
        self.write(clamp(f64::from(next)), cx);
    }

    /// A double-click on the divider: back to the default, written at once.
    fn reset(&mut self, cx: &mut Context<Self>) {
        self.current.current(cx).drag = None;
        self.write(default_width(), cx);
    }

    /// Send `width` as the helm's `sidebar_width` preference and hold it on screen until the helm replies (the store
    /// rule's one exception; see the module docs). The write is a keyless one ([`operations::send`]): nothing draws it
    /// busy and no refusal is kept for it. Any reply ends the hold if it is to the latest write, a refusal and a
    /// command the channel lost included, so the sidebar then shows what the helm stores; a failure is logged, never
    /// shown. A reset drops the reply, and the hold goes with the UI session's state. While the window is inert
    /// nothing is sent: the sidebar is not drawn then, so nothing calls this.
    fn write(&mut self, width: u32, cx: &mut Context<Self>) {
        let path = match ApiPath::api(&["preferences"]) {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(%error, "could not save the sidebar's width");
                return;
            }
        };
        let current = self.current.current(cx);
        current.sent += 1;
        let number = current.sent;
        current.held = Some((number, width));
        let sidebar = cx.weak_entity();
        operations::send(
            Method::Put,
            path,
            Some(serde_json::json!({ "sidebar_width": width })),
            "PUT /api/preferences",
            cx,
            move |answer, cx| {
                if let Err(reason) = answer {
                    tracing::warn!(?reason, "could not save the sidebar's width");
                }
                // A reply to anything but the latest write leaves the latest one held.
                let _ = sidebar.update(cx, |sidebar, cx| {
                    let current = sidebar.current.current(cx);
                    if current.held.is_some_and(|(held, _)| held == number) {
                        current.held = None;
                        cx.notify();
                    }
                });
            },
        );
        cx.notify();
    }

    /// The divider standing for `width`, wired to this sidebar: drawn as the strip by [`Sidebar::render`] and as the
    /// grip by [`Sidebar::grip`], each from its own copy built here, so the two cannot be configured apart.
    fn divider(&self, width: u32, cx: &mut Context<Self>) -> Divider {
        let (dragged, stepped, reset) = (cx.weak_entity(), cx.weak_entity(), cx.weak_entity());
        Divider::new(
            "sidebar-divider",
            "Sidebar width",
            HoverHelp::tooltip(DIVIDER_HELP),
            Role::Splitter,
            &self.divider_focus,
            width,
            SIDEBAR_WIDTH_RANGE,
        )
        .on_drag(move |event, _, cx| {
            let _ = dragged.update(cx, |sidebar, cx| sidebar.drag(event, cx));
        })
        .on_step(move |step, _, cx| {
            let _ = stepped.update(cx, |sidebar, cx| sidebar.step(step, cx));
        })
        .on_double_click(move |_, cx| {
            let _ = reset.update(cx, |sidebar, cx| sidebar.reset(cx));
        })
    }

    /// The divider's grip, for the main view to draw after the pane, in the row that holds the sidebar and the pane
    /// (which must be its positioned ancestor): over the strip's box, from the column's edge to 4 pixels into the pane.
    ///
    /// NOTE: Its position is worked out here, from the shown width ([`Self::shown`]), rather than from where the strip
    /// was drawn, so it stays over the strip only while two things hold: the sidebar starts at the row's left edge, and
    /// the window redraws whenever the shown width changes (it does: the main view and the sidebar are uncached views,
    /// redrawn on every frame, and a width change, an expansion's write and its push all notify).
    pub fn grip(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let width = self.width(cx);
        let shown = self.shown(window, cx);
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(shown as f32) - px(1.))
            .w(DIVIDER_WIDTH)
            .child(self.divider(width, cx).grip())
            .into_any_element()
    }
}

impl Sidebar {
    /// The app header, for the main view to draw across the window's top above both panes, when the macOS window is too
    /// narrow for the sidebar and the pane side by side; `None` otherwise, when the sidebar's own column draws it.
    ///
    /// SPEC.md, "Desktop window chrome": "In narrow macOS windows, the app-level row stays fixed above both scrolling
    /// panes so horizontal scrolling cannot move application controls underneath native window buttons." The panes
    /// scroll sideways once the window cannot fit both, so a header inside the sidebar's column would slide under the
    /// traffic lights. The cutoff is the old app's `sidebar-narrow-window` class (`window width <= sidebar width + 321`
    /// in `assets/sidebar-width.js` at reference commit `24bc0580`): the pane's floor and the sidebar's 1-pixel edge.
    pub fn floating_header(&mut self, window: &Window, cx: &App) -> Option<AnyElement> {
        let width = self.shown(window, cx);
        header_floats(width, window)
            .then(|| header::header(self.readout.clone(), window, cx).into_any_element())
    }
}

/// Whether the app header floats above both panes ([`Sidebar::floating_header`]) for a sidebar shown `width` wide.
fn header_floats(width: u32, window: &Window) -> bool {
    title_band::ON
        && linux_title_bar::content_width(window) <= px(width as f32) + MAIN_PANE_MIN_WIDTH + px(1.)
}

impl Render for Sidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("sidebar", cx);
        let palette = theme::palette(cx);
        let width = self.width(cx);
        let shown = self.shown(window, cx);
        let column = div()
            .id("sidebar")
            .w(px(shown as f32))
            .flex_none()
            .h_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(theme::color(palette.bg_1))
            .border_r_1()
            .border_color(theme::color(palette.border_sidebar_edge))
            .when(!header_floats(shown, window), |column| {
                column.child(header::header(self.readout.clone(), window, cx))
            })
            .child(div().flex_1().min_h_0().child(self.session_list.clone()));
        // The column inside a box of exactly its size with no border of its own, which records the column's outer edge
        // for help beside the sidebar (the module docs): positioned, so the edge's tracker covers it.
        let column = div()
            .relative()
            .flex()
            .flex_none()
            .h_full()
            .child(column)
            .child(layers::beside_edge(window, cx));
        // The column, then the divider's strip over the column's edge and reaching over the pane, its negative right
        // margin handing those pixels back so the pane starts at the column's edge.
        div()
            .flex()
            .flex_row()
            .flex_none()
            .h_full()
            .child(column)
            .child(
                div()
                    .flex_none()
                    .w(DIVIDER_WIDTH)
                    .h_full()
                    .ml(px(-1.))
                    .mr(-DIVIDER_REACH)
                    .child(self.divider(width, cx)),
            )
    }
}

/// The width the helm stores, or the default while it stores none. A stored width outside the range (which the helm
/// reads back as unset, so no helm sends one) is the default too.
fn stored(cx: &App) -> u32 {
    session::current_store(cx)
        .and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<Preferences>(PREFERENCES_KEY)
                .and_then(|preferences| preferences.sidebar_width)
        })
        .filter(|width| SIDEBAR_WIDTH_RANGE.contains(width))
        .unwrap_or_else(default_width)
}

/// `width` in whole pixels, rounded, within [`SIDEBAR_WIDTH_RANGE`].
fn clamp(width: f64) -> u32 {
    let (min, max) = (*SIDEBAR_WIDTH_RANGE.start(), *SIDEBAR_WIDTH_RANGE.end());
    width.round().clamp(f64::from(min), f64::from(max)) as u32
}
