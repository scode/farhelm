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
//! What the list holds besides the draft (the open layer, the highlight) is the launcher's alone; its rows are worked out
//! from the draft and the history each time they are needed.

use farhelm_proto::text::{escaped, is_presentation_unsafe, notation};
use gpui::{
    AnyElement, App, AppContext as _, Context, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, WeakEntity, Window, div, px,
};

use super::Launcher;
use crate::components::{HoverHelp, OptionRow, Role, Vertical};
use crate::focus::Mark;
use crate::launcher::draft::Field;
use crate::launcher::entry::local_host;
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::theme::{self, metrics};

/// `folder_suggestions`: at most eight recent folders.
const RECENT_LIMIT: usize = 8;
/// `folder_tail_label`: labels longer than this many characters keep their end.
const LABEL_CHARS: usize = 36;
/// `.launch-composer-folder-options`, as the composer's other floating lists: `max-height: 260px; padding: 6px; gap: 2px`.
const LIST_MAX_HEIGHT: f32 = 260.;
const LIST_PAD: f32 = 6.;
const LIST_GAP: f32 = 2.;

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
    /// The open list's layer, if open. The layer manager closes it itself on Escape or a press outside, without telling
    /// the field, so [`Launcher::folder_list_open`] asks it rather than trusting this.
    layer: Option<LayerId>,
    /// The row an arrow moved to, as an index into the current rows; none until one does. Dropped whenever the rows can
    /// change under it (typing, a history answer, the list closing), so Enter never takes a row the arrow did not reach.
    active: Option<usize>,
    /// Whether the user has typed in the field since the folder was last seeded or picked: only then do the recent
    /// folders filter by its text (SPEC.md: "Untouched prefilled fields show the newest eight; typing filters"). A picked
    /// folder is the user's own choice for a later host change, but picking is not typing, so it does not filter.
    filtering: bool,
    /// Where the folder field is drawn, for the list to open under it; refreshed every frame.
    pub(super) anchor: Option<Anchor>,
}

