//! Hover help: the layer manager's one tooltip, whose help it shows when the pointer is over several controls, and when
//! it shows and goes.
//!
//! This is the tooltip half of the layer manager. Its rules are the layer manager's ("One tooltip at a time" and "The
//! innermost help wins" in the parent module's docs), its state lives in the window's [`LayerStack`], and
//! [`super::overlay`] draws its card on the top rung of the parent module's z-order ladder. Everything public here is
//! re-exported from [`crate::layers`], which is the path callers use.
//!
//! Help comes in three forms ([`Help`]: a line, paragraphs, a card holding a view) and in two placements: ordinary
//! help against its control, centered above it or else below as the old app placed it, and help a control asks to have
//! beside the sidebar ([`Hint::beside`]); [`super::beside`] places both, from the bounds the control's tracker records
//! while its help waits or shows ([`TooltipOwner::track`], [`help_drawn`]). A card's view is either the caller's, made
//! with the help, or made by the tooltip only when it shows ([`Help::card_with`]) and dropped when it goes, for a view
//! that costs something for as long as it lives.
//!
//! A tooltip waits [`HOVER_DELAY`] (the old app's 300 ms) before it shows, through a ticker deadline, and shows at once
//! when another tooltip is showing or one went away less than that long ago, so a user scanning a row of icons does
//! not wait at each. A pointer moving inside the control while its help waits starts the wait again ([`hover_moved`]),
//! so help shows only once the pointer comes to rest. Escape, a press of any mouse button on a control with help
//! ([`pointer_pressed`]) or a layer opening puts the tooltip away without that grace, as in the old app. After a press
//! made without moving, the help stays down while the pointer rests on that control, since nothing changes whose help
//! the pointer calls for.
//!
//! Keyboard focus calls for help too, on every control that can hold it, as the old app showed every focus-visible
//! control's help: focus the keyboard moves onto a control shows its help after the same delay, and focus leaving
//! takes it down ([`focus_drawn`] has how the pointer and focus share the one tooltip). Help shown by focus is placed
//! against the control by the same ordinary rule, from the bounds the focus coordinator records for its stop, or beside
//! the sidebar for help that asks for it.
//!
//! NOTE: Known deviations from the old app's `tooltip.js`, which kept a dismissed control's help down until the pointer
//! left it: after Escape, the next pointer move brings the help back after the delay; a key other than Escape takes
//! help down (GPUI ends every hover on a key press, and `tooltip.js` had no key handler but Escape's); after a press, a
//! key press and then a pointer move bring the help back; a press on a control right after a key press, or a click
//! whose pointer moves while the button is held, brings the help back after the delay once the button is released (GPUI
//! ends the control's hover during either and begins it again after); and after a press on a row, a visit to one of its
//! marks and back shows the row's help again.
//!
//! NOTE: Two more deviations, in help shown on keyboard focus. The old app landed focus quietly in a few host flows,
//! keeping the ring but showing no help (`focusWithoutTooltip`, from `modal_dialog.rs`'s quiet `FocusTarget`): the
//! host add dialog's provisioning confirm and Cancel, the remembered first focus of host settings and of the uninstall
//! confirmation, and the attention row's handoff to a host row's menu. Here every keyboard landing shows help; a quiet
//! landing can come with those flows' port. And pointer help stays up when the window loses focus, where the old app
//! hid every tooltip on the window's `blur`; help keyboard focus owns goes then, since GPUI reports focus leaving an
//! inactive window's controls, but it does not come back when the window regains focus, where the old browser fired
//! `focusin` again.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyView, App, Bounds, ElementId, Entity, EntityId, FocusHandle, Hsla, InteractiveElement as _,
    IntoElement, ParentElement, Pixels, Point, SharedString, Styled, Subscription, WeakEntity,
    Window, div, point, px,
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
    help: Hint,
    /// Drops the entry, and settles again, when the control is gone: no hover-leave fires for a control that simply
    /// stops being drawn, and the control around it, still under the pointer, should then have its help back.
    _release: Subscription,
}

