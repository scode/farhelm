//! The text input, over GPUI Kit's single-line `Input`; for a field that must keep pasted line breaks, Kit's
//! multi-line `Textarea` held to one row; and for multi-line text, Kit's `Textarea` growing with its text up to a cap.
//!
//! Kit supplies the hard part (editing, selection, undo, input methods), which is why SPEC_impl.md keeps Kit for text
//! inputs. Two things are Farhelm's: the label and hover help every control must have, and Kit's built-in right-click
//! menu, which is turned off because the old app's release builds had no right-click menu on text (browser-behavior
//! inventory, entry B1). The only ways to get an input's state are [`TextInputState::new`],
//! [`TextInputState::new_keeping_line_breaks`] and [`TextInputState::new_growing`], so no input can skip that.
//!
//! An editor learns what happens in the field through [`TextInputState::subscribe`]: every edit the user makes
//! ([`TextInputEvent::Changed`], which a draft mirrors) and Enter ([`TextInputEvent::Submitted`], which saves). Kit's
//! own event type stays behind this module, so the rest of the app never matches on Kit's variants. Every kind of field
//! reports edits the same way and is drawn by the same [`TextInput`]; only the growing box reports no Enter.
//!
//! ## Line breaks
//!
//! Kit's single-line input drops line breaks. Its `InputState` removes every `\n` and `\r` from whatever is inserted
//! (typing, an input method, a paste, `set_text`; `normalize_input` and `insert_clipboard` in gpui-base 0.9.1), and
//! offers no hook to keep them: its paste hook can only swallow a paste or let Kit insert it, stripped. That is right
//! for a filter, and wrong for a field whose value a supervisor judges. SPEC.md refuses a supplied session title that
//! holds a newline, and the old Rename field was a one-row textarea precisely so that a pasted line feed reached the
//! supervisor, which refuses it, rather than being repaired by the client (`rename.rs` at reference commit
//! `24bc0580`).
//!
//! [`TextInputState::new_keeping_line_breaks`] is that field. It is Kit's `TextareaState` set up to behave as a single
//! line, as the old field's stylesheet set up its textarea (`.rename-input` at the same commit): one row tall whatever
//! it holds, no soft wrap, so a long title scrolls sideways, and Enter and Shift+Enter submit instead of breaking the
//! line. Tab still moves focus, because Kit indents only in a fixed-height text area, and this one is an auto-growing
//! one held at one row. Pressing keys therefore cannot make a line break; text that arrives whole (a paste,
//! `set_text`, an input method's committed text) keeps any it holds. Like the old field, it then shows one line of the
//! value at a time, with the others out of view. The old stylesheet chose that on purpose ("a horizontal scroll for
//! the pasted case rather than a growing box"), so this field does not grow either.
//!
//! ## The growing box
//!
//! [`TextInputState::new_growing`] is the multi-line field: a launch's first message, a template's prompt, a recipe or
//! a credential. The old app's were `<textarea>`s with a drag-to-resize corner, which is not rebuilt (decided by the
//! maintainer on 2026-11-13); instead the box grows with its text, from the rows its caller starts it at to
//! [`GROWING_MAX_ROWS`], and then scrolls. Lines wrap softly at the box's width. Enter and Shift+Enter type a line
//! break; it reports no [`TextInputEvent::Submitted`], so an editor saves it with its Save. Tab moves focus, as it did
//! out of the old textarea; a tab character pasted or set into the value stays in it, as do line breaks (SPEC.md for
//! the template prompt: "Newline and tab remain editable").
//!
//! NOTE: A tab character advances to the next stop two spaces apart, Kit's default, where the old textarea used the
//! browser's default of eight (the old stylesheet set no `tab-size`). Kit offers a tab size only for its fixed-height
//! and code-editor layouts, not for a box that grows, so this is a recorded deviation rather than something this field
//! corrects. A tab just short of a stop draws almost no gap, which is why the golden scene leaves tabs out.
//!
//! NOTE: A lone `\r` is kept but is not a line break in Kit's layout; it draws as whatever the font has for it. A
//! pasted `\r\n` breaks the line at its `\n` and keeps the `\r` at the end of the first line.
//!
//! NOTE: Enter's event comes from Kit's binding, which runs on every auto-repeat, so a held Enter submits again on each
//! repeat. An editor that saves through the operation store is refused while its save is in flight, which covers the
//! Rename editor; SPEC.md's "Repeated or composing Enter cannot answer" a question is the prompt's, not this field's.
//!
//! ## Up and Down
//!
//! Kit binds Up and Down in its input context to its own caret moves, and its single-line field then does nothing with
//! them, so the keys stop at the field and never reach its ancestors (SPEC_impl.md, "GPUI cookbook: traps", "Kit
//! details"). A field that drives a list beside it (the quick switcher's search, whose arrows choose a result while
//! typing stays in the field) takes them with [`TextInput::on_vertical`], which catches Kit's two actions on their way
//! down to the field, before Kit's own handler runs, and hands them to the field's owner instead. Kit's action types
//! stay inside this module, as its event type does.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, ElementId, Entity, FocusHandle, Focusable as _,
    InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Subscription,
    Window, div,
};
use gpui_component::input::{
    Enter, Input, InputEvent, InputState, MoveDown, MoveUp, Textarea, TextareaState,
};

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
    /// The tests meet one such call on every Enter in the single-line field. GPUI's simulated keystrokes give Enter a
    /// line feed as its typed character (gpui-pre 0.3.12's `Keystroke::with_simulated_ime`); Kit's single-line field
    /// reports `Submitted` and lets the key go on, so GPUI hands the line feed to the field, which strips it and
    /// inserts nothing. In tests, Enter with text selected in that field therefore deletes the selection, which a real
    /// keyboard does not do on Linux, where Enter carries no typed character; what macOS sends back for it was not
    /// established. The field that keeps line breaks stops Enter's typed character before it reaches the field, so
    /// Enter never edits it.
    Changed,
    /// The user pressed Enter in the field (with or without Shift or the platform modifier): the editor's main action.
    /// Never reported by a growing box ([`TextInputState::new_growing`]), where Enter types a line break.
    Submitted,
}

