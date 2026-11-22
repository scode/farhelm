//! Golden scenes for the desktop integration: the Linux title bar, the Open-Source Notices window and the version
//! readout. The scene list itself, `SCENES` with each scene's name and size, stays in `mod.rs`.

use crate::desktop::updates;
use crate::focus::{self, Mark};
use crate::layers;
use crate::theme;
use crate::ticker;
use crate::update_readout::view::VersionReadout;
use crate::update_readout::{self, Activity, Readout};
use gpui::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement, Styled, Window, div, px,
};
use std::rc::Rc;

use super::{SceneState, press_tab, rest_pointer_on_control};

/// The width the Linux title bar scene draws its bars at: the window width of the screenshot batch's narrow app variant,
/// a typical small window.
const LINUX_TITLE_BAR_WIDTH: f32 = 900.;

/// The Linux window's own title bar, the one it draws where the compositor draws none (`desktop::linux_title_bar`), as
/// the main window has it: above, the window in its ordinary state, with Maximize; below, maximized, with Restore in
/// its place. Drawn through the bar's drawing function rather than its window predicate, since GPUI's test platform
/// reports server-side decorations and so would never show the bar; the predicate has its own unit tests.
pub(super) fn linux_title_bars(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let bar = |id, maximized| {
        crate::desktop::linux_title_bar::draw(
            id,
            crate::bootstrap::WINDOW_TITLE,
            "Close Farhelm",
            maximized,
            cx,
        )
    };
    div()
        .w(px(LINUX_TITLE_BAR_WIDTH))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(bar("title-bar-ordinary", false))
        .child(bar("title-bar-maximized", true))
        .into_any_element()
}

/// The notices scene's text: a short stand-in for `licenses_text()` with its sections' shape (a ported-code preamble,
/// then each section opened by the 80-column `=` rule `scripts/dependency-notices.py` writes, a title line and, for a
/// dependency, the line naming what uses it), including an unwrapped paragraph longer than the window's wrap column, so
/// the picture shows a cut line continuing on the next, and a tab-indented line. The rule is the window's most repeated
/// line, so the picture pins how the terminal face draws it.
const NOTICES_FIXTURE: &str = "\
Farhelm includes code ported from other projects, under the notices below.

================================================================================
example-terminal 1.2.3 (Apache-2.0)
Used by: farhelm-gui

\tCopyright 2020 The Example Terminal Authors.