/// How long the pointer must rest on a control before its hover help shows: the old app's tooltip delay.
pub const HOVER_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// The token that says which control a tooltip belongs to: its entity id is the owner's identity, and its release is
/// the signal that the control is gone. It also holds where the control was last drawn, which its help is placed
/// against, as the control's bounds tracker records it ([`TooltipOwner::track`]).
#[derive(Default)]
pub struct TooltipOwner {
    target: HelpTarget,
}

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
        window.use_keyed_state(key, cx, |_, _| TooltipOwner::default())
    }

    /// The control's bounds tracker, a zero-cost child that records where the control is drawn and the clip in force
    /// there as each frame is prepared, before the overlay places the help against it. It covers the box inside the
    /// control's border. [`crate::components::with_hover_help`] draws it only while the control's help waits or shows
    /// ([`help_drawn`]), so a control whose help is not in play draws nothing extra.
    pub fn track(&self) -> impl IntoElement + use<> {
        self.target.track()
    }
}

/// What a tooltip says, in one of the three forms the old app drew hover help in. Controls hand it over through
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
///
/// A third form, the card ([`Help::card`], or [`Help::card_with`] to make the view only while it shows), carries a
/// view of the caller's own on a card of fixed width: the old session row's summary card (`data-kind="card"`,
/// `app.css` lines 1932-1938: `width: min(310px, 100vw - 8px)`, no padding, no clamp), which shared the other forms'
/// fill, border, radius, shadow and height cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Help {
    /// Short help: the app's words, or a sentence naming host values.
    Line(Line),
    /// Paragraph help: each sentence a paragraph of its own, drawn one below the other with a blank line's gap
    /// between them, on the wider card, never clamped.
    Paragraphs(Vec<HostSentence>),
    /// Rich help: the caller's view on a card of fixed width, with the caller's words for assistive technology.
    Card(Card),
}

/// The card form of hover help ([`Help::card`], [`Help::card_with`]): a view drawn on the tooltip's card at a fixed
/// width.
///
/// Two cards are the same help when they have the same width and words and the same view: the same entity for a view
/// made with the help, and for one the tooltip makes ([`CardView::Later`]) the same builder allocation, so clones of
/// one help are equal and two renders' help is not. The one use of equality is [`help_drawn`] deciding whether to
/// replace a hovered control's stored help, so each render of a lazy card replaces it with that render's builder, and
/// a tooltip that shows later builds the view that render describes (the right host, after a row changed hosts). A
/// waiting or showing tooltip is never refreshed from it, so the view is still made only when the tooltip shows.
#[derive(Clone)]
pub struct Card {
    /// The card's width, kept 8 pixels inside a narrower window (the old `min(310px, 100vw - 8px)`).
    width: Pixels,
    view: CardView,
    /// What the card says, as assistive technology reads it ([`Help::accessible_text`]): a line, so that a host value
    /// in it (the session's title and host, in the summary) is escaped as in every other form.
    text: Line,
}

/// What makes a lazy card's view ([`Help::card_with`]), shared by every clone of the help.
type BuildCard = Rc<dyn Fn(&mut Window, &mut App) -> AnyView>;

/// Where a card's view comes from.
#[derive(Clone)]
enum CardView {
    /// Made by the caller with the help ([`Help::card`]), and drawn as is.
    Made(AnyView),
    /// Made by `build` when the tooltip shows and kept by the showing tooltip ([`Help::shown`]), so it lives only
    /// while the card shows. The builder's allocation is its identity ([`Card`]).
    Later { build: BuildCard },
}

