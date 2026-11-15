//! Hover help: the layer manager's one tooltip, whose help it shows when the pointer is over several controls, and when
//! it shows and goes.
//!
//! This is the tooltip half of the layer manager. Its rules are the layer manager's ("One tooltip at a time" and "The
//! innermost help wins" in the parent module's docs), its state lives in the window's [`LayerStack`], and
//! [`super::overlay`] draws its card on the top rung of the parent module's z-order ladder. Everything public here is
//! re-exported from [`crate::layers`], which is the path callers use.
//!
//! A tooltip waits [`HOVER_DELAY`] (the old app's 300 ms) before it shows, through a ticker deadline, and shows at once
//! when another tooltip is showing or one went away less than that long ago, so a user scanning a row of icons does
//! not wait at each. Escape, a press of any mouse button on a control with help ([`pointer_pressed`]) or a layer
//! opening puts the tooltip away without that grace, as in the old app. After a press made without moving, the help
//! stays down while the pointer rests on that control, since nothing changes whose help the pointer calls for.
//!
//! NOTE: Known deviations from the old app's `tooltip.js`, which kept a dismissed control's help down until the pointer
//! left it: after Escape, the next pointer move brings the help back after the delay; a key other than Escape takes
//! help down (GPUI ends every hover on a key press, and `tooltip.js` had no key handler but Escape's); after a press, a
//! key press and then a pointer move bring the help back; a press on a control right after a key press, or a click
//! whose pointer moves while the button is held, brings the help back after the delay once the button is released (GPUI
//! ends the control's hover during either and begins it again after); and after a press on a row, a visit to one of its
//! marks and back shows the row's help again. Still to come here: the tooltip's pointer notch and the target ring on
//! its control (both follow element anchoring for tooltips). Until then a tooltip sits below and right of where the
//! pointer was, and a pointer moving inside a control does not restart the wait.

use gpui::{
    App, ElementId, Entity, EntityId, ParentElement, Pixels, Point, SharedString, Styled,
    Subscription, WeakEntity, Window, div, px,
};

use super::{LayerStack, menu_shadow, stack};
use crate::host_text::{HostSentence, Line};
use crate::theme::{self, metrics};
use crate::ticker;

/// One control the pointer is over, as [`LayerStack::hovered`] keeps it.
pub(super) struct Hovered {
    owner: WeakEntity<TooltipOwner>,
    id: EntityId,
    /// The help it asked for when its hover began, refreshed each time the control is drawn ([`help_drawn`]). An outer
    /// control's help may show long after its hover began (the pointer returning from a mark onto its row), and by then
    /// the control may say something else: a session row's help follows the session's status, and the frame that takes
    /// a stopped session's live dot away also changes its row's help.
    help: Help,
    /// Drops the entry, and settles again, when the control is gone: no hover-leave fires for a control that simply
    /// stops being drawn, and the control around it, still under the pointer, should then have its help back.
    _release: Subscription,
}

/// How long the pointer must rest on a control before its hover help shows: the old app's tooltip delay.
pub const HOVER_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// The token that says which control a tooltip belongs to. It carries no data: its entity id is the owner's identity,
/// and its release is the signal that the control is gone.
pub struct TooltipOwner;

impl TooltipOwner {
    /// The owner token of the control being rendered under `key`, created on its first frame and kept for as long as
    /// the control is drawn in consecutive frames (GPUI's keyed element state, which cached views carry over when they
    /// replay). The token is unique per control instance in the window: it is keyed by the control's full element path,
    /// not its sibling-unique id. `key` must differ from the control's own element id.
    pub fn for_control(
        key: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<TooltipOwner> {
        window.use_keyed_state(key, cx, |_, _| TooltipOwner)
    }
}

/// What a tooltip says, in one of the two forms the old app drew hover help in. Controls hand it over through
/// [`crate::components::HoverHelp`], which picks the form.
///
/// Short help is one [`Line`] on a card at most 280 pixels wide and cut at eight lines (`.farhelm-tooltip`,
/// `crates/farhelm-ui/assets/app.css` lines 1854-1877 at reference commit `24bc0580`). Paragraph help is for help
/// whose last sentence matters as much as its first (the agent mark's and the permission shield's: the agent, what its
/// permission means and the caveat that closes it, then the stored command): the old app marked such a control with
/// `data-tooltip-paragraphs` (`tooltip.js` lines 444-445), and its card was at most 440 pixels wide with no line
/// clamp, its text `white-space: pre-line` so a blank line set paragraphs apart (`app.css` lines 1919-1926). It kept
/// the short card's padding and its `max-height: calc(100vh - 8px)`, so a paragraph card taller than the window is
/// clipped at the window, not at a line count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Help {
    /// Short help: the app's words, or a sentence naming host values.
    Line(Line),
    /// Paragraph help: each sentence a paragraph of its own, drawn one below the other with a blank line's gap
    /// between them, on the wider card, never clamped.
    Paragraphs(Vec<HostSentence>),
}

