//! The Keyboard shortcuts overview: a dialog listing every app-wide shortcut and the keys inside a quick-launch
//! sequence, each in the one form this client handles, with the current ⚡ template and the assigned number keys.
//!
//! SPEC.md, "Desktop window chrome" (the Keyboard shortcuts paragraph), and "Feedback" for its place in the `?` menu.
//! Ported from the old app's `keyboard_shortcuts.rs` (its rows, groups and per-platform forms), the chord labels of its
//! `launch_sequence.rs` (`ShortcutChord`, `TEMPLATE_SHORTCUTS`), and the `.keyboard-shortcuts-*` and
//! `.quick-switcher-template-keys kbd` rules of its stylesheet, at reference commit `24bc0580`. Here's the TLDR:
//!
//! - **Two ways in.** Cmd+/ on a Mac, Ctrl+Shift+/ on Linux, is [`crate::actions::ShowKeyboardShortcuts`], a
//!   physical-key row of the keymap: the keystroke interceptor takes it by the Slash key's position before a focused
//!   terminal or text field sees it, and drops its held repeats. The main view's ground hands it to
//!   [`open_from_chord`]. The `?` menu's Keyboard shortcuts item opens the same dialog through [`open_from_menu`].
//! - **Refusals.** The chord does nothing while a dialog or menu holds the keyboard (this dialog included), while the
//!   window is inert, or before the helm's preferences have arrived with the snapshot, which stands in for SPEC.md's
//!   "before preferences finish loading", exactly as the quick switcher's chord does.
//! - **Closing.** Escape, Close and a press outside close it, and focus goes back where it was: to what held it when
//!   the chord opened the dialog (the terminal, usually), or to the `?` button when the menu did.
//! - **A static table, per platform.** [`sections`] is the old `rows` ported: fixed rows with their chords written out
//!   for a Mac and for Linux, of which the dialog shows only this build's ([`THIS_PLATFORM`]). It deliberately does not
//!   ask the keymap how to spell a chord: the keymap's tables are rules other features are judged by, and growing a
//!   display form there for one reader would change them. A test checks instead that every chord the table shows is
//!   bound, on its platform, to the action its row names, so the two cannot drift apart unnoticed.
//! - **Bindings from the store.** The ⚡ row and the number-key rows come from the helm's pushed preferences, read each
//!   time the dialog draws ([`Bindings::current`]), so they are current at every opening and follow a change made
//!   elsewhere while the dialog is open, as the old dialog followed its shared preferences.
//!
//! NOTE: SPEC.md also says composition does not open the dialog. The old listener checked `isComposing`; here the
//! interceptor takes the chord before the element that owns the input handler sees it, so refusing it falls to
//! [`open_from_chord`], and the app has no way yet to ask the focused input whether it is composing. That gap is
//! recorded in `docs/gpui/parity/other-dialogs.md`; a chord typed mid-composition opens the dialog.

#[cfg(test)]
mod tests;

use std::rc::Rc;

use farhelm_input_mapping::Platform;
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    AnyElement, App, AppContext as _, Context, Div, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};

use crate::actions::{ChordModifiers, THIS_PLATFORM};
use crate::components::{Button, DialogFrame, HoverHelp, Role, Tier};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Dialog, DialogPlacement, LayerId, MainAction};
use crate::session;
use crate::templates::shortcuts::{Bindings, KEYS};
use crate::theme::{self, Palette, Token, metrics};

// ---- Layout: the old `.keyboard-shortcuts-*` rules ------------------------------------------------------------------

