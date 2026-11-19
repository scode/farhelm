//! The folder field's dropdown: the chosen host's recent folders and two resets, floating under the field while it has
//! focus. Ported from the old launcher's folder picker (`list/create_form.rs` at reference commit `24bc0580`: the
//! `.launch-composer-folder-options` markup, `folder_suggestions`, `folder_tail_label` and `choose_folder`).
//!
//! How it behaves, as the old one did:
//!
//! - **Opens with focus.** Focusing the editable folder field opens the list under it, on the layer manager's
//!   non-focusing popover, so focus and every key stay in the field; leaving the field (Tab, a press elsewhere) closes
//!   it, and so does Escape, which closes the list before the launcher.
//! - **Recent**: up to eight folders from the chosen host's launch history ([`super::history`]), newest first. A folder
//!   seeded or picked (the open session's, `~`, a Clone's, a row taken) shows them all; once the user types in the
//!   field, only those holding what is typed, ignoring case, until the next pick. With none: "No recent folders".
//! - **Reset**: Home (`~` on the chosen host) and Local home (`~` on this machine, which also makes the local host the
//!   target). Local home is left out where the launcher keeps its host fixed (Replace with).
//! - **Keys.** No row is highlighted until an arrow moves to one (Down from nothing is the first, Up the last; both
//!   wrap); Enter then takes it without launching, and with nothing highlighted Enter launches as from any field (SPEC.md:
//!   "In the editable folder field, Enter chooses an arrow-highlighted dropdown entry without launching; with no
//!   highlighted entry it launches as before"). Typing drops the highlight. A click takes a row.
//! - **Long paths** keep their end: a label is at most 36 characters, the rest replaced by a leading ellipsis, and never
//!   cuts through an escaped character's `<U+XXXX>`; the full escaped path is the row's accessible name and hover help.
//!
//! What the list holds besides the draft (the open layer, the highlight, `field_list`) is the launcher's alone; its rows
//! are worked out from the draft and the history each time they are needed.

use farhelm_proto::text::{escaped, is_presentation_unsafe, notation};
use gpui::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    px,
};

use super::Launcher;
use super::field_list::{Drawn, FieldList};
use crate::components::{HoverHelp, OptionRow, Role, Vertical, option_heading};
use crate::launcher::draft::Field;
use crate::launcher::entry::local_host;
use crate::theme::{self, metrics};

/// `folder_suggestions`: at most eight recent folders.
const RECENT_LIMIT: usize = 8;
/// `folder_tail_label`: labels longer than this many characters keep their end.
const LABEL_CHARS: usize = 36;
/// The list's element id (`.launch-composer-folder-options`), which tests find it by.
const FOLDER_LIST: &str = "launcher-folder-list";

/// One row of the folder list, in its keyboard order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum FolderChoice {
    /// A recent folder, as the user typed it when it launched.
    Recent(String),
    /// `~` on the chosen host.
    Home,
    /// `~` on this machine: the local host becomes the target.
    LocalHome,
}

/// What the folder list holds besides the draft. None of it is shared with another surface.
#[derive(Default)]
pub(super) struct FolderList {
    /// The list under the field, with the row an arrow moved to; none until one does. The highlight is dropped whenever
    /// the rows can change under it (typing, a history answer, the list closing), so Enter never takes a row the arrow
    /// did not reach. The launcher's draw notes where the field is for it to open under.
    pub(super) list: FieldList,
    /// Whether the user has typed in the field since the folder was last seeded or picked: only then do the recent
    /// folders filter by its text (SPEC.md: "Untouched prefilled fields show the newest eight; typing filters"). A picked
    /// folder is the user's own choice for a later host change, but picking is not typing, so it does not filter.
    filtering: bool,
}

/// The recent folders `folders` (newest first, as typed) offer for the field's `text`: all of them while the folder
/// is only seeded, else those holding `text` ignoring case, at most [`RECENT_LIMIT`]. Both inputs are raw paths so an
/// invisible character cannot match the literal characters of its spelling on disk.
pub(in crate::launcher) fn recent_folders<'a>(
    folders: impl IntoIterator<Item = &'a str>,
    text: &str,
    filtering: bool,
) -> Vec<&'a str> {
    let query = text.to_lowercase();
    folders
        .into_iter()
        .filter(|folder| !filtering || folder.to_lowercase().contains(&query))
        .take(RECENT_LIMIT)
        .collect()
}

