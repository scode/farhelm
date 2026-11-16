//! The armed quick-launch sequence in the window: the prefix that arms it, the keys it takes, the pill it shows, what
//! cancels it, and what Enter does with it.
//!
//! SPEC.md, "Creation", "Quick-launch sequences", with "Desktop window chrome" for the prefix and "Losing the connection
//! to the helm" for the reset. The model, with no GPUI types, is [`super::sequence`]; this module turns GPUI's key
//! events into its inputs and draws it. Ported from the old app's armed listener, key classifier and context rule
//! (`assets/template-shortcuts.js`) and its indicator (`LaunchSequenceIndicator` in `launch_sequence.rs`) at reference
//! commit `24bc0580`. The old listener's relays, press snapshots and generation fences are not ported: GPUI hands keys
//! over in order on one thread, and the sequence's own copy of the bindings is what the pill shows and Enter reads.
//!
//! Here's the TLDR:
//!
//! - **The prefix** ([`begin`], the main view's handler of `BeginSequence`) arms a sequence with a copy of the ⚡ mark
//!   and number keys as the store holds them, unless a dialog keeps the keyboard or the window is inert. Text fields
//!   keep the chord (`actions.rs`, `TEXT_FIELD_KEYS`, and GPUI Kit's own Cut on a Mac), except the launcher's with
//!   nothing selected, which dispatch the prefix's action themselves (`launcher::view::launcher_input`), while a
//!   focused terminal hands it on, since it is an app shortcut there.
//! - **The capture.** While armed, the actions module's key capture slot hands every key-down here first, ahead of the
//!   terminal, the bindings and the number-key rows. [`classify`] sorts it: sequence keys feed the model and stop there;
//!   Escape and a second prefix end the sequence and stop there; Enter ends it and resolves it; anything else ends it
//!   and goes on to wherever it would have gone ([`Captured::Cancel`]). Held repeats of a sequence key are taken and do
//!   nothing. Modifiers alone never arrive (GPUI reports them as modifier changes), so they cancel nothing. There is no
//!   timeout.
//! - **Other cancels**: a press anywhere in the window ([`watch_presses`]), the window going inactive, and a reset of
//!   the connection (the slot's reset callback). A dialog that opens while armed (something the user started finishing,
//!   say) cancels at the next key, which then goes to that dialog.
//! - **The pill** is the layer manager's display-only pill, drawn by [`crate::components::SequencePill`] from the model's
//!   view, with the template names read from the store on every draw.
//! - **Enter** routes the model's [`Outcome`]: nothing for an empty sequence, the Templates dialog with its binding hint
//!   for an unbound key, New with the refusal for any other refusal, and the resolved names to the quick-launch
//!   dispatcher ([`super::dispatch`]), which launches them or opens New.
//!
//! - **Inside the launcher**, when it is the dialog holding the keyboard (`launcher::entry::open_launcher`), the prefix
//!   arms the launcher's form of the sequence: the pill says "apply" and sits at the bottom of the main area, and Enter
//!   applies the queue to the launcher's draft or shows the refusal there, opening nothing.
//!
//! NOTE: Input-method text is not refused while armed. A composition's committed text (a CJK input source, a dead key's
//! accent, a Wayland commit of several characters) reaches the focused element's input handler without a key-down, so
//! the capture never sees it, and a focused terminal still sends it to its program. The actions module's NOTE on the
//! slot names the fix, the terminal's input handler dropping text while `actions::key_capture_armed` says so, which is
//! the terminal's code.
//!
//! NOTE: A keypad digit cancels the sequence and goes on to its target, as in the old app ("Numpad digits retain
//! cancellation and pass-through"), so a word never starts with a keypad digit by accident. Only a key that carries its
//! native identity can be told apart: a keypad digit an input method commits, or one the test bridge injects, has none
//! and reads as the main row's digit (`actions::PhysicalKeyId::typed_in`).