Licensed under the Apache License, Version 2.0 (the \"License\"); you may not use this file except in compliance with the License, a copy of which is printed with the dependency notices.
================================================================================
Example Sans (SIL Open Font License 1.1)

Copyright 2021 The Example Sans Project Authors.

This Font Software is licensed under the SIL Open Font License, Version 1.1.
No warranty is given; see the license for details.";

/// The height the notices scene crops the window to: the real window's text area is 640 pixels high, mostly empty
/// below a fixture this short, so the scene keeps the bar and the fixture's lines with some room under them.
const NOTICES_HEIGHT: f32 = 340.;

/// The Open-Source Notices window (`desktop::licenses`) over [`NOTICES_FIXTURE`], at the window's real opening width
/// (`licenses::opening_width`, which on Linux is what is left once GPUI Kit's frame takes its allowance back) and
/// cropped in height: the text in the terminal face, wrapped at the window's column. On Linux it has the window's own
/// title bar above, as under a compositor that draws none, in the ordinary state the window opens in; on macOS the
/// system draws the title bar, so the scene has none.
pub(super) fn notices_window(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    use crate::desktop::{licenses, linux_title_bar};
    let lines: Rc<[gpui::SharedString]> = licenses::wrap(NOTICES_FIXTURE)
        .into_iter()
        .map(Into::into)
        .collect();
    let bar = cfg!(target_os = "linux").then(|| {
        linux_title_bar::draw(
            "notices-title-bar",
            licenses::TITLE,
            "Close this window",
            false,
            cx,
        )
    });
    div()
        .w(licenses::opening_width())
        .h(px(NOTICES_HEIGHT))
        .child(licenses::content(&lines, bar, cx))
        .into_any_element()
}

// ---- The version readout --------------------------------------------------------------------------------------------
//
// The sidebar header's version while the desktop updater runs (`update_readout::view`), one scene per group of states.
// No reference screenshot of these states exists (the old web build ran no updater), so each picture was checked
// against the old stylesheet's values (`crates/farhelm-ui/assets/app.css` at the reference commit) and the view's own
// docs. The version text is this build's own (`0.0.0-unreleased` on main), so a version bump redraws every one of
// these scenes.

/// One row of a version readout scene: the readout its fixture updater shows, and the step the row holds whatever spins
/// at (`VersionReadout::holding_step`), if it holds one.
type ReadoutRow = (Readout, Option<u8>);

/// The readouts a version readout scene made, one per row in the scene's order, kept among the scene's kept values.
struct Readouts(Vec<Entity<VersionReadout>>);

/// A readout in `activity`, with an update waiting when `ready`. Only the tooltip scene draws hover words, and it
/// builds its own, so these just name the scene.
fn golden_readout(activity: Activity, ready: bool) -> Readout {
    Readout {
        update_ready: ready,
        needs_reinstall: false,
        withdrawn: false,
        tooltip: "Golden scene readout".to_string(),
        activity,
        notice_id: 1,
        user_update_ready: false,
    }
}

/// The readouts a version readout scene has made so far, in row order; empty before its first frame.
fn kept_readouts(state: &SceneState) -> Vec<Entity<VersionReadout>> {
    state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Readouts>())
        .map(|Readouts(readouts)| readouts.clone())
        .unwrap_or_default()
}

/// The rows `rows` describes, each a readout over a fixture updater of its own (`updates::fixture`: the app keeps one
/// updater, a scene shows several), in a row with the app header's look: the sidebar's width, 40 pixels high, 12
/// pixels of padding across, `--bg-1` with the row hairline under it, and the readout at the right end, as the header
/// draws it.
///
/// Unlike other scenes' entities, the readouts are made while drawing, on the scene's first frame, so that the frame
/// drawn before the setup already shows them: a setup that gives a readout keyboard focus, opens its menu or shows its
/// hover help needs it drawn, and a step deferred to a later frame could be missed by [`settle`], which stops at the
/// first two identical frames. Motion is reduced before any readout exists, so no spinner ever ticks and the capture
/// settles; a row that shows a turn holds its step instead.
fn readout_rows(state: &SceneState, rows: fn() -> Vec<ReadoutRow>, cx: &mut App) -> AnyElement {
    let mut readouts = kept_readouts(state);
    if readouts.is_empty() {
        ticker::set_reduced_motion(true, cx);
        readouts = rows()
            .into_iter()
            .map(|(readout, step)| {
                let updater = updates::fixture(readout, cx);
                cx.new(|cx| VersionReadout::new(updater, cx).holding_step(step))
            })
            .collect();
        state
            .keep
            .borrow_mut()
            .push(Box::new(Readouts(readouts.clone())));
    }
    let palette = theme::palette(cx);
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .children(readouts.into_iter().map(|readout| {
            div()
                .w(theme::metrics::SIDEBAR_WIDTH)
                .h(px(40.))
                .px(px(12.))
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .bg(theme::color(palette.bg_1))
                .border_b_1()
                .border_color(theme::color(palette.border_row))
                // The main view's ink, which the header inherits there.
                .text_color(theme::color(palette.fg_0))
                .child(div().min_w_0().child(readout))
        }))
        .into_any_element()
}