/// The dialog's title, which is also its accessible name, and the name of the body's scroll stop, as the old dialog
/// named both.
const TITLE: &str = "Keyboard shortcuts";
/// `.keyboard-shortcuts-dialog`'s width (`min(640px, 100%)`; the frame caps it at the window).
const WIDTH: f32 = 640.;
/// `.keyboard-shortcuts-body`'s gap between groups.
const GROUP_GAP: f32 = 20.;
/// `.keyboard-shortcuts-section h3`: the group heading's size and its gap to the first row.
const HEADING_TEXT: f32 = 11.;
const HEADING_GAP: f32 = 10.;
/// `.keyboard-shortcuts-row`: the gap between the action and its chip, and the row's vertical padding.
const ROW_GAP: f32 = 16.;
const ROW_PAD: f32 = 5.;
/// `.quick-switcher-template-keys kbd`: the chip's padding (vertical, horizontal) and text size.
const CHIP_PAD: (f32, f32) = (2., 5.);
const CHIP_TEXT: f32 = 11.;
/// `.host-settings-header`'s gap, kept between the title and the header's Close (see [`KeyboardShortcuts`]'s render).
const HEADER_GAP: f32 = 14.;

// ---- The table ------------------------------------------------------------------------------------------------------

/// One chord as a chip shows it: its exact modifiers on one platform, and its key's label (the old `key_label`: `/`
/// for Slash, `+` for Equal, `−` for Minus, a letter in capitals, or a placeholder such as `0–9` for a family of keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Chord {
    modifiers: ChordModifiers,
    key: &'static str,
}

impl Chord {
    /// The chip's text, spelled as the old app's `modifier_label` spelled it: Command alone is a bare `⌘` before the
    /// key (`⌘K`), and any other set joins its names with `+` in the order ⌘, Ctrl, Shift and ends in one
    /// (`⌘+Shift+−`, `Ctrl+Shift+K`). Every chord in the table holds at least one modifier.
    fn label(self) -> String {
        let parts: Vec<&str> = [
            self.modifiers.command.then_some("⌘"),
            self.modifiers.control.then_some("Ctrl"),
            self.modifiers.shift.then_some("Shift"),
        ]
        .into_iter()
        .flatten()
        .collect();
        match parts.as_slice() {
            ["⌘"] => format!("⌘{}", self.key),
            parts => format!("{}+{}", parts.join("+"), self.key),
        }
    }
}

/// What a row names beside its chip.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Name {
    /// The app's own words.
    Words(&'static str),
    /// The ⚡ row: "Quick launch: ⚡" and the marked template's name, or "Quick launch: none marked". The name is the
    /// helm's raw value, drawn as host text, since an agent or the command line may have written it.
    QuickLaunch(Option<String>),
    /// An assigned number key: its digit and the name of the template that holds it, drawn as host text.
    NumberKey {
        digit: &'static str,
        template: String,
    },
}

/// What a row's name says: the app's own words, then, for a row that names a template, the id its host sentence is
/// drawn under and the template's raw name. The one place the rows' wording is spelled, for drawing and for tests.
fn wording(name: &Name) -> (String, Option<(String, &str)>) {
    match name {
        Name::Words(words) => ((*words).to_owned(), None),
        Name::QuickLaunch(None) => ("Quick launch: none marked".to_owned(), None),
        Name::QuickLaunch(Some(template)) => (
            "Quick launch: ⚡ ".to_owned(),
            Some(("keyboard-shortcuts-quick-launch".to_owned(), template)),
        ),
        Name::NumberKey { digit, template } => (
            format!("{digit} · "),
            Some((format!("keyboard-shortcuts-number-{digit}"), template)),
        ),
    }
}

/// What a row's chip shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Keys {
    /// A chord, in this platform's form.
    Chord(Chord),
    /// Keys typed inside an armed quick-launch sequence, which no binding takes (the sequence's key capture reads
    /// them), spelled as the old rows spelled them: "Space", "1–9, 0".
    Typed(&'static str),
}

/// One row of the table: an action and its keys.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    name: Name,
    keys: Keys,
}

/// One group of rows under its heading.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Section {
    title: &'static str,
    rows: Vec<Row>,
}

