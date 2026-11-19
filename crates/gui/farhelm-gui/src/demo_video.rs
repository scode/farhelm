//! Paint for the staged demo recording. The bridge sends a complete scene after measuring the current frame.
//! The capture harness owns placement and timing, including the offsets and wrapping it shares with docs callouts.
//! Nothing here receives input or participates in focus. The user's action still goes to the real control under it.

use std::time::Duration;

use gpui::private::schemars::JsonSchema;
use gpui::{
    Action, AnyElement, App, Bounds, Div, Global, InteractiveElement as _, IntoElement,
    ParentElement as _, PathBuilder, Pixels, Point, Styled as _, Window, canvas, div, fill, img,
    point, px, size,
};
use serde::Deserialize;

use crate::{
    assets,
    components::recording_transition,
    desktop::linux_title_bar,
    theme::{color, video},
};

// The recorder can ask for three seconds at its slowest rate. Cap other bridge requests to bound redraws.
const MAX_TRANSITION_MS: u32 = 3_000;

/// A complete scene replaces the prior one. Empty data removes the overlay between recordings.
// The action registry needs the schema version GPUI uses for its own actions.
#[derive(Action, Clone, Debug, Default, Deserialize, JsonSchema, PartialEq)]
#[action(namespace = farhelm_agent)]
#[serde(default, deny_unknown_fields)]
#[schemars(crate = "gpui::private::schemars")]
struct SetDemoScene {
    annotations: Vec<Annotation>,
    caption: Option<String>,
    pointer: Option<Pointer>,
    title: Option<Title>,
}

impl Global for SetDemoScene {}

/// The harness supplies content coordinates and prewrapped callout lines from the shared placement code.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(crate = "gpui::private::schemars")]
enum Annotation {
    Ring {
        rect: [f32; 4],
        spotlight: bool,
    },
    Callout {
        rect: [f32; 4],
        lines: Vec<String>,
        /// Start, control, end and the two wing tips of the arrow.
        arrow: [[f32; 2]; 5],
    },
}

/// Each move has a fresh key for the one-shot glide. Other scene updates keep that key to preserve the glide.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq)]
#[schemars(crate = "gpui::private::schemars")]
struct Pointer {
    from: [f32; 2],
    to: [f32; 2],
    key: usize,
    duration_ms: u32,
}

/// A fade in and a fade out each have their own key. The harness removes the title after the fade out.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq)]
#[schemars(crate = "gpui::private::schemars")]
struct Title {
    subtitle: String,
    key: usize,
    leaving: bool,
    duration_ms: u32,
}

/// Register the bridge action only for a staged agent run. Refresh the already-open window when a cue changes.
pub(crate) fn install(cx: &mut App) {
    cx.on_action(|scene: &SetDemoScene, cx| {
        // Each cue starts with motion enabled. The capture process runs on the harness's private desktop bus.
        cx.set_reduce_motion(false);
        cx.set_global(scene.clone());
        cx.refresh_windows();
    });
}

/// Hand the root's layer painter only the elements for the current recording cue. Unadorned divs and canvases
/// have no hit boxes, even where the title or spotlight paints across an underlying control. Kit already insets this
/// root inside the frame, so only the app's own title bar needs another offset.
pub(crate) fn overlay(window: &Window, cx: &App) -> Option<AnyElement> {
    let scene = cx.try_global::<SetDemoScene>()?;
    if scene == &SetDemoScene::default() {
        return None;
    }
    Some(
        div()
            .id("demo-video-overlay")
            .absolute()
            .left_0()
            .right_0()
            .top(linux_title_bar::bar_height(window))
            .bottom_0()
            .children(scene.annotations.iter().cloned().map(annotation))
            .children(scene.caption.clone().map(caption))
            .children(scene.pointer.clone().map(pointer))
            .children(scene.title.clone().map(title))
            .into_any_element(),
    )
}

/// Place a display-only element inside the captured content area.
fn placed([x, y, width, height]: [f32; 4]) -> Div {
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(width))
        .h(px(height))
}