use farhelm_input_mapping::Platform;
use farhelm_proto::launcher::LaunchTemplate;
use farhelm_proto::ui_channel::{Availability, Collection};
use gpui::{
    AnyWindowHandle, App, AppContext as _, Context, DispatchPhase, Entity, FocusHandle, Global,
    IntoElement, KeyDownEvent, KeybindingKeystroke, Keystroke, MouseDownEvent, NativeKeyIdentity,
    Render, Styled as _, Subscription, Window, canvas, px,
};

use super::dispatch::{self, Origin, Reference};
use super::sequence::{Chip, Entry, Input, Outcome, Refusal, Sequence, printable};
use super::shortcuts::{Bindings, Unread};
use crate::actions::{
    self, Captured, PhysicalKey, QUICK_LAUNCH_LINUX, QUICK_LAUNCH_MAC, SEQUENCE_PREFIX_LINUX,
    SEQUENCE_PREFIX_MAC, TemplateNumberKey,
};
use crate::components::{SequenceChip, SequencePill};
use crate::desktop::title_band;
use crate::host_text::HostText;
use crate::layers::{self, Anchor, Pill, PillId, PillPlacement};
use crate::session_pane::details;
use crate::{launcher, local_state, session};

/// The pill's words outside the launcher and inside it, the old indicator's.
const LAUNCH_LABEL: &str = "Launch sequence: ";
const LAUNCH_HINT: &str = "Type names or number keys · Tab complete · Enter launch · Esc cancel";
const APPLY_LABEL: &str = "Apply sequence: ";
const APPLY_HINT: &str = "Type names or number keys · Tab complete · Enter apply · Esc cancel";

/// How far the old Mac window moved the centered pill down while a session was open: half the difference between its
/// 48-pixel title band and the web's 30-pixel bars, so the pill stayed centered on the terminal (`app.css`, `top:
/// calc(50% + 9px)` under `.macos-window`).
const MAC_LOWERED: f32 = 9.;

/// The macOS virtual keycodes of the keypad's digits, 0 to 9 (`kVK_ANSI_Keypad0` to `kVK_ANSI_Keypad9`; 8 and 9 skip
/// 0x5A).
const MAC_KEYPAD_DIGITS: [u16; 10] = [0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5B, 0x5C];

// ---- Sorting keys ----------------------------------------------------------------------------------------------------

/// What one key-down means to an armed sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Key {
    /// A key the model takes: a character, a bare number key, a number or ⚡ chord, Space or Backspace.
    Input(Input),
    /// Tab: complete the word against the store's names.
    Tab,
    /// Plain Enter: end the sequence and resolve it.
    Enter,
    /// Escape, with any modifiers: cancel, and take the key.
    Escape,
    /// The prefix pressed again: cancel, and take the key. The old app cancelled the sequence on it and its terminal
    /// dropped the chord, so nothing else happened; here the key would otherwise reach the prefix's binding and arm a
    /// new, empty sequence.
    Prefix,
    /// Anything else (arrows, other chords, Shift+Enter, a key that types nothing, a keypad digit): cancel, and let the
    /// key go on to its normal target.
    Other,
}