/// The overview's groups and rows on `platform`, with the ⚡ row and the assigned number keys from `bindings`: the old
/// `rows`, row for row. Keys used only inside other dialogs are not listed (SPEC.md).
///
/// Every app shortcut is Command on a Mac and Ctrl+Shift on Linux. In the desktop app the old split between chords that
/// follow the operating system and chords that follow the build (SPEC.md: "Each shortcut shows only the form this
/// client handles") comes to the same thing, since a Mac build runs only on a Mac. Three rows differ: New exists in the
/// Mac app only (SPEC.md: "the Linux app has no such shortcut"; decided for the overview by the migration's
/// orchestrator on 2026-11-13); Delete is Cmd+W on a Mac and Ctrl+Shift+Backspace on Linux, the one shortcut whose key
/// differs; and terminal text size takes Shift on a Mac too, where unshifted Command was the old web page's own zoom.
///
/// The number keys get one generic row whatever is assigned ("Launch a numbered template", since an unassigned key
/// still opens Templates with a binding hint), then one row per assigned key in the keys' own order, 1 to 9 then 0.
/// A binding to a name no template has any more is listed as stored: the store says what the key would try to start.
fn sections(platform: Platform, bindings: &Bindings) -> Vec<Section> {
    let mac = matches!(platform, Platform::Mac);
    let app = |key: &'static str| {
        let modifiers = ChordModifiers {
            command: mac,
            control: !mac,
            shift: !mac,
        };
        Keys::Chord(Chord { modifiers, key })
    };
    let row = |name: Name, keys: Keys| Row { name, keys };
    let words = |words: &'static str, keys: Keys| row(Name::Words(words), keys);
    let typed = |name: &'static str, keys: &'static str| words(name, Keys::Typed(keys));

    let mut sessions = Vec::new();
    if mac {
        sessions.push(words("New session", app("N")));
    }
    sessions.extend([
        words("Quick switcher", app("K")),
        words("Delete session", app(if mac { "W" } else { "Backspace" })),
        words("Rename session", app("L")),
        words("Keyboard shortcuts", app("/")),
    ]);

    let mut launching = vec![
        words("Templates", app("J")),
        row(Name::QuickLaunch(bindings.marked.clone()), app("P")),
        words("Launch a numbered template", app("0–9")),
    ];
    launching.extend(KEYS.into_iter().filter_map(|digit| {
        let template = bindings.keys.get(digit)?.clone();
        Some(row(Name::NumberKey { digit, template }, app(digit)))
    }));
    launching.extend([
        words("Begin launch sequence", app("X")),
        typed(
            "In a sequence: type a template name",
            "Characters (digits continue a word)",
        ),
        typed("In a sequence: queue the word", "Space"),
        typed("In a sequence: complete a unique name", "Tab"),
        typed("In a sequence: edit the word", "Backspace"),
        typed(
            "In a sequence: add a numbered template (no word in progress)",
            "1–9, 0",
        ),
        words("In a sequence: add the ⚡ template", app("P")),
        typed("In a sequence: launch", "Enter"),
        typed("In a sequence: cancel", "Escape"),
    ]);

    let text_size = |key: &'static str| {
        let modifiers = ChordModifiers {
            command: mac,
            control: !mac,
            shift: true,
        };
        Keys::Chord(Chord { modifiers, key })
    };
    let terminal = vec![
        words("Smaller terminal text", text_size("−")),
        words("Larger terminal text", text_size("+")),
    ];

    vec![
        Section {
            title: "Sessions",
            rows: sessions,
        },
        Section {
            title: "Launching",
            rows: launching,
        },
        Section {
            title: "Terminal",
            rows: terminal,
        },
    ]
}

// ---- Opening --------------------------------------------------------------------------------------------------------

