//! The text input, over GPUI Kit's single-line `Input`; for a field that must keep pasted line breaks, Kit's
//! multi-line `Textarea` held to one row; and for multi-line text, Kit's `Textarea` growing with its text up to a cap.
//!
//! Kit supplies the hard part (editing, selection, undo, input methods), which is why SPEC_impl.md keeps Kit for text
//! inputs. Two things are Farhelm's: the label and hover help every control must have, and Kit's built-in right-click
//! menu, which is turned off because the old app's release builds had no right-click menu on text (the GPUI
//! migration's browser behaviors survey, `docs/gpui/browser-behaviors.md` at commit `d352ceb3`, since removed; item B1). The only ways to get an input's state are [`TextInputState::new`],
//! [`TextInputState::new_masked`], [`TextInputState::new_keeping_line_breaks`] and [`TextInputState::new_growing`], so
//! no input can skip that.
//!
//! An editor learns what happens in the field through [`TextInputState::subscribe`]: every edit the user makes
//! ([`TextInputEvent::Changed`], which a draft mirrors) and Enter ([`TextInputEvent::Submitted`], which saves). Kit's
//! own event type stays behind this module, so the rest of the app never matches on Kit's variants. Every kind of field
//! reports edits the same way and is drawn by the same [`TextInput`]; only the growing box reports no Enter. Each Enter
//! says whether it was a held key's repeat (the section "A held Enter" below). A state takes one subscriber at a time,
//! which debug builds check ([`TextInputState::subscribe`] says why).
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
//! ## A held Enter
//!
//! Enter's event comes from Kit's binding, which runs on every auto-repeat, so a held Enter submits again on each
//! repeat. The field does not drop those repeats, since that would change every editor at once: one that saves through
//! the operation store is refused while its save is in flight, which covers the Rename editor, and SPEC.md's "Repeated
//! or composing Enter cannot answer" a question is the prompt's, not this field's. It says instead which Enter was a
//! repeat ([`TextInputEvent::Submitted`]'s `held`), so an owner whose main action must run once per press (the
//! launcher's: "A held Enter never launches") skips the held ones, as the checkbox, the radio group and the select skip
//! held keys.
//!
//! Only a key-down says whether it was held, and Kit's binding runs before any key listener sees it (SPEC_impl.md,
//! "GPUI cookbook: traps", "Key bindings beat key listeners"). [`TextInput`] records the Enter key-down as it passes
//! the field's wrapper, after Kit has emitted its event; the event reaches the owner's subscription only when GPUI
//! flushes effects after the whole keystroke ("Effects flush only after the outermost update returns"), and reads the
//! record then.
//!
//! NOTE: Shift+Enter in the field that keeps line breaks is turned into a plain Enter at the action, which then stops
//! the keystroke, so no key listener sees its key-down. It is reported as not held, a held one included. No owner of
//! that field yet runs an action that must ignore a held Enter.
//!
//! ## Up and Down
//!
//! Kit binds Up and Down in its input context to its own caret moves, and its single-line field then does nothing with
//! them, so the keys stop at the field and never reach its ancestors (SPEC_impl.md, "GPUI cookbook: traps", "Kit
//! details"). A field that drives a list beside it (the quick switcher's search, whose arrows choose a result while
//! typing stays in the field) takes them with [`TextInput::on_vertical`], which catches Kit's two actions on their way
//! down to the field, before Kit's own handler runs, and hands them to the field's owner instead. Kit's action types
//! stay inside this module, as its event type does.
//!
//! ## The sequence prefix
//!
//! A text field keeps the quick-launch sequence prefix, Cmd+X on a Mac and Ctrl+Shift+X on Linux, as its own key
//! (`crate::actions`, `TEXT_FIELD_KEYS`), except that SPEC.md has the session launcher's fields begin a sequence with
//! it when nothing is selected ("In the launcher's own text fields it starts a sequence only when no text is selected;
//! with a selection it keeps Cut"). [`TextInput::on_unselected_prefix`] is that exception, for the launcher's fields
//! alone. On a Mac the chord is Kit's Cut binding, so the field catches Kit's Cut action on its way down, before Kit's
//! handler runs, and takes it only when nothing is selected; with a selection Kit cuts as usual. A Cut from the Edit
//! menu is the same action, so with nothing selected it begins a sequence too, as its Cmd+X would. On Linux the chord
//! is not Cut (that is Ctrl+X, which this leaves alone) and Kit binds it to nothing, so the field takes the chord's
//! key-down instead, again only with nothing selected; with a selection the chord keeps a field's ordinary meaning,
//! which in Kit's fields is none.
//!
//! ## The Edit menu
//!
//! The Mac app's Edit menu has one action per item, whatever holds focus ([`EditCommand`]). Its Copy, Paste and Select
//! All are the terminal's actions, so that they reach a focused terminal, and [`TextInput`] turns those three back into
//! Kit's own for its field on their way up from it. A field therefore answers the menu as it answers Cmd+C, Cmd+V and
//! Cmd+A, and no key binding changes on either platform.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use farhelm_input_mapping::Platform;
use gpui::{
    App, AppContext as _, Context, ElementId, Entity, FocusHandle, Focusable as _,
    InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Styled,
    Subscription, Window, div, prelude::FluentBuilder as _,
};
use gpui::{KeybindingKeystroke, Keystroke};

