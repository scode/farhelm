//! The component layer: every control the GUI draws, built so that the old app's coverage rules hold by construction.
//!
//! SPEC_impl.md, "The component layer and theme": a control's constructor takes its label, its hover help and its
//! accessibility role, so a control without them does not compile. The old app checked "every control has hover help"
//! with a browser test that walked the page; here the type does it, and the test bridge reads the same role and label
//! to find controls. Colors come only from theme tokens ([`crate::theme`]), and hover help is drawn by the layer
//! manager ([`crate::layers`]), never by a component itself.
//!
//! Among the controls, GPUI Kit is used only for the text input (Kit's `Input`, or its `Textarea` held to one row for a
//! field that must keep pasted line breaks, both with the right-click menu turned off as in the old app's release
//! builds). Buttons, the select and the dialog frame are plain GPUI, which matches the old stylesheet's
//! control tiers exactly and keeps Kit's own focus and overlay machinery out of them (Kit's `Select` draws its own
//! dropdown, which only the layer manager may). Outside this module Kit appears
//! only where SPEC_impl.md allows it: its `Root` around the window (the bootstrap and the screenshot goldens) and the
//! projection of Farhelm's tokens onto Kit's theme (the theme module), which Kit's input reads.
//!
//! Some modules here draw no control. Drawing-only pieces such as `pulse`, `menu_separator`, `spinner`, `status_card`
//! (the accent card a pane stands over a terminal, which its caller fills with controls of its own) and `rail_entry`
//! (what a host rail button shows inside it) have no input, focus or name, so the label, hover help and role rule does
//! not reach them. `transition`'s [`entrance`] fades an element in as it appears: it is the one place in the GUI crates'
//! own code where GPUI's animation may be used, for one-shot transitions only (its module docs say why the ticker cannot
//! draw them and what it keeps). `menu_keys` draws nothing either: its two key handlers go on a control or panel an
//! owner draws (a menu's toggle, the open menu), whose label, hover help and role that control carries, so the rule has
//! nothing of its own to reach.
//!
//! Two of the layer's rules make a box its caller draws into a keyboard stop rather than drawing a control.
//! [`focus_stop`] is a tab stop on a focus handle its caller owns, with a role, a name and the keyboard's ring, for a
//! place where focus rests (the session pane's interrupted card, home while it shows), which must be able to be home
//! before its first frame. [`scroll_stop`] is the same stop with a handle of its own plus the scroll keys, for a region
//! that scrolls and holds nothing focusable (`scroll_box.rs` has the full account), and [`scroll_stop_on`] is that stop
//! on a handle its caller owns, for a region that scrolls and is also where focus rests (the session pane's unavailable
//! card). None of them draws hover help.

mod banner;
mod button;
mod checkbox;
mod copy_button;
mod copy_field;
mod custom_flags;
mod dialog;
mod divider;
mod list_row;
mod mark_toggle;
mod menu_header;
mod menu_item;
mod menu_keys;
mod menu_separator;
mod option_row;
mod pulse;
mod radio_group;
mod rail_entry;
mod scroll_box;
mod scrollbar;
mod select;
mod sequence_pill;
mod spinner;
mod status_card;
mod status_mark;
mod tab_strip;
mod text_input;
mod transition;

#[cfg(test)]
mod tests;

pub use banner::Banner;
pub use button::{Button, CONDENSED_GAP, Condensed, IconButton, SplitPart, Tier};
pub use checkbox::Checkbox;
pub use copy_button::{copy_button, copy_button_then};
pub use copy_field::{CONFIRMATION, COPIED, CopyFeedback, CopyField};
pub use custom_flags::CustomFlags;
pub use dialog::DialogFrame;
pub(crate) use dialog::dialog_shadow;
pub use divider::{Divider, DividerDrag, DividerGrip, DividerStep};
pub use gpui::Role;
pub use list_row::ListRow;
pub use mark_toggle::MarkToggle;
pub use menu_header::MenuHeader;
pub use menu_item::MenuItem;
pub use menu_keys::{MenuStart, home_end_keys, toggle_arrow_keys};
pub use menu_separator::menu_separator;
pub use option_row::OptionRow;
pub use pulse::pulse_opacity;
pub use radio_group::{RadioGroup, RadioOption};
pub use rail_entry::{RAIL_ICON_BOX, rail_entry};
pub use scroll_box::ScrollBox;
pub use scrollbar::{SCROLLBAR_WIDTH, Scrollbar, ScrollbarEvent, ScrollbarLines};
pub use select::{Select, SelectOption};
pub use sequence_pill::{SequenceChip, SequencePill};
pub use spinner::{Spinner, spinner_icon};
pub use status_card::{status_card, status_heading};
pub use status_mark::StatusMark;
pub use tab_strip::{TabOption, TabStrip};
pub use text_input::{EditCommand, TextInput, TextInputEvent, TextInputState, Vertical};
pub use transition::{ENTRANCE, entrance};
pub use transition::{outline_fade, slide_down};