impl PartialEq for Card {
    fn eq(&self, other: &Self) -> bool {
        let same_view = match (&self.view, &other.view) {
            (CardView::Made(one), CardView::Made(other)) => one.entity_id() == other.entity_id(),
            (CardView::Later { build: one }, CardView::Later { build: other }) => {
                Rc::ptr_eq(one, other)
            }
            _ => false,
        };
        same_view && self.width == other.width && self.text == other.text
    }
}

impl Eq for Card {}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let view = match &self.view {
            CardView::Made(view) => format!("{:?}", view.entity_id()),
            CardView::Later { .. } => "built when shown".to_string(),
        };
        f.debug_struct("Card")
            .field("width", &self.width)
            .field("view", &view)
            .field("text", &self.text)
            .finish()
    }
}

impl Help {
    /// Card help: `view` drawn on the tooltip's card, `width` wide (less where the window is narrower), with no
    /// padding and no line clamp, cut only at the window's height less 8 pixels as every form is. `text` is what
    /// assistive technology reads for it, since a view has no words of its own to read; it names host values only as
    /// a host sentence, whose accessible text escapes them.
    pub fn card(width: Pixels, view: impl Into<AnyView>, text: impl Into<Line>) -> Help {
        Help::Card(Card {
            width,
            view: CardView::Made(view.into()),
            text: text.into(),
        })
    }

    /// [`Help::card`] whose view `build` makes only when the tooltip shows, and which the tooltip drops when it hides
    /// or is replaced: for a view that holds something for as long as it lives (the host progress card holds the
    /// ticker's clock and a running step's pulse), which one view per row would keep running while nothing draws it.
    /// While the card shows, the one view it made is drawn in every frame. See [`Card`] for when two are the same help.
    pub fn card_with(
        width: Pixels,
        build: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
        text: impl Into<Line>,
    ) -> Help {
        Help::Card(Card {
            width,
            view: CardView::Later {
                build: Rc::new(build),
            },
            text: text.into(),
        })
    }

    /// This help as a showing tooltip holds it: a card whose view is made later gets it made now, once.
    fn shown(self, window: &mut Window, cx: &mut App) -> Help {
        match self {
            Help::Card(Card {
                width,
                view: CardView::Later { build },
                text,
            }) => Help::Card(Card {
                width,
                view: CardView::Made(build(window, cx)),
                text,
            }),
            help => help,
        }
    }

    /// The help as one string, as assistive technology reads it: for a line, its [`Line::accessible_text`]; for
    /// paragraphs, every paragraph's [`HostSentence::accessible_name`], separated by a blank line as the old app's
    /// one description string separated them; for a card, its line's accessible text. Each host value is escaped. For
    /// a control's accessible description and [`ShownTooltip::text`]; never for display.
    pub fn accessible_text(&self) -> SharedString {
        match self {
            Help::Line(line) => line.accessible_text(),
            Help::Paragraphs(paragraphs) => paragraphs
                .iter()
                .map(|paragraph| paragraph.accessible_name().to_string())
                .collect::<Vec<_>>()
                .join("\n\n")
                .into(),
            Help::Card(card) => card.text.accessible_text(),
        }
    }
}

/// What a control asks of help placed beside the sidebar ([`Hint::beside`]): the ring's color and its corners. The
/// ring is the old `.tooltip-target-ring` (`app.css` lines 1899-1917): an accent line 1 pixel inside the control's
/// bounds, or the on-fill color in a selected row, which already wears the accent fill. Its corners are the
/// control's own (the old ring was an inset shadow, which follows the element's radius), so a round status dot
/// passes half its size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BesideCues {
    /// Draw the ring in the on-fill color rather than the accent.
    pub on_fill: bool,
    /// The ring's corner radius: the control's own.
    pub radius: Pixels,
}