impl Help {
    /// The help as one string, as assistive technology reads it: for a line, its [`Line::accessible_text`]; for
    /// paragraphs, every paragraph's [`HostSentence::accessible_name`], separated by a blank line as the old app's
    /// one description string separated them. Each host value is escaped. For a control's accessible description and
    /// [`ShownTooltip::text`]; never for display.
    pub fn accessible_text(&self) -> SharedString {
        match self {
            Help::Line(line) => line.accessible_text(),
            Help::Paragraphs(paragraphs) => paragraphs
                .iter()
                .map(|paragraph| paragraph.accessible_name().to_string())
                .collect::<Vec<_>>()
                .join("\n\n")
                .into(),
        }
    }
}

/// Short help in the app's own words, so a caller with a plain string writes `"Save it".into()`.
impl From<&'static str> for Help {
    fn from(words: &'static str) -> Self {
        Help::Line(words.into())
    }
}

/// The hover help currently showing, if any. At most one tooltip shows at a time, which is what the old app did too.
#[derive(Clone, Debug, PartialEq)]
pub struct ShownTooltip {
    /// Which control asked for it, so only that control can take it down again.
    pub owner: EntityId,
    /// What the tooltip says, as one string, for tests and the bridge's snapshot: the app's wording, or for help that
    /// names a host value the sentence's accessible text, each host value escaped ([`Help::accessible_text`]). The
    /// card never draws it; it draws `help`.
    pub text: SharedString,
    /// Where the pointer was when the hover began, in window coordinates.
    pub at: Point<Pixels>,
    /// The help the card draws. Host values reach a tooltip only as host sentences (a [`Line::Sentence`], or the
    /// paragraphs of [`Help::Paragraphs`]), whose host parts are drawn by host text's own element, escaped and each in
    /// a text element of its own (SPEC_impl.md, "Host-supplied text"); a [`Line::App`] is Farhelm's own wording.
    help: Help,
}

/// The pointer came onto the control `owner`, whose hover help is `help`. This is the hover listener's half of the
/// tooltip rule, which [`crate::components::with_hover_help`] wires on every control, beside [`hover_moved`] and
/// [`hover_ended`]. Which hovered control's help shows is decided once, after GPUI has finished telling every
/// listener about the event ([`settle_hover`]). When it is this control's, it shows after [`HOVER_DELAY`] at the
/// pointer's position, or at once while another tooltip is showing or has only just gone.
///
/// A host value reaches a tooltip only as part of a host sentence ([`Help`]). The app's own wording is a
/// [`Line::App`], which a caller makes from a plain string with `.into()`.
///
/// # Nested hover help
///
/// A control with help can sit inside another that has its own (a session row's marks inside the row), so the pointer
/// can be over several at once. The innermost one's help shows, as in the old app, whose tooltip took the help nearest
/// the element under the pointer. Moving from it back onto the control around it shows that control's help again, and
/// moving from the outer control onto the inner shows the inner one's. GPUI draws a child after its parent, so on a
/// pointer move "innermost" means drawn last: of two hovered controls that overlap without nesting, the one drawn over
/// the other wins. Under a still pointer the newest hover wins until the pointer next moves, which for a control and
/// the ones inside it is the same answer, since they appear together or the inner one later.
///
/// The hover listeners alone cannot say which control is innermost, because the order they arrive in depends on why
/// the hover changed. On a pointer move GPUI calls them from the last drawn to the first, so a pointer that lands on a
/// mark in one move tells the mark first and the row last. When a frame moves controls under a still pointer (a
/// scroll, a row appearing) GPUI calls them in drawing order, row first. And a pointer going from the mark back onto
/// the row tells only the mark, because the row was hovered all along. So the layer manager keeps every hovered control
/// ([`LayerStack::hovered`]), newest hover first, which is right for a frame's news and for a pointer that crossed
/// into a control from the one around it. On a pointer move it takes GPUI's order instead: each hovered control's move
/// listener ([`hover_moved`]) runs on every move, last drawn first, so the order they ran in is exact.
pub fn hover_began(owner: &Entity<TooltipOwner>, help: Help, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, cx| {
        let release = cx.observe_release(owner, move |stack, _, cx| {
            stack.hovered.retain(|hovered| hovered.id != id);
            stack.settle_after_this_event(cx);
        });
        stack.hovered.retain(|hovered| hovered.id != id);
        stack.hovered.insert(
            0,
            Hovered {
                owner: owner.downgrade(),
                id,
                help,
                _release: release,
            },
        );
        stack.settle_after_this_event(cx);
    });
}

