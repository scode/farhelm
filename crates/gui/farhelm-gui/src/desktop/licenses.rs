//! The Open-Source Notices window: one plain window of scrolling read-only text holding every notice that must travel
//! with the app (the code it ported from other projects, its dependencies' licenses, the fonts it embeds), which the
//! Mac app menu's "Open-Source Notices" item opens. The maintainer decided on 2026-11-13 that the notices live in a
//! plain scrolling text window and nothing else, and the same day that text is copyable only where the old app had a
//! copy button, so the window has no controls and its text is not selectable. Until it existed, `farhelm-desktop
//! --licenses` was the only surface; that flag stays.
//!
//! There is at most one. Choosing the item again brings the open one forward instead of opening a second, so the window
//! is kept by its handle; once it has closed, the next choice opens a new one. Closing it never quits the app: only the
//! main window's close does (`desktop::quit_when_closed`). It is the app's only second window, and like every root
//! view it carries its own layer stack and focus state (`root::attach`), so the main window's dialogs and focus never
//! see it.
//!
//! The text is close to a megabyte, about 16,600 lines, so it is drawn as a virtualized list of lines (`uniform_list`),
//! which draws only the rows in view, rather than as one text element GPUI would lay out whole. A `uniform_list` row
//! has one fixed height and cannot wrap, so the lines are wrapped once, when the window opens, at [`COLUMNS`]
//! characters (a few hundred dependency notices have unwrapped paragraphs, the longest some 1,500 characters), and the
//! window is never narrower than that column. The text comes from the host's function each time a window opens and goes
//! with the window when it closes, so the app does not hold it for its whole life.
//!
//! The menu item dispatches `actions::OpenSourceNotices`, which the menu module handles app-wide by calling [`open`].
//! Linux has no menu bar: there the window is meant to open from an item in the in-window `?` menu, which the
//! other-dialogs area draws and which will dispatch the same action; until it exists, `--licenses` is the Linux
//! surface.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Div, Global, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, Render, Role, SharedString, Size,
    StatefulInteractiveElement as _, Styled as _, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions, div, px, size, uniform_list,
};
use gpui_component::Root;

use super::{linux_title_bar, title_band};
use crate::theme::{self, metrics};
use crate::{assets, bootstrap, render_count, root};

/// The window's title, which macOS also lists under the app menu's open windows.
pub(crate) const TITLE: &str = "Open-Source Notices";

/// The column the notices are wrapped at: wide enough that the notices files' own 80-column wrapping is never broken
/// again, narrow enough that the window fits a small laptop screen.
pub(crate) const COLUMNS: usize = 100;

/// A tab stop, in columns. A few dependency notices indent with tabs, and the list's text draws no tab stops of its
/// own.
const TAB: usize = 8;

/// One line's height: the 12-pixel text with the same spacing the session list's small text uses.
const LINE_HEIGHT: Pixels = px(17.);

/// The space around the text.
const PAD: Pixels = px(16.);

/// The width of one character of the terminal font at the 12-pixel text size (JetBrains Mono's advance is 0.6 em),
/// which sets the narrowest the window may get: [`COLUMNS`] of them and the padding.
pub(crate) const CHAR_WIDTH: f32 = 7.2;

/// The function the host gave for the notices' text (`Host::licenses`), kept by [`keep`].
struct Licenses(fn() -> String);

impl Global for Licenses {}

/// The open notices window, while there is one, so a second choice brings it forward. Its handle stays here after it
/// closes; a closed window is noticed when its update fails, which is what a handle to a gone window answers.
struct Open(WindowHandle<Root>);

impl Global for Open {}

/// Keep the host's source of the notices for the windows [`open`] makes. Called once by the bootstrap, before the main
/// window opens.
pub(crate) fn keep(licenses: fn() -> String, cx: &mut App) {
    cx.set_global(Licenses(licenses));
}