/// Hover help as a control hands it to the layer manager: what it says, and for help that sits beside the sidebar, the
/// cues it asks for. Ordinary help (a [`Help`] or a string, converted) sits against its control, centered above it or
/// else below, as the old app placed it ([`super::beside`] has the numbers). Either is placed from where the control's
/// owner token last saw it drawn ([`TooltipOwner::track`]). Help whose control has no recorded bounds, ordinary or
/// beside, sits at the pointer instead: a probe tests and goldens show for an owner never drawn as a control
/// ([`show_tooltip_at`]), or a control in a cached view (`cache.rs`, NOTE).
/// [`crate::components::with_hover_help`] makes these for every control, from its [`crate::components::HoverHelp`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    help: Help,
    /// The cues of help placed beside the sidebar; `None` for ordinary help.
    beside: Option<BesideCues>,
}

impl Hint {
    /// Help placed beside the sidebar's edge, centered on its control. The module docs of [`super::beside`] give the
    /// placement and when it falls back.
    pub fn beside(help: Help, cues: BesideCues) -> Hint {
        Hint {
            help,
            beside: Some(cues),
        }
    }
}

/// The name each prepaint of a bounds tracker ([`TooltipOwner::track`]) is counted under in test builds
/// ([`crate::render_count`]), where it counts the trackers a frame draws.
pub(crate) const TRACKER: &str = "hover-help-tracker";

/// Where a control with hover help was last drawn, as its tracker recorded it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Drawn {
    /// The control's bounds inside its border, unclipped: what the help is placed against, and for help beside the
    /// sidebar, where the ring is drawn.
    pub(super) bounds: Bounds<Pixels>,
    /// The clip in force where the control is drawn (its scrolling list's, say), which the ring is drawn inside, as the
    /// old ring, an inset shadow on the control, was clipped with it.
    pub(super) clip: Bounds<Pixels>,
}

/// Where a control was last drawn, as its owner token keeps it ([`TooltipOwner::track`]). It keeps the last bounds
/// recorded after its tracker stops being drawn; the frame that shows the help draws the tracker again and records the
/// control's bounds before the help is placed ([`help_drawn`]). Two are the same target only when they are one record,
/// not two that happen to hold the same place.
#[derive(Clone, Debug, Default)]
pub(super) struct HelpTarget(Rc<Cell<Option<Drawn>>>);

impl PartialEq for HelpTarget {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl HelpTarget {
    /// Where the control was last drawn, or `None` while its tracker has never been drawn.
    pub(super) fn drawn(&self) -> Option<Drawn> {
        self.0.get()
    }

    /// The tracker itself: a zero-cost child that records the control's bounds and the clip in force there, as
    /// [`super::Anchor::track`] records an anchor's.
    fn track(&self) -> impl IntoElement + use<> {
        let drawn = self.0.clone();
        let record = move |bounds, window: &mut Window, cx: &mut App| {
            // Counted in test builds only, so a test sees which frames draw a tracker at all.
            crate::render_count::record(TRACKER, cx);
            let clip = window.content_mask().bounds;
            drawn.set(Some(Drawn { bounds, clip }));
        };
        // Pinned to the control's corner, for the reason `Anchor::track` is.
        gpui::canvas(record, |_, _, _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full()
    }
}

/// Ordinary help, placed against its control.
impl From<Help> for Hint {
    fn from(help: Help) -> Self {
        Hint { help, beside: None }
    }
}

/// Ordinary short help in the app's own words, so a caller with a plain string writes `"Save it".into()`.
impl From<&'static str> for Hint {
    fn from(words: &'static str) -> Self {
        Help::from(words).into()
    }
}

/// Where the showing tooltip was placed in the last frame drawn ([`ShownTooltip::side`]): the old box's `data-side`,
/// plus the pointer placement of help with no control to sit against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipSide {
    /// Below and right of where the pointer was: help whose control has no recorded bounds, such as a probe a test or
    /// golden shows for an owner token no control drew ([`show_tooltip_at`]), or a control in a cached view.
    Pointer,
    /// Beside the sidebar, with the notch and the ring (the old `"right"`).
    Beside,
    /// Centered above its control: ordinary help, help keyboard focus owns, and beside placement's fallback in a
    /// window too narrow for it, wherever there is room above.
    Above,
    /// Below its control: the same, with no room above.
    Below,
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
    /// Where the pointer was when the tooltip was asked for, in window coordinates. Only help with no drawn control to
    /// sit against is placed from it ([`TooltipSide::Pointer`]).
    pub at: Point<Pixels>,
    /// The help the card draws. Host values reach a tooltip only as host sentences (a [`Line::Sentence`], or the
    /// paragraphs of [`Help::Paragraphs`]), whose host parts are drawn by host text's own element, escaped and each in
    /// a text element of its own (SPEC_impl.md, "Host-supplied text"); a [`Line::App`] is Farhelm's own wording.
    pub(super) help: Help,
    /// Where the owning control was last drawn, which the help is placed against ([`TooltipOwner::track`]).
    pub(super) target: HelpTarget,
    /// The cues the control asked for, for help placed beside the sidebar; `None` for ordinary help.
    pub(super) beside: Option<BesideCues>,
    /// Where the overlay placed it in the last frame: written as that frame is laid out, so it is shared with every
    /// clone ([`ShownTooltip::side`]).
    pub(super) side: Rc<Cell<TooltipSide>>,
    /// The ring the last frame painted, written as it is painted ([`ShownTooltip::ring`]).
    pub(super) ring: Rc<Cell<Option<DrawnRing>>>,
}