/// The pointer moved while over the control `owner`: the move listener's half of the tooltip rule. It says nothing
/// about the help itself. GPUI runs these on every pointer move, for every hovered control, from the last drawn to the
/// first, and that order is how [`settle_hover`] tells the innermost control from the ones around it (see "Nested
/// hover help" at [`hover_began`]).
pub fn hover_moved(owner: &Entity<TooltipOwner>, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, cx| {
        stack.settle_after_this_event(cx);
        if let Some(moved) = &mut stack.round
            && !moved.contains(&id)
        {
            moved.push(id);
        }
    });
}

/// The control `owner` is being drawn with `help` as its hover help: refresh the copy kept while the pointer is over
/// it, so a tooltip of it that begins later says what the control says then (see [`Hovered::help`]). It redraws
/// nothing: a tooltip already waiting or showing keeps the words it began with. Called while the control renders, so a
/// cached view's controls refresh exactly when they re-render, which is when their help can change.
pub fn help_drawn(owner: &Entity<TooltipOwner>, help: &Help, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, _| {
        if let Some(hovered) = stack.hovered.iter_mut().find(|hovered| hovered.id == id)
            && hovered.help != *help
        {
            hovered.help = help.clone();
        }
    });
}

/// The pointer left the control `owner`. Its tooltip goes only if no other hovered control's help takes its place, so
/// a control leaving never hides another's tooltip: that matters when the pointer moves straight from one control
/// onto the next and the new control's hover arrives first, and when it moves from a mark back onto the row around
/// it, whose help then shows.
pub fn hover_ended(owner: &Entity<TooltipOwner>, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, cx| {
        stack.hovered.retain(|hovered| hovered.id != id);
        stack.settle_after_this_event(cx);
    });
}

/// A mouse button went down on a control with help: put the tooltip away, as the old `tooltip.js` did on
/// `pointerdown` whatever the button, and make the control under the press (the innermost hovered one) the winner, so
/// that its help stays down while the pointer rests there and its hover lasts (the module NOTE lists when GPUI ends
/// it): [`settle_hover`] acts only when the winner changes. Every hovered control's press listener calls this for the
/// same press; the later calls change nothing.
pub fn pointer_pressed(window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    stack.update(cx, |stack, cx| {
        let shown = stack.tooltip.is_some();
        // The press may come in the same update as the pointer move that brought the pointer here, before that move's
        // settle (the test bridge sends a click's move, press and release together): order the hovers by that move.
        if let Some(moved) = &stack.round {
            innermost_first(&mut stack.hovered, moved);
        }
        stack.winner = stack.hovered.first().map(|hovered| hovered.id);
        stack.put_tooltip_away();
        if shown {
            cx.notify();
        }
    });
}

/// Put `hovered` in the order a pointer move's move listeners ran in (`moved`), which is innermost first and beats the
/// order the hovers began in. A control whose move listener did not run keeps its place after them (the sort is
/// stable), and an empty `moved` (no pointer move: a frame's news) changes nothing.
fn innermost_first(hovered: &mut [Hovered], moved: &[EntityId]) {
    if moved.is_empty() {
        return;
    }
    hovered.sort_by_key(|hovered| {
        moved
            .iter()
            .position(|id| *id == hovered.id)
            .unwrap_or(usize::MAX)
    });
}