/// Sort `event` as typed on `platform`, by the old armed listener's rules, in its order: Escape; a number chord, by
/// physical key; a bare number key, by physical key, with the character the layout prints; the ⚡ chord; plain Enter;
/// the prefix; a keypad digit (which cancels); Space with or without Shift; plain Tab and Backspace; a printable
/// character with nothing but Shift held; and everything else. The platform is a parameter so the Mac's chords can be
/// tested where the suite runs.
///
/// A character is the one the platform reports the key types (`key_char`), never the key's name, so a dead key or a
/// key held mid-composition, which types nothing yet, cancels.
pub(super) fn classify(platform: Platform, event: &KeyDownEvent) -> Key {
    let keystroke = &event.keystroke;
    let held = &keystroke.modifiers;
    // The native event's Shift, where there is one: GPUI drops Shift from a symbol's logical keystroke.
    let shift = event
        .native_key
        .as_ref()
        .map_or(held.shift, |native| native.shift);
    let textual = !held.platform && !held.control && !held.alt && !held.function;
    let bare = textual && !shift;
    let (prefix, mark) = match platform {
        Platform::Mac => (SEQUENCE_PREFIX_MAC, QUICK_LAUNCH_MAC),
        Platform::Linux => (SEQUENCE_PREFIX_LINUX, QUICK_LAUNCH_LINUX),
    };
    let key = keystroke.key.as_str();
    if key == "escape" {
        return Key::Escape;
    }
    if let Some(digit) = actions::physical_on(platform, event).and_then(number_key) {
        return Key::Input(Input::NumberChord(digit));
    }
    if bare && let Some(digit) = actions::physical_key_at(event).and_then(number_key) {
        return match typed(keystroke) {
            Some(text) => Key::Input(Input::Digit { key: digit, text }),
            None => Key::Other,
        };
    }
    if is_chord(keystroke, mark) {
        return Key::Input(Input::MarkChord);
    }
    if bare && key == "enter" {
        return Key::Enter;
    }
    if is_chord(keystroke, prefix) {
        return Key::Prefix;
    }
    if keypad_digit(event) {
        return Key::Other;
    }
    match key {
        "space" if textual => Key::Input(Input::Space),
        "tab" if bare => Key::Tab,
        "backspace" if bare => Key::Input(Input::Backspace),
        _ => match typed(keystroke) {
            Some(c) if textual => Key::Input(Input::Character(c)),
            _ => Key::Other,
        },
    }
}

/// The digit of a number key's row of the physical-key table, as the bindings map spells it; `None` for another row.
fn number_key(row: &PhysicalKey) -> Option<String> {
    row.action
        .as_any()
        .downcast_ref::<TemplateNumberKey>()
        .map(|key| key.digit.to_string())
}

/// The one printable character `keystroke` types, if it types one.
fn typed(keystroke: &Keystroke) -> Option<char> {
    keystroke.key_char.as_deref().and_then(printable)
}

/// Whether `keystroke` is `chord`, matched as GPUI matches a binding for it.
fn is_chord(keystroke: &Keystroke, chord: &'static str) -> bool {
    Keystroke::parse(chord)
        .is_ok_and(|chord| keystroke.should_match(&KeybindingKeystroke::from_keystroke(chord)))
}

/// Whether `event` was typed on one of the keypad's digits, by its native identity (the module docs' NOTE).
fn keypad_digit(event: &KeyDownEvent) -> bool {
    match event.native_key.as_ref().map(|native| &native.identity) {
        Some(NativeKeyIdentity::Xkb(name)) => name
            .strip_prefix("KP")
            .is_some_and(|rest| rest.len() == 1 && rest.as_bytes()[0].is_ascii_digit()),
        Some(NativeKeyIdentity::Mac(code)) => MAC_KEYPAD_DIGITS.contains(code),
        None => false,
    }
}

// ---- Arming and ending -----------------------------------------------------------------------------------------------

/// The sequence armed now: its pill's view (which holds the model), the pill, the window it shows in, the layer that
/// held the keyboard when it began (`None` on the page), and the window-activation observer that cancels it.
struct Armed {
    view: Entity<PillView>,
    /// Why the bindings the sequence copied could not be known when the prefix was accepted, if they could not
    /// ([`Bindings::read`]). The copy is then empty or stale, so Enter refuses a sequence holding a number key or ⚡
    /// rather than resolve them through it, which would call a bound key unbound. Judged at the prefix, not at Enter,
    /// because the copy is what Enter resolves (SPEC.md: "Bindings refresh when the prefix starts").
    unread_bindings: Option<Unread>,
    pill: PillId,
    window: AnyWindowHandle,
    layer: Option<FocusHandle>,
    _activation: Subscription,
}

/// The app's one armed sequence, if any. App-wide, like the key capture slot it holds.
#[derive(Default)]
struct ArmedSlot(Option<Armed>);

impl Global for ArmedSlot {}