use std::rc::Rc;

use gpui::{
    AnyView, App, ClickEvent, Div, ElementId, FocusHandle, InteractiveElement, KeyDownEvent,
    ParentElement, Pixels, ScrollHandle, SharedString, Stateful, StatefulInteractiveElement,
    Styled as _, Window, div, point, prelude::FluentBuilder as _, px,
};

use crate::focus;
use crate::host_text::{HostSentence, Line};
use crate::layers::{self, BesideCues, Help, Hint, TooltipOwner};
use crate::theme;

/// What a control says when the user hovers it, moves keyboard focus onto it, or asks assistive technology about it.
///
/// Every control has one: the layer manager shows it as a tooltip on hover and on keyboard focus ([`with_hover_help`]),
/// and it doubles as the control's accessible description. SPEC_impl.md lets a control that already shows its
/// explanation beside itself (a menu item with a description line) count that text as its hover help instead. That
/// form is not built: [`MenuItem`], the first control to draw such a description, takes a tooltip as well and makes its
/// description line its accessible description.
///
/// Help is the app's own wording ([`HoverHelp::tooltip`]), a sentence that names host values beside it
/// ([`HoverHelp::sentence`]), or several such sentences as paragraphs ([`HoverHelp::paragraphs`], for help such as the
/// agent mark's and the permission shield's, which runs past the short tooltip's eight lines). A host value goes in
/// only as a sentence. Help formatted from [`HostText::raw`](crate::host_text::HostText::raw) would reach the screen
/// unescaped and with its direction unisolated, which SPEC_impl.md, "Host-supplied text", rules out; `tooltip` takes
/// any string, so that is held by review rather than by the type, as for a dialog's title. Rich help that is not
/// words, the session row's summary card, is a view of the caller's on a card of fixed width ([`HoverHelp::card`]).
///
/// Help sits against its control, centered above it or else below, as the old app's ordinary tooltip did. Any form can
/// ask to sit beside the sidebar instead ([`HoverHelp::beside`]), as SPEC.md has every session-row hover and the host
/// progress card do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverHelp {
    help: Help,
    /// The cues of help placed beside the sidebar; `None` for ordinary help, placed against the control.
    beside: Option<BesideCues>,
}

impl HoverHelp {
    /// Ordinary help in the given form.
    fn of(help: Help) -> Self {
        HoverHelp { help, beside: None }
    }

    /// Help shown as a tooltip, in the app's own words.
    pub fn tooltip(text: impl Into<SharedString>) -> Self {
        HoverHelp::of(Help::Line(Line::App(text.into())))
    }

    /// Help that names host values: `sentence` mixes the app's words with them, and the tooltip draws it as a
    /// [`HostSentence`] (each host value escaped, in a text element of its own). The control's accessible description
    /// is the sentence's [`HostSentence::accessible_name`]: the same words with each host value escaped, which is what
    /// assistive technology reads for host text everywhere else. Inside the tooltip the sentence is one labelled node
    /// under the id it was made with, as a [`HostSentence`] is wherever it is drawn.
    pub fn sentence(sentence: HostSentence) -> Self {
        HoverHelp::of(Help::Line(Line::Sentence(sentence)))
    }