/// The chord ([`crate::actions::ShowKeyboardShortcuts`]): open the overview over the window's page. The main view
/// calls this for the action wherever focus is, a focused terminal included, and the action counts as taken whatever
/// this does, as `ONCE_PER_PRESS` requires of a listed action's handler.
///
/// Does nothing while a dialog or menu holds the keyboard (SPEC.md: "does nothing while another modal is open"; a
/// popover that never takes focus is not a modal), while the window is inert, or before the snapshot has brought the
/// helm's preferences. Closing returns focus to what held it when the chord was pressed ([`ReturnTo::Opener`]), as for
/// the quick switcher's chord.
pub(crate) fn open_from_chord(window: &mut Window, cx: &mut App) {
    // A key pressed inside a layer reaches the main view's ground too, since the overlay is drawn inside it; this is
    // what keeps a second overview from opening over the first, or over any other dialog or menu.
    if layers::focus_layer_open(window, cx) || !preferences_arrived(cx) {
        return;
    }
    let mark = Mark::take(cx);
    let _ = open(ReturnTo::Opener, &mark, window, cx);
}

/// Open the overview from the `?` menu. `opener` is the `?` button, where closing returns focus (SPEC.md: "restores the
/// opener's focus, including the terminal or the **?** button"); `mark` is the [`Mark`] taken in the menu item's click
/// handler. Opens nothing when the user has moved on since `mark`. Returns the dialog's layer.
pub(crate) fn open_from_menu(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open(ReturnTo::Control(opener.clone()), mark, window, cx)
}

/// Whether the current UI session's store holds the helm's preferences: false while the window is inert and before
/// the snapshot has brought them.
fn preferences_arrived(cx: &App) -> bool {
    session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .is_some()
    })
}

/// Open the dialog, with closing returning focus as `returns_to` says. Focus starts on Close, as the old dialog's did,
/// and a press on the veil closes it the way Escape does (SPEC.md: "Escape, Close or a click outside closes the
/// ordinary modal").
fn open(returns_to: ReturnTo, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    let dialog = cx.new(|cx| KeyboardShortcuts {
        close: cx.focus_handle().tab_stop(true),
        layer: None,
    });
    // Enter on the dialog itself (after a click on its text) presses Close, its one action.
    let weak = dialog.downgrade();
    let main_action: MainAction = Rc::new(move |window, cx| close(&weak, window, cx));
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: cx.focus_handle(),
            start: dialog.read(cx).close.clone(),
            // What it shows is read from the store as it draws, so nothing it is about can go away under it.
            subject: None,
            main_action: Some(main_action),
            keeps_focus_on_empty_click: false,
            returns_to,
            on_dismiss: None,
            placement: DialogPlacement::Centered,
            dismiss_on_outside_press: true,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    #[cfg(test)]
    cx.set_global(tests::Opened(dialog.downgrade()));
    Some(layer)
}

// ---- The dialog -----------------------------------------------------------------------------------------------------

/// The open overview: Close's focus handle (where it starts) and its layer. It keeps no copy of the bindings; each draw
/// reads them from the store.
struct KeyboardShortcuts {
    close: FocusHandle,
    /// Set right after the layer opens; every way of closing goes through the layer manager's dismissal.
    layer: Option<LayerId>,
}

/// Dismiss the dialog `weak` through the layer manager, as Escape does; nothing once it is gone.
fn close(weak: &WeakEntity<KeyboardShortcuts>, window: &mut Window, cx: &mut App) {
    let layer = weak.upgrade().and_then(|dialog| dialog.read(cx).layer);
    if let Some(layer) = layer {
        layers::dismiss(layer, window, cx);
    }
}

