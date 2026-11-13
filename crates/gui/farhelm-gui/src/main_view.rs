//! The main window's root view: the ground the sidebar and the session pane will be drawn on, and the overlay layer.
//!
//! In this first phase it draws only the window's ground and whatever the layer manager has to show, so the bootstrap
//! has a window to open and the test harness a root to drive. The views of the end-to-end slice (the session list, its
//! row menu, the terminal) are added here as they are built.

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Subscription, Window, div};

use crate::{layers, render_count, theme};

/// The window's root view.
pub struct MainView {
    /// Keeps the theme following the operating system's appearance for this window's lifetime.
    _appearance: Subscription,
    /// Redraws this view, and so the overlay it draws, when the layer manager's content changes.
    _layers: Option<Subscription>,
}

impl MainView {
    /// The main view, following the operating system's appearance and the layer manager while it lives.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        MainView {
            _appearance: theme::observe_window(window),
            _layers: layers::observe(cx),
        }
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("main", cx);
        let palette = theme::palette(cx);
        div()
            .size_full()
            .bg(theme::color(palette.bg_0))
            .text_color(theme::color(palette.fg_0))
            .font_family(crate::assets::UI_FONT)
            .children(layers::overlay(window, cx))
    }
}