    /// Help in paragraphs, one per sentence, for help whose closing sentences must stay visible (the agent mark's and
    /// the permission shield's, as SPEC.md's "Session list" describes them: the agent, what its permission means and
    /// the caveat after it, the stored command). The tooltip is the old app's paragraph help: up to 440 pixels wide
    /// rather than 280, a blank line's gap between paragraphs, and no eight-line cut ([`Help::Paragraphs`] has the
    /// old values and where they come from).
    ///
    /// Each paragraph is drawn as a [`HostSentence`] is anywhere: one labelled node under its own id, host values
    /// escaped, so the ids must differ from each other. An app-only paragraph is a sentence with only
    /// [`HostSentence::text`] parts. A line break inside a paragraph is drawn as one only in a paragraph of the app's
    /// own words: a sentence lays its parts out as one wrapping row, so a break before a host part would leave that
    /// part beside the first line rather than on a line of its own; give such a part its own paragraph. The control's
    /// accessible description is every paragraph's accessible name, a blank line between them
    /// ([`Help::accessible_text`]), so assistive technology hears the whole help.
    ///
    /// ```ignore
    /// let help = HoverHelp::paragraphs([
    ///     HostSentence::new("agent-help-mode").text("Codex — Guarded · Codex sandboxed auto"),
    ///     HostSentence::new("agent-help-command").text("Launched with: ").mono(&command),
    /// ]);
    /// ```
    pub fn paragraphs(paragraphs: impl IntoIterator<Item = HostSentence>) -> Self {
        HoverHelp::of(Help::Paragraphs(paragraphs.into_iter().collect()))
    }

    /// Help that is a view of the caller's on a card `width` wide (less in a narrower window), with no padding and no
    /// line clamp: the session row's summary card, which the old app drew at `min(310px, 100vw - 8px)`
    /// ([`Help::card`]). `text` is the control's accessible description and what the tooltip reports
    /// ([`layers::ShownTooltip::text`]), since the view has no words to read; it should say what the card shows. It is
    /// a line, so the session's title or host goes in as a host sentence, escaped as in any other help.
    ///
    /// The view is drawn only by the tooltip, only while the card shows, and drawn again whenever the overlay is. Never
    /// also draw the same entity inline: two places drawing one view in a frame share its element state. Hand the same
    /// entity every frame, so the help stays the same while the control is unchanged. A view that should not live
    /// while nothing draws it goes in through [`HoverHelp::card_with`] instead.
    pub fn card(width: Pixels, view: impl Into<AnyView>, text: impl Into<Line>) -> Self {
        HoverHelp::of(Help::card(width, view, text))
    }

    /// [`HoverHelp::card`] for a view that costs something for as long as it lives, made only when the tooltip shows:
    /// `build` makes it then (not when the help is made or while the hover waits), the tooltip draws that one view in
    /// every frame while it shows, and drops it when it hides or another tooltip replaces it. It is for the host
    /// progress card, whose view holds the ticker's clock and a running step's pulse, so one per row would keep
    /// them running while nothing draws them. It works beside the sidebar too ([`HoverHelp::beside`]).
    ///
    /// `build` may capture what the view shows (the host it is about): each render's help replaces the hovered
    /// control's stored copy, so a tooltip that begins later builds from the latest render's `build`, while a re-render
    /// never makes the view or touches a tooltip already waiting or showing.
    pub fn card_with(
        width: Pixels,
        build: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
        text: impl Into<Line>,
    ) -> Self {
        HoverHelp::of(Help::card_with(width, build, text))
    }

    /// This help, placed beside the sidebar: 6 pixels past the edge the sidebar draws ([`layers::beside_edge`]),
    /// centered on this control, with a notch pointing back at it and a ring just inside its bounds, or against the
    /// control without either cue in a window too narrow for that (`layers/beside.rs` has the numbers). It is for
    /// SPEC.md's session-row hovers and host progress card, and nothing else. The control needs nothing more: its
    /// bounds are recorded as part of wiring its help ([`with_hover_help`]). The ring takes the accent and square
    /// corners unless [`HoverHelp::ring_on_fill`] or [`HoverHelp::ring_radius`] say otherwise.
    ///
    /// The control must be drawn inside the sidebar column that draws the edge. The old app checked that itself (its
    /// target had to be inside `.app-sidebar`, or it got the ordinary placement); here the help is placed against the
    /// edge wherever the control is, so a control elsewhere would get a card at the sidebar's edge, away from it.
    pub fn beside(mut self) -> Self {
        self.beside.get_or_insert_default();
        self
    }

    /// Beside help ([`HoverHelp::beside`], which this implies) whose ring takes the on-fill color: for a control in a
    /// selected row, which already wears the accent fill, as the old ring did under `.session-row.selected`.
    pub fn ring_on_fill(mut self) -> Self {
        self.beside.get_or_insert_default().on_fill = true;
        self
    }