/// Draw finished ring and arrow geometry. Only the harness chooses where a callout sits.
fn annotation(item: Annotation) -> AnyElement {
    match item {
        Annotation::Ring { rect, spotlight } => div()
            .absolute()
            .inset_0()
            .children(spotlight.then(|| {
                canvas(
                    |_, _, _| (),
                    move |area, (), window, _| shade(area, rect, window),
                )
                .size_full()
            }))
            .child(
                placed(rect)
                    .rounded(px(10.))
                    .border(px(4.))
                    .border_color(color(video::ACCENT)),
            )
            .into_any_element(),
        Annotation::Callout { rect, lines, arrow } => div()
            .absolute()
            .inset_0()
            .child(
                canvas(
                    |_, _, _| (),
                    move |area, (), window, _| draw_arrow(area.origin, arrow, window),
                )
                .size_full(),
            )
            .child(
                placed(rect)
                    .flex()
                    .flex_col()
                    .justify_center()
                    .px(px(18.))
                    .rounded(px(10.))
                    .border_2()
                    .border_color(color(video::ACCENT))
                    .bg(color(video::BOX))
                    .text_color(color(video::INK))
                    .font_family(assets::TERMINAL_FONT)
                    .text_size(px(20.))
                    .line_height(px(28.))
                    .children(lines.into_iter().map(|line| div().child(line))),
            )
            .into_any_element(),
    }
}

/// Dim the four rectangles around the spotlight. Clamp to the frame because a padded ring may touch its edge.
fn shade(area: Bounds<Pixels>, [x, y, width, height]: [f32; 4], window: &mut Window) {
    let left = px(x).clamp(px(0.), area.size.width);
    let right = px(x + width).clamp(left, area.size.width);
    let top = px(y).clamp(px(0.), area.size.height);
    let bottom = px(y + height).clamp(top, area.size.height);
    for (x, y, width, height) in [
        (px(0.), px(0.), area.size.width, top),
        (px(0.), bottom, area.size.width, area.size.height - bottom),
        (px(0.), top, left, bottom - top),
        (right, top, area.size.width - right, bottom - top),
    ] {
        window.paint_quad(fill(
            Bounds::new(area.origin + point(x, y), size(width, height)),
            color(video::SHADE).opacity(0.55),
        ));
    }
}

/// Paint a quadratic arrow and its open head using points prepared by the callout engine.
fn draw_arrow(origin: Point<Pixels>, points: [[f32; 2]; 5], window: &mut Window) {
    let [start, control, end, wing1, wing2] = points.map(|[x, y]| origin + point(px(x), px(y)));
    let mut path = PathBuilder::stroke(px(4.));
    path.move_to(start);
    path.curve_to(end, control);
    path.move_to(wing1);
    path.line_to(end);
    path.line_to(wing2);
    if let Ok(path) = path.build() {
        window.paint_path(path, color(video::ACCENT));
    }
}

/// A painted arrow pointer. The tip coincides with the bridge's click position in the finished glide.
fn pointer(pointer: Pointer) -> AnyElement {
    let graphic = canvas(
        |_, _, _| (),
        |area, (), window, _| {
            let points = [
                [0., 0.],
                [5., 26.],
                [10., 18.],
                [17., 26.],
                [21., 23.],
                [14., 15.],
                [25., 14.],
            ];
            let points = points.map(|[x, y]| area.origin + point(px(x), px(y)));
            let mut body = PathBuilder::fill();
            body.add_polygon(&points, true);
            if let Ok(path) = body.build() {
                window.paint_path(path, color(video::INK));
            }
            let mut outline = PathBuilder::stroke(px(1.5));
            outline.add_polygon(&points, true);
            if let Ok(path) = outline.build() {
                window.paint_path(path, color(video::BOX));
            }
        },
    )
    .size_full();
    recording_transition(
        ("demo-video-pointer", pointer.key),
        placed([pointer.from[0], pointer.from[1], 28., 28.]).child(graphic),
        Duration::from_millis(u64::from(pointer.duration_ms.clamp(1, MAX_TRANSITION_MS))),
        move |element, progress| {
            let [x, y] = [0, 1].map(|axis| {
                pointer.from[axis] + (pointer.to[axis] - pointer.from[axis]) * progress
            });
            element.left(px(x)).top(px(y))
        },
    )
}