/// One group: its heading (the old `h3`, uppercase by its stylesheet, so drawn in capitals here and named in the words
/// as written) and its rows.
fn section(section: Section, palette: &Palette) -> Div {
    let id = SharedString::from(format!(
        "keyboard-shortcuts-group-{}",
        section.title.to_lowercase()
    ));
    // NOTE: The old heading also had `letter-spacing: 0.06em`, which GPUI's text style cannot express.
    let heading = div()
        .id(id)
        .role(Role::Heading)
        .aria_label(section.title)
        .mb(px(HEADING_GAP))
        .text_size(px(HEADING_TEXT))
        .text_color(theme::color(palette.fg_1))
        .child(section.title.to_uppercase());
    div()
        .flex()
        .flex_col()
        .child(heading)
        .children(section.rows.into_iter().map(|entry| row(entry, palette)))
}

/// One row: the action, which takes the room the chip leaves and wraps, and the chip at the row's end. A row naming a
/// template draws the name through a host sentence, so a hidden character in it shows escaped and a right-to-left name
/// reorders only within itself.
fn row(entry: Row, palette: &Palette) -> Div {
    let (words, template) = wording(&entry.name);
    let name: AnyElement = match template {
        None => div().child(words).into_any_element(),
        Some((id, template)) => HostSentence::new(SharedString::from(id))
            .text(words)
            .host(&HostText::from_host(template.to_owned()))
            .into_any_element(),
    };
    let keys: SharedString = match entry.keys {
        Keys::Chord(chord) => chord.label().into(),
        Keys::Typed(keys) => keys.into(),
    };
    div()
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .py(px(ROW_PAD))
        .child(div().flex_1().min_w_0().child(name))
        .child(chip(keys, palette.fg_1, palette))
}

/// A key chip (`.quick-switcher-template-keys kbd`, the rule the old overview's rows shared): `keys` in 11-pixel `ink`
/// inside a small rounded box with the dialog's border ink. The overview draws every chord in one, and the template list
/// draws a template's ⚡ and number-key chords in them, so both lists show a chord the same way. `ink` is the caller's
/// because a chip on a selected row takes the dimmer inverse ink, as the old sheet's selected rule had it.
pub(crate) fn chip(keys: impl Into<SharedString>, ink: Token, palette: &Palette) -> Div {
    div()
        .flex_none()
        .py(px(CHIP_PAD.0))
        .px(px(CHIP_PAD.1))
        .border_1()
        .border_color(theme::color(palette.border_dialog))
        .rounded(metrics::RADIUS_SM)
        .text_size(px(CHIP_TEXT))
        .text_color(theme::color(ink))
        .child(keys.into())
}

impl Render for KeyboardShortcuts {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let bindings = Bindings::current(cx);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(GROUP_GAP))
            // `.keyboard-shortcuts-dialog`'s text size, over the frame's smaller default.
            .text_size(metrics::TEXT_MD)
            .children(
                sections(THIS_PLATFORM, &bindings)
                    .into_iter()
                    .map(|group| section(group, palette)),
            );
        let close = Button::new(
            "keyboard-shortcuts-close",
            "Close",
            HoverHelp::tooltip("Close: return to where you opened keyboard shortcuts"),
            Role::Button,
        )
        .accessible_name("Close keyboard shortcuts")
        .tier(Tier::Neutral)
        .focus(&self.close)
        .on_click(move |_, window, cx| close(&weak, window, cx));
        // The old dialog's header held its title and a worded Close button, and it had no footer; the frame's own
        // header close is the "×" icon the other dialogs use, and an answer would add a footer the old one did not
        // have. So the header's heading slot draws the title, in the frame's title style, with Close beside it. The
        // title stays the dialog's accessible name, and Close stays first in Tab order, before the body's stop.
        let heading = div()
            .flex()
            .items_center()
            .gap(px(HEADER_GAP))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(metrics::TEXT_TITLE)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(TITLE),
            )
            .child(div().flex_none().child(close));
        DialogFrame::new("keyboard-shortcuts", TITLE, body, Vec::new())
            .heading(heading)
            .width(px(WIDTH))
            // The rows hold nothing focusable, so the body is a stop of its own under the old region's name, and a long
            // table in a short window still scrolls from the keyboard.
            .scroll_stop(TITLE)
    }
}