/// Give the readout in row `row` keyboard focus as a keyboard user would have it, then press Tab `tabs` more times.
/// The first Tab makes the keyboard the window's last input, which a focus ring needs ([`focus::shows_ring`]), and the
/// coordinator's move puts focus on the row's check button or toggle (their shared handle) without walking every row
/// before it; the further Tabs walk on from there, to a control the readout draws without a handle of its own (the
/// Check failed button).
fn focus_readout(state: &SceneState, row: usize, tabs: usize, window: &mut Window, cx: &mut App) {
    window.activate_window();
    press_tab(window, cx);
    let readout = kept_readouts(state)
        .get(row)
        .cloned()
        .expect("the scene's first frame made its readouts");
    let handle = readout.read(cx).shared_focus();
    focus::focus_control(&handle, &Mark::take(cx), window, cx);
    for _ in 0..tabs {
        press_tab(window, cx);
    }
}

/// Rows of the check button with no update waiting: at rest (the version in `--fg-1`, the `↻` in `--fg-2`); lit, as
/// under the pointer (both in `--fg-0`, the glyph a quarter turned); with a reinstall warning (`⚠` and the version in
/// `--warn`); the warning lit (the version in `--fg-0`, as the old sheet's later hover rule won, the `⚠` still
/// `--warn`); and the button with keyboard focus (lit, with the accent ring and no fill), which the setup gives it. The
/// setup also rests the pointer on the two lit rows ([`LIT_CHECK_ROWS`]).
fn version_readout_check_rows() -> Vec<ReadoutRow> {
    let warned = Readout {
        needs_reinstall: true,
        ..golden_readout(Activity::Idle, false)
    };
    vec![
        (golden_readout(Activity::Idle, false), None),
        (golden_readout(Activity::Idle, false), None),
        (warned.clone(), None),
        (warned, None),
        (golden_readout(Activity::Idle, false), None),
    ]
}

/// The rows of [`version_readout_check_rows`] drawn lit, as under the pointer: the plain button and the warned one.
const LIT_CHECK_ROWS: [usize; 2] = [1, 3];

/// [`version_readout_check_rows`], drawn.
pub(super) fn version_readout_check(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    readout_rows(state, version_readout_check_rows, cx)
}

/// The pointer resting on each of [`LIT_CHECK_ROWS`], given through the hover state each readout reads, since a capture
/// has no pointer and one pointer could not rest on two rows; then keyboard focus on the last row's check button.
pub(super) fn version_readout_check_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let readouts = kept_readouts(state);
    for row in LIT_CHECK_ROWS {
        let readout = readouts
            .get(row)
            .expect("the scene's first frame made its readouts");
        readout.update(cx, |readout, cx| readout.rest_pointer(cx));
    }
    focus_readout(state, 4, 0, window, cx);
}

/// The step the progress rows hold: three eighths of a turn, clearly turned and unlike the lit quarter turn.
const HELD_STEP: u8 = 3;

/// Rows of work in progress: a check with the glyph spinning (held at [`HELD_STEP`], in `--fg-0`) and "Checking…"
/// beside it; an install, the same with "Installing…"; and both again while an update waits, where the red toggle with
/// `↑` has no glyph to spin, so the accent ring (at the same step) comes before the words.
fn version_readout_progress_rows() -> Vec<ReadoutRow> {
    let turned = Some(HELD_STEP);
    let installing = || Activity::Installing("0.40.0".to_string());
    vec![
        (golden_readout(Activity::Checking, false), turned),
        (golden_readout(installing(), false), turned),
        (golden_readout(Activity::Checking, true), turned),
        (golden_readout(installing(), true), turned),
    ]
}

/// [`version_readout_progress_rows`], drawn.
pub(super) fn version_readout_progress(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    readout_rows(state, version_readout_progress_rows, cx)
}

/// Rows of a check's outcome: Up to date in `--fg-1` beside the check button; Check failed, the failure button in
/// `--warn`; Check failed beside the red toggle while an update waits; and the failure button with keyboard focus
/// (the accent ring, no fill), which the setup gives it.
fn version_readout_outcomes_rows() -> Vec<ReadoutRow> {
    let failed = || Activity::Failed("the network is unreachable".to_string());
    vec![
        (
            golden_readout(Activity::UpToDate("0.39.0".to_string()), false),
            None,
        ),
        (golden_readout(failed(), false), None),
        (golden_readout(failed(), true), None),
        (golden_readout(failed(), false), None),
    ]
}