impl ShownTooltip {
    /// Where the tooltip sits, as the last frame drawn placed it, for tests (the bridge does not read it yet). Help
    /// shown before any frame placed it reads [`TooltipSide::Pointer`].
    pub fn side(&self) -> TooltipSide {
        self.side.get()
    }

    /// The ring around the control as the last frame painted it, or `None` when it painted none (help not beside the
    /// sidebar). Read from the very quad painted, so a test sees the color, corners and clip the user sees.
    pub fn ring(&self) -> Option<DrawnRing> {
        self.ring.get()
    }
}

/// The ring a frame painted around the control of help beside the sidebar ([`ShownTooltip::ring`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnRing {
    /// Its bounds: the control's, inside its border. The line is 1 pixel wide, inside them.
    pub bounds: Bounds<Pixels>,
    /// The clip it was painted in: the window's, narrowed to the clip in force where the control is drawn.
    pub clip: Bounds<Pixels>,
    /// The line's color: the accent, or on-fill for a control in a selected row.
    pub color: Hsla,
    /// Its corners' radius: the control's own, as its help gave it.
    pub radius: Pixels,
}

/// How far below and right of the pointer help with no control to sit against starts, like a native tooltip, so the
/// pointer never covers the text.
pub(super) const POINTER_OFFSET: Point<Pixels> = point(px(8.), px(16.));

/// The pointer came onto the control `owner`, whose hover help is `help`. This is the hover listener's half of the
/// tooltip rule, which [`crate::components::with_hover_help`] wires on every control, beside [`hover_moved`] and
/// [`hover_ended`]. Which hovered control's help shows is decided once, after GPUI has finished telling every
/// listener about the event ([`settle_hover`]). When it is this control's, it shows against the control after
/// [`HOVER_DELAY`], or at once while another tooltip is showing or has only just gone.
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
pub fn hover_began(owner: &Entity<TooltipOwner>, help: Hint, window: &mut Window, cx: &mut App) {
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

/// The pointer moved while over the control `owner`: the move listener's half of the tooltip rule. GPUI runs these on
/// every pointer move, for every hovered control, from the last drawn to the first, and that order is how
/// [`settle_hover`] tells the innermost control from the ones around it (see "Nested hover help" at [`hover_began`]).
///
/// A move inside the control whose help is waiting starts the wait again, as the old `tooltip.js` restarted its timer
/// on every move inside the target, so help shows only once the pointer comes to rest. Help that shows already stays,
/// and so does a wait keyboard focus began, which the pointer did not start.
pub fn hover_moved(owner: &Entity<TooltipOwner>, window: &mut Window, cx: &mut App) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    let waiting = stack.update(cx, |stack, cx| {
        stack.settle_after_this_event(cx);
        if let Some(moved) = &mut stack.round
            && !moved.contains(&id)
        {
            moved.push(id);
        }
        let pending = stack.pending_tooltip.as_ref().map(|(pending, _)| *pending);
        if pending != Some(id) || stack.by_focus.is_some() {
            return None;
        }
        let hovered = stack.hovered.iter().find(|hovered| hovered.id == id)?;
        Some(hovered.help.clone())
    });
    // Begun again, the wait replaces the one waiting, whose deadline is dropped with it.
    if let Some(help) = waiting {
        begin_tooltip(owner, help, None, window, cx);
    }
}