/// Whether a sequence is armed.
pub(super) fn is_armed(cx: &App) -> bool {
    cx.try_global::<ArmedSlot>()
        .is_some_and(|slot| slot.0.is_some())
}

/// Begin a sequence for the prefix, its pill centered over `anchor` (the main pane's): the main view's handler of
/// `BeginSequence`. Does nothing while a dialog other than the launcher keeps the keyboard (SPEC.md: "Other visible
/// dialogs keep the keyboard"; under the launcher it arms the launcher's form), while the window is inert, before
/// the main pane was drawn, or while a sequence is armed already. On the page it also does nothing while the launcher
/// is busy (a quick launch being prepared, a launch pending, a refused launch waiting behind its notice) or a host
/// change is in flight, when the old app refused the prefix too and the sequence's Enter could start nothing
/// ([`crate::launcher::entry::quick_launch_available`]); in the open launcher, as for the ⚡ chord there, none of those
/// apply, since the sequence only edits its draft.
///
/// The ⚡ mark and the number keys are copied from the store here, once: SPEC.md's "Bindings refresh when the prefix
/// starts; Enter captures the bound names currently shown in the pill" holds because the pill and Enter both read this
/// one copy. Template names are not copied; they are read from the store whenever they are needed. Bindings that could
/// not be read still arm the sequence, as the old prefix did, since a sequence of words needs none; the copy is
/// whatever the store holds, for the pill to draw, and Enter refuses whatever reads it ([`Armed::unread_bindings`]).
pub(super) fn begin(anchor: Option<Anchor>, window: &mut Window, cx: &mut App) {
    let open = launcher::entry::open_launcher(window, cx);
    let in_launcher = open.is_some();
    // While Save as template's panel is open the prefix changes nothing in the launcher (SPEC.md, "Quick launch": "while
    // saving a template, the chord is consumed without changing the draft"; the old launcher swallowed the prefix then).
    let saving = open.is_some_and(|launcher| launcher.read(cx).save_panel_open());
    if (layers::focus_layer_open(window, cx) && !in_launcher)
        || saving
        || is_armed(cx)
        || session::current_store(cx).is_none()
        || (!in_launcher && !launcher::entry::quick_launch_available(cx))
    {
        return;
    }
    let Some(anchor) = anchor else {
        return;
    };
    let unread_bindings = Bindings::read(cx).err();
    let bindings = Bindings::current(cx);
    let view = cx.new(|_| PillView {
        sequence: Sequence::new(bindings),
        in_launcher,
    });
    let placement = if in_launcher {
        PillPlacement::Bottom
    } else {
        PillPlacement::Centered
    };
    let pill = Pill {
        view: view.clone().into(),
        anchor,
        placement,
    };
    let Some(pill) = layers::show_pill(pill, window, cx) else {
        return;
    };
    // A reset already took the pill down with every overlay (`layers::close_all`); only the state is left to drop.
    let on_reset = |cx: &mut App| cx.default_global::<ArmedSlot>().0 = None;
    if !actions::arm_key_capture(on_key, on_reset, cx) {
        layers::hide_pill(pill, window, cx);
        return;
    }
    let activation = view.update(cx, |_, cx| {
        cx.observe_window_activation(window, |_, window, cx| {
            if !window.is_window_active() {
                end(window, cx);
            }
        })
    });
    let armed = Armed {
        view,
        unread_bindings,
        pill,
        window: window.window_handle(),
        layer: layers::top_container(window, cx),
        _activation: activation,
    };
    cx.default_global::<ArmedSlot>().0 = Some(armed);
}