    /// Beside help ([`HoverHelp::beside`], which this implies) whose ring has the control's own corner radius: half
    /// the size of a round status dot, the control's radius for a rounded button. The default is 0, square corners,
    /// where the old inset-shadow ring followed the control's corners by itself, so a rounded control passes its
    /// radius.
    pub fn ring_radius(mut self, radius: Pixels) -> Self {
        self.beside.get_or_insert_default().radius = radius;
        self
    }

    /// The control's accessible description: the help as one string, with every host value escaped
    /// ([`Help::accessible_text`]). Private to the component layer, like the help itself.
    fn description(&self) -> SharedString {
        self.help.accessible_text()
    }
}

/// Give `element`, the control drawn under `id`, its hover help: the tooltip the layer manager shows while the pointer
/// is over it, owned by this control so it closes when the control is gone, and the help as the control's accessible
/// description where the element can carry one ([`HoverHelp::description`]: for help naming a host value, the
/// sentence's text with each host value escaped, and for paragraphs, every paragraph's).
///
/// Every control wires its help through here, and nothing else can: the help's text is private to this module, so a
/// control that skipped this would never read its own help, and the unused field fails the build (`-D warnings`). That
/// is what keeps a future control from silently dropping its tooltip.
///
/// A control may sit inside another that has help of its own (a mark inside a row), and the innermost one's help wins
/// ([`layers::hover_began`], "Nested hover help"). The move listener wired here exists for that rule: it tells the
/// layer manager the order GPUI drew the hovered controls in. So a container does not need its help moved onto a
/// separate surface to keep it from covering its children's.
///
/// Every render hands the layer manager the help as drawn ([`layers::help_drawn`]), so an outer control's help that
/// shows later is what it says then, not what it said when the pointer arrived. A press of any mouse button on the
/// control puts its help away while the pointer rests there ([`layers::pointer_pressed`]), in the capture phase so a
/// child that stops the press (a text field's input) cannot hide it.
///
/// The move listener makes every control with help a mouse listener in GPUI's driver record, which the test bridge
/// reads: in its snapshots a status mark is an interactive entry, and a row button holding marks lists them under it
/// rather than being a leaf.
///
/// Help is placed against the control's own bounds, centered above it or else below, or beside the sidebar for help
/// that asks for it ([`HoverHelp::beside`]), so while the control's help waits or shows this also adds a zero-cost
/// child that records them in every frame ([`TooltipOwner::track`]); that is why the element must take children. A
/// control whose help is not in play draws no such child: [`layers::help_drawn`] says when it is, and why the bounds
/// are in place by the time the help shows.
///
/// `focus` is the handle of the control's own keyboard stop, or `None` for a control that never holds focus. Its help
/// also shows when the keyboard puts focus there ([`layers::focus_drawn`], which says how that and the pointer share
/// the tooltip), as the old app showed every focus-visible control's help, placed against the control from the bounds
/// the focus coordinator records for the stop. Every caller says which, so a focusable control cannot leave its
/// keyboard users without its help by omission. A control that is disabled passes its handle all the same: the layer
/// manager's record of the control last seen holding focus is cleared only by that control drawn without it, so a
/// control disabled under focus and given `None` would stay recorded, and focus handed back to it from code in the
/// update that enables it again would not be seen landing. (A Tab back cannot meet this: Tab reaches the control only
/// after a frame has drawn it enabled and unfocused, which clears the record.) Focus inside the control on another stop of its own does
/// not count: only `focus` itself.
#[allow(
    clippy::too_many_arguments,
    reason = "one control's help wiring, each part given once; a struct would only repeat them"
)]
pub(crate) fn with_hover_help<E: StatefulInteractiveElement + ParentElement>(
    element: E,
    id: &SharedString,
    help: &HoverHelp,
    describe: bool,
    focus: Option<&FocusHandle>,
    window: &mut Window,
    cx: &mut App,
) -> E {
    let owner = TooltipOwner::for_control(
        ElementId::Name(format!("{id}-tooltip-owner").into()),
        window,
        cx,
    );
    let element = if describe {
        element.aria_description(help.description())
    } else {
        element
    };
    let help = match help.beside {
        Some(cues) => Hint::beside(help.help.clone(), cues),
        None => Hint::from(help.help.clone()),
    };
    // The bounds tracker only while this control's help waits or shows, so an idle frame draws nothing for it.
    let element = if layers::help_drawn(&owner, &help, window, cx) {
        element.child(owner.read(cx).track())
    } else {
        element
    };
    if let Some(focus) = focus {
        layers::focus_drawn(&owner, &help, focus, window, cx);
    }
    let moved = owner.clone();
    element
        .on_hover(move |hovered, window, cx| {
            if *hovered {
                layers::hover_began(&owner, help.clone(), window, cx);
            } else {
                layers::hover_ended(&owner, window, cx);
            }
        })
        .capture_any_mouse_down(|_, window, cx| layers::pointer_pressed(window, cx))
        .on_mouse_move(move |_, window, cx| layers::hover_moved(&moved, window, cx))
}