use crate::actions::{self, THIS_PLATFORM};
use gpui_component::input::{
    self, Cut, Enter, Input, InputEvent, InputState, MoveDown, MoveUp, Textarea, TextareaState,
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
    /// Never reported by a growing box ([`TextInputState::new_growing`]), where Enter types a line break. A subscriber
    /// that treats a held Enter like any other matches `Submitted { .. }`.
    Submitted {
        /// Whether this Enter was an auto-repeat of a key already down: the platform's `is_held`, which the checkbox,
        /// the radio group and the select read the same way. That is so wherever the key's first press went, so a held
        /// Enter that carried focus into this field from another control is held here too. Reported, never acted on:
        /// whether a held Enter may run the editor's action is the editor's to decide (the module docs, "A held
        /// Enter"). Always `false` for a Shift+Enter in the field that keeps line breaks
        /// ([`TextInputState::new_keeping_line_breaks`]), whose key-down no listener sees (the module docs' NOTE).
        held: bool,
    },
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
/// against, whether the field is read-only ([`TextInputState::set_read_only`]), and the held flag of the last Enter
/// key-down that passed [`TextInput`] and no submission has read yet (the module docs, "A held Enter"). An Enter that
/// submits nothing, such as Alt+Enter, leaves its flag there until the next Enter key-down overwrites it or a
/// submission reads or clears it. They also share how many of the state's subscriptions are kept, since a state takes
/// only one at a time ([`TextInputState::subscribe`]).
#[derive(Clone)]
pub struct TextInputState(
    Field,
    Rc<RefCell<SharedString>>,
    Rc<Cell<bool>>,
    Rc<Cell<Option<bool>>>,
    Rc<Cell<usize>>,
);

impl TextInputState {
    /// A new, empty single-line input state with Kit's right-click menu turned off. Line breaks inserted in any way
    /// are dropped (see the module docs).
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self::line(false, window, cx)
    }

    /// A new, empty single-line input state whose text draws masked, for a secret the user types but nobody should
    /// read over their shoulder (the old `type="password"` fields: the ntfy access token). It is [`Self::new`]'s field
    /// otherwise, with the same events, line-break stripping and right-click menu turned off.
    pub fn new_masked(window: &mut Window, cx: &mut App) -> Self {
        Self::line(true, window, cx)
    }

    /// The single-line field behind [`Self::new`] and [`Self::new_masked`]: Kit's input with its right-click menu off,
    /// its text drawn masked when `masked` is set.
    fn line(masked: bool, window: &mut Window, cx: &mut App) -> Self {
        TextInputState(
            Field::Line(cx.new(|cx| {
                InputState::new(window, cx)
                    .context_menu(false)
                    .masked(masked)
            })),
            Rc::new(RefCell::new(SharedString::default())),
            Rc::new(Cell::new(false)),
            Rc::new(Cell::new(None)),
            Rc::new(Cell::new(0)),
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
            Rc::new(Cell::new(None)),
            Rc::new(Cell::new(0)),
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
            Rc::new(Cell::new(None)),
            Rc::new(Cell::new(0)),
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

    /// Whether the field has no text selected (its caret alone), for a key whose meaning turns on that
    /// ([`TextInput::on_unselected_prefix`]).
    pub(crate) fn selection_empty(&self, cx: &App) -> bool {
        match &self.0 {
            Field::Line(input) => input.read(cx).selected_range().is_empty(),
            Field::Row(area) | Field::Area(area) => area.read(cx).selected_range().is_empty(),
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

    /// Select the whole text, as Kit's own select-all does, so the next key the user types replaces it: for a dialog
    /// that opens on a value the user is likely to replace rather than edit (the Rename editor's naming mode, which
    /// SPEC.md has open with "The actual created name ... focused and fully selected"). Not an edit: no change event.
    pub fn select_all(&self, window: &mut Window, cx: &mut App) {
        match &self.0 {
            Field::Line(input) => input.update(cx, |state, cx| state.select_all(window, cx)),
            Field::Row(area) | Field::Area(area) => {
                area.update(cx, |state, cx| state.select_all(window, cx))
            }
        }
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
    /// One subscriber per state, clones included, at a time. Subscriptions share the record of the last reported text
    /// and the last Enter's held flag, so the first one takes every edit and a second never hears of one (and reads
    /// every Enter as not held). Nothing on screen shows that: both Credentials editors once watched a field whose
    /// [`crate::drafts::DraftInput`] already held its subscription, and an error and a pending file read that an edit
    /// should have cleared stayed. A field a draft owns is watched through the draft.
    ///
    /// Debug builds panic, naming the call that subscribed second, when a new subscription becomes active while another
    /// to the same state is still kept. GPUI activates a subscription only once the outermost update that made it has
    /// returned, so replacing one by assignment (`self.events = state.subscribe(..)`) is fine: the old one is dropped
    /// by then. A detached subscription counts as kept for the rest of the state's life, even after its view is gone,
    /// so detaching is not for a field another view will watch later.
    #[track_caller]
    pub fn subscribe<V: 'static>(
        &self,
        window: &mut Window,
        cx: &mut Context<V>,
        on_event: impl Fn(&mut V, TextInputEvent, &mut Window, &mut Context<V>) + 'static,
    ) -> Subscription {
        let kept = self.4.clone();
        // How many others were kept as this one was made.
        let others = kept.get();
        kept.set(others + 1);
        #[cfg(debug_assertions)]
        let caller = std::panic::Location::caller();
        let last = self.1.clone();
        let held = self.3.clone();
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
                // Taken, so a later Enter whose key-down this field never saw cannot read this one's.
                InputEvent::PressEnter { .. } if submits => TextInputEvent::Submitted {
                    held: held.take().unwrap_or(false),
                },
                InputEvent::PressEnter { .. } => return,
                InputEvent::Focus | InputEvent::Blur => return,
            };
            on_event(view, event, window, cx);
        };
        let events = match &self.0 {
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
        };
        // Checked when GPUI activates the new subscription rather than now: activation is a deferred effect queued
        // just above (`App::new_subscription`), and effects run in order, so this runs right after it. Checked any
        // earlier, a replacement by assignment would still count the subscription it replaces, which is dropped only
        // once this call returns. Only a subscription made while another was kept can fail, so of two the later one
        // does, and the panic names the call that subscribed second even when both were made in one update.
        #[cfg(debug_assertions)]
        {
            let kept = kept.clone();
            cx.defer(move |_| {
                assert!(
                    others == 0 || kept.get() <= 1,
                    "TextInputState::subscribe at {caller}: the field already has a subscription, and a state takes \
                     one subscriber, since a second never hears of an edit. Keep one subscription, or watch a field \
                     a DraftInput owns through the draft"
                )
            });
        }
        // Counted down when the caller drops the subscription handed back, which also drops the callback. A detached
        // subscription is never counted down, although GPUI drops its callback once its view is released.
        Subscription::join(events, Subscription::new(move || kept.set(kept.get() - 1)))
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
    /// Drawn in the monospace face ([`TextInput::monospace`]).
    monospace: bool,
    /// The field's height in place of Kit's ([`TextInput::height`]).
    height: Option<Pixels>,
    /// Drawn with the warning outline ([`TextInput::invalid`]).
    invalid: bool,
    /// What the sequence prefix does in this field with nothing selected, and by which platform's rule, if anything
    /// ([`TextInput::on_unselected_prefix`]).
    on_unselected_prefix: Option<(Platform, PrefixHandler)>,
}

/// What a field runs for the sequence prefix with nothing selected ([`TextInput::on_unselected_prefix`]).
type PrefixHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// What a field hands Up and Down to ([`TextInput::on_vertical`]).
type VerticalHandler = Rc<dyn Fn(Vertical, &mut Window, &mut App)>;

/// Which arrow a field handed to its owner ([`TextInput::on_vertical`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vertical {
    Up,
    Down,
}

/// One of the Mac app's Edit menu commands (Undo, Redo, Cut, Copy, Paste, Select All, as the old app's Edit menu had
/// them), each with the action the menu dispatches for it to whatever holds focus.
///
/// Undo, Redo and Cut dispatch Kit's own text-field actions, so only a focused text field handles them. Copy, Paste and
/// Select All dispatch the terminal's actions ([`actions::CopyTerminalSelection`], [`actions::Paste`],
/// [`actions::SelectAllInTerminal`]), so they act on a focused terminal as its Cmd+C, Cmd+V and Cmd+A do, and every
/// [`TextInput`] handles those three too, by handing Kit's own Copy, Paste or Select All to its field. Either way the
/// item does what the focused element's keys do. GPUI enables a menu item only while something on the focused
/// element's path handles its action: a focused editable field enables all six, a read-only one only Copy and Select
/// All (Kit handles its Paste, Cut, Undo and Redo only while the field is editable, and [`TextInput`] hands Paste on
/// only then too), a focused terminal Copy, Paste and Select All, and anywhere else none.
///
/// Beside each item the menu shows its action's first binding with no context, or failing that its first binding of
/// all. The terminal's three actions have no context-free binding, so on a Mac each shows that fallback, its single row
/// of [`actions::TERMINAL_KEYS`] (⌘C, ⌘V, ⌘A); a test pins those rows. This is the one way
/// Kit's editing actions leave this module, boxed, so the menu never names a Kit type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCommand {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl EditCommand {
    /// The action the Edit menu dispatches for this command, which a focused text field handles for every command and
    /// a focused terminal for Copy, Paste and Select All.
    pub fn action(self) -> Box<dyn gpui::Action> {
        match self {
            Self::Undo => Box::new(input::Undo),
            Self::Redo => Box::new(input::Redo),
            Self::Cut => Box::new(input::Cut),
            Self::Copy => Box::new(actions::CopyTerminalSelection),
            Self::Paste => Box::new(actions::Paste),
            Self::SelectAll => Box::new(actions::SelectAllInTerminal),
        }
    }
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
            monospace: false,
            height: None,
            invalid: false,
            on_unselected_prefix: None,
        }
    }

    /// Draw the text in the monospace face, as the old launcher drew its paths, model ids, wrapper prefix and command
    /// lines (`var(--font-mono)`, JetBrains Mono, which the app bundles for the terminal). The single-line field and
    /// the growing box take it (Settings' post-clone command is a growing box of shell text, the old
    /// `.checkout-post-clone-field textarea`); the field that keeps line breaks ignores it, since no such field has
    /// needed it.
    pub fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }

    /// Make the field `height` tall in place of Kit's 32 pixels, its text centered in it, for a form whose old fields
    /// had a height of their own (the old Add host form's `min-height: 36px`). Only the single-line field takes it, as
    /// with [`TextInput::monospace`].
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    /// Draw the field with a 1-pixel border in `--warn` while `invalid`: a value the feature has checked and found
    /// wrong, which a line beside the field explains (the old `input[aria-invalid="true"]` outline, as on the launcher's
    /// folder after its check). It replaces the focus ring while set, as the old outline did, so the mark stays while
    /// the user is in the field. Only the single-line field takes it; the others ignore it. It is a look, not a refusal: the field stays
    /// editable and the feature decides what the value blocks.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Run `handler` instead of the field's own handling when the sequence prefix is pressed in the field with nothing
    /// selected, and stop the key there (the module docs, "The sequence prefix"); with a selection the field keeps the
    /// chord. For the session launcher's fields only, whose prefix begins a sequence: every other field keeps it.
    pub fn on_unselected_prefix(self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_unselected_prefix_on(THIS_PLATFORM, handler)
    }

    /// [`TextInput::on_unselected_prefix`] by `platform`'s rule rather than this build's, so that both platforms' rules
    /// can be tested on either: on a Mac Kit's Cut action, on Linux the prefix chord's key-down.
    pub(crate) fn on_unselected_prefix_on(
        mut self,
        platform: Platform,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_unselected_prefix = Some((platform, Rc::new(handler)));
        self
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

/// Send Kit's `action` to the field behind `handle`, for an Edit menu command that reached the field's wrapper as the
/// terminal's action ([`EditCommand`]), and end that command there. The stop is explicit because GPUI starts the nested
/// dispatch with propagation on, so the outer command would otherwise go on up past the field whenever nothing in the
/// nested one stopped it.
fn hand_to_field(
    handle: &FocusHandle,
    action: &dyn gpui::Action,
    window: &mut Window,
    cx: &mut App,
) {
    handle.dispatch_action(action, window, cx);
    cx.stop_propagation();
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
        let wrapper = super::with_hover_help(
            wrapper,
            &self.id,
            &self.help,
            false,
            Some(&handle),
            window,
            cx,
        );
        // Whether the Enter is held, for its submission (the module docs, "A held Enter"). In the capture phase, so the
        // field's own listeners cannot stop the key-down before it is recorded.
        let (held, unheld) = (self.state.3.clone(), self.state.3.clone());
        let wrapper = wrapper.capture_key_down(move |event, _, _| {
            if event.keystroke.key == "enter" {
                held.set(Some(event.is_held));
            }
        });
        // The Edit menu's Copy, Paste and Select All dispatch the terminal's actions ([`EditCommand`]), which Kit's
        // field does not handle, so they bubble up to here and go back down to the field as Kit's own. The menu then
        // does in a field what Cmd+C, Cmd+V and Cmd+A do. Paste is handed on only while the field is editable, because
        // Kit handles its Paste only then, and a handler here would enable the menu item over a read-only field.
        let (copy, paste, select_all) = (handle.clone(), handle.clone(), handle.clone());
        let wrapper = wrapper
            .on_action(move |_: &actions::CopyTerminalSelection, window, cx| {
                hand_to_field(&copy, &input::Copy, window, cx)
            })
            .on_action(move |_: &actions::SelectAllInTerminal, window, cx| {
                hand_to_field(&select_all, &input::SelectAll, window, cx)
            })
            .when(!self.state.2.get(), |wrapper| {
                wrapper.on_action(move |_: &actions::Paste, window, cx| {
                    hand_to_field(&paste, &input::Paste, window, cx)
                })
            });
        // The sequence prefix with nothing selected (the module docs, "The sequence prefix"). On a Mac it is Kit's
        // Cut, caught in the capture phase, on its way down to the field, so Kit's own Cut handler never sees one taken
        // here.
        // On Linux it is a key-down listener, because the do-nothing binding for that chord in Kit's text-field context
        // (`actions.rs`, `TEXT_FIELD_KEYS`) hides the app's binding, so no action arrives, while key listeners still
        // see the key.
        let wrapper = match self.on_unselected_prefix {
            Some((Platform::Mac, handler)) => {
                let state = self.state.clone();
                wrapper.capture_action(move |_: &Cut, window, cx| {
                    if state.selection_empty(cx) {
                        cx.stop_propagation();
                        handler(window, cx);
                    }
                })
            }
            Some((Platform::Linux, handler)) => {
                let state = self.state.clone();
                let chord = Keystroke::parse(crate::actions::SEQUENCE_PREFIX_LINUX)
                    .map(KeybindingKeystroke::from_keystroke)
                    .ok();
                wrapper.capture_key_down(move |event, window, cx| {
                    let pressed = chord
                        .as_ref()
                        .is_some_and(|chord| event.keystroke.should_match(chord));
                    if pressed && state.selection_empty(cx) {
                        cx.stop_propagation();
                        handler(window, cx);
                    }
                })
            }
            None => wrapper,
        };
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
                    .text_size(metrics::TEXT_MD)
                    .when(self.monospace, |input| {
                        input.font_family(crate::assets::TERMINAL_FONT)
                    })
                    // Kit's focused style recolors the border with its focus ring, which would hide the mark exactly
                    // while the user is still in the field; the old warning outline won over focus, so the ring goes.
                    .when(self.invalid, |input| {
                        input
                            .border_color(theme::color(palette.warn))
                            .focus_bordered(false)
                    })
                    // Kit centers its text in whatever height the field gets, and a refinement replaces Kit's own
                    // height. Called through `Styled` on purpose: Kit's `Input` has an inherent `h` of its own, for
                    // multi-line fields only, which a single-line field ignores.
                    .when_some(self.height, Styled::h),
            ),
            Field::Row(area) => wrapper
                // Kit's text area breaks the line on Shift+Enter even when Enter submits. Caught on the way down,
                // before Kit's own handler on the field sees it, it is sent on as a plain Enter, which Kit submits.
                .capture_action(move |action: &Enter, window, cx| {
                    if action.shift {
                        // Stopped at the action, so no key-down is recorded: reported as not held, rather than as
                        // whatever an earlier Enter left (the module docs, "A held Enter").
                        unheld.set(None);
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
                    .line_height(metrics::GROWING_LINE_HEIGHT)
                    .when(self.monospace, |area| {
                        area.font_family(crate::assets::TERMINAL_FONT)
                    }),
            ),
        }
    }
}