/// The caption stays centered near the foot of the app without changing its layout.
fn caption(text: String) -> Div {
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(px(48.))
        .flex()
        .justify_center()
        .child(
            div()
                .px(px(26.))
                .py(px(14.))
                .rounded(px(12.))
                .border_2()
                .border_color(color(video::ACCENT))
                .bg(color(video::BOX))
                .text_color(color(video::INK))
                .font_family(assets::TERMINAL_FONT)
                .text_size(px(24.))
                .child(text),
        )
}

/// Use the same embedded wordmark as the app. A translucent card preserves continuity through the fade.
fn title(title: Title) -> AnyElement {
    let card = div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(24.))
        .bg(color(video::CARD).opacity(0.88))
        .text_color(color(video::SUBTITLE))
        .font_family(assets::TERMINAL_FONT)
        .text_size(px(26.))
        .child(
            img(assets::WORDMARK_DARK)
                .w(px(284. * 64. / 72.))
                .h(px(64.)),
        )
        .child(title.subtitle);
    recording_transition(
        ("demo-video-title", title.key),
        card,
        Duration::from_millis(u64::from(title.duration_ms.clamp(1, MAX_TRANSITION_MS))),
        move |element, progress| {
            element.opacity(if title.leaving {
                1. - progress
            } else {
                progress
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{DriverFlags, Role, TestAppContext};
    use serde_json::json;

    use super::*;
    use crate::{
        components::{Button, HoverHelp},
        test_support::{bounds, click, frame, open, settle},
    };

    /// The recorder click must reach the real control even when a callout paints across it. Drive the same
    /// JSON action as the bridge and establish that the callout painted and overlaps the button before pressing.
    #[gpui::test]
    fn a_recording_callout_draws_without_blocking_the_click(cx: &mut TestAppContext) {
        let clicks = Rc::new(Cell::new(0));
        let counter = clicks.clone();
        let cx = open(cx, move |_, _| {
            let counter = counter.clone();
            div()
                .pt(px(60.))
                .child(
                    Button::new(
                        "demo-target",
                        "Real target",
                        HoverHelp::tooltip("Press the real control"),
                        Role::Button,
                    )
                    .on_click(move |_, _, _| counter.set(counter.get() + 1)),
                )
                .into_any_element()
        });
        assert_eq!(clicks.get(), 0);
        let target = bounds(cx, "demo-target");
        let area = cx.update(|window, _| linux_title_bar::content_area(window));
        assert!(Bounds::new(area.origin, size(px(240.), px(120.))).contains(&target.center()));
        cx.update(|window, cx| {
            install(cx);
            let action = cx
                .build_action(
                    "farhelm_agent::SetDemoScene",
                    Some(json!({
                        "annotations": [{
                            "kind": "callout",
                            "rect": [0, 0, 240, 120],
                            "lines": ["Recording cue"],
                            "arrow": [[0, 10], [8, 17], [22, 18], [14, 12], [14, 25]]
                        }]
                    })),
                )
                .expect("the bridge builds the scene from JSON");
            window.dispatch_action(action, cx);
        });
        settle(cx);
        let nodes = frame(cx);
        assert!(
            nodes
                .iter()
                .flat_map(|node| &node.texts)
                .any(|line| line.text.as_ref() == "Recording cue")
        );
        let overlay = nodes
            .iter()
            .position(|node| node.element_id.as_deref() == Some("demo-video-overlay"))
            .expect("the bridge recorded the overlay");
        let panels = Bounds::new(area.origin, size(px(240.), px(120.)));
        let mut callout_overlaps = false;
        for (index, node) in nodes.iter().enumerate().skip(overlay) {
            let mut parents = std::iter::successors(Some(index), |index| nodes[*index].parent);
            if node.visible_bounds.contains(&target.center())
                && parents.any(|index| index == overlay)
            {
                callout_overlaps |= node.bounds == panels;
                assert_eq!(node.flags, DriverFlags::default());
            }
        }
        assert!(
            callout_overlaps,
            "the painted callout overlaps the real control"
        );
        click(cx, "demo-target");
        assert_eq!(clicks.get(), 1);
    }
}