/// A control's activation handler: what a click, or Enter or Space while the control has focus, does.
pub(crate) type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Make a pointer click on `element` run `handler`: the click half of the controls' activation rule, beside
/// [`activate_on_enter_or_space`]'s keys. GPUI also reports the release of an Enter or Space pressed on the focused
/// element as a click; that one is ignored here, because the key handler already ran on the press, and running again
/// on the release would act twice for one key press wherever focus stays on the control.
///
/// The held key is the case that matters most. The key handler skips a held key's repeats, but GPUI arms its click on
/// a repeat too, so without this filter a held Enter that answered one question would answer the next question its
/// focus landed on when the key came up, which SPEC.md rules out ("Repeated or composing Enter cannot answer the new
/// question").
///
/// Only the keyboard click is dropped. A screen reader's activation and the test bridge's `click` both arrive as a
/// pointer press and release at the control, so they still run `handler`; do not narrow the filter further to catch
/// them.
pub(crate) fn activate_on_click<E: StatefulInteractiveElement>(
    element: E,
    handler: Option<Handler>,
) -> E {
    match handler {
        Some(handler) => element.on_click(move |event, window, cx| {
            if !event.is_keyboard() {
                handler(event, window, cx);
            }
        }),
        None => element,
    }
}

/// Make Enter and Space activate `element`, the focused control, the way a click does: the key rule the buttons and
/// menu items share, so they cannot drift apart on it. (The checkbox takes Space only, as a native checkbox did.)
///
/// A held key does not repeat the activation and a modified key (Shift+Enter, say) is not an activation: SPEC.md,
/// "Repeated or composing Enter cannot answer the new question", whose case is a held Enter that confirmed a question
/// and would otherwise answer the next one too. The key stops at the control that handled it, so the same press does
/// not also reach an action bound further up. A disabled control passes `None` and handles no keys.
pub(crate) fn activate_on_enter_or_space<E: InteractiveElement>(
    element: E,
    handler: Option<Handler>,
) -> E {
    element.on_key_down(move |event, window, cx| {
        let key = event.keystroke.key.as_str();
        if event.is_held || event.keystroke.modifiers.modified() {
            return;
        }
        if (key == "enter" || key == "space")
            && let Some(handler) = &handler
        {
            cx.stop_propagation();
            handler(&ClickEvent::default(), window, cx);
        }
    })
}