/// Which of Kit's fields holds the text.
#[derive(Clone)]
enum Field {
    /// Kit's single-line input, which strips line breaks.
    Line(Entity<InputState>),
    /// Kit's multi-line text area behaving as a single line, which keeps them (see the module docs, "Line breaks").
    Row(Entity<TextareaState>),
    /// Kit's multi-line text area growing with its text up to a cap (see the module docs, "The growing box").
    Area(Entity<TextareaState>),
}

/// How tall a growing box ([`TextInputState::new_growing`]) gets before it scrolls, in rows of text. The old boxes had
/// no cap (their resize corner let the user drag them taller); twelve rows keeps a long prompt readable inside a dialog
/// whose body scrolls anyway.
pub(crate) const GROWING_MAX_ROWS: usize = 12;

/// The editable state behind a [`TextInput`]: the text, the selection and the undo history, owned by the view that
/// shows the input for as long as it shows it.
///
/// Clones share the state, and with it the text as last reported or set, which [`TextInputEvent::Changed`] is judged
/// against, and whether the field is read-only ([`TextInputState::set_read_only`]).
#[derive(Clone)]
pub struct TextInputState(Field, Rc<RefCell<SharedString>>, Rc<Cell<bool>>);

impl TextInputState {
    /// A new, empty single-line input state with Kit's right-click menu turned off. Line breaks inserted in any way
    /// are dropped (see the module docs).
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        TextInputState(
            Field::Line(cx.new(|cx| InputState::new(window, cx).context_menu(false))),
            Rc::new(RefCell::new(SharedString::default())),
            Rc::new(Cell::new(false)),
        )
    }

    /// A new, empty input state that keeps line breaks a paste or [`Self::set_text`] puts in it, for a field whose
    /// value something else must judge as given (the Rename title). It has the same events and is drawn by the same
    /// [`TextInput`] as [`Self::new`]'s, and it too is one row tall; it differs in keeping line breaks (the module
    /// docs, "Line breaks"). Typing cannot make one: Enter and Shift+Enter submit.
    pub fn new_keeping_line_breaks(window: &mut Window, cx: &mut App) -> Self {
        let area = cx.new(|cx| {
            let mut area = TextareaState::new(window, cx)
                .auto_grow(1, 1)
                .submit_on_enter(true)
                .context_menu(false);
            area.set_soft_wrap(false, window, cx);
            area
        });
        TextInputState(
            Field::Row(area),
            Rc::new(RefCell::new(SharedString::default())),
            Rc::new(Cell::new(false)),
        )
    }

    /// A new, empty multi-line box `rows` rows tall that grows with its text to [`GROWING_MAX_ROWS`] and then scrolls,
    /// wrapping lines at its width (the module docs, "The growing box"). `rows` is where the old app's `<textarea>`
    /// started (a launch's first message 3, a template's prompt 4, a credential 8); a `rows` above the cap is held to
    /// it. Enter types a line break rather than submitting.
    pub fn new_growing(rows: usize, window: &mut Window, cx: &mut App) -> Self {
        let rows = rows.clamp(1, GROWING_MAX_ROWS);
        let area = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(rows, GROWING_MAX_ROWS)
                .context_menu(false)
        });
        TextInputState(
            Field::Area(area),
            Rc::new(RefCell::new(SharedString::default())),
            Rc::new(Cell::new(false)),
        )
    }

    /// The field's focus handle, for a dialog whose focus starts in the field (the Rename editor's title field:
    /// SPEC.md, "initial focus remains on the title field"), handed to the layer manager as the dialog's start. Kit
    /// tracks the field's focus itself, so this is the handle the field takes focus through; moving focus to it stays
    /// the focus coordinator's business, as for every handle.
    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.0 {
            Field::Line(input) => input.read(cx).focus_handle(cx),
            Field::Row(area) | Field::Area(area) => area.read(cx).focus_handle(cx),
        }
    }

    /// Make the field read-only, or editable again. A read-only field keeps its look, focus, caret, selection and copy,
    /// and refuses only the user's edits; [`Self::set_text`] still replaces its text. For an editor whose value is
    /// being saved (the Rename editor holds its field read-only for the save's round trip, as the old app's field was
    /// `readonly`), so the text on screen stays the text sent without the caret or the undo history being disturbed.
    ///
    /// The flag lives here as well as in Kit's state because Kit's element puts its own `readonly` setting into the
    /// state every time it is drawn: set only on the state, it lasted until the next frame. [`TextInput`] draws the
    /// element with this flag; setting the state too covers keys that arrive before that frame.
    pub fn set_read_only(&self, read_only: bool, cx: &mut App) {
        self.2.set(read_only);
        match &self.0 {
            Field::Line(input) => input.update(cx, |state, cx| state.set_readonly(read_only, cx)),
            Field::Row(area) | Field::Area(area) => {
                area.update(cx, |state, cx| state.set_readonly(read_only, cx))
            }
        }
    }

    /// The current text.
    pub fn text(&self, cx: &App) -> SharedString {
        match &self.0 {
            Field::Line(input) => input.read(cx).value(),
            Field::Row(area) | Field::Area(area) => area.read(cx).value(),
        }
    }

    /// Replace the whole text, as when a dialog opens prefilled with the current value. Not an edit: it joins no undo
    /// history and raises no change event.
    ///
    /// What [`TextInputEvent::Changed`] is judged against afterwards is the text the field then holds, not `text`: the
    /// single-line field strips line breaks from what it is given (see the module docs).
    pub fn set_text(&self, text: impl Into<SharedString>, window: &mut Window, cx: &mut App) {
        let text: SharedString = text.into();
        match &self.0 {
            Field::Line(input) => input.update(cx, |state, cx| state.set_value(text, window, cx)),
            Field::Row(area) | Field::Area(area) => {
                area.update(cx, |state, cx| state.set_value(text, window, cx))
            }
        }
        self.1.replace(self.text(cx));
    }

    /// Show `text` in the field while it is empty, as a hint of what goes there (the quick switcher's "Jump to a
    /// session"). It is the app's own wording, never a value: it raises no event, and [`Self::text`] never returns it.
    pub fn set_placeholder(
        &self,
        text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let text: SharedString = text.into();
        match &self.0 {
            Field::Line(input) => {
                input.update(cx, |state, cx| state.set_placeholder(text, window, cx))
            }
            Field::Row(area) | Field::Area(area) => {
                area.update(cx, |state, cx| state.set_placeholder(text, window, cx))
            }
        }
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
        // Kit emits its Enter event in a growing box too, after typing the line break; there it is an edit, not a
        // submission. Exhaustive, so a new kind decides whether Enter submits it.
        let submits = match self.0 {
            Field::Line(_) | Field::Row(_) => true,
            Field::Area(_) => false,
        };
        // Kit's two states are different types emitting the same event, so each arm reads its own text and hands the
        // rest to this.
        let report = move |view: &mut V,
                           event: &InputEvent,
                           text: SharedString,
                           window: &mut Window,
                           cx: &mut Context<V>| {
            let event = match event {
                InputEvent::Change => {
                    if *last.borrow() == text {
                        return;
                    }
                    last.replace(text);
                    TextInputEvent::Changed
                }
                InputEvent::PressEnter { .. } if submits => TextInputEvent::Submitted,
                InputEvent::PressEnter { .. } => return,
                InputEvent::Focus | InputEvent::Blur => return,
            };
            on_event(view, event, window, cx);
        };
        match &self.0 {
            Field::Line(input) => cx.subscribe_in(
                input,
                window,
                move |view, input, event: &InputEvent, window, cx| {
                    report(view, event, input.read(cx).value(), window, cx)
                },
            ),
            Field::Row(area) | Field::Area(area) => cx.subscribe_in(
                area,
                window,
                move |view, area, event: &InputEvent, window, cx| {
                    report(view, event, area.read(cx).value(), window, cx)
                },
            ),
        }
    }
}