/// The key capture's handler while a sequence is armed (the module docs' "The capture").
fn on_key(event: &KeyDownEvent, window: &mut Window, cx: &mut App) -> Captured {
    let Some((view, layer)) = cx
        .try_global::<ArmedSlot>()
        .and_then(|slot| slot.0.as_ref())
        .map(|armed| (armed.view.clone(), armed.layer.clone()))
    else {
        // The slot is armed only while a sequence is: nothing to feed, so the key is not this module's.
        return Captured::Cancel;
    };
    if layers::top_container(window, cx) != layer {
        // A dialog opened since the prefix (over the page, or over the launcher the sequence began in): it keeps the
        // keyboard, and this key is its.
        end(window, cx);
        return Captured::Cancel;
    }
    let key = classify(actions::THIS_PLATFORM, event);
    if event.is_held {
        // SPEC.md: "Repeated held keys add nothing and never launch again". A held key that is not the sequence's is
        // one more non-sequence key.
        if key == Key::Other {
            end(window, cx);
            return Captured::Cancel;
        }
        return Captured::Consumed;
    }
    match key {
        Key::Input(input) => view.update(cx, |pill, cx| {
            pill.sequence.input(input);
            cx.notify();
        }),
        Key::Tab => {
            let names = advisory_names(cx);
            view.update(cx, |pill, cx| {
                pill.sequence.tab(names.as_deref());
                cx.notify();
            });
        }
        Key::Enter => {
            if let Some(armed) = end(window, cx) {
                enter(&armed, window, cx);
            }
        }
        Key::Escape | Key::Prefix => {
            end(window, cx);
        }
        Key::Other => {
            end(window, cx);
            return Captured::Cancel;
        }
    }
    Captured::Consumed
}

/// End the armed sequence, if one is: drop it, disarm the key capture and take its pill down. Returns what was armed,
/// for Enter. From inside the capture's handler, the handler's answer still decides what happens to the key at hand.
fn end(window: &mut Window, cx: &mut App) -> Option<Armed> {
    let armed = cx.default_global::<ArmedSlot>().0.take()?;
    actions::disarm_key_capture(cx);
    if window.window_handle() == armed.window {
        layers::hide_pill(armed.pill, window, cx);
    } else {
        let pill = armed.pill;
        let _ = armed
            .window
            .update(cx, |_, window, cx| layers::hide_pill(pill, window, cx));
    }
    Some(armed)
}

/// The sequence's one window-wide listener, drawn once by the main view (a listener a frame registers through
/// `on_mouse_event` sees every press in the window, whatever is under the pointer, a dialog's veil and the pill
/// included): a press of any mouse button cancels an armed sequence (SPEC.md: "a click ... cancels"), in the capture
/// phase, before the press does whatever else it does. Nothing happens while no sequence is armed.
pub(super) fn watch_presses() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |_, _, window, _| {
            window.on_mouse_event(|_: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture && is_armed(cx) {
                    end(window, cx);
                }
            });
        },
    )
    .absolute()
    .size_0()
}

// ---- Enter -----------------------------------------------------------------------------------------------------------

/// Resolve the sequence `armed` held, already ended, against the store's templates, and act on the outcome (SPEC.md,
/// "Quick-launch sequences"). With nothing queued nothing opens. A refusal goes to the dispatcher's one refusal path
/// ([`dispatch::refused`]): on the page an unbound number key or ⚡ opens Templates with its binding hint and any other
/// refusal opens New with its sentence; in the launcher the sequence began in, every refusal, an unbound key's
/// included, is shown there, leaving the draft alone. The resolved names go to the dispatcher with the sequence origin,
/// which launches them directly, opens New with the banner "This sequence doesn't make a complete setup; complete it
/// below", or, in the launcher, applies them to its draft as one refusal unit. While the helm reports its templates
/// unavailable, anything queued is refused as a failed read ([`dispatch::refused_unavailable`]), and so is a sequence
/// holding a number key or ⚡ whose bindings could not be read when it began ([`Armed::unread_bindings`]), in the same
/// words with that reason. All of it happens inside the Enter press's own handling, so what opens opens for this Enter.
fn enter(armed: &Armed, window: &mut Window, cx: &mut App) {
    let sequence = armed.view.read(cx).sequence.clone();
    let reads_bindings = sequence.entries().iter().any(Entry::reads_bindings);
    let outcome = sequence.enter(&current_names(cx));
    // A sequence resolved against templates the helm could not read is refused as a failed read, not by its words.
    if outcome != Outcome::Nothing && dispatch::refused_unavailable(window, cx) {
        return;
    }
    // Its keys and ⚡ were copied from bindings that were not known, so what they name is not known either. A sequence
    // of words alone reads no binding and goes on.
    if reads_bindings && let Some(unread) = &armed.unread_bindings {
        dispatch::refused(&Refusal::from(unread.clone()), window, cx);
        return;
    }
    match outcome {
        Outcome::Nothing => {}
        Outcome::Refused(refusal) => dispatch::refused(&refusal, window, cx),
        Outcome::Apply(names) => {
            let references: Vec<Reference> = names.into_iter().map(Reference::Name).collect();
            dispatch::dispatch(Origin::Sequence, &references, window, cx);
        }
    }
}