/// `path`'s label in the list: its escaped spelling, or, past [`LABEL_CHARS`] characters, an ellipsis and its last
/// characters, without keeping part of an escape the cut would split (the old `folder_tail_label`): such an escape goes
/// with the cut-off start, so a fragment of one can never pose as path text.
pub(in crate::launcher) fn tail_label(path: &str) -> String {
    let visible = escaped(path);
    let count = visible.chars().count();
    if count <= LABEL_CHARS {
        return visible;
    }
    // The first visible character kept; one place goes to the ellipsis.
    let mut start = count - (LABEL_CHARS - 1);
    // Walk the path as `escaped` spells it: a character it escapes (an unsafe one, or the `<` of a literal `<U+`, the
    // same rule as `escaped`'s) is one indivisible run of its escape's width; everything else is one character.
    let mut shown = 0;
    for (index, ch) in path.char_indices() {
        let width = if is_presentation_unsafe(ch) || (ch == '<' && path[index..].starts_with("<U+"))
        {
            notation(ch).chars().count()
        } else {
            1
        };
        let end = shown + width;
        if shown < start && start < end {
            start = end;
            break;
        }
        if end >= start {
            break;
        }
        shown = end;
    }
    format!("…{}", visible.chars().skip(start).collect::<String>())
}

// ---------------------------------------------------------------------------------------------------------------------
// The list's behavior
// ---------------------------------------------------------------------------------------------------------------------

impl Launcher {
    /// The list's rows now, in keyboard order: the recent folders, Home, and Local home unless the host is held.
    pub(super) fn folder_choices(&self, cx: &App) -> Vec<FolderChoice> {
        let folders = self
            .launch_history(cx)
            .map(|history| {
                history
                    .folders
                    .iter()
                    .map(|folder| folder.display_cwd.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut choices: Vec<FolderChoice> = recent_folders(
            folders,
            self.draft.folder.value(),
            self.folder_list.filtering,
        )
        .into_iter()
        .map(|folder| FolderChoice::Recent(folder.to_owned()))
        .collect();
        choices.push(FolderChoice::Home);
        if !self.draft.holds(Field::Host) {
            choices.push(FolderChoice::LocalHome);
        }
        choices
    }

    /// The folder field took focus: open the list, with nothing highlighted.
    pub(super) fn folder_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_list.list.highlighted = None;
        if !self.folder_list.list.is_open(window, cx) {
            self.open_folder_list(window, cx);
        }
        cx.notify();
    }

    /// The folder was set by a pick, not typed (a dropdown row, or a folder taken from the composer search): the next list
    /// shows every recent folder again, and no highlight survives the change.
    pub(super) fn folder_picked(&mut self) {
        self.folder_list.filtering = false;
        self.folder_list.list.highlighted = None;
    }

    /// The rows may have changed under the highlight (a history answer, or the host or its connection changing), so
    /// Enter must not take whatever row now sits at its index.
    pub(super) fn drop_folder_highlight(&mut self) {
        self.folder_list.list.highlighted = None;
    }

    /// The folder field lost focus: close the list.
    pub(super) fn folder_blurred(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_list.list.close(window, cx);
        cx.notify();
    }

    /// The user typed in the folder field: the recents filter by the text from now on, the highlight goes (so Enter
    /// launches again until an arrow picks a row), and a list Escape closed opens again (SPEC.md: "typing reopens it").
    pub(super) fn folder_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_list.filtering = true;
        self.folder_list.list.highlighted = None;
        if !self.folder_list.list.is_open(window, cx) {
            self.open_folder_list(window, cx);
        }
    }

    /// Up or Down in the folder field: move the highlight, wrapping, opening the list if Escape had closed it.
    pub(super) fn folder_arrow(
        &mut self,
        arrow: Vertical,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.folder_choices(cx).len();
        if !self.folder_list.list.is_open(window, cx) {
            self.folder_list.list.highlighted = None;
            self.open_folder_list(window, cx);
        }
        // Home is always a row, so there is always one to move to.
        self.folder_list.list.arrow(arrow, count);
        cx.notify();
    }

    /// Enter in the folder field: take the highlighted row, returning whether it did. With nothing highlighted (or the
    /// list closed) it takes nothing and Enter launches as from any field; the list closes first, so a refused launch's
    /// controls below the field are not left under it (SPEC.md: "Enter that submits a launch also closes the list,
    /// whether the launch is accepted or refused").
    pub(super) fn folder_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.folder_list.list.is_open(window, cx) {
            return false;
        }
        let Some(choice) = self
            .folder_list
            .list
            .highlighted
            .and_then(|index| self.folder_choices(cx).get(index).cloned())
        else {
            self.folder_list.list.close(window, cx);
            return false;
        };
        self.choose_folder(choice, window, cx);
        true
    }