/// A labeled text input, of whichever kind its state was made as. Its accessible role is the one Kit gives its field,
/// so the constructor takes no role: the single-line input is a text input, and the field that keeps line breaks is a
/// multi-line text input, as the old Rename field's `<textarea>` was (although Enter submits it), and so is the growing
/// box.
///
/// NOTE: Kit's `Input` takes an accessible name but no accessible description, so the hover help reaches assistive
/// technology only as the tooltip, unlike a button's. That is a gap in Kit, recorded rather than worked around.
#[derive(IntoElement)]
pub struct TextInput {
    id: SharedString,
    state: TextInputState,
    label: SharedString,
    help: HoverHelp,
    /// Who takes Up and Down instead of Kit, if anyone ([`TextInput::on_vertical`]).
    on_vertical: Option<VerticalHandler>,
}

/// What a field hands Up and Down to ([`TextInput::on_vertical`]).
type VerticalHandler = Rc<dyn Fn(Vertical, &mut Window, &mut App)>;

/// Which arrow a field handed to its owner ([`TextInput::on_vertical`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vertical {
    Up,
    Down,
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
            on_vertical: None,
        }
    }

    /// Hand Up and Down to `handler` instead of letting Kit take them (the module docs, "Up and Down"). Every press and
    /// held repeat reaches it, and none moves the caret or reaches anything else, so the field keeps its text, caret
    /// and focus while its owner moves a selection beside it.
    ///
    /// For single-line fields ([`TextInputState::new`]) only: on a field that keeps line breaks it would take Up and
    /// Down away from moving between its lines.
    pub fn on_vertical(
        mut self,
        handler: impl Fn(Vertical, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_vertical = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TextInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        // Not described: the wrapper is not the input's accessible node, and Kit's input takes no description.
        let wrapper = div().id(ElementId::Name(self.id.clone())).w_full();
        // Kit tracks the input's focus itself; the coordinator only records where it is drawn, so a field can take over
        // focus when the control beside it disappears.
        let handle = self.state.focus_handle(cx);
        let wrapper = crate::focus::record_position(wrapper, &handle);
        let wrapper = super::with_hover_help(wrapper, &self.id, &self.help, false, window, cx);
        // Caught on the way down, in the capture phase, because Kit's own handler on the field consumes both actions
        // and stops them there (the module docs, "Up and Down").
        let wrapper = match self.on_vertical {
            Some(up) => {
                let down = up.clone();
                wrapper
                    .capture_action(move |_: &MoveUp, window, cx| {
                        cx.stop_propagation();
                        up(Vertical::Up, window, cx);
                    })
                    .capture_action(move |_: &MoveDown, window, cx| {
                        cx.stop_propagation();
                        down(Vertical::Down, window, cx);
                    })
            }
            None => wrapper,
        };
        // Kit fills a dark-mode input with its border color at 30%, not the old stylesheet's recessed `--well`, and
        // rounds it at its general radius; these override that with the old input's fill, corner and type size. Kit's
        // padding and height stay (a few pixels taller than the old 4-by-8 padding). Kit sizes its text area from its
        // line height and padding instead of the single-line input's fixed height; with the old Rename field's line
        // height it is 35 pixels high, as that field was (`metrics::RENAME_LINE_HEIGHT`).
        match self.state.0 {
            Field::Line(input) => wrapper.child(
                Input::new(&input)
                    .readonly(self.state.2.get())
                    .aria_label(self.label)
                    .bg(theme::color(palette.well))
                    .rounded(metrics::RADIUS_SM)
                    .text_size(metrics::TEXT_MD),
            ),
            Field::Row(area) => wrapper
                // Kit's text area breaks the line on Shift+Enter even when Enter submits. Caught on the way down,
                // before Kit's own handler on the field sees it, it is sent on as a plain Enter, which Kit submits.
                .capture_action(move |action: &Enter, window, cx| {
                    if action.shift {
                        let plain = Enter {
                            secondary: action.secondary,
                            shift: false,
                        };
                        handle.dispatch_action(&plain, window, cx);
                        cx.stop_propagation();
                    }
                })
                // Kit submits Enter and lets the key go on, as its single-line field does, and GPUI then types the
                // key's character, a line feed where the platform gives Enter one (the simulated test platform
                // does), which this field would keep. Stopping every Enter here, after the field's own handling,
                // keeps typing from making a line break whatever the modifiers.
                .on_key_down(|event, _, cx| {
                    if event.keystroke.key == "enter" {
                        cx.stop_propagation();
                    }
                })
                .child(
                    Textarea::new(&area)
                        .readonly(self.state.2.get())
                        .aria_label(self.label)
                        .bg(theme::color(palette.well))
                        .rounded(metrics::RADIUS_SM)
                        .text_size(metrics::TEXT_MD)
                        .line_height(metrics::RENAME_LINE_HEIGHT),
                ),
            // The old multi-line boxes' look (`.templates-field textarea`: the well, the small radius, 13-pixel text at
            // a line height of 1.45); Kit sizes the box from its rows and that line height. Each old box had its own
            // padding and minimum height (the launcher's first message: 8 by 12 pixels, at least 80 high); this kind
            // has one look, and a consumer that needs its box's own adds a builder for it.
            Field::Area(area) => wrapper.child(
                Textarea::new(&area)
                    .readonly(self.state.2.get())
                    .aria_label(self.label)
                    .bg(theme::color(palette.well))
                    .rounded(metrics::RADIUS_SM)
                    .text_size(metrics::TEXT_MD)
                    .line_height(metrics::GROWING_LINE_HEIGHT),
            ),
        }
    }
}