/// Decide whose help the pointer calls for now that GPUI has told every listener about the event, and show it, or
/// take the tooltip down when the pointer is over no control with help. Nothing changes while the answer stays the
/// same control ([`LayerStack::winner`]).
fn settle_hover(window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let change = stack.update(cx, |stack, _| {
        let moved = stack.round.take().unwrap_or_default();
        stack
            .hovered
            .retain(|hovered| hovered.owner.upgrade().is_some());
        innermost_first(&mut stack.hovered, &moved);
        // This runs on every pointer move over a control with help, so the unchanged case clones nothing.
        let winner = stack.hovered.first().map(|hovered| hovered.id);
        if winner == stack.winner {
            return None;
        }
        let previous = std::mem::replace(&mut stack.winner, winner);
        let next = stack
            .hovered
            .first()
            .and_then(|hovered| Some((hovered.owner.upgrade()?, hovered.help.clone())));
        Some((previous, next))
    });
    match change {
        None => {}
        Some((_, Some((owner, help)))) => begin_tooltip(&owner, help, window, cx),
        Some((previous, None)) => end_tooltip(&stack, previous, cx),
    }
}

/// Start `owner`'s tooltip, now that its help is the one the pointer calls for: after [`HOVER_DELAY`], or at once
/// while another tooltip is showing or has only just gone. Either way it replaces any other tooltip, shown or waiting.
fn begin_tooltip(owner: &Entity<TooltipOwner>, help: Help, window: &mut Window, cx: &mut App) {
    let at = window.mouse_position();
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let at_once = {
        let stack = stack.read(cx);
        stack.tooltip.is_some() || stack.warm
    };
    if at_once {
        show_tooltip_from(owner, help, at, window, cx);
        return;
    }
    let owner_id = owner.entity_id();
    let weak_owner = owner.downgrade();
    let handle = window.window_handle();
    let deadline = ticker::after(HOVER_DELAY, cx, move |cx| {
        let Some(owner) = weak_owner.upgrade() else {
            return;
        };
        let _ = handle.update(cx, |_, window, cx| {
            show_tooltip_from(&owner, help, at, window, cx)
        });
    });
    stack.update(cx, |stack, _| {
        stack.pending_tooltip = Some((owner_id, deadline));
    });
}

/// Take down `previous`'s tooltip, shown or waiting: the pointer has left it and is over no other control whose help
/// shows. A tooltip some other control showed stays, so a control leaving never hides another's. A tooltip that was
/// showing leaves the grace behind it, so the next one within the delay shows at once.
fn end_tooltip(stack: &Entity<LayerStack>, previous: Option<EntityId>, cx: &mut App) {
    let Some(previous) = previous else {
        return;
    };
    let hidden = stack.update(cx, |stack, cx| {
        if stack
            .pending_tooltip
            .as_ref()
            .is_some_and(|(pending, _)| *pending == previous)
        {
            stack.pending_tooltip = None;
        }
        if !stack
            .tooltip
            .as_ref()
            .is_some_and(|shown| shown.owner == previous)
        {
            return false;
        }
        stack.tooltip = None;
        stack.tooltip_release = None;
        cx.notify();
        true
    });
    if hidden {
        let weak = stack.downgrade();
        let cool_down = ticker::after(HOVER_DELAY, cx, move |cx| {
            let _ = weak.update(cx, |stack, _| stack.warm = false);
        });
        stack.update(cx, |stack, _| {
            stack.warm = true;
            stack.cool_down = Some(cool_down);
        });
    }
}

/// [`hover_began`]'s tooltip shown at once, with the hover beginning at a fixed point, for the screenshot goldens.
#[cfg(test)]
pub(crate) fn show_tooltip_at(
    owner: &Entity<TooltipOwner>,
    help: Help,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    show_tooltip_from(owner, help, at, window, cx);
}

