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

mod banner;
mod button;
mod checkbox;
mod dialog;
mod divider;
mod list_row;
mod mark_toggle;
mod menu_item;
mod option_row;
mod radio_group;
mod scrollbar;
mod select;
mod status_mark;
mod text_input;

#[cfg(test)]
mod tests;

pub use banner::Banner;
pub use button::{Button, IconButton, Tier};
pub use checkbox::Checkbox;
pub use dialog::DialogFrame;
pub(crate) use dialog::dialog_shadow;
pub use divider::{Divider, DividerDrag, DividerGrip, DividerStep};
pub use gpui::Role;
pub use list_row::ListRow;
pub use mark_toggle::MarkToggle;
pub use menu_item::MenuItem;
pub use option_row::OptionRow;
pub use radio_group::{RadioGroup, RadioOption};
pub use scrollbar::{SCROLLBAR_WIDTH, Scrollbar, ScrollbarEvent, ScrollbarLines};
pub use select::{Select, SelectOption};
pub use status_mark::StatusMark;
pub use text_input::{TextInput, TextInputEvent, TextInputState, Vertical};

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, KeyDownEvent, SharedString,
    StatefulInteractiveElement, Window,
};

use crate::host_text::{HostSentence, Line};
use crate::layers::{self, TooltipOwner};

/// What a control says when the user hovers it or asks assistive technology about it.
///
/// Every control has one: the layer manager shows it as a tooltip on hover, and it doubles as the control's accessible
/// description. SPEC_impl.md lets a control that already shows its explanation beside itself (a menu item with a
/// description line) count that text as its hover help instead. That form is not built: [`MenuItem`], the first control
/// to draw such a description, takes a tooltip as well and makes its description line its accessible description.
///
/// Help is the app's own wording ([`HoverHelp::tooltip`]) or a sentence that names host values beside it
/// ([`HoverHelp::sentence`], for help such as the agent mark's and the permission shield's). A host value goes in only
/// the second way. Help formatted from [`HostText::raw`](crate::host_text::HostText::raw) would reach the screen
/// unescaped and with its direction unisolated, which SPEC_impl.md, "Host-supplied text", rules out; `tooltip` takes
/// any string, so that is held by review rather than by the type, as for a dialog's title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverHelp(Line);

impl HoverHelp {
    /// Help shown as a tooltip, in the app's own words.
    pub fn tooltip(text: impl Into<SharedString>) -> Self {
        HoverHelp(Line::App(text.into()))
    }

    /// Help that names host values: `sentence` mixes the app's words with them, and the tooltip draws it as a
    /// [`HostSentence`] (each host value escaped, in a text element of its own). The control's accessible description
    /// is the sentence's [`HostSentence::accessible_name`]: the same words with each host value escaped, which is what
    /// assistive technology reads for host text everywhere else. Inside the tooltip the sentence is one labelled node
    /// under the id it was made with, as a [`HostSentence`] is wherever it is drawn.
    pub fn sentence(sentence: HostSentence) -> Self {
        HoverHelp(Line::Sentence(sentence))
    }

    /// The control's accessible description: the help as one string, with every host value escaped
    /// ([`Line::accessible_text`]). Private to the component layer, like the help itself.
    fn description(&self) -> SharedString {
        self.0.accessible_text()
    }
}

/// Give `element`, the control drawn under `id`, its hover help: the tooltip the layer manager shows while the pointer
/// is over it, owned by this control so it closes when the control is gone, and the help as the control's accessible
/// description where the element can carry one ([`HoverHelp::description`]: for help naming a host value, the
/// sentence's text with each host value escaped).
///
/// Every control wires its help through here, and nothing else can: the help's text is private to this module, so a
/// control that skipped this would never read its own help, and the unused field fails the build (`-D warnings`). That
/// is what keeps a future control from silently dropping its tooltip.
pub(crate) fn with_hover_help<E: StatefulInteractiveElement>(
    element: E,
    id: &SharedString,
    help: &HoverHelp,
    describe: bool,
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
    let help = help.0.clone();
    element.on_hover(move |hovered, window, cx| {
        if *hovered {
            layers::show_tooltip(&owner, help.clone(), window, cx);
        } else {
            layers::hide_tooltip(&owner, window, cx);
        }
    })
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the launcher's tab strip is its first caller; until it is built only the tests call it"
    )
)]
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