/// The control `owner` is being drawn with `help` as its hover help: refresh the copy kept while the pointer is over
/// it, so a tooltip of it that begins later says what the control says then (see [`Hovered::help`]). It redraws
/// nothing: a tooltip already waiting or showing keeps the words it began with. Called while the control renders, so a
/// cached view's controls refresh exactly when they re-render, which is when their help can change.
///
/// Returns whether this control's help waits or shows, by the pointer or by keyboard focus: only then does the control
/// draw its bounds tracker ([`TooltipOwner::track`]), so a control whose help is not in play draws nothing extra in an
/// idle frame. The tracker is in place by the time the help shows, because showing it notifies the layer stack, which
/// the root view observes, so the frame that first draws the card re-renders every uncached view, the owning control
/// included, and its tracker records the control's bounds before the overlay places the card.
///
/// A cached view (`cache.rs`; none holds a control with help in the app) does not re-render when its help shows, so its
/// control draws no tracker then: its help, ordinary or beside, is placed against bounds an earlier render recorded,
/// which may be stale, or at the pointer, without cues, when none was ever recorded.
pub fn help_drawn(
    owner: &Entity<TooltipOwner>,
    help: &Hint,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(stack) = stack(window, cx) else {
        return false;
    };
    let id = owner.entity_id();
    stack.update(cx, |stack, _| {
        if let Some(hovered) = stack.hovered.iter_mut().find(|hovered| hovered.id == id)
            && hovered.help != *help
        {
            hovered.help = help.clone();
        }
        stack.current_owner() == Some(id)
    })
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
    let news = stack.update(cx, |stack, _| stack.focus_news.take());
    settle_focus(&stack, news, window, cx);
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
        // The pointer coming onto the control whose help shows already (by focus) changes nothing, and the pointer
        // leaving keeps help that keyboard focus owns, as the old app's focus-owned hover stayed.
        match &next {
            Some((owner, _)) if stack.current_owner() == Some(owner.entity_id()) => None,
            None if stack.by_focus.is_some() => None,
            _ => Some((previous, next)),
        }
    });
    match change {
        None => {}
        Some((_, Some((owner, help)))) => begin_tooltip(&owner, help, None, window, cx),
        Some((previous, None)) => end_tooltip(&stack, previous, cx),
    }
}

/// Act on keyboard focus news from [`focus_drawn`]: show the help of the control focus landed on, as the pointer
/// would, replacing any other, or, when that control's help shows already, make it focus-owned.
fn settle_focus(
    stack: &Entity<LayerStack>,
    news: Option<FocusNews>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(FocusNews { owner, help, focus }) = news else {
        return;
    };
    let Some(owner) = owner.upgrade() else {
        return;
    };
    let by_focus = ByFocus::watching(&owner, focus, window, cx);
    if stack.read(cx).current_owner() == Some(owner.entity_id()) {
        stack.update(cx, |stack, _| stack.by_focus = Some(by_focus));
    } else {
        begin_tooltip(&owner, help, Some(by_focus), window, cx);
    }
}