/// [`version_readout_outcomes_rows`], drawn.
pub(super) fn version_readout_outcomes(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    readout_rows(state, version_readout_outcomes_rows, cx)
}

/// Keyboard focus on the last row's Check failed button: the row's check button first, then one Tab on to the button
/// beside it.
pub(super) fn version_readout_outcomes_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    focus_readout(state, 3, 1, window, cx);
}

/// Rows of the update-menu toggle once an update is installed: red with `↑` before the version; with a withdrawal
/// (`⚠` in `--warn`, then `↑`, the rest red); and the toggle with keyboard focus (the accent ring and the control-hover
/// fill, the one readout control a focused fill is ported for), which the setup gives it.
fn version_readout_update_ready_rows() -> Vec<ReadoutRow> {
    let withdrawn = Readout {
        withdrawn: true,
        ..golden_readout(Activity::Idle, true)
    };
    vec![
        (golden_readout(Activity::Idle, true), None),
        (withdrawn, None),
        (golden_readout(Activity::Idle, true), None),
    ]
}

/// [`version_readout_update_ready_rows`], drawn.
pub(super) fn version_readout_update_ready(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    readout_rows(state, version_readout_update_ready_rows, cx)
}

/// Keyboard focus on the last row's toggle.
pub(super) fn version_readout_update_ready_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    focus_readout(state, 2, 0, window, cx);
}

/// One row, the red toggle, at the scene's top left, so the menu has room beside it.
pub(super) fn version_readout_menu(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    top_left(readout_rows(
        state,
        || vec![(golden_readout(Activity::Idle, true), None)],
        cx,
    ))
}

/// The toggle pressed: its menu open beside it (10 pixels past its right edge, 4 above its top), headed "Farhelm update"
/// and "An installed update is ready", with Restart to update (focus on it, as a menu opens) and What's new. The toggle
/// keeps the readout's look with the menu open: no fill (the old toggle drew nothing for its expanded state).
pub(super) fn version_readout_menu_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let readout = kept_readouts(state)
        .first()
        .cloned()
        .expect("the scene's first frame made its readout");
    readout.update(cx, |readout, cx| readout.press_toggle(window, cx));
    assert!(
        layers::any_open(window, cx),
        "the toggle's press opened the update menu (it needs the toggle drawn)"
    );
}

/// `content` at the scene's top left, leaving the rest of the scene free for a menu or hover help beside it.
fn top_left(content: AnyElement) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_start()
        .child(content)
        .into_any_element()
}

/// One row, the check button with the reinstall warning, at the scene's top left, with room under it and to its right
/// for its hover, so the card is not pushed back by the window's edge (in the app it reaches over the main view).
/// The upgrade-floor refusal exercises the longest reinstall guidance, including the command.
pub(super) fn version_readout_tooltip(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    top_left(readout_rows(
        state,
        || {
            let readout = Readout {
                needs_reinstall: true,
                tooltip: update_readout::upgrade_floor_reinstall_tooltip(
                    &update_readout::idle_tooltip(farhelm_proto::BUILD_VERSION, true),
                ),
                ..golden_readout(Activity::Idle, false)
            };
            vec![(readout, None)]
        },
        cx,
    ))
}

/// The check button's hover help for the reinstall readout, with the pointer come to rest over its version: the
/// paragraph help, up to 440 pixels wide with no line limit, so the reinstall command at the end of the words shows
/// (the old app's short tooltip cut it at eight lines; the maintainer chose the wider help on 2026-11-16), placed as
/// the old app placed the version's help at the window's top edge, centered below the button with the 28-pixel gap
/// that clears the pointer.
pub(super) fn version_readout_tooltip_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    rest_pointer_on_control(state, "app-version-check", window, cx);
}