/// The recent folders `folders` (newest first, as typed) offer for the field's `text`: all of them while the folder
/// is only seeded, else those holding `text` ignoring case, matched on the folder as typed (not its escaped spelling, so
/// a visible `<U+…>` never stands for the character it escapes), at most [`RECENT_LIMIT`].
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
    /// Whether the folder list is open: opened by the field and not since closed by the layer manager.
    pub(super) fn folder_list_open(&self, window: &Window, cx: &App) -> bool {
        self.folder_list
            .layer
            .is_some_and(|layer| layers::is_open(layer, window, cx))
    }

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
        let text = self.folder.text(cx);
        let mut choices: Vec<FolderChoice> =
            recent_folders(folders, &text, self.folder_list.filtering)
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
        self.folder_list.active = None;
        if !self.folder_list_open(window, cx) {
            self.open_folder_list(window, cx);
        }
        cx.notify();
    }

    /// The folder was set by a pick, not typed (a dropdown row, or a folder taken from the composer search): the next list
    /// shows every recent folder again, and no highlight survives the change.
    pub(super) fn folder_picked(&mut self) {
        self.folder_list.filtering = false;
        self.folder_list.active = None;
    }

    /// The rows may have changed under the highlight (a history answer, or the host or its connection changing), so
    /// Enter must not take whatever row now sits at its index.
    pub(super) fn drop_folder_highlight(&mut self) {
        self.folder_list.active = None;
    }

    /// The folder field lost focus: close the list.
    pub(super) fn folder_blurred(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_folder_list(window, cx);
        cx.notify();
    }

    /// The user typed in the folder field: the recents filter by the text from now on, the highlight goes (so Enter
    /// launches again until an arrow picks a row), and a list Escape closed opens again (SPEC.md: "typing reopens it").
    pub(super) fn folder_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.folder_list.filtering = true;
        self.folder_list.active = None;
        if !self.folder_list_open(window, cx) {
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
        if !self.folder_list_open(window, cx) {
            self.folder_list.active = None;
            self.open_folder_list(window, cx);
        }
        self.folder_list.active = Some(match (arrow, self.folder_list.active) {
            (Vertical::Down, None) => 0,
            (Vertical::Down, Some(index)) => (index + 1) % count,
            (Vertical::Up, None) => count - 1,
            (Vertical::Up, Some(index)) => (index + count - 1) % count,
        });
        cx.notify();
    }

    /// Enter in the folder field: take the highlighted row, returning whether it did. With nothing highlighted (or the
    /// list closed) it takes nothing and Enter launches as from any field; the list closes first, so a refused launch's
    /// controls below the field are not left under it (SPEC.md: "Enter that submits a launch also closes the list,
    /// whether the launch is accepted or refused").
    pub(super) fn folder_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.folder_list_open(window, cx) {
            return false;
        }
        let Some(choice) = self
            .folder_list
            .active
            .and_then(|index| self.folder_choices(cx).get(index).cloned())
        else {
            self.close_folder_list(window, cx);
            return false;
        };
        self.choose_folder(choice, window, cx);
        true
    }

    /// Make `choice` the folder, as typing it would (a deliberate choice, which a later host change keeps), and close
    /// the list; the next list shows every recent folder again, since picking is not typing. Local home first makes the
    /// local host the target, as picking it in the host list does.
    fn choose_folder(&mut self, choice: FolderChoice, window: &mut Window, cx: &mut Context<Self>) {
        self.close_folder_list(window, cx);
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
        let Some(anchor) = self.folder_list.anchor.clone() else {
            return;
        };
        let launcher = cx.weak_entity();
        let rows = cx.new(|cx| FolderRows::new(launcher, cx));
        let container = cx.focus_handle();
        let mark = Mark::take(cx);
        self.folder_list.layer = layers::open_popover(
            Popover::floating(rows, container, anchor),
            &mark,
            window,
            cx,
        );
    }

    /// Close the list if it is open, and drop the highlight.
    fn close_folder_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(layer) = self.folder_list.layer.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
        }
        self.folder_list.active = None;
    }

    /// Forget a list the layer manager closed on its own (Escape, a press outside), which tells the field nothing; the
    /// highlight goes with it. Called while drawing, as the model field does.
    pub(super) fn follow_folder_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder_list.layer.is_some() && !self.folder_list_open(window, cx) {
            self.folder_list.layer = None;
            self.folder_list.active = None;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The rows
// ---------------------------------------------------------------------------------------------------------------------

/// The open folder list's rows, drawn from the launcher's state, and kept scrolled so the highlighted row is in view.
pub(super) struct FolderRows {
    launcher: WeakEntity<Launcher>,
    scroll: ScrollHandle,
    /// The child last scrolled into view, so a wheel scroll is not pulled back on the next redraw.
    scrolled_to: Option<usize>,
    /// Redraws the rows when the launcher changes (what is typed, the highlight, the history).
    _redraw: Option<gpui::Subscription>,
}

impl FolderRows {
    fn new(launcher: WeakEntity<Launcher>, cx: &mut Context<Self>) -> Self {
        let redraw = launcher
            .upgrade()
            .map(|entity| cx.observe(&entity, |_, _, cx| cx.notify()));
        FolderRows {
            launcher,
            scroll: ScrollHandle::new(),
            scrolled_to: None,
            _redraw: redraw,
        }
    }
}

/// A group heading (`.launch-composer-folder-heading`), named for assistive technology.
fn heading(id: &'static str, words: &'static str, palette: &theme::Palette) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(words)
        .pt(px(4.))
        .px(px(6.))
        .pb(px(1.))
        .text_size(metrics::TEXT_SM)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(palette.fg_1))
        .child(words)
        .into_any_element()
}

impl Render for FolderRows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let list = layers::surface(cx)
            .id("launcher-folder-list")
            // Tests find the list by where it is drawn; this records nothing outside test builds.
            .debug_selector(|| "launcher-folder-list".into())
            .role(Role::ListBox)
            .aria_label("Folders")
            .flex()
            .flex_col()
            .gap(px(LIST_GAP))
            .p(px(LIST_PAD))
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .text_size(metrics::TEXT_MD);
        let Some(launcher) = self.launcher.upgrade() else {
            return list;
        };
        let launcher_ref = launcher.read(cx);
        let choices = launcher_ref.folder_choices(cx);
        let active = launcher_ref.folder_list.active;
        // Headings and rows are all the list's own children, so the scroll handle can bring any row into view.
        let mut children: Vec<AnyElement> =
            vec![heading("launcher-folder-recent", "Recent", palette)];
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
                children.push(heading("launcher-folder-reset", "Reset", palette));
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
            let weak = self.launcher.clone();
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
        if active_child != self.scrolled_to {
            if let Some(child) = active_child {
                self.scroll.scroll_to_item(child);
            }
            self.scrolled_to = active_child;
        }
        list.children(children)
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