/// Keyboard focus landing on the control `owner`, whose help is `help` and whose focus stop is `focus`, as
/// [`focus_drawn`] saw it, for [`settle_focus`].
pub(super) struct FocusNews {
    owner: WeakEntity<TooltipOwner>,
    help: Hint,
    focus: FocusHandle,
}

/// Keyboard focus owning the tooltip shown or waiting: the control's focus stop, against which the help is placed
/// ([`super::overlay`]), and the watch that takes the help down once focus leaves that stop, whatever the reason (a
/// key, a click, the control disabled or gone, the window losing focus) and whatever order the frame draws
/// controls in. Dropping it ends the watch.
pub(super) struct ByFocus {
    pub(super) focus: FocusHandle,
    _left: Subscription,
}

impl ByFocus {
    /// Own the tooltip of `owner` by its focus stop `focus`.
    fn watching(
        owner: &Entity<TooltipOwner>,
        focus: FocusHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> ByFocus {
        let owner = owner.entity_id();
        let left = window.on_focus_out(&focus, cx, move |_, window, cx| {
            if let Some(stack) = stack(window, cx) {
                end_tooltip(&stack, Some(owner), cx);
            }
        });
        ByFocus { focus, _left: left }
    }
}

/// The control `owner`, whose help is `help` and whose focus stop is `focus`, was drawn: the focus half of the
/// tooltip rule, which [`crate::components::with_hover_help`] wires on every control that can hold focus. Focus
/// landing on that stop while the user's last input was the keyboard (the old `:focus-visible`, as the focus ring has
/// it) shows its help as the pointer resting on it would: after [`HOVER_DELAY`], or at once in the warm grace, placed
/// against the control by the old ordinary rule (centered above it, else below: [`super::beside`]). A click does not,
/// and nor does a later key press on a control a click focused: only the landing counts, as the old app scheduled
/// help on `focusin` alone. Focus leaving the stop takes it down ([`ByFocus`]).
///
/// Pointer and focus help share the one tooltip, and the latest wins: focus landing on a control replaces the
/// pointer's help; the pointer coming onto another control replaces focus help; the pointer leaving leaves focus
/// help up; and focus landing on the control whose help the pointer shows makes that help focus-owned, so it stays
/// when the pointer goes, as in the old `tooltip.js`. Escape with no layer open, or a press, puts it away until focus
/// next lands on a control, the same one included; with a layer open, Escape closes that layer instead.
///
/// It reads focus as each frame renders the control, which a focus move always causes (it refreshes the window), and
/// only to notice a landing: [`LayerStack::focused`] is the control last seen holding focus, so the next render of it
/// holding focus is not news.
pub fn focus_drawn(
    owner: &Entity<TooltipOwner>,
    help: &Hint,
    focus: &FocusHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let id = owner.entity_id();
    let focused = focus.is_focused(window);
    let keyboard = window.last_input_was_keyboard();
    stack.update(cx, |stack, cx| {
        if focused == (stack.focused == Some(id)) {
            return;
        }
        if !focused {
            stack.focused = None;
            return;
        }
        stack.focused = Some(id);
        if keyboard {
            stack.focus_news = Some(FocusNews {
                owner: owner.downgrade(),
                help: help.clone(),
                focus: focus.clone(),
            });
            stack.settle_after_this_event(cx);
        }
    });
}

/// Start `owner`'s tooltip, now that its help is the one the pointer calls for: after [`HOVER_DELAY`], or at once
/// while another tooltip is showing or has only just gone. Either way it replaces any other tooltip, shown or waiting.
fn begin_tooltip(
    owner: &Entity<TooltipOwner>,
    help: Hint,
    by_focus: Option<ByFocus>,
    window: &mut Window,
    cx: &mut App,
) {
    let at = window.mouse_position();
    let Some(stack) = stack(window, cx) else {
        return;
    };
    stack.update(cx, |stack, _| stack.by_focus = by_focus);
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
            stack.by_focus = None;
        }
        if !stack
            .tooltip
            .as_ref()
            .is_some_and(|shown| shown.owner == previous)
        {
            return false;
        }
        stack.by_focus = None;
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

/// [`hover_began`]'s tooltip shown at once, with the hover beginning at a fixed point, for the screenshot goldens and
/// tests: any help, beside help included.
#[cfg(test)]
pub(crate) fn show_tooltip_at(
    owner: &Entity<TooltipOwner>,
    help: Hint,
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
    hint: Hint,
    at: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(stack) = stack(window, cx) else {
        return;
    };
    let owner_id = owner.entity_id();
    let target = owner.read(cx).target.clone();
    let text = hint.help.accessible_text();
    // Made here, when it shows, and dropped with the tooltip it belongs to.
    let help = hint.help.shown(window, cx);
    stack.update(cx, |stack, cx| {
        stack.pending_tooltip = None;
        stack.tooltip = Some(ShownTooltip {
            owner: owner_id,
            text,
            at,
            help,
            target,
            beside: hint.beside,
            side: Rc::new(Cell::new(TooltipSide::Pointer)),
            ring: Rc::new(Cell::new(None)),
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
                stack.by_focus = None;
                cx.notify();
            }
        }));
        cx.notify();
    });
}