/// Show `help` for `owner` with the hover beginning at `at`. The one place a [`ShownTooltip`] is made, so its `text`
/// is always its `help`'s accessible text.
fn show_tooltip_from(
    owner: &Entity<TooltipOwner>,
    help: Help,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let owner_id = owner.entity_id();
    stack.update(cx, |stack, cx| {
        stack.pending_tooltip = None;
        stack.tooltip = Some(ShownTooltip {
            owner: owner_id,
            text: help.accessible_text(),
            at,
            help,
        });
        // The tooltip closes when its control is gone, though no hover-leave ever fires for a control that simply
        // stops being drawn (a dialog closing on Enter, a deleted row).
        stack.tooltip_release = Some(cx.observe_release(owner, move |stack, _, cx| {
            if stack
                .tooltip
                .as_ref()
                .is_some_and(|shown| shown.owner == owner_id)
            {
                stack.tooltip = None;
                cx.notify();
            }
        }));
        cx.notify();
    });
}

impl LayerStack {
    /// Run [`settle_hover`] once, after the event being handled: GPUI tells each hovered control's listeners about one
    /// pointer move (or one frame's hover changes) in turn, and the help to show depends on all of them. A settle runs
    /// as a deferred effect, which GPUI runs only once the event's listeners have all returned, and the first piece of
    /// news schedules it.
    fn settle_after_this_event(&mut self, cx: &mut App) {
        if self.round.is_some() {
            return;
        }
        self.round = Some(Vec::new());
        let window = self.window;
        // The update fails only when the window is gone, and its stack with it, so the round it leaves open is never
        // needed again.
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| settle_hover(window, cx));
        });
    }

    /// Take the tooltip down, and any waiting one, without the grace that lets the next one skip the delay: after
    /// Escape or a layer opening, the user has just put one away.
    pub(super) fn put_tooltip_away(&mut self) {
        self.tooltip = None;
        self.tooltip_release = None;
        self.pending_tooltip = None;
        self.warm = false;
        self.cool_down = None;
    }
}

/// The tooltip showing now in `window`, for tests and the bridge's snapshot.
pub fn shown_tooltip(window: &Window, cx: &App) -> Option<ShownTooltip> {
    stack(window, cx)?.read(cx).tooltip.clone()
}

/// The tooltip's card, styled as the old `.farhelm-tooltip`: short help cut at eight lines inside the 280-pixel cap,
/// paragraph help one paragraph below the other inside the 440-pixel cap and never cut ([`Help`]).
pub(super) fn tooltip_card(shown: &ShownTooltip, window: &Window, cx: &App) -> gpui::Div {
    let palette = theme::palette(cx);
    let (pad_y, pad_x) = metrics::TOOLTIP_PAD;
    let viewport = window.viewport_size();
    // `max-width: min(280px, 100vw - 8px)` (440 for paragraphs) and `max-height: 100vh - 8px`, as the old tooltip.
    let cap = match shown.help {
        Help::Line(_) => metrics::TOOLTIP_MAX_WIDTH,
        Help::Paragraphs(_) => metrics::TOOLTIP_PARAGRAPHS_MAX_WIDTH,
    };
    let card = div()
        .max_w(cap.min(viewport.width - px(8.)))
        .max_h(viewport.height - px(8.))
        .overflow_hidden()
        .py(pad_y)
        .px(pad_x)
        .bg(theme::color(palette.bg_2))
        .text_color(theme::color(palette.fg_0))
        .border_1()
        .border_color(theme::color(palette.border))
        .rounded(metrics::RADIUS_MD)
        .shadow(menu_shadow(palette))
        .text_size(metrics::TEXT_SM)
        .line_height(metrics::TOOLTIP_LINE_HEIGHT);
    // The help itself, not its accessible text: a sentence draws each host value through host text's element.
    match &shown.help {
        Help::Line(line) => card
            .line_clamp(metrics::TOOLTIP_MAX_LINES)
            .child(line.clone()),
        // No clamp here: the clamp is a text style every paragraph would inherit, and it would cut each one at eight
        // lines, which is exactly what this form exists to avoid (a guarded launch's closing caveat).
        Help::Paragraphs(paragraphs) => card
            .flex()
            .flex_col()
            .gap(metrics::TOOLTIP_PARAGRAPH_GAP)
            .children(paragraphs.iter().cloned()),
    }
}