    /// Make `choice` the folder, as typing it would (a deliberate choice, which a later host change keeps), and close
    /// the list; the next list shows every recent folder again, since picking is not typing. Local home first makes the
    /// local host the target, as picking it in the host list does.
    fn choose_folder(&mut self, choice: FolderChoice, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_list.list.close(window, cx);
        self.folder_picked();
        match choice {
            // A recent folder is this host's history, and stays bound to its installation.
            FolderChoice::Recent(folder) => self.take_history_folder(folder, cx),
            FolderChoice::Home => self.draft.edit_folder("~".to_owned()),
            FolderChoice::LocalHome => {
                // `folder_choices` offers Local home only where the host is not held; the guard stays so a release build
                // could never move a held host if that ever changed.
                debug_assert!(!self.draft.holds(Field::Host), "Local home on a held host");
                let local = local_host(self.store.read(cx).collections());
                if let Some(local) = local.filter(|_| !self.draft.holds(Field::Host)) {
                    self.pick_host(local, window, cx);
                }
                self.draft.edit_folder("~".to_owned());
            }
        }
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Open the list under the folder field, as a popover that never takes focus.
    fn open_folder_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_list
            .list
            .open(FOLDER_LIST, draw_folder_rows, window, cx);
    }

    /// The folder list's state, for tests of the shared list plumbing it is built on.
    #[cfg(test)]
    pub(in crate::launcher) fn folder_field_list_for_tests(&mut self) -> &mut FieldList {
        &mut self.folder_list.list
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The rows
// ---------------------------------------------------------------------------------------------------------------------

/// The open folder list's rows (`field_list`): the Recent heading and folders (or "No recent folders"), then the Reset
/// heading and its rows, with the highlighted row selected.
fn draw_folder_rows(
    launcher: &Launcher,
    list: Stateful<Div>,
    weak: &WeakEntity<Launcher>,
    _: &Window,
    cx: &App,
) -> Drawn {
    let palette = theme::palette(cx);
    let list = list.role(Role::ListBox).aria_label("Folders");
    let choices = launcher.folder_choices(cx);
    let active = launcher.folder_list.list.highlighted;
    // Headings and rows are all the list's own children, so the scroll handle can bring any row into view.
    let mut children: Vec<AnyElement> =
        vec![option_heading("launcher-folder-recent", "Recent", palette)];
    if !choices
        .iter()
        .any(|choice| matches!(choice, FolderChoice::Recent(_)))
    {
        children.push(
            div()
                .id("launcher-folder-empty")
                // A plain line, as the old one was: a live status would be announced every time the list opens.
                .role(Role::Label)
                .aria_label("No recent folders")
                .p(px(6.))
                .text_color(theme::color(palette.fg_1))
                .child("No recent folders")
                .into_any_element(),
        );
    }
    let mut active_child = None;
    for (index, choice) in choices.into_iter().enumerate() {
        if choice == FolderChoice::Home {
            children.push(option_heading("launcher-folder-reset", "Reset", palette));
        }
        if active == Some(index) {
            active_child = Some(children.len());
        }
        let selected = active == Some(index);
        let ink = theme::color(if selected {
            palette.on_fill
        } else {
            palette.fg_0
        });
        let meaning_ink = theme::color(if selected {
            palette.on_fill
        } else {
            palette.fg_1
        });
        let id = format!("launcher-folder-{index}");
        // A recent folder's hover help is its whole path, escaped, as the old row's tooltip was.
        let (label, help, content): (SharedString, HoverHelp, AnyElement) = match &choice {
            FolderChoice::Recent(folder) => {
                let spelled = escaped(folder);
                (
                    spelled.clone().into(),
                    HoverHelp::tooltip(spelled),
                    div()
                        .font_family(crate::assets::TERMINAL_FONT)
                        .text_color(ink)
                        .child(tail_label(folder))
                        .into_any_element(),
                )
            }
            FolderChoice::Home => (
                "Reset folder to home".into(),
                HoverHelp::tooltip("Home: start in your home folder on this host"),
                reset_content("Home", "~ on this host", ink, meaning_ink),
            ),
            FolderChoice::LocalHome => (
                "Reset destination to local home".into(),
                HoverHelp::tooltip("Local home: start in your home folder on this machine"),
                reset_content("Local home", "~ on this machine", ink, meaning_ink),
            ),
        };
        let weak = weak.clone();
        children.push(
            OptionRow::new(id, label, help, Role::ListBoxOption)
                .selected(selected)
                .child(content)
                .on_click(move |_, window, cx| {
                    let choice = choice.clone();
                    let _ = weak.update(cx, |launcher, cx| {
                        launcher.choose_folder(choice, window, cx)
                    });
                })
                .into_any_element(),
        );
    }
    Drawn {
        list,
        children,
        highlighted: active_child,
    }
}

/// A reset row's words: its name, then what it means in the dimmer ink (`.launch-composer-folder-meaning`).
fn reset_content(
    name: &'static str,
    meaning: &'static str,
    ink: gpui::Hsla,
    meaning_ink: gpui::Hsla,
) -> AnyElement {
    div()
        .flex()
        .gap(px(8.))
        .child(div().text_color(ink).child(name))
        .child(
            div()
                .text_size(metrics::TEXT_SM)
                .text_color(meaning_ink)
                .child(meaning),
        )
        .into_any_element()
}
