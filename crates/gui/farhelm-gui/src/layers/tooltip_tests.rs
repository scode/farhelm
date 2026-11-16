//! Behavior tests for the layer manager's tooltip ([`super::tooltip`]): how a tooltip lays out help in paragraphs,
//! with the real text system, in a headless app rather than on GPUI's simulated platform. The tooltip's hover rules
//! (the delay, the innermost help, a press putting it away) are tested through the controls that show it, in the
//! component layer's hover help tests.

use gpui::{AppContext as _, Context, Entity, IntoElement, Render, Styled, Window, div, point, px};

use crate::layers;

/// Nothing but the layer manager's overlay over an empty window, wired as every root view is, so a tooltip shown for
/// `_owner` is the only thing the frame draws.
struct TooltipOnly {
    root: crate::root::Attachment,
    /// The control the tooltip belongs to. It is kept alive here, since the layer manager takes a tooltip down when
    /// its owner is released.
    _owner: Entity<layers::TooltipOwner>,
}

impl Render for TooltipOnly {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = div().size_full().font_family(crate::assets::UI_FONT);
        crate::root::wrap(content, &self.root, window, cx)
    }
}

/// The driver record of a frame showing `help` as a tooltip near the top-left corner of a 1000 by 800 window, laid out
/// with the real text system and the bundled fonts in a headless app (no renderer, which layout does not need). Line
/// counts and widths are the point of the tests below, and GPUI's test text system gives every character the same
/// advance (SPEC_impl.md, "GPUI cookbook: traps"), so it would wrap these words at made-up places.
fn tooltip_frame(help: layers::Help) -> Vec<gpui::DriverNode> {
    use std::sync::Arc;

    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        crate::assets::UI_FONT,
    ));
    let mut cx =
        gpui::HeadlessAppContext::with_asset_source(text_system, Arc::new(crate::assets::Assets));
    cx.update(crate::bootstrap::install).unwrap();
    let owner = cx.update(|cx| cx.new(|_| layers::TooltipOwner));
    let kept = owner.clone();
    let window = cx
        .open_window(gpui::size(px(1000.), px(800.)), move |window, cx| {
            let view = cx.new(|cx| TooltipOnly {
                root: crate::root::attach(window, cx),
                _owner: kept,
            });
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        })
        .unwrap();
    cx.update_window(window.into(), |_, window, cx| {
        layers::show_tooltip_at(&owner, help.into(), point(px(20.), px(20.)), window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window.into(), |_, window, cx| {
        window.set_driver_enabled(true);
        window.draw(cx).clear(cx);
        window.driver_nodes().to_vec()
    })
    .unwrap()
}

/// The one line of text in `nodes` that starts with `start`: a tooltip's help as painted, which a wrapped text element
/// records as a single entry whose bounds hold all its lines. A clamped one holds only the lines it kept: GPUI's clamp
/// stops wrapping at the last kept line and lets the rest of the text run on past the card's edge, where the card
/// clips it, so the entry's text is the whole help either way and only its height tells a cut one apart.
fn painted_text(nodes: &[gpui::DriverNode], start: &str) -> gpui::DriverText {
    let found: Vec<_> = nodes
        .iter()
        .flat_map(|node| node.texts.iter())
        .filter(|text| text.text.starts_with(start))
        .cloned()
        .collect();
    match found.as_slice() {
        [text] => text.clone(),
        _ => panic!("expected one painted text starting {start:?}, found {found:?}"),
    }
}

/// The bounds of the node drawn under `id` in `nodes`; fails if nothing was.
fn node_bounds(nodes: &[gpui::DriverNode], id: &str) -> gpui::Bounds<gpui::Pixels> {
    match nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
    {
        Some(node) => node.bounds,
        None => panic!("{id} was not drawn"),
    }
}

/// Help long enough to run well past eight lines inside either tooltip cap: the shape of a guarded launch's permission
/// help, whose caveat comes last.
fn long_help() -> String {
    "Your own allow rules, hooks, plugins, always-allow answers, sandbox settings or switching modes inside the \
     session can loosen it. "
        .repeat(8)
}

/// Why this matters: SPEC.md's "Session list" gives the agent mark and the permission shield one hover text (the agent,
/// what its launch's permission means, the stored command), and the permission's meaning ends in the caveat that the
/// agent's own rules can loosen it. In the short tooltip, 280 pixels wide and cut at eight lines, a guarded Codex
/// launch's help stopped inside that caveat. The old app drew such help as paragraphs on a wider card with no line
/// clamp, and this pins that port.
///
/// Specification: laid out with the real text system, short help that runs past eight lines is painted within the
/// 280-pixel card (less its padding and border) and cut at eight lines. The same words as the middle paragraph of
/// paragraph help are painted wider than the short card allows and within the 440-pixel one, in more than eight lines
/// (which is also the premise that the short help was long enough to be cut: it wraps narrower, so to more lines); and
/// every paragraph is drawn in order, one tooltip line height (a blank line) below the one before it.
#[test]
fn paragraph_help_is_wider_and_unclamped_with_a_blank_line_between_paragraphs() {
    use crate::host_text::{HostSentence, HostText};
    use crate::theme::metrics;

    // The card's padding and its one-pixel border on each side.
    let inset = 2. * f32::from(metrics::TOOLTIP_PAD.1) + 2.;
    let line = f32::from(metrics::TOOLTIP_LINE_HEIGHT);
    let eight_lines = metrics::TOOLTIP_MAX_LINES as f32 * line;

    let short = tooltip_frame(layers::Help::Line(long_help().into()));
    let short = painted_text(&short, "Your own allow rules");
    assert!(
        f32::from(short.bounds.size.width) <= f32::from(metrics::TOOLTIP_MAX_WIDTH) - inset + 0.5,
        "short help wraps inside the 280-pixel card: {:?}",
        short.bounds
    );
    assert!(
        f32::from(short.bounds.size.height) <= eight_lines + 0.5,
        "short help is cut at eight lines: {:?}",
        short.bounds
    );

    let paragraphs = tooltip_frame(layers::Help::Paragraphs(vec![
        HostSentence::new("help-agent").text("Codex — Guarded · Codex sandboxed auto"),
        HostSentence::new("help-caveat").text(long_help()),
        HostSentence::new("help-command")
            .text("Launched with: ")
            .mono(&HostText::from_host(
                "codex -a on-request -s workspace-write",
            )),
    ]));
    let caveat = painted_text(&paragraphs, "Your own allow rules");
    let width = f32::from(caveat.bounds.size.width);
    assert!(
        width > f32::from(metrics::TOOLTIP_MAX_WIDTH) - inset + 0.5
            && width > f32::from(short.bounds.size.width),
        "paragraph help is painted wider than the short card allows: {:?} against {:?}",
        caveat.bounds,
        short.bounds
    );
    assert!(
        width <= f32::from(metrics::TOOLTIP_PARAGRAPHS_MAX_WIDTH) - inset + 0.5,
        "and wraps inside the 440-pixel card: {:?}",
        caveat.bounds
    );
    assert!(
        f32::from(caveat.bounds.size.height) > eight_lines + 0.5,
        "paragraph help is not cut at eight lines: {:?}",
        caveat.bounds
    );

    let drawn =
        ["help-agent", "help-caveat", "help-command"].map(|id| node_bounds(&paragraphs, id));
    for pair in drawn.windows(2) {
        let gap = f32::from(pair[1].top() - pair[0].bottom());
        assert!(
            (gap - line).abs() < 0.5,
            "a blank line's gap ({line}) between paragraphs, not {gap}: {pair:?}"
        );
    }
}
