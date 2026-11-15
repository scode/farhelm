//! Launch templates and everything that launches from them (SPEC.md, "Creation": Launch templates, Quick launch,
//! Template number shortcuts, Quick-launch sequences).
//!
//! The session launcher owns its draft, its dialog and its one submit path (`crate::launcher`); this module owns what a
//! template does to that draft and everything that starts from a template. Built in pieces. So far:
//!
//! - [`apply`]: applying one or more templates to a launcher draft, all or nothing, through the draft's own hand-edit
//!   methods, plus the launcher snapshot that Save as template reads too. No GPUI types.
//! - [`summary`]: a template's one-line summary, which the Templates list, the quick switcher and the Cmd+J list show.
//! - [`fields`]: the template editor's model: which fields a template sets, adding and removing them, and why a draft
//!   cannot be saved. No GPUI types.
//! - [`dialog`]: the Templates dialog beside New, with each template's ⚡ and number-key buttons.
//! - [`editor`]: the dialog's editor pane, where a template is opened, edited and saved.
//! - [`shortcuts`]: which template holds ⚡ and each number key, and how a change to them is written.
//! - [`sequence`]: the quick-launch sequence's state, its keys' rules, what its pill shows and what Enter resolves it
//!   to. No GPUI types.
//! - [`armed`]: the sequence in the window: the prefix, the key capture, its cancels, the pill and Enter.
//!
//! The entry points other areas call are plain functions here: [`open`] for the list header's Templates button, with
//! [`button_help`] for its hover text, [`open_with_hint`] for a shortcut that found nothing to launch, and, for the main
//! view, [`begin_sequence`] for the sequence prefix with [`watch_sequence_presses`] for the press that cancels it.
//!
//! Templates, the ⚡ mark and the number keys are read from the store the helm pushes, never with a request per use.
//! SPEC.md says the shortcuts read "the mark and templates fresh on each use, so an agent's or the command line's
//! change takes effect at once"; the old app needed a request for that because it had no pushed copy. Here every
//! change reaches the store as the helm sees it, and a change this window makes is pushed before its reply, so the
//! store is that fresh read.

pub mod apply;
pub mod armed;
pub mod dialog;
pub mod editor;
pub mod fields;
pub mod sequence;
pub mod shortcuts;
pub mod summary;

#[cfg(test)]
mod apply_tests;
#[cfg(test)]
mod armed_tests;
#[cfg(test)]
mod dialog_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod fields_tests;
#[cfg(test)]
mod sequence_tests;
#[cfg(test)]
mod shortcuts_tests;
#[cfg(test)]
mod summary_tests;

use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{App, FocusHandle, IntoElement, Window};

use crate::components::HoverHelp;
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{Anchor, LayerId};
use crate::session;

pub use dialog::Hint;

/// The ⚡ chord as the Templates dialog and its hover help name it on this platform (SPEC.md, "Quick launch": "Cmd+P on
/// macOS or Ctrl+Shift+P elsewhere"). The old app, a web page that could not tell, printed both ("⌘P / Ctrl+Shift+P");
/// the desktop app knows which one it handles, as the Keyboard shortcuts dialog's rule has it ("Each shortcut shows
/// only the form this client handles").
pub const QUICK_LAUNCH_SHORTCUT_NAME: &str = if cfg!(target_os = "macos") {
    "⌘P"
} else {
    "Ctrl+Shift+P"
};

/// The quick-launch sequence prefix as the dialog names it on this platform (SPEC.md, "Quick-launch sequences": Cmd+X on
/// macOS, Ctrl+Shift+X on Linux).
pub const SEQUENCE_PREFIX_NAME: &str = if cfg!(target_os = "macos") {
    "⌘X"
} else {
    "Ctrl+Shift+X"
};

/// A number key's chord as the dialog names it on this platform (SPEC.md, "Template number shortcuts": Cmd+digit on
/// macOS, Ctrl+Shift+digit on Linux), the old app's `number_chord`: `key` is the digit, or a placeholder such as
/// "number" in the help text.
pub fn number_key_name(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+Shift+{key}")
    }
}

/// Open the Templates dialog for the list header's Templates button. `opener` is the button's focus handle, where
/// closing returns focus on every platform (on macOS a click leaves the button unfocused), and `mark` the [`Mark`] taken
/// in the button's click handler. Nothing opens when the user has moved on since, or while the window is inert.
pub fn open(
    opener: &FocusHandle,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    dialog::open(ReturnTo::Control(opener.clone()), None, mark, window, cx)
}

/// Open the Templates dialog because a shortcut found nothing to launch, with `hint`'s line saying why (SPEC.md: "With
/// no usable mark it opens Templates with an explanation"; "Unbound keys open Templates with a binding hint"). Closing
/// returns focus to what held it when the dialog opened: a shortcut is a key press, which moved no focus, so that is
/// where the user was.
pub fn open_with_hint(
    hint: Hint,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    dialog::open(ReturnTo::Opener, Some(hint), mark, window, cx)
}

/// Begin a quick-launch sequence for the prefix (SPEC.md, "Quick-launch sequences"), its pill centered over `anchor`,
/// the main pane as last drawn: the main view's handler of `BeginSequence`. Nothing happens while a dialog keeps the
/// keyboard, while the window is inert, or before the pane was drawn ([`armed::begin`]).
pub fn begin_sequence(anchor: Option<Anchor>, window: &mut Window, cx: &mut App) {
    armed::begin(anchor, window, cx);
}

/// The window-wide press watcher that cancels an armed sequence on a press anywhere in the window, which the main view
/// draws once ([`armed::watch_presses`]).
pub fn watch_sequence_presses() -> impl IntoElement {
    armed::watch_presses()
}

/// The Templates button's hover help: it advertises the ⚡ chord and the marked template (SPEC.md: "Templates' hover and
/// a separate quick-switcher row advertise the chord and marked name"), in the old button's words. The marked name is
/// a template name, which an agent may have written, so it is a host value in the sentence. Read from the store each
/// time the button draws, so a mark set anywhere shows at once.
pub fn button_help(cx: &App) -> HoverHelp {
    let marked = session::current_store(cx).and_then(|store| {
        store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| preferences.quick_launch_template.clone())
    });
    match marked {
        Some(name) => HoverHelp::sentence(
            HostSentence::new("templates-button-help")
                .text("Templates: quick launch ")
                .host(&HostText::from_host(name))
                .text(format!(" ({QUICK_LAUNCH_SHORTCUT_NAME})")),
        ),
        None => HoverHelp::tooltip(format!(
            "Templates: mark one with ⚡ for quick launch ({QUICK_LAUNCH_SHORTCUT_NAME})"
        )),
    }
}