/// A dialog's main action (the launcher's Launch), which Enter on one of its choice controls runs through
/// [`enter_choice`].
pub(crate) type MainAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// The choice controls' Enter rule: with the owner's main action given, a plain Enter on the focused choice applies it
/// and then runs that action, so a launcher's Enter launches from any of its choices. The segments and chips of a
/// button group, plain radios, the checkbox and the closed select all go through here, and so do an area's own
/// controls through [`on_enter_choice`], so they cannot drift apart on it. It is the old launcher's `enter_choice`
/// (`launch_controls.rs` at reference commit `24bc0580`), which SPEC.md's launcher describes control by control.
///
/// `apply` is what the choice under focus means: choosing a segment or a tab. A control that shows a value rather than
/// a pending choice (a radio, a checkbox, a closed select) passes a no-op, so the action runs with the values shown,
/// which is what the old launcher did for its YOLO radios, its Resume checkbox and its selects. Space and a click never
/// come here: they only choose, as before.
///
/// `main` runs right after `apply` returns and reads what `apply` wrote, so `apply` must change the owner's state
/// directly (an entity update, a cell). A choice sent through `window.dispatch_action` runs only after this handler,
/// since GPUI defers it, and `main` would launch with the previous choice.
///
/// It knows nothing of the owner's busy state, which the old `enter_choice` checked: `main` must refuse a run already
/// in flight itself (the launcher's Launch must ignore an Enter while it is busy). A disabled control takes no keys,
/// so it never gets here.
///
/// Returns whether the key was this rule's. It is not, and the control's own keys decide, when `main` is `None` (no
/// owner opted in, so Enter keeps each control's own behavior), when the key is not Enter, or when Enter is modified:
/// a Shift+Enter is not an activation under the shared key rule ([`activate_on_enter_or_space`]), where the old
/// `enter_choice` ignored modifiers and launched on it.
///
/// A held Enter is the rule's too, and it does nothing but stop there: a held key's repeats neither run the action
/// again nor reach what holds the control (SPEC.md: "A held Enter never launches"). A composing Enter cannot reach
/// these controls at all, since none of them has an input handler for an input method to compose into.
pub(crate) fn enter_choice(
    event: &KeyDownEvent,
    main: Option<&MainAction>,
    apply: impl FnOnce(&mut Window, &mut App),
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(main) = main else {
        return false;
    };
    if event.keystroke.key != "enter" || event.keystroke.modifiers.modified() {
        return false;
    }
    cx.stop_propagation();
    if !event.is_held {
        apply(window, cx);
        main(window, cx);
    }
    true
}

/// Give an area's own choice control the choice controls' Enter rule ([`enter_choice`]): a plain Enter while it has
/// focus runs `apply` and then `main`, a held Enter does nothing, and a modified Enter or any other key goes on to the
/// element's other key listeners. It is for the launcher's Agent and Command tabs, which the launcher draws itself
/// (SPEC_impl.md, "The component layer and theme"); this layer's controls take the rule through their `on_enter`
/// builders. As with [`enter_choice`], `apply` must write the owner's state directly, because `main` reads it right
/// after; a choice sent through `window.dispatch_action` would reach `main` too late.
///
/// It handles keys only. Pair it with [`activate_on_click`] for the control's click, which ignores the click GPUI makes
/// from the released Enter; a plain `on_click` would apply the choice a second time on that release. Put it before the
/// element's own key listener, since GPUI stops at the first listener on an element that takes the key.
///
/// ```ignore
/// let tab = div().id("launch-kind-agent"); // and its focus tracking, role and label
/// let tab = components::on_enter_choice(
///     tab,
///     move |window, cx| choose_tab(LaunchTab::Agent, window, cx),
///     move |window, cx| launch(window, cx),
/// )
/// .on_key_down(move |event, window, cx| tab_strip_arrows(event, window, cx));
/// ```
pub(crate) fn on_enter_choice<E: InteractiveElement>(
    element: E,
    apply: impl Fn(&mut Window, &mut App) + 'static,
    main: impl Fn(&mut Window, &mut App) + 'static,
) -> E {
    let main: MainAction = Rc::new(main);
    element.on_key_down(move |event, window, cx| {
        enter_choice(event, Some(&main), &apply, window, cx);
    })
}

/// The scroll handle kept under `key` for as long as its owner (a frame, a scroll box) is drawn in consecutive frames,
/// so a scrolled body stays where it was. `key` must differ from the scrolling element's own id, because GPUI keys
/// element state by path and type.
fn scroll_handle(key: String, window: &mut Window, cx: &mut App) -> ScrollHandle {
    window
        .use_keyed_state(ElementId::Name(key.into()), cx, |_, _| ScrollHandle::new())
        .read(cx)
        .clone()
}

/// WebKit's scroll by one line for an arrow key (`Scrollbar::pixelsPerLineStep`), a scroll stop's Up and Down.
const LINE_STEP: Pixels = px(40.);

/// WebKit's scroll by one page, as a share of the visible height (`Scrollbar::minFractionToStepWhenPaging`), a scroll
/// stop's Page Up, Page Down and Space.
const PAGE_SHARE: f32 = 0.875;