// ---- The pill's view -------------------------------------------------------------------------------------------------

/// What the pill draws: the armed sequence's model, and whether it began inside the launcher, which changes its words
/// and its place. It holds the model so that a key's change redraws it like any view (a notify), and it reads the
/// template names from the store each time it draws, so a template added or renamed while the sequence is armed shows
/// in its candidates at once.
pub(super) struct PillView {
    pub(super) sequence: Sequence,
    pub(super) in_launcher: bool,
}

impl Render for PillView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let names = advisory_names(cx);
        let shown = self.sequence.view(names.as_deref());
        let (label, hint) = if self.in_launcher {
            (APPLY_LABEL, APPLY_HINT)
        } else {
            (LAUNCH_LABEL, LAUNCH_HINT)
        };
        let mut pill = SequencePill::new(label, hint);
        for chip in shown.chips {
            pill = pill.chip(match chip {
                Chip::Name(name) => SequenceChip::Name(HostText::from_host(name)),
                Chip::Unbound(words) => SequenceChip::Words(words.into()),
            });
        }
        if !shown.word.is_empty() {
            pill = pill.word(HostText::from_host(shown.word));
        }
        pill = pill.candidates(shown.candidates.into_iter().map(HostText::from_host));
        // One reading for both places, so the Mac lowering and the details bar's room cannot disagree.
        let session_open = local_state::selected(cx).is_some();
        if self.in_launcher {
            let reserved = if session_open {
                px(details::MIN_HEIGHT)
            } else {
                px(0.)
            };
            pill.at_bottom(reserved)
        } else if title_band::ON && session_open {
            pill.lowered(px(MAC_LOWERED))
        } else {
            pill
        }
    }
}

/// The template names for the candidates and Tab, in the helm's order, or `None` when there are none to offer: before
/// the first snapshot, or while the helm says it could not read its templates (SPEC.md: Tab "does nothing while the
/// advisory catalog is loading or unavailable").
fn advisory_names(cx: &App) -> Option<Vec<String>> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    match &collections.meta(Collection::Templates)?.availability {
        Availability::Available => Some(names_in(collections)),
        Availability::Unavailable { .. } => None,
    }
}

/// The template names Enter resolves against: whatever the store holds now, even while the helm says it could not
/// read them again, since the names never gate Enter (SPEC.md) and the store keeps the last ones it had.
fn current_names(cx: &App) -> Vec<String> {
    session::current_store(cx)
        .map(|store| names_in(store.read(cx).collections()))
        .unwrap_or_default()
}

/// Every template's name in `collections`, in the helm's order.
fn names_in(collections: &farhelm_helm_client::Collections) -> Vec<String> {
    collections
        .ordered::<LaunchTemplate>()
        .into_iter()
        .map(|template| template.name.clone())
        .collect()
}

/// The armed sequence's model, for tests that check what a key did to it.
#[cfg(test)]
pub(super) fn armed_sequence(cx: &App) -> Option<Sequence> {
    cx.try_global::<ArmedSlot>()
        .and_then(|slot| slot.0.as_ref())
        .map(|armed| armed.view.read(cx).sequence.clone())
}