/// Bring the open notices window forward, or open one when there is none. Does nothing before the host's source of
/// the text was kept, which the bootstrap does before any window exists.
pub(crate) fn open(cx: &mut App) {
    if let Some(&Open(handle)) = cx.try_global::<Open>()
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let Some(&Licenses(licenses)) = cx.try_global::<Licenses>() else {
        tracing::error!("the open-source notices were asked for before the host gave their source");
        return;
    };
    let lines: Rc<[SharedString]> = wrap(&licenses()).into_iter().map(Into::into).collect();
    match cx.open_window(options(cx), move |window, cx| {
        let view = cx.new(|cx| NoticesView::new(lines, window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    }) {
        Ok(handle) => cx.set_global(Open(handle)),
        Err(error) => tracing::error!("the open-source notices window could not open: {error:#}"),
    }
}

/// The notices window's handle while one was opened, for tests that drive it.
#[cfg(test)]
pub(crate) fn window(cx: &App) -> Option<gpui::AnyWindowHandle> {
    cx.try_global::<Open>().map(|Open(handle)| (*handle).into())
}

/// The narrowest the window may get: the wrapped text and its padding across, and a few lines down.
fn min_size() -> Size<Pixels> {
    size(px(COLUMNS as f32 * CHAR_WIDTH) + PAD * 2., px(240.))
}

/// The width the window opens at, before any Linux frame allowance: the full column with a little room to spare. The
/// screenshot goldens draw the window's content at this width.
pub(crate) fn opening_width() -> Pixels {
    min_size().width + px(24.)
}

/// An ordinary window with the platform's own title bar (the title band is the main window's alone), centered on the
/// primary display at a size that shows the full column, and never higher than that display.
pub(crate) fn options(cx: &App) -> WindowOptions {
    let mut min = min_size();
    let mut wanted = size(opening_width(), px(640.));
    // On Linux GPUI Kit's frame and, where the compositor draws none, the window's own title bar take room inside the
    // window, so both sizes allow for them and the text keeps its column (`linux_title_bar`).
    if cfg!(target_os = "linux") {
        min = min + linux_title_bar::minimum_allowance();
        wanted = wanted + linux_title_bar::opening_allowance();
    }
    if let Some(display) = cx.primary_display() {
        wanted.height = wanted.height.min(display.bounds().size.height);
    }
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, wanted, cx))),
        window_min_size: Some(min),
        app_id: Some(bootstrap::APP_ID.into()),
        titlebar: Some(TitlebarOptions {
            title: Some(TITLE.into()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// The notices' text as the lines the window draws: tabs expanded to stops every [`TAB`] columns, then every line
/// longer than [`COLUMNS`] characters cut into lines no longer than that, after the last space that fits, or at the
/// column itself where a run of text has no space. Every character survives in order: joining the lines of one cut line
/// gives it back (the space a cut follows stays at the end of the first part, where it draws nothing; a run that fills
/// the column exactly leaves the space after it at the start of the next part, one column in).
pub(crate) fn wrap(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.split('\n') {
        let mut rest: Vec<char> = expand_tabs(line).chars().collect();
        while rest.len() > COLUMNS {
            // The last space within the column ends the first part, so no part is longer than the column.
            let cut = rest[..COLUMNS]
                .iter()
                .rposition(|c| *c == ' ')
                .filter(|space| *space > 0)
                .map_or(COLUMNS, |space| space + 1);
            lines.push(rest.drain(..cut).collect());
        }
        lines.push(rest.into_iter().collect());
    }
    lines
}

/// `line` with each tab replaced by the spaces that reach the next tab stop.
fn expand_tabs(line: &str) -> String {
    if !line.contains('\t') {
        return line.to_string();
    }
    let mut out = String::new();
    for c in line.chars() {
        if c == '\t' {
            let to_stop = TAB - out.chars().count() % TAB;
            out.extend(std::iter::repeat_n(' ', to_stop));
        } else {
            out.push(c);
        }
    }
    out
}

/// The notices window's root view: the wrapped lines and the window's wiring, nothing else (no controls: the plain
/// window the maintainer decided on). It draws in the app's theme like every window; the main window is the one that
/// follows the system's appearance changes.
struct NoticesView {
    lines: Rc<[SharedString]>,
    root: root::Attachment,
    /// Redraws the window when the compositor's answer about decorations changes (`linux_title_bar`).
    _decorations: gpui::Subscription,
}

impl NoticesView {
    fn new(lines: Rc<[SharedString]>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        NoticesView {
            lines,
            root: root::attach(window, cx),
            _decorations: linux_title_bar::follow_decorations(window),
        }
    }
}

impl Render for NoticesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("open-source-notices", cx);
        let bar = linux_title_bar::bar("notices-title-bar", TITLE, "Close this window", window, cx);
        let ground = content(&self.lines, bar, cx)
            // The bar's drag starts from this window's own press watcher, as the main window's band does.
            .child(title_band::watch_presses());
        root::wrap(ground, &self.root, window, cx)
    }
}

/// What the notices window draws, filling its parent: `bar` (the Linux window's own title bar, where it draws one) over
/// `lines` as the scrolling list. Apart from the view so that the screenshot goldens can draw the window's look without
/// opening a second window or attaching a second root to theirs; the window's wiring (its root attachment and its press
/// watcher) stays in [`NoticesView`].
pub(crate) fn content(lines: &Rc<[SharedString]>, bar: Option<AnyElement>, cx: &App) -> Div {
    let palette = theme::palette(cx);
    let lines = Rc::clone(lines);
    let list = uniform_list(
        "open-source-notices-lines",
        lines.len(),
        move |range, _, _| {
            range
                .map(|index| {
                    div()
                        .h(LINE_HEIGHT)
                        .line_height(LINE_HEIGHT)
                        .whitespace_nowrap()
                        .child(lines[index].clone())
                })
                .collect()
        },
    )
    .size_full()
    .px(PAD)
    .py(PAD);
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme::color(palette.bg_0))
        .text_color(theme::color(palette.fg_0))
        .font_family(assets::TERMINAL_FONT)
        .text_size(metrics::TEXT_SM)
        // The Linux window's own title bar, where the compositor draws none (`linux_title_bar`), in the interface face
        // like every bar.
        .children(bar.map(|bar| div().font_family(assets::UI_FONT).child(bar)))
        .child(
            div()
                .id("open-source-notices")
                .role(Role::Region)
                .aria_label(TITLE)
                .flex_1()
                .min_h_0()
                .w_full()
                .child(list),
        )
}
