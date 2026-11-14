//! The single-line text input, over GPUI Kit's `Input`.
//!
//! Kit supplies the hard part (editing, selection, undo, input methods), which is why SPEC_impl.md keeps Kit for text
//! inputs. Two things are Farhelm's: the label and hover help every control must have, and Kit's built-in right-click
//! menu, which is turned off because the old app's release builds had no right-click menu on text (browser-behavior
//! inventory, entry B1). The only way to get an input's state is [`TextInputState::new`], so no input can skip that.
//!
//! An editor learns what happens in the field through [`TextInputState::subscribe`]: every edit the user makes
//! ([`TextInputEvent::Changed`], which a draft mirrors) and Enter ([`TextInputEvent::Submitted`], which saves). Kit's
//! own event type stays behind this module, so the rest of the app never matches on Kit's variants.
//!
//! NOTE: Kit's single-line input drops line breaks. Its `InputState` removes every `\n` and `\r` from whatever is
//! inserted (typing, an input method, a paste, `set_text`; `normalize_input` and `insert_clipboard` in gpui-base
//! 0.9.1), and offers no hook to keep them. The old Rename field was a one-row textarea precisely so that a pasted line
//! feed reached the supervisor, which refuses it, rather than being repaired by the client (`rename.rs` at reference
//! commit `24bc0580`). Keeping them here would take Kit's multi-line `TextareaState` set up as one row (`auto_grow(1,
//! 1)`, which also keeps Tab moving focus, no soft wrap, `submit_on_enter`, and Shift+Enter, which Kit turns into a
//! line break, taken away from it), as a variant of this component with a golden of its own. That is not built; until
//! it is, a pasted multi-line title arrives without its line breaks.
//!
//! NOTE: Enter's event comes from Kit's binding, which runs on every auto-repeat, so a held Enter submits again on each
//! repeat. An editor that saves through the operation store is refused while its save is in flight, which covers the
//! Rename editor; SPEC.md's "Repeated or composing Enter cannot answer" a question is the prompt's, not this field's.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, ElementId, Entity, Focusable as _, InteractiveElement,
    IntoElement, ParentElement, RenderOnce, SharedString, Styled, Subscription, Window, div,
};
use gpui_component::input::{Input, InputEvent, InputState};

use super::HoverHelp;
use crate::theme::{self, metrics};

/// What happened in a text input, for the view that owns its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputEvent {
    /// The user changed the text: typed, deleted, pasted, undid. Raised only when the text differs from what it was at
    /// the last `Changed` or [`TextInputState::set_text`]: not by `set_text` itself, so a pushed value an editor puts
    /// in the field never counts as the user's edit, and not by an edit call that left the text as it was. Kit reports
    /// a change on every edit call, including ones that change nothing (a pasted lone line feed, which the single-line
    /// field strips), and an editor mirroring that into its draft would mark an untouched draft touched.
    ///
    /// The tests meet one such call on every Enter. GPUI's simulated keystrokes give Enter a line feed as its typed
    /// character (gpui-pre 0.3.12's `Keystroke::with_simulated_ime`); Kit's single-line field reports `Submitted` and
    /// lets the key go on, so GPUI hands the line feed to the field, which strips it and inserts nothing. In tests,
    /// Enter with text selected in the field therefore deletes the selection, which a real keyboard does not do on
    /// Linux, where Enter carries no typed character; what macOS sends back for it was not established.
    Changed,
    /// The user pressed Enter in the field (with or without Shift or the platform modifier): the editor's main action.
    Submitted,
}

/// The editable state behind a [`TextInput`]: the text, the selection and the undo history, owned by the view that
/// shows the input for as long as it shows it.
///
/// Clones share the state, and with it the text as last reported or set, which [`TextInputEvent::Changed`] is judged
/// against.
#[derive(Clone)]
pub struct TextInputState(Entity<InputState>, Rc<RefCell<SharedString>>);