/// Where a focus stop draws its ring ([`focus_stop`], and so every scroll stop): `inset_x` from the stop's padding box
/// at its sides and `inset_y` at its top and bottom (negative to cover a border, or to reach past the stop), with
/// corners of `radius`. The two differ only for a dialog's notices, whose ring reaches out to the card's sides as the
/// old full-width region's did ([`DialogFrame::notices_stop`]).
pub(crate) struct Ring {
    pub(crate) inset_x: Pixels,
    pub(crate) inset_y: Pixels,
    pub(crate) radius: Pixels,
}

/// Make `stop`, the box drawn under `id`, a focus stop on `focus`, a handle its caller owns: a tab stop with `role` and
/// `label` as its accessible role and name, and `ring` while the keyboard has put focus on it. It takes no keys. The
/// ring, role and Tab reach every stop shares, scroll stops included ([`scroll_stop`] is this plus the scroll keys), so
/// they cannot drift apart.
///
/// Use it for a stop that is a place for focus to rest rather than a region to scroll: a session pane's interrupted
/// card, which is home while it shows (SPEC.md, "Where focus rests"). The handle is the caller's, made with
/// `.tab_stop(true)`, because such a stop must be able to be home before it is first drawn: the pane makes the card
/// home ([`focus::set_home`]) as it learns the card is to show, and a handle kept in the stop's element state would not
/// exist until the card's first frame. Focus reaches it once a frame draws it, since the coordinator gives no focus to
/// a home the frame on screen does not draw. Use [`scroll_stop`] instead for a region whose content scrolls and holds
/// nothing focusable (SPEC.md, "Reaching everything by keyboard"), whose handle is its own, or [`scroll_stop_on`] for
/// such a region that is also where focus rests.
///
/// It makes nothing focusable but `focus` and draws nothing focusable inside the stop. Controls the caller draws inside
/// it (the card's buttons) keep their own stops, which come after it in Tab order, since GPUI orders Tab by paint
/// order, and their own keys, since it takes none; its ring shows only while the stop itself holds focus. Like a scroll
/// stop it counts as a text field for Tab reach (`focus::OnPress::Focus`, which also has a press of any mouse button on
/// the stop's own space focus it), and it draws no hover help: the caller adds its own if it wants one.
#[allow(
    clippy::too_many_arguments,
    reason = "one stop's parts, each drawn once; a struct would only repeat them"
)]
pub(crate) fn focus_stop(
    stop: Stateful<Div>,
    id: &str,
    focus: &FocusHandle,
    role: Role,
    label: SharedString,
    ring: Ring,
    window: &Window,
    cx: &App,
) -> Stateful<Div> {
    debug_assert!(focus.tab_stop, "a focus stop's handle must be a tab stop");
    let palette = theme::palette(cx);
    let ringed = focus::shows_ring(focus, window);
    let stop = stop
        .relative()
        .role(role)
        .aria_label(label)
        .when(ringed, |stop| {
            stop.child(
                div()
                    .debug_selector(|| format!("{id}-ring"))
                    .absolute()
                    .top(ring.inset_y)
                    .left(ring.inset_x)
                    .right(ring.inset_x)
                    .bottom(ring.inset_y)
                    .border_2()
                    .border_color(theme::color(palette.accent))
                    .rounded(ring.radius),
            )
        });
    focus::track(stop, focus, focus::OnPress::Focus)
}

/// Make `stop`, the box drawn under `id` around the scrolling element whose scroll state is `scroll`, a scroll stop: a
/// [`focus_stop`] with `role`, `label` and `ring`, plus the scroll keys. The rule every scroll stop shares (the scroll
/// box's, the dialog body's and the dialog notices'), so their keys cannot drift apart; `scroll_box.rs`'s module docs
/// say what it does and why the stop wraps the scrolling element rather than being it. Its focus handle is its own,
/// kept under `id`.
///
/// It handles no hover help: the box takes its own, as its constructor does, while a dialog body takes none, since a
/// tooltip over a whole body would cover what the user is reading.
#[allow(
    clippy::too_many_arguments,
    reason = "one stop's parts, each drawn once; a struct would only repeat them"
)]
pub(crate) fn scroll_stop(
    stop: Stateful<Div>,
    id: &SharedString,
    scroll: &ScrollHandle,
    role: Role,
    label: SharedString,
    ring: Ring,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let focus = focus::control_handle(ElementId::Name(format!("{id}-focus").into()), window, cx);
    scroll_stop_on(stop, id, &focus, scroll, role, label, ring, window, cx)
}

