// Modified for Farhelm: changed or added by the patch layers in vendor/patches/gpui/ (vendor/README.md, "GPUI").
//! Check rectangle occlusion against a frame committed by GPUI.

use std::{cell::Cell, rc::Rc};

use gpui::{
    Bounds, ContentMask, Context, HitboxBehavior, HitboxId, IntoElement, Pixels, Render, Styled,
    TestAppContext, Window, canvas, point, px, size,
};

/// A later hitbox is clipped to the part its surface can actually cover.
#[derive(Clone, Copy)]
struct Cover {
    bounds: Bounds<Pixels>,
    mask: Option<Bounds<Pixels>>,
    behavior: HitboxBehavior,
}

/// A probe sits above its own card's blocking hitbox and below the candidate cover.
struct Scene {
    shown: Rc<Cell<Option<HitboxId>>>,
    cover: Option<Cover>,
}

impl Render for Scene {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let cover = self.cover;
        let shown = self.shown.clone();
        canvas(
            move |_, window, _| {
                let panel = rect(20., 20., 30., 30.);
                window.insert_hitbox(panel, HitboxBehavior::BlockMouse);
                let probe = window.insert_hitbox(panel, HitboxBehavior::Normal);
                if let Some(cover) = cover {
                    window.with_content_mask(
                        cover.mask.map(|bounds| ContentMask { bounds }),
                        |window| {
                            window.insert_hitbox(cover.bounds, cover.behavior);
                        },
                    );
                }
                probe.id
            },
            move |_, id, _, _| shown.set(Some(id)),
        )
        .size_full()
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// A probe covers the whole disclosure panel. Sampling the pointer position would miss a
/// clipped sliver at the edge, while counting every overlapping rectangle would include the
/// panel's own hitbox and pass-through surfaces. A retired probe must not describe a new frame.
#[gpui::test]
fn only_later_mouse_blocking_hitboxes_cover_the_visible_rectangle(cx: &mut TestAppContext) {
    use HitboxBehavior::{BlockMouse, BlockMouseExceptScroll, Normal};
    let shown = Rc::new(Cell::new(None));
    let (view, cx) = cx.add_window_view(|_, _| Scene {
        shown: shown.clone(),
        cover: None,
    });
    cx.run_until_parked();
    let mut previous = shown.get().expect("the probe must have painted");
    assert_eq!(
        cx.update(|window, _| previous.is_occluded(window)),
        Some(false)
    );

    let overlap = rect(49., 25., 20., 10.);
    for (bounds, mask, behavior, covered) in [
        (overlap, None, BlockMouse, true),
        (overlap, None, BlockMouseExceptScroll, true),
        (overlap, None, Normal, false),
        (rect(50., 25., 20., 10.), None, BlockMouse, false),
        (overlap, Some(rect(50., 25., 19., 10.)), BlockMouse, false),
        (overlap, Some(rect(49., 25., 1., 10.)), BlockMouse, true),
    ] {
        view.update(cx, |scene, cx| {
            scene.cover = Some(Cover {
                bounds,
                mask,
                behavior,
            });
            cx.notify();
        });
        cx.run_until_parked();
        let probe = shown.get().expect("the panel painted in the new frame");
        assert_ne!(probe, previous, "the frame must have committed a new probe");
        assert_eq!(cx.update(|window, _| previous.is_occluded(window)), None);
        assert_eq!(
            cx.update(|window, _| probe.is_occluded(window)),
            Some(covered),
            "{behavior:?}, bounds {bounds:?}, mask {mask:?}"
        );
        previous = probe;
    }
}
