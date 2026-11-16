//! Launch templates and everything that launches from them (SPEC.md, "Creation": Launch templates, Quick launch,
//! Template number shortcuts, Quick-launch sequences).
//!
//! The session launcher owns its draft, its dialog and its one submit path (`crate::launcher`); this module owns what a
//! template does to that draft and everything that starts from a template. Built in pieces. So far:
//!
//! - [`apply`]: applying one or more templates to a launcher draft, all or nothing, through the draft's own hand-edit
//!   methods, plus the launcher snapshot that Save as template reads too. No GPUI types.
//! - [`checkout_name`]: what the launcher's Name field says about a template's base name in a managed checkout.
//! - [`summary`]: a template's one-line summary, which the Templates list, the quick switcher and the Cmd+J list show.
//! - [`fields`]: the template editor's model: which fields a template sets, adding and removing them, and why a draft
//!   cannot be saved. No GPUI types.
//! - [`dialog`]: the Templates dialog beside New, with each template's ⚡ and number-key buttons.
//! - [`editor`]: the dialog's editor pane, where a template is opened, edited and saved.
//! - [`shortcuts`]: which template holds ⚡ and each number key, and how a change to them is written.
//! - [`sequence`]: the quick-launch sequence's state, its keys' rules, what its pill shows and what Enter resolves it
//!   to. No GPUI types.
//! - [`armed`]: the sequence in the window: the prefix, the key capture, its cancels, the pill and Enter.
//! - [`dispatch`]: the quick-launch dispatcher every shortcut ends in: it resolves the templates a shortcut names,
//!   applies them to New's prefill, and starts the launch unseen or opens New.
//! - [`save`]: Save as template's model, what the launcher offers to keep and what starts checked. No GPUI types.
//! - [`save_panel`]: the launcher's inline Save as template panel, which the launcher draws and which saves.
//! - [`rows`]: the template rows for a query, which the quick switcher's templates section, the launcher's `tl:` search
//!   and the Cmd+J list share, and the switcher's template pick and Quick launch row.
//!
//! The entry points other areas call are plain functions here: [`open`] for the list header's Templates button, with
//! [`button_help`] for its hover text, [`open_with_hint`] for a shortcut that found nothing to launch, and, for the main
//! view, [`quick_launch`] for the ⚡ chord, and [`begin_sequence`] for the sequence prefix with
//! [`watch_sequence_presses`] for the press that cancels it; and [`open_at`] for the launcher's Save as template, which
//! opens Templates on the template it saved. The quick switcher calls [`rows::template_query`] and [`rows::rows`] for
//! its templates section, [`rows::pick_from_switcher`] for a pick there, [`rows::quick_launch_row_label`] and
//! [`rows::quick_launch_from_switcher`] for its Quick launch row, and [`rows::launch_from_list`] for a pick in its
//! templates-only mode, the Cmd+J list.
//!
//! Templates, the ⚡ mark and the number keys are read from the store the helm pushes, never with a request per use.
//! SPEC.md says the shortcuts read "the mark and templates fresh on each use, so an agent's or the command line's
//! change takes effect at once"; the old app needed a request for that because it had no pushed copy. Here every
//! change reaches the store as the helm sees it, and a change this window makes is pushed before its reply, so the
//! store is that fresh read.

pub mod apply;
pub mod armed;
pub mod checkout_name;
pub mod dialog;
pub mod dispatch;
pub mod editor;
pub mod fields;
pub mod rows;
pub mod save;
pub mod save_panel;
pub mod sequence;
pub mod shortcuts;
pub mod summary;

#[cfg(test)]
mod apply_tests;
#[cfg(test)]
mod armed_tests;
#[cfg(test)]
mod checkout_name_tests;
#[cfg(test)]
mod dialog_tests;
#[cfg(test)]
mod dispatch_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod fields_tests;
#[cfg(test)]
mod launcher_modes_tests;
#[cfg(test)]
mod leave_tests;
#[cfg(test)]
mod naming_tests;
#[cfg(test)]
mod rows_tests;
#[cfg(test)]
mod save_tests;
#[cfg(test)]
mod sequence_tests;
#[cfg(test)]
mod shortcuts_tests;
#[cfg(test)]
mod summary_tests;

use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{AnyWindowHandle, App, FocusHandle, IntoElement, Window};

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

/// Open the Templates dialog with the stored template `name` in its editor: the launcher's Save as template, once its
/// save succeeded and the launcher closed. Nothing opens when the user has moved on since `mark`.
pub fn open_at(name: &str, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    dialog::open_at(name, mark, window, cx)
}

/// Quick launch the template marked ⚡ (SPEC.md, "Quick launch"): the main view's handler of `QuickLaunch`, the ⚡
/// chord. It starts the marked template directly, opens New when the template needs more choices, or opens Templates
/// with its hint when no usable template holds the mark. With the session launcher open it applies the marked template
/// to its draft instead; under any other dialog, or while the launcher is busy, it does nothing ([`dispatch::dispatch`]).
pub fn quick_launch(window: &mut Window, cx: &mut App) {
    dispatch::dispatch(
        dispatch::Origin::Mark,
        &[dispatch::Reference::Mark],
        window,
        cx,
    );
}

/// Start the template holding number key `digit` (SPEC.md, "Template number shortcuts"): the main view's handler of
/// `TemplateNumberKey`, which the physical-key table runs for Cmd+digit on a Mac and Ctrl+Shift+digit on Linux. It
/// goes the ⚡ chord's way through the dispatcher, with two differences the number-key origin makes: an unbound key
/// opens Templates with the binding hint, and a template that cannot launch alone (number keys may hold one) opens New
/// with the banner explaining how to combine it with the sequence prefix.
pub fn number_key(digit: u8, window: &mut Window, cx: &mut App) {
    dispatch::dispatch(
        dispatch::Origin::NumberKey,
        // The bindings map spells its keys as the digit's decimal text ("1", ..., "0").
        &[dispatch::Reference::Key(digit.to_string())],
        window,
        cx,
    );
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

/// Name the session a quick launch just created and opened, under `key` and titled `created` by the create's reply, in
/// `window`: the launcher's success path calls this when New was never shown and its success selected the session
/// (SPEC.md, "Quick launch": "When New was never shown and success selects its session, the rename dialog opens over
/// it while its agent is already starting"). The launch is already under way, so naming holds nothing up. It is the
/// Rename editor in naming mode (`session_list::rename::open_naming`), unless the helm's preferences turn naming off:
/// Settings' "Name sessions on quick launch", on by default, so an unset preference names too. The same for every
/// origin, since the launcher's fact covers them all.
pub(crate) fn name_quick_launched(key: &str, created: &str, window: AnyWindowHandle, cx: &mut App) {
    let turned_off = session::current_store(cx)
        .and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<Preferences>(PREFERENCES_KEY)
                .and_then(|preferences| preferences.name_session_on_quick_launch)
        })
        .is_some_and(|on| !on);
    if turned_off {
        return;
    }
    let _ = window.update(cx, |_, window, cx| {
        // Taken now, at the success, rather than at the press: the caller has just found that the user has not moved
        // on since the press, so this mark only lets the editor open over what the success selected.
        let mark = Mark::take(cx);
        crate::session_list::rename::open_naming(key, created, &mark, window, cx)
    });
}