impl LayerStack {
    /// The control whose tooltip shows or waits, if any.
    fn current_owner(&self) -> Option<EntityId> {
        let shown = self.tooltip.as_ref().map(|shown| shown.owner);
        shown.or(self.pending_tooltip.as_ref().map(|(owner, _)| *owner))
    }

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
        self.by_focus = None;
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
/// paragraph help one paragraph below the other inside the 440-pixel cap and never cut, card help its caller's view at
/// its fixed width with no padding ([`Help`]). Every form shares the fill, border, radius, shadow and height cap.
pub(super) fn tooltip_card(shown: &ShownTooltip, window: &Window, cx: &App) -> gpui::Div {
    let palette = theme::palette(cx);
    let (pad_y, pad_x) = metrics::TOOLTIP_PAD;
    // Every form keeps 8 pixels inside the window: `max-width: min(280px, 100vw - 8px)` (440 for paragraphs, a fixed
    // `width: min(310px, 100vw - 8px)` for the summary card) and `max-height: 100vh - 8px`, as the old tooltip. The old
    // viewport was the page: the content area here (`desktop::linux_title_bar::content_area`).
    let area = crate::desktop::linux_title_bar::content_area(window).size;
    let room = area.width - px(8.);
    let card = div()
        .debug_selector(|| "tooltip-card".into())
        .max_h(area.height - px(8.))
        .overflow_hidden()
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
            .max_w(metrics::TOOLTIP_MAX_WIDTH.min(room))
            .py(pad_y)
            .px(pad_x)
            .line_clamp(metrics::TOOLTIP_MAX_LINES)
            .child(line.clone()),
        // No clamp here: the clamp is a text style every paragraph would inherit, and it would cut each one at eight
        // lines, which is exactly what this form exists to avoid (a guarded launch's closing caveat).
        Help::Paragraphs(paragraphs) => card
            .max_w(metrics::TOOLTIP_PARAGRAPHS_MAX_WIDTH.min(room))
            .py(pad_y)
            .px(pad_x)
            .flex()
            .flex_col()
            .gap(metrics::TOOLTIP_PARAGRAPH_GAP)
            .children(paragraphs.iter().cloned()),
        Help::Card(form) => {
            let card = card.w(form.width.min(room));
            match &form.view {
                CardView::Made(view) => card.child(view.clone()),
                // Never here: a showing tooltip's card was made when it showed ([`Help::shown`]).
                CardView::Later { .. } => {
                    debug_assert!(false, "a showing tooltip's lazy card was never made");
                    card
                }
            }
        }
    }
}