impl TextInputState {
    /// A new, empty input state with Kit's right-click menu turned off.
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        TextInputState(
            cx.new(|cx| InputState::new(window, cx).context_menu(false)),
            Rc::new(RefCell::new(SharedString::default())),
        )
    }

    /// The current text.
    pub fn text(&self, cx: &App) -> SharedString {
        self.0.read(cx).value()
    }

    /// Replace the whole text, as when a dialog opens prefilled with the current value. Not an edit: it joins no undo
    /// history and raises no change event.
    ///
    /// What [`TextInputEvent::Changed`] is judged against afterwards is the text the field then holds, not `text`: the
    /// single-line field strips line breaks from what it is given (see the module docs).
    pub fn set_text(&self, text: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        let text: SharedString = text.into();
        self.0
            .update(cx, |state, cx| state.set_value(text, window, cx));
        self.1.replace(self.text(cx));
    }

    /// Call `on_event` for what happens in this input, for as long as the returned subscription is kept: the view that
    /// owns the state keeps it beside the state. Focus changes are not reported; the focus coordinator owns focus.
    ///
    /// Subscribe once per state. Subscriptions share the record of the last reported text, so a second one would miss
    /// the changes the first had already reported.
    pub fn subscribe<V: 'static>(
        &self,
        window: &mut Window,
        cx: &mut Context<V>,
        on_event: impl Fn(&mut V, TextInputEvent, &mut Window, &mut Context<V>) + 'static,
    ) -> Subscription {
        let last = self.1.clone();
        cx.subscribe_in(
            &self.0,
            window,
            move |view, input, event: &InputEvent, window, cx| {
                let event = match event {
                    InputEvent::Change => {
                        let text = input.read(cx).value();
                        if *last.borrow() == text {
                            return;
                        }
                        last.replace(text);
                        TextInputEvent::Changed
                    }
                    InputEvent::PressEnter { .. } => TextInputEvent::Submitted,
                    InputEvent::Focus | InputEvent::Blur => return,
                };
                on_event(view, event, window, cx);
            },
        )
    }
}

/// A labeled single-line text input. Its accessible role is Kit's text-input role, the one role an editable line has,
/// so the constructor takes no role.
///
/// NOTE: Kit's `Input` takes an accessible name but no accessible description, so the hover help reaches assistive
/// technology only as the tooltip, unlike a button's. That is a gap in Kit, recorded rather than worked around.
#[derive(IntoElement)]
pub struct TextInput {
    id: SharedString,
    state: TextInputState,
    label: SharedString,
    help: HoverHelp,
}

impl TextInput {
    /// An input showing `state`. `label` is its accessible name; `help` its hover help.
    pub fn new(
        id: impl Into<SharedString>,
        state: &TextInputState,
        label: impl Into<SharedString>,
        help: HoverHelp,
    ) -> Self {
        TextInput {
            id: id.into(),
            state: state.clone(),
            label: label.into(),
            help,
        }
    }
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        // Not described: the wrapper is not the input's accessible node, and Kit's input takes no description.
        let wrapper = div().id(ElementId::Name(self.id.clone())).w_full();
        // Kit tracks the input's focus itself; the coordinator only records where it is drawn, so a field can take over
        // focus when the control beside it disappears.
        let handle = self.state.0.read(cx).focus_handle(cx);
        let wrapper = crate::focus::record_position(wrapper, &handle);
        super::with_hover_help(wrapper, &self.id, &self.help, false, window, cx).child(
            // Kit fills a dark-mode input with its border color at 30%, not the old stylesheet's recessed `--well`,
            // and rounds it at its general radius; these override that with the old input's fill, corner and type
            // size. Kit's padding and height stay (a few pixels taller than the old 4-by-8 padding).
            Input::new(&self.state.0)
                .aria_label(self.label)
                .bg(theme::color(palette.well))
                .rounded(metrics::RADIUS_SM)
                .text_size(metrics::TEXT_MD),
        )
    }
}