/// [`scroll_stop`] on `focus`, a handle its caller owns, made with `.tab_stop(true)`: the same keys, ring, role and Tab
/// reach, for a region that both scrolls and is a place where focus rests. The session pane's unavailable card is one:
/// it holds nothing focusable and its remedy can run past a short pane, so it needs the scroll keys, and it is home
/// while it shows, so the pane must be able to make it home before its first frame (focus reaches it on that frame),
/// which a handle kept in the stop's element state cannot do ([`focus_stop`] says why). The scroll box is the other: it
/// hands the same handle to its hover help, which shows on keyboard focus of it. Everything else takes
/// [`scroll_stop`], whose handle is its own.
#[allow(
    clippy::too_many_arguments,
    reason = "one stop's parts, each drawn once; a struct would only repeat them"
)]
pub(crate) fn scroll_stop_on(
    stop: Stateful<Div>,
    id: &SharedString,
    focus: &FocusHandle,
    scroll: &ScrollHandle,
    role: Role,
    label: SharedString,
    ring: Ring,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let keys = scroll.clone();
    let focused = focus.clone();
    // GPUI hands a key to every element on the focused element's path, so a control inside the stop (a link in a box)
    // would scroll it too: the keys are the stop's only while the stop itself holds focus.
    let stop = stop.on_key_down(move |event, window, cx| {
        if !focused.is_focused(window) {
            return;
        }
        if let Some(top) = scrolled_to(event, &keys) {
            cx.stop_propagation();
            keys.set_offset(point(keys.offset().x, top));
            window.refresh();
        }
    });
    focus_stop(stop, id, focus, role, label, ring, window, cx)
}

/// Let Page Up, Page Down, Home and End scroll `region`, the scrolling element whose scroll state is `scroll`, while a
/// control inside it holds focus: the browser's scrolling from a focused button, which WebKit applied to the button's
/// scrolling container. It is the other half of a region that holds controls, where [`scroll_stop`] is no stop (and
/// would refuse its keys anyway, since they are its own only while it holds focus itself): without it, a question's
/// text scrolled above view stays there while one of its answers holds focus.
///
/// Only those four keys, and only unmodified: a narrower port than WebKit, which also scrolled on Up and Down from a
/// focused button. The arrows are left out because some controls pass them on rather than stopping them (a closed
/// select passes a held Up or Down on), so they would leak into scrolling; Space presses a button. A control that takes
/// one of the four keeps it: a text field keeps all four, which GPUI Kit binds, because bindings run before any key
/// listener, and a listener deeper in the tree that takes the key and stops it (a tab strip's Home and End) runs before
/// this one. A held key keeps scrolling, as the stop's do. It scrolls this region only: at its end the key goes
/// nowhere, where WebKit went on to scroll the next container out.
pub(crate) fn scroll_from_controls(region: Stateful<Div>, scroll: &ScrollHandle) -> Stateful<Div> {
    let keys = scroll.clone();
    region.on_key_down(move |event, window, cx| {
        if !matches!(
            event.keystroke.key.as_str(),
            "pageup" | "pagedown" | "home" | "end"
        ) {
            return;
        }
        if let Some(top) = scrolled_to(event, &keys) {
            cx.stop_propagation();
            keys.set_offset(point(keys.offset().x, top));
            window.refresh();
        }
    })
}

/// Where `event`, a key pressed on a scroll stop, puts the scrolling content's offset (zero at the top, negative
/// further down, as GPUI counts it), within what there is to scroll; `None` for a key that does not scroll, or one
/// pressed with a modifier. Held repeats scroll too.
fn scrolled_to(event: &KeyDownEvent, scroll: &ScrollHandle) -> Option<Pixels> {
    if event.keystroke.modifiers.modified() {
        return None;
    }
    let top = scroll.offset().y;
    let bottom = -scroll.max_offset().y;
    let page = scroll.bounds().size.height * PAGE_SHARE;
    let to = match event.keystroke.key.as_str() {
        "up" => top + LINE_STEP,
        "down" => top - LINE_STEP,
        "pageup" => top + page,
        "pagedown" | "space" => top - page,
        "home" => px(0.),
        "end" => bottom,
        _ => return None,
    };
    Some(to.clamp(bottom, px(0.)))
}
