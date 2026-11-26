//! The Templates dialog beside New: the saved templates as a list, each with a one-line summary, beside the editor.
//!
//! SPEC.md, "Launch templates": "Templates are created, edited, duplicated, and deleted in the Templates dialog beside
//! New. A list of names and summaries sits beside the selected template's editor". Ported from the old app's
//! `TemplatesDialog` (`list/templates.rs`) and its `.templates-*` rules in `assets/app.css`, both at reference commit
//! `24bc0580`.
//!
//! Here's the TLDR of its rules:
//!
//! - **The frame.** The shared dialog frame with the Templates tile, the title "Templates", the old summary and the
//!   header's Close, 960 pixels wide at most. Its body holds the help paragraph, an optional hint line (set when a
//!   shortcut opened the dialog, [`Hint`]), the two panes and the list's unavailable line. There is no footer: the
//!   editor carries its own action row.
//! - **The list.** "+ New template", then every template the helm pushed, in its order (by name), each drawn as its
//!   name over its summary ([`super::summary`]); a count under the list. Each row is a button that opens its template
//!   in the editor, drawn pressed while the editor holds that template; opening the row the editor already holds does
//!   nothing, so its edits stay. Names and the values a summary shows are host-supplied text, since an agent may have
//!   written them, so they are drawn through the host-text element.
//! - **The editor** ([`super::editor`]) beside the list: "Choose a template to edit, or make a new one." until a row or
//!   "+ New template" opens one. Focus goes to its name field when it opens. What it refuses or the helm refuses of a
//!   save is said in the status line under the panes.
//! - **Leaving an edited draft** ([`Departure`]): another row, "+ New template", Duplicate, the header Close and
//!   Escape first ask "Save these changes before leaving?" with Keep editing, Discard and Save, in that reading order,
//!   focus starting on Save (SPEC.md), centered over the dialog, when the draft holds edits Save would store
//!   ([`Editor::needs_leave_confirmation`]). Keep editing or Escape returns focus to the name field. Discard goes where
//!   the user was heading. Save saves, and goes there only once the save has succeeded; a refused save stays with its
//!   reason in the status line. The veil keeps the editor and the departure fixed while the question is open.
//! - **Deleting.** Delete takes effect at once. Its editor goes, focus moves to "+ New template", and a notice under the
//!   panes offers Undo for about ten seconds ([`UNDO_FOR`]): Undo saves the copy the dialog had loaded back under its old
//!   name, unless another template has taken that name since. A later deletion replaces the notice, and closing the
//!   dialog ends it. Undo never restores the ⚡ mark or a number key, which the helm cleared with the deletion, and
//!   "Delete permanently" offers no Undo ([`super::editor::DeleteKind`]).
//! - **Writes, one at a time.** A save, a delete, an Undo, and a ⚡ or number-key write are never in flight together:
//!   while one is, the rows, "+ New template", the bolts, the keys, Save, Delete and Undo wait (busy, keeping focus),
//!   and neither Escape nor Close closes the dialog, so the write's refusal can still be reported.
//! - **Reading.** Everything comes from the store as the helm pushes it; the dialog keeps no copy. A template that
//!   changes or appears while the dialog is open shows up at once. When the helm could not read its templates the list
//!   says so with the helm's words, and keeps showing the last templates it had.
//! - **⚡ and the number keys.** Each row starts with its bolt and its number-key button ([`super::shortcuts`]). The
//!   bolt marks or unmarks the template; the key button opens a menu of No shortcut and the ten keys, each saying which
//!   template holds it. Both write the helm's shared preferences, one write at a time, and show what the store holds,
//!   never what was just clicked. A refused write is said under the panes in the helm's words. While the helm's
//!   preferences are unavailable, or before they have arrived, neither is sent and the same line says why.
//!
//! NOTE: The old list's Retry beside a failed read is not ported: the helm pushes its templates again whenever they
//! change, so there is nothing for the dialog to ask for (recorded in the parity file).
//!
//! NOTE: The old dialog scrolled as one piece, header included. Here the frame keeps its header (title and Close) on
//! screen and scrolls the body, as every other dialog of the app does; at a window too short even for the header, the
//! whole card scrolls (the frame's rule).

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use farhelm_proto::launcher::{LaunchCatalogModel, LaunchTemplate};
use farhelm_proto::ui_channel::{Availability, Collection};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Role, StatefulInteractiveElement as _, Styled as _,
    WeakEntity, Window, div, px, svg,
};

use super::available_names;
use super::editor::{self, DeleteKind, Editor, Ended};
use super::fields::taken_refusal;
use super::shortcuts::{self, Bindings, KEYS};
use super::summary;
use crate::assets::Icon;
use crate::components::{
    Button, DialogFrame, HoverHelp, IconButton, MenuHeader, MenuItem, TEMPLATE_ROW_VERTICAL_INSET,
    Tier,
};
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::launcher::read_model_catalog;
use crate::layers::{self, Anchor, Dialog, Dismissal, LayerId, Popover};
use crate::operations::{Command, OpKey, Outcome};
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::theme::{self, metrics};
use crate::ticker::{self, Deadline};

/// `.templates-dialog`'s width: `min(960px, 100%)`; the dialog frame applies the `100%` (the window less the old
/// backdrop's side margins).
const WIDTH: f32 = 960.;
/// `.templates-panes`: the list column (`minmax(0, 260px)`), the gap between the panes and their floor.
const LIST_WIDTH: f32 = 260.;
const PANES_GAP: f32 = 24.;
const PANES_MIN_HEIGHT: f32 = 340.;
/// `.templates-sidebar`: the list column's inner gap and its padding before its right rule.
const LIST_COLUMN_GAP: f32 = 12.;
const LIST_COLUMN_PAD_RIGHT: f32 = 18.;
/// `.templates-content`: the gap between the body's blocks.
const CONTENT_GAP: f32 = 14.;
/// `.templates-list`: the gap between rows. A row's padding is its button's ([`Tier::TemplateRow`]).
const ROW_GAP: f32 = 6.;
/// `.templates-row`: the gap between a row's name and its summary.
const ROW_LINE_GAP: f32 = 4.;
/// `.templates-hint` and `.templates-row-summary`: 12-pixel text at a 1.5 line height.
const SMALL_LINE_HEIGHT: f32 = 18.;

/// A selected template uses inverse ink for all its detail lines because they sit on the same accent fill.
pub(super) fn row_detail_ink(
    palette: &theme::Palette,
    selected: bool,
    idle: theme::Token,
) -> theme::Token {
    if selected {
        palette.on_accent_dim
    } else {
        idle
    }
}
/// The header tile's glyph: the Templates action glyph at its own 14-pixel size, as the old tile drew it (Settings'
/// gear is drawn larger in its tile; the old dialogs differed).
const TILE_GLYPH: f32 = 14.;

/// The header's summary, the old dialog's words.
const SUMMARY: &str = "Saved edits to combine in the session launcher";

/// The header Close's name (the old dialog's words) and its hover help (this app's, saying what a close asks).
const CLOSE_LABEL: &str = "Close templates";
const CLOSE_HELP: &str =
    "Close Templates. If you edited the template, you can save or discard your changes.";

/// How long a deletion's Undo is offered: SPEC.md's "about ten seconds", the old notice's 10,000 ms.
pub(super) const UNDO_FOR: Duration = Duration::from_secs(10);

/// Why a shortcut opened the dialog instead of launching, said in a line above the list (the old app's
/// `quick_launch_hint` and `number_shortcut_hint`). The Templates button opens the dialog with neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    /// The ⚡ chord, or a sequence's ⚡, found no usable marked template.
    QuickLaunch,
    /// A number key, alone or in a sequence, has no template bound to it.
    NumberKey,
}

impl Hint {
    /// The line the dialog shows for this hint.
    pub fn text(self) -> String {
        match self {
            Hint::QuickLaunch => format!(
                "Mark a template with ⚡ to launch it directly with {}.",
                super::QUICK_LAUNCH_SHORTCUT_NAME
            ),
            Hint::NumberKey => format!(
                "Give a template a number key with the key button beside its ⚡; {} launches it directly.",
                super::number_key_name("number")
            ),
        }
    }
}

/// The help paragraph above the panes, the old dialog's words with this platform's chords.
fn help_text() -> String {
    format!(
        "A template is a named set of launcher edits. In New session, type tl:name to apply one; it changes only the \
         fields it sets. Templates stack, the later one winning: tl:my-claude then tl:webbuilder starts Claude in a \
         managed checkout of acme/web on build-box. ⚡ marks the template {} launches directly; a number key launches \
         its template with {}. A number key can hold a partial template; press {} and then several keys to combine \
         them, Enter to launch.",
        super::QUICK_LAUNCH_SHORTCUT_NAME,
        super::number_key_name("number"),
        super::SEQUENCE_PREFIX_NAME,
    )
}

/// The Templates dialog. See the module docs.
pub struct TemplatesDialog {
    /// The dialog's layer, once opened, for its Close.
    layer: Option<LayerId>,
    /// The line saying why a shortcut opened the dialog, if one did.
    hint: Option<Hint>,
    /// Each row's number-key button, by template name, as last drawn: what its menu is anchored to when the button is
    /// clicked, and the button's focus handle, where closing the menu returns focus. A template gone from the list leaves
    /// its entry behind, which holds only a weak reference and a handle nothing draws, and is harmless.
    key_anchors: HashMap<String, (Anchor, FocusHandle)>,
    /// The number-key menu that is open, if one is: its template's name and its layer.
    key_menu: Option<(String, LayerId)>,
    /// What this dialog last wrote to the preferences, so a refusal on that key is worded as the write it answers. A
    /// refusal there that some other dialog's write left (a Settings switch) is not this dialog's to show.
    last_write: Option<Write>,
    /// A ⚡ or number-key write this dialog refused before sending, because the bindings could not be known, with the
    /// reason ([`TemplatesDialog::bindings_for`]). Shown where a refused write is, in place of the helm's refusal of an
    /// earlier write, since it answers the user's latest click; the next write the dialog sends, or one the editor
    /// sends, clears it.
    unread_refusal: Option<HostSentence>,
    /// "+ New template"'s focus handle: where focus starts, as the old dialog started it there.
    new_focus: FocusHandle,
    /// The open template, once a row or "+ New template" opened one.
    pub(super) editor: Option<Editor>,
    /// The model catalog for the editor's effort choices and model suggestions, once read; read when the first
    /// editor opens, as nothing before needs it.
    catalog: Option<Vec<LaunchCatalogModel>>,
    /// Whether the catalog read was sent, so it is sent once per dialog. A failed read is not retried: without a
    /// catalog the editor refuses no effort and suggests no model, as the old editor did.
    catalog_requested: bool,
    /// The unanswered leave question and its initial Save focus. A departure before the veil draws can reach
    /// the editor's buttons and take focus away from the question that just opened.
    leave_question: Option<(LayerId, FocusHandle)>,
    /// Where the user was heading when they answered the leave question with Save, and their press: the departure runs
    /// once that save has succeeded, and is forgotten if it fails.
    leaving: Option<(Departure, Mark)>,
    /// The last deletion's Undo, while it is offered.
    undo: Option<Undo>,
    /// Whether an Undo's save is in flight. Kept here rather than read from the operation store under the deleted name:
    /// the notice, and with it that name, can expire while the save is still in flight, and the dialog's writes still
    /// go one at a time until it settles. A connection reset closes the dialog, so a completion that never runs cannot
    /// leave it set.
    restoring: bool,
    /// The status line's sentence once no editor is open to hold it: why an Undo was refused. Cleared when a template
    /// opens, and when the next Save, Delete or Undo starts, as the old dialog cleared its one line.
    status: Option<HostSentence>,
}

/// Where leaving the editor goes ([`TemplatesDialog::leave`]): every way out that asks about an edited draft first,
/// the old dialog's `Departure`.
#[derive(Clone, Debug)]
pub(super) enum Departure {
    /// The stored template of this name, from its row.
    Open(String),
    /// "+ New template".
    New,
    /// The editor's Duplicate, which copies the template as last saved.
    Duplicate,
    /// The header Close or Escape: the dialog closes.
    Close,
}

/// A deletion's Undo: the copy the dialog had loaded, which Undo saves back, until `_expires` ends the offer.
/// Replacing or dropping it (a later deletion, a successful Undo, the dialog closing) cancels the deadline with it.
struct Undo {
    template: LaunchTemplate,
    _expires: Deadline,
}

/// A preference write this dialog sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Write {
    Mark,
    Keys,
}

impl Write {
    /// The line a refusal of this write is shown with, before the helm's words.
    fn refused(self) -> &'static str {
        match self {
            Write::Mark => "The quick-launch mark was not saved: ",
            Write::Keys => "The number key was not saved: ",
        }
    }

    /// The line a write refused before sending is shown with, before the reason the bindings could not be read: the
    /// rename's "shortcut bindings could not be read" words, so the two refusals read alike.
    fn unread(self) -> &'static str {
        match self {
            Write::Mark => {
                "The quick-launch mark was not saved; shortcut bindings could not be read: "
            }
            Write::Keys => "The number key was not saved; shortcut bindings could not be read: ",
        }
    }
}

/// Open the Templates dialog, with `hint`'s line when a shortcut opened it rather than the Templates button. `returns_to`
/// is where closing returns focus: the Templates button's [`ReturnTo::Control`], since on macOS a click leaves a button
/// unfocused, and what held focus before the click is not where the old app returned it (the Templates button, by
/// name); [`ReturnTo::Opener`] for a shortcut, a key press that moved no focus. `mark` is the [`Mark`] the opener took
/// when the user acted; nothing opens when the user has moved on since, or while the window is inert (it has no store to
/// draw templates from).
///
/// Focus starts on "+ New template", as the old dialog started it (`DialogFocus::remembered(".templates-new", ..)`).
///
/// Returns the dialog's layer, or `None` when nothing opened.
pub fn open(
    returns_to: ReturnTo,
    hint: Option<Hint>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_dialog(returns_to, hint, mark, window, cx).map(|(layer, _)| layer)
}

/// Open the dialog with the stored template `name` in its editor, focus in its name field: where a successful Save as
/// template lands (SPEC.md: "Successful saving ... opens the new template in the Templates editor"). The helm pushes
/// a command's changes before its reply, so the store holds the template by the time its save succeeded; a name it
/// does not hold leaves the editor empty. Closing returns focus to what held it when the dialog opened. Otherwise as
/// [`open`].
pub fn open_at(name: &str, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    let (layer, dialog) = open_dialog(ReturnTo::Opener, None, mark, window, cx)?;
    dialog.update(cx, |dialog, cx| dialog.open_template(name, window, cx));
    Some(layer)
}

/// [`open`], also handing back the dialog, for [`open_at`] to open a template in it.
fn open_dialog(
    returns_to: ReturnTo,
    hint: Option<Hint>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<(LayerId, gpui::Entity<TemplatesDialog>)> {
    session::current_store(cx)?;
    if focus::has_moved_on(mark, cx) {
        return None;
    }
    let new_focus = cx.focus_handle().tab_stop(true);
    let dialog = cx.new(|_| TemplatesDialog {
        layer: None,
        hint,
        key_anchors: HashMap::new(),
        key_menu: None,
        last_write: None,
        unread_refusal: None,
        new_focus: new_focus.clone(),
        editor: None,
        catalog: None,
        catalog_requested: false,
        leave_question: None,
        leaving: None,
        undo: None,
        restoring: false,
        status: None,
    });
    let container = cx.focus_handle();
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container,
            start: new_focus,
            subject: None,
            // The dialog has no one main action: Enter on the dialog itself does nothing, as in the old app.
            main_action: None,
            keeps_focus_on_empty_click: false,
            returns_to,
            // Asked on Escape and the header Close ([`TemplatesDialog::dismissal`]).
            on_dismiss: Some(Rc::new({
                let dialog = dialog.downgrade();
                move |window: &mut Window, cx: &mut App| {
                    dialog
                        .update(cx, |dialog, cx| dialog.dismissal(window, cx))
                        .unwrap_or(Dismissal::Close)
                }
            })),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    #[cfg(test)]
    layers::record_opened(&dialog, cx);
    Some((layer, dialog))
}

/// What the header's Close does: dismiss the dialog through the layer manager, so its dismissal hook decides, as for
/// Escape. An open "+ Add field" menu is forgotten first, so the hook closes the dialog (or asks the leave question about
/// an edited draft): only Escape closes that menu on its own, as in the old dialog, whose Close left the editor
/// whatever was open in it. While a write is in flight the hook keeps the dialog open, so the menu is left alone too:
/// forgetting it then, with no redraw, would make it vanish later from under the user.
fn dismiss(
    dialog: WeakEntity<TemplatesDialog>,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let layer = dialog
            .update(cx, |dialog, cx| {
                if !dialog.writing(cx)
                    && let Some(editor) = dialog.editor.as_mut()
                {
                    editor.forget_add_menu();
                }
                dialog.layer
            })
            .ok()
            .flatten();
        if let Some(layer) = layer {
            layers::dismiss(layer, window, cx);
        }
    }
}

/// `.templates-quick-mark`: the bolt button's column width.
const BOLT_WIDTH: f32 = 28.;
/// `.templates-number-key`: the key button's text size.
const KEY_TEXT: f32 = 10.;
/// The number-key menu's width, as wide as the old flyout drew its longest description.
const KEY_MENU_WIDTH: f32 = 260.;
/// Center the controls on the first two lines and their insets. The marked row grows only below this area,
/// so its new chord line cannot move the bolt or cap away from a pointer resting on either one.
const ROW_CONTROLS_HEIGHT: f32 =
    2. * (SMALL_LINE_HEIGHT + TEMPLATE_ROW_VERTICAL_INSET) + ROW_LINE_GAP;
/// Keep one chord line's space in the list before a template is marked. A long list sets the centered
/// dialog's height, and growing it on the first mark would move the entire list.
const ROW_CHORD_HEIGHT: f32 = 17.;

impl TemplatesDialog {
    /// Whether a write this dialog sent is in flight: a ⚡ or number-key write, the editor's save or delete, or an Undo.
    fn writing(&self, cx: &App) -> bool {
        let preferences = self.last_write.is_some()
            && session::current_operations(cx).is_some_and(|operations| {
                operations
                    .read(cx)
                    .in_flight(&OpKey::preferences())
                    .is_some()
            });
        preferences || self.restoring || self.editor.as_ref().is_some_and(Editor::is_writing)
    }

    /// Whether the number-key button of `name` has been drawn, so its menu has an anchor to open on (the screenshot
    /// goldens wait for it).
    #[cfg(test)]
    pub(crate) fn key_button_drawn(&self, name: &str) -> bool {
        self.key_anchors.contains_key(name)
    }

    /// What Escape and the header Close do. The dialog stays while one of its writes is in flight, as the old dialog
    /// held its Close while busy: closing then would lose the line that reports the write if it is refused (SPEC.md,
    /// "One GUI at a time": a failed save is reported). An open "+ Add field" menu closes first, focus going back to
    /// "+ Add field", as the old dialog's Escape closed it before the dialog. An edited draft stays and asks the leave
    /// question, whose answer closes the dialog or not.
    fn dismissal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Dismissal {
        if self.writing(cx) {
            return Dismissal::Stay;
        }
        if let Some(editor) = self.editor.as_mut()
            && editor.close_add_menu(window, cx)
        {
            cx.notify();
            return Dismissal::Stay;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(Editor::needs_leave_confirmation)
        {
            self.ask_leave(Departure::Close, window, cx);
            return Dismissal::Stay;
        }
        Dismissal::Close
    }

    /// Open the stored template `name` in the editor, with focus on its name field. Opening the template the editor
    /// already holds does nothing, so its unsaved edits stay (the old dialog's rule); opening another asks first about
    /// unsaved edits ([`TemplatesDialog::leave`]).
    pub(crate) fn open_template(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.previous() == Some(name))
        {
            return;
        }
        self.leave(Departure::Open(name.to_owned()), window, cx);
    }

    /// The open editor's Duplicate, once any unsaved edits are answered for ([`TemplatesDialog::leave`]).
    pub(super) fn duplicate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave(Departure::Duplicate, window, cx);
    }

    /// The editor's Save, with the dialog's model catalog to judge the draft's effort by. Returns whether the save was
    /// sent. Nothing while any write of the dialog's is in flight (a ⚡ or number-key write included): the dialog's
    /// writes go one at a time, and Save's busy look only reaches the screen with the next frame.
    pub(super) fn save_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.writing(cx) {
            return false;
        }
        self.status = None;
        self.editor
            .as_mut()
            .is_some_and(|editor| editor.save(self.catalog.as_deref(), window, cx))
    }

    /// Delete the named template with the Undo promise from the footer that was shown. Ignore the old footer if a
    /// row click changed editors before redraw, or if a write is in flight.
    pub(super) fn delete_editor(
        &mut self,
        name: &str,
        kind: DeleteKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.writing(cx) {
            return;
        }
        let Some(editor) = self
            .editor
            .as_mut()
            .filter(|editor| editor.previous() == Some(name))
        else {
            return;
        };
        self.status = None;
        editor.delete(kind, window, cx);
    }

    /// One of the editor's writes settled ([`Editor::settled`]); act on what reaches beyond the editor. A save the
    /// leave question's Save waited for lets its departure go; any other end forgets it. A deletion drops the editor,
    /// offers its Undo (or ends an earlier one, for "Delete permanently") and moves focus to "+ New template", as the
    /// old dialog did, unless the user has moved on since pressing Delete.
    pub(super) fn write_settled(
        &mut self,
        outcome: Outcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        match editor.settled(outcome, cx) {
            Ended::Going => {}
            Ended::Saved => {
                if let Some((departure, mark)) = self.leaving.take() {
                    self.depart(departure, &mark, window, cx);
                }
            }
            Ended::Stopped => self.leaving = None,
            Ended::Deleted { undo, mark } => {
                self.editor = None;
                self.leaving = None;
                self.undo = undo.map(|template| self.offer_undo(*template, cx));
                focus::focus_in_layer(&self.new_focus, &mark, window, cx);
            }
        }
    }

    // ---- Leaving the editor -----------------------------------------------------------------------------------------

    /// Leave the editor for `departure`: at once when it holds no edits Save would store, or else after the leave
    /// question ([`TemplatesDialog::ask_leave`]). Nothing while a write is in flight. The header Close and Escape come
    /// through the dismissal hook instead, which must answer the layer manager itself.
    fn leave(&mut self, departure: Departure, window: &mut Window, cx: &mut Context<Self>) {
        if self.writing(cx) {
            return;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(Editor::needs_leave_confirmation)
        {
            self.ask_leave(departure, window, cx);
        } else {
            let mark = Mark::take(cx);
            self.depart(departure, &mark, window, cx);
        }
    }

    /// Ask once for the editor that was open at the departure press. Its name field identifies that editor even
    /// for a new template. Set the return target without focusing it first, which would scroll the editor.
    fn ask_leave(&mut self, departure: Departure, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((layer, start)) = &self.leave_question
            && layers::is_open(*layer, window, cx)
        {
            if layers::top_layer(window, cx) == Some(*layer)
                && !start.is_focused(window)
                && !layers::focus_is_where_dialogs_allow(window, cx)
            {
                focus::enter_layer(start, window, cx);
            }
            return;
        }
        let Some(name) = self
            .editor
            .as_ref()
            .and_then(|editor| editor.name_focus(cx))
        else {
            return;
        };
        let mark = Mark::take(cx);
        let answer = |label: &'static str, help: &'static str, tier| {
            Answer::new(label, HoverHelp::tooltip(help), tier)
        };
        let discard = cx.weak_entity();
        let question = Question::new(
            "Save these changes before leaving?",
            answer("Save", "Save these changes, then continue", Tier::Primary),
            (departure, name.clone()),
        )
        .returns_to(name)
        .cancel_answer(answer(
            "Keep editing",
            "Stay here with the unsaved draft",
            Tier::Neutral,
        ))
        .alternative(
            answer(
                "Discard",
                "Discard these changes, then continue",
                Tier::Neutral,
            ),
            move |(departure, editor), window, cx| {
                let _ = discard.update(cx, |dialog, cx| {
                    if dialog.is_open_editor(&editor, cx) {
                        let mark = Mark::take(cx);
                        dialog.depart(departure, &mark, window, cx);
                    }
                });
            },
        );
        let save = cx.weak_entity();
        let asked = prompt::ask(
            question,
            None,
            &mark,
            move |(departure, editor), window, cx| {
                let _ = save.update(cx, |dialog, cx| {
                    let mark = Mark::take(cx);
                    if dialog.is_open_editor(&editor, cx) && dialog.save_editor(window, cx) {
                        dialog.leaving = Some((departure, mark));
                    }
                });
            },
            window,
            cx,
        );
        self.leave_question = asked.zip(window.focused(cx));
    }

    /// A footer press or leave answer belongs to its editor instance, even if another editor has the same name.
    pub(super) fn is_open_editor(&self, name_field: &FocusHandle, cx: &App) -> bool {
        self.editor
            .as_ref()
            .and_then(|editor| editor.name_focus(cx))
            .as_ref()
            == Some(name_field)
    }

    /// Go where `departure` leads, whatever the editor holds, for the user's action at `mark`: a stored template (by
    /// its name, as the store holds it now; nothing if it has gone), a new or duplicated draft, or the dialog closed.
    /// A write in flight keeps the draft, including any eventual refusal shown in its status line.
    fn depart(
        &mut self,
        departure: Departure,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.writing(cx) {
            return;
        }
        let editor = match departure {
            Departure::Open(name) => session::current_store(cx)
                .and_then(|store| {
                    store
                        .read(cx)
                        .collections()
                        .get::<LaunchTemplate>(&name)
                        .cloned()
                })
                .map(|template| Editor::open(&template, window, cx)),
            Departure::New => Some(Editor::new_template(window, cx)),
            // The copy is named over the stored names, or over none while the helm cannot read them (the save checks
            // the name again).
            Departure::Duplicate => {
                let names = available_names(cx).unwrap_or_default();
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                self.editor
                    .as_ref()
                    .and_then(|editor| editor.duplicate(&names, window, cx))
            }
            Departure::Close => {
                if let Some(layer) = self.layer {
                    layers::close(layer, window, cx);
                }
                None
            }
        };
        if let Some(editor) = editor {
            self.show_editor(editor, mark, window, cx);
        }
    }

    // ---- Undo -------------------------------------------------------------------------------------------------------

    /// Offer Undo for the deleted `template` for [`UNDO_FOR`], after which the notice goes.
    fn offer_undo(&self, template: LaunchTemplate, cx: &mut Context<Self>) -> Undo {
        let dialog = cx.weak_entity();
        let expires = ticker::after(UNDO_FOR, cx, move |cx| {
            let _ = dialog.update(cx, |dialog, cx| {
                dialog.undo = None;
                cx.notify();
            });
        });
        Undo {
            template,
            _expires: expires,
        }
    }

    /// Undo: save the deleted template's loaded copy back under its old name (`PUT /api/templates/{name}`), without
    /// workspace trust, as every save drops it (SPEC.md). Refused, with the status line saying why in Undo's own words,
    /// while another template has the name or the list cannot show it is free ([`taken_refusal`]'s rule; not the
    /// new-name whitespace rule, so a stored spaced name keeps its Undo). Once the helm has stored it the notice goes,
    /// the template shows in the list as pushed, the editor stays as it is, and focus goes where the old dialog's
    /// recovery put it once the Undo button was gone: the open editor's name field, or else "+ New template" (unless
    /// the user has moved on since the press). A refusal keeps the notice, for as long as it had left.
    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.writing(cx) {
            return;
        }
        let Some(undo) = self.undo.as_ref() else {
            return;
        };
        let mut template = undo.template.clone();
        template.fields.workspace_trust = None;
        let names = available_names(cx);
        let names: Option<Vec<&str>> = names
            .as_ref()
            .map(|names| names.iter().map(String::as_str).collect());
        self.status = taken_refusal(&template.name, names.as_deref()).map(|_| match names {
            None => editor::said(
                "The template list could not be read, so Undo cannot check that the deleted template's name is \
                 still free; try again once it is back",
            ),
            Some(_) => editor::said("Another template is now named ")
                .host(&HostText::from_host(template.name.clone()))
                .text(", so the deleted template cannot be restored under that name"),
        });
        cx.notify();
        if self.status.is_some() {
            return;
        }
        let (Some(command), Some(operations)) = (
            editor::save_command(&template),
            session::current_operations(cx),
        ) else {
            return;
        };
        let dialog = cx.weak_entity();
        let handle = window.window_handle();
        let mark = Mark::take(cx);
        let started = operations.update(cx, |operations, cx| {
            operations.start(
                &OpKey::template(&template.name),
                command,
                cx,
                move |settled, cx| {
                    let outcome = settled.outcome.clone();
                    // As for the editor's writes, a completion runs outside any window's update, and a closed window
                    // has closed the dialog with it.
                    let _ = handle.update(cx, |_, window, cx| {
                        dialog.update(cx, |dialog, cx| {
                            dialog.restoring = false;
                            match outcome {
                                Outcome::Done(_) | Outcome::Accepted(_) => {
                                    dialog.undo = None;
                                    let target = dialog
                                        .editor
                                        .as_ref()
                                        .and_then(|editor| editor.name_focus(cx))
                                        .unwrap_or_else(|| dialog.new_focus.clone());
                                    focus::focus_in_layer(&target, &mark, window, cx);
                                }
                                Outcome::Refused(refusal) => {
                                    dialog.status = Some(editor::helm_said(&refusal.words));
                                }
                            }
                            cx.notify();
                        })
                    });
                },
            )
        });
        self.restoring = started.is_ok();
    }

    /// The `template-leave-question` golden's state: the open editor's name changed to `codex-deep-unsaved`, as the
    /// reference picture typed it, and the header Close pressed, which asks the leave question.
    #[cfg(test)]
    pub(crate) fn ask_leave_for_golden(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = self.editor.as_mut() {
            editor.rename_draft("codex-deep-unsaved", window, cx);
        }
        self.ask_leave(Departure::Close, window, cx);
    }

    /// Forget which preference write this dialog last sent, once the editor has written the preferences itself (a
    /// rename moving the ⚡ mark or a number key), so the line under the panes that reports a refused ⚡ or key write
    /// does not word the editor's refusal as the list's.
    pub(super) fn forget_preference_write(&mut self) {
        self.last_write = None;
        self.unread_refusal = None;
    }

    /// "+ New template": an empty new draft in the editor, with focus on its name field, once any unsaved edits are
    /// answered for ([`TemplatesDialog::leave`]).
    fn new_template(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave(Departure::New, window, cx);
    }

    /// Put `editor` in the pane, move focus to its name field for the action marked `mark` (the field is drawn in
    /// the next frame, which the focus coordinator waits for), and read the catalog if no editor has yet. An answered
    /// leave question closes before this runs. The dialog's status line is cleared when the editor changes.
    fn show_editor(
        &mut self,
        editor: Editor,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = editor.name_focus(cx);
        self.editor = Some(editor);
        self.status = None;
        if let Some(name) = name {
            focus::focus_in_layer(&name, mark, window, cx);
        }
        self.read_catalog(cx);
        cx.notify();
    }

    /// Read the release's model catalog (`GET /api/launch-catalog`), once per dialog, for the editor's effort choices
    /// and model suggestions. The helm does not push it, so it is read, as the launcher reads it.
    fn read_catalog(&mut self, cx: &mut Context<Self>) {
        if self.catalog_requested {
            return;
        }
        self.catalog_requested = true;
        let weak = cx.weak_entity();
        read_model_catalog(cx, move |answer, cx| {
            let Ok(catalog) = answer else {
                return;
            };
            let _ = weak.update(cx, |dialog, cx| {
                dialog.catalog = Some(catalog);
                cx.notify();
            });
        });
    }

    /// One row of the list (`li` of `.templates-list`): the bolt, the number-key button, and the template's name over
    /// its summary, with the ⚡ chord as a third line on the marked row.
    fn row(
        &mut self,
        template: &LaunchTemplate,
        bindings: &Bindings,
        busy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = theme::palette(cx);
        let name = HostText::from_host(template.name.clone());
        let marked = bindings.is_marked(&template.name);
        let eligible = shortcuts::may_mark(template);
        let bound = bindings.key_of(&template.name);
        let weak = cx.weak_entity();
        let weak_open = weak.clone();

        let bolt_name = template.name.clone();
        let bolt = IconButton::new(
            format!("template-bolt-{}", template.name),
            if marked { Icon::BoltFilled } else { Icon::Bolt },
            HostSentence::new("template-bolt-label")
                .text("Quick launch ")
                .host(&name)
                .accessible_name(),
            HoverHelp::tooltip(shortcuts::bolt_help(marked, eligible)).beside_list(false),
            Role::Button,
        )
        .tier(Tier::QuickMark { lit: marked })
        .pressed(marked)
        // A lit bolt stays clickable so a template that no longer qualifies can still be unmarked (SPEC.md).
        .disabled(!eligible && !marked)
        // Busy rather than disabled while a write is in flight: a disabled button gives up focus, which would fall to
        // the dialog itself (Close is held too), losing the user's place in the list.
        .busy(busy)
        .on_click({
            let weak = weak.clone();
            // What the click means is decided by what the user saw: this frame's bolt, lit or not.
            move |_, _, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.toggle_mark(&bolt_name, marked, cx));
            }
        });

        // The anchor's element state lives only while it is asked for in every frame, so it is built afresh on each
        // draw and the latest kept for the menu to open on.
        let anchor = Anchor::for_element(format!("template-anchor-{}", template.name), window, cx);
        // The button's focus handle is element state too, so its menu can name the button as where closing returns
        // focus.
        let key_focus = focus::control_handle(
            format!("template-key-{}-focus-handle", template.name),
            window,
            cx,
        );
        self.key_anchors
            .insert(template.name.clone(), (anchor.clone(), key_focus.clone()));
        let key_name = template.name.clone();
        let key = Button::new(
            format!("template-key-{}", template.name),
            // The old button's name: which template's key it is, so rows read apart (its hover help says the key).
            HostSentence::new("template-key-label")
                .text("Number shortcut ")
                .host(&name)
                .accessible_name(),
            HoverHelp::tooltip(shortcuts::key_help(eligible, bound)).beside_list(false),
            Role::Button,
        )
        .tier(Tier::NumberKey {
            bound: bound.is_some(),
        })
        .content(
            // The cap fits the abbreviated label. Hover help carries the full chord.
            div()
                .font_family(crate::assets::TERMINAL_FONT)
                .text_size(px(KEY_TEXT))
                .whitespace_nowrap()
                .child(shortcuts::key_button_text(bound)),
        )
        // Open as the layer manager says: Escape or a press outside closes the menu without telling this dialog.
        .expanded(self.key_menu.as_ref().is_some_and(|(open, layer)| {
            *open == template.name && layers::is_open(*layer, window, cx)
        }))
        .focus(&key_focus)
        .busy(busy)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |dialog, cx| {
                dialog.toggle_key_menu(&key_name, window, cx)
            });
        });

        // The row the editor holds is drawn pressed. Its summary and shortcut use the quieter inverse ink, and its
        // name takes the button's own inverse ink.
        let open = self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.previous() == Some(template.name.as_str()));
        let mut lines = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(ROW_LINE_GAP))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(metrics::TEXT_MD)
                    .line_height(px(SMALL_LINE_HEIGHT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(host_text::element(&name).truncate()),
            )
            .child(
                div()
                    .min_w_0()
                    .text_size(metrics::TEXT_SM)
                    .line_height(px(SMALL_LINE_HEIGHT))
                    .text_color(theme::color(row_detail_ink(palette, open, palette.fg_1)))
                    .child(summary::line(
                        format!("template-summary-{}", template.name),
                        &summary::parts(&template.fields),
                    )),
            );
        if marked {
            lines = lines.child(
                div()
                    .text_size(px(summary::CHORD_TEXT))
                    .line_height(px(ROW_CHORD_HEIGHT))
                    .font_family(crate::assets::TERMINAL_FONT)
                    .text_color(theme::color(row_detail_ink(palette, open, palette.fg_2)))
                    .child(super::QUICK_LAUNCH_SHORTCUT_NAME),
            );
        }
        let open_name = template.name.clone();
        let opener = Button::new(
            format!("template-open-{}", template.name),
            HostSentence::new("template-open-label")
                .host(&name)
                .accessible_name(),
            HoverHelp::sentence(
                HostSentence::new("template-open-help")
                    .text("Edit template ")
                    .host(&name),
            )
            .beside_list(false),
            Role::Button,
        )
        .tier(Tier::TemplateRow)
        .pressed(open)
        .busy(busy)
        .content(lines)
        .on_click(move |_, window, cx| {
            let _ = weak_open.update(cx, |dialog, cx| {
                dialog.open_template(&open_name, window, cx)
            });
        });

        div()
            .id(gpui::ElementId::Name(
                format!("template-row-{}", template.name).into(),
            ))
            .role(Role::ListItem)
            .aria_label(
                HostSentence::new("template-row-name")
                    .host(&name)
                    .accessible_name(),
            )
            .flex()
            .items_start()
            .gap(px(4.))
            .child(
                div()
                    .flex_none()
                    .h(px(ROW_CONTROLS_HEIGHT))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(div().flex_none().w(px(BOLT_WIDTH)).child(bolt))
                    .child(
                        div()
                            .flex_none()
                            .relative()
                            .child(key)
                            .child(anchor.track()),
                    ),
            )
            .child(div().flex_1().min_w_0().child(opener))
            .into_any_element()
    }

    /// The bolt of `name` was clicked while it was drawn lit (`was_lit`) or not: clear the mark, or give it to `name`
    /// ([`shortcuts::mark_write`] says why the drawn state decides). Nothing is written while any write of the dialog's
    /// is in flight ([`TemplatesDialog::write`]). Visible to the tests, which call it as a click delivered in the same
    /// frame as a Delete press would.
    pub(super) fn toggle_mark(&mut self, name: &str, was_lit: bool, cx: &mut Context<Self>) {
        let Some(bindings) = self.bindings_for(Write::Mark, cx) else {
            return;
        };
        if let Some(wanted) = shortcuts::mark_write(name, was_lit, bindings.marked.as_deref()) {
            self.write(Write::Mark, shortcuts::mark_command(wanted), cx);
        }
    }

    /// Give `name` the number key `key`, or none, from the map the store holds now, written whole.
    fn choose_key(&mut self, name: &str, key: Option<&str>, cx: &mut Context<Self>) {
        let Some(bindings) = self.bindings_for(Write::Keys, cx) else {
            return;
        };
        let keys = bindings.keys_with(name, key);
        if keys == bindings.keys {
            return;
        }
        self.write(Write::Keys, shortcuts::keys_command(&keys), cx);
    }

    /// The bindings a ⚡ or key `write` is built from, or `None` when it must not be sent: while a write of the
    /// dialog's is in flight ([`TemplatesDialog::write`] says why), and while the bindings cannot be known
    /// ([`Bindings::read`]), when the refusal says why, as the old list refused a ⚡ or key edit whose fresh read of the
    /// preferences failed. A key write is the whole map, so one built from bindings that never arrived would wipe
    /// every other key; a mark write is one field, but the bolt the user clicked was drawn from the same unknown state,
    /// so its intent is no surer.
    fn bindings_for(&mut self, write: Write, cx: &mut Context<Self>) -> Option<Bindings> {
        if self.writing(cx) {
            return None;
        }
        match Bindings::read(cx) {
            Ok(bindings) => Some(bindings),
            Err(unread) => {
                self.unread_refusal = Some(unread.appended_to(
                    HostSentence::new("templates-write-refused").text(write.unread()),
                ));
                cx.notify();
                None
            }
        }
    }

    /// Send a preference write as work on the preferences' key, unless any write of the dialog's is in flight. The
    /// bolts and keys are drawn busy then, but a click delivered in the same frame as a Delete or an Undo press still
    /// reaches them, and the store would send both (they are on different keys): a mark naming the template the
    /// deletion just removed, say.
    fn write(
        &mut self,
        write: Write,
        command: Result<Command, farhelm_helm_client::PathError>,
        cx: &mut Context<Self>,
    ) {
        if self.writing(cx) {
            return;
        }
        let (Some(operations), Ok(command)) = (session::current_operations(cx), command) else {
            return;
        };
        let started = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::preferences(), command, cx, |_, _| {})
        });
        match started {
            Ok(()) => {
                self.last_write = Some(write);
                self.unread_refusal = None;
                cx.notify();
            }
            Err(not_started) => {
                tracing::debug!(?write, ?not_started, "template shortcut not saved")
            }
        }
    }

    /// The key button of `name` was clicked: open its menu, or close it if it is open. Focus starts on the key the
    /// template holds, or on No shortcut.
    pub(crate) fn toggle_key_menu(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A menu still open closes first; a second click on its own button only closes it.
        if let Some((open, layer)) = self.key_menu.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
            if open == name {
                cx.notify();
                return;
            }
        }
        let Some((anchor, button)) = self.key_anchors.get(name).cloned() else {
            return;
        };
        let mark = Mark::take(cx);
        let bound = Bindings::current(cx).key_of(name);
        let focus: Vec<FocusHandle> = (0..=KEYS.len())
            .map(|_| cx.focus_handle().tab_stop(true))
            .collect();
        let start = match bound {
            Some(key) => KEYS
                .iter()
                .position(|candidate| *candidate == key)
                .map_or(0, |index| index + 1),
            None => 0,
        };
        let dialog = cx.weak_entity();
        let view = cx.new(|_| KeyMenu {
            name: name.to_owned(),
            dialog,
            focus: focus.clone(),
        });
        let opened = layers::open_popover(
            Popover::menu(view, cx.focus_handle(), anchor, focus[start].clone())
                // The key button, not what held focus before: on macOS the click that opened the menu left the button
                // unfocused, and the old menu returned focus to its button when it closed.
                .returns_to(ReturnTo::Control(button)),
            &mark,
            window,
            cx,
        );
        self.key_menu = opened.map(|layer| (name.to_owned(), layer));
        cx.notify();
    }
}

/// A template's number-key menu (the old `NumberShortcutMenu`): No shortcut, then 1 to 9 and 0, each saying which
/// template holds it. Choosing one closes the menu and writes the keys.
struct KeyMenu {
    /// The template the menu is for.
    name: String,
    dialog: WeakEntity<TemplatesDialog>,
    /// The items' focus handles, No shortcut first, so the menu can start on the key the template holds.
    focus: Vec<FocusHandle>,
}

impl KeyMenu {
    /// What choosing `key` (or No shortcut) does: close this menu, then write.
    fn choose(
        &self,
        key: Option<&'static str>,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let (dialog, name) = (self.dialog.clone(), self.name.clone());
        move |_, window, cx| {
            let _ = dialog.update(cx, |dialog, cx| {
                if let Some((_, layer)) = dialog.key_menu.take() {
                    layers::close(layer, window, cx);
                }
                dialog.choose_key(&name, key, cx);
            });
        }
    }
}

impl Render for KeyMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bindings = Bindings::current(cx);
        let name = HostText::from_host(self.name.clone());
        // The template's name over "Number shortcut", as the old flyout's header named its subject.
        let header = MenuHeader::new(
            &name,
            HostSentence::new("templates-key-menu-summary").text("Number shortcut"),
        );
        let none = MenuItem::new(
            "templates-key-none",
            Icon::Templates,
            "No shortcut",
            HoverHelp::tooltip("Remove this template's number shortcut"),
            Role::MenuItem,
        )
        .description("Remove this template's number shortcut")
        .focus(&self.focus[0])
        .on_click(self.choose(None));
        let keys = KEYS.iter().enumerate().map(|(index, &key)| {
            let item = MenuItem::new(
                format!("templates-key-{key}-item"),
                Icon::Templates,
                super::number_key_name(key),
                HoverHelp::tooltip(format!(
                    "Give this template {}",
                    super::number_key_name(key)
                )),
                Role::MenuItem,
            )
            .focus(&self.focus[index + 1])
            .on_click(self.choose(Some(key)));
            match bindings.keys.get(key) {
                Some(holder) if *holder == self.name => item.description("This template"),
                Some(holder) => item.description_sentence(
                    HostSentence::new(format!("templates-key-{key}-holder"))
                        .host(&HostText::from_host(holder.clone()))
                        .text(" (moves here)"),
                ),
                None => item.description("No template uses this key"),
            }
        });
        let label = HostSentence::new("templates-key-menu-label")
            .text("Number key for ")
            .host(&name)
            .accessible_name();
        // The header comes before the element with the menu role, so it is read as the menu's subject, not as one of
        // its commands (the shared header's rule).
        layers::panel(cx)
            .id("templates-key-menu")
            .debug_selector(|| "templates-key-menu".into())
            .w(px(KEY_MENU_WIDTH))
            .child(header)
            .child(
                div()
                    .id("templates-key-menu-items")
                    .role(Role::Menu)
                    .aria_label(label)
                    .flex()
                    .flex_col()
                    .child(none)
                    .children(keys),
            )
    }
}

/// The list's count line under the rows (`.templates-count`): "1 template", "3 templates".
pub(crate) fn count_text(count: usize) -> String {
    if count == 1 {
        "1 template".to_owned()
    } else {
        format!("{count} templates")
    }
}

impl Render for TemplatesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let bindings = Bindings::current(cx);
        // One write at a time: while a preference write or the editor's save is in flight every row, bolt and key
        // button waits, as the old list's controls did while it was busy.
        let (preferences_busy, refused) = match session::current_operations(cx) {
            Some(operations) => {
                let operations = operations.read(cx);
                let refused = self
                    .last_write
                    .zip(operations.failure(&OpKey::preferences()))
                    .map(|(write, failure)| {
                        HostSentence::new("templates-write-refused")
                            .text(write.refused())
                            .host(&failure.message)
                    });
                (
                    operations.in_flight(&OpKey::preferences()).is_some(),
                    refused,
                )
            }
            None => (false, None),
        };
        // A write refused before sending answers a later click than any write the helm refused.
        let refused = self.unread_refusal.clone().or(refused);
        let busy = preferences_busy
            || self.restoring
            || self.editor.as_ref().is_some_and(Editor::is_writing);
        let store = session::current_store(cx);
        let (templates, unavailable) = match &store {
            Some(store) => {
                let collections = store.read(cx).collections();
                let templates: Vec<LaunchTemplate> = collections
                    .ordered::<LaunchTemplate>()
                    .into_iter()
                    .cloned()
                    .collect();
                let unavailable = match collections
                    .meta(Collection::Templates)
                    .map(|meta| &meta.availability)
                {
                    Some(Availability::Unavailable { error }) => Some(error.clone()),
                    Some(Availability::Available) | None => None,
                };
                (templates, unavailable)
            }
            None => (Vec::new(), None),
        };

        let small = |text: String| {
            div()
                .text_size(metrics::TEXT_SM)
                .line_height(px(SMALL_LINE_HEIGHT))
                .text_color(theme::color(palette.fg_1))
                .child(text)
        };

        // `aside.templates-sidebar`: "+ New template", the rows, then the count pushed to the column's foot.
        let weak_new = cx.weak_entity();
        let new_template = Button::new(
            "templates-new",
            "+ New template",
            HoverHelp::tooltip("Start a new template. If you edited the open one, you can save or discard your changes."),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .focus(&self.new_focus)
        .busy(busy)
        .on_click(move |_, window, cx| {
            let _ = weak_new.update(cx, |dialog, cx| dialog.new_template(window, cx));
        });
        let list = div()
            .id("templates-list")
            .relative()
            .role(Role::List)
            .aria_label("Template list")
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .when(
                !templates.is_empty()
                    && !templates
                        .iter()
                        .any(|template| bindings.is_marked(&template.name)),
                |list| list.pb(px(ROW_CHORD_HEIGHT + ROW_LINE_GAP)),
            )
            .child(layers::beside_edge("templates-help-edge", window, cx))
            .children(
                templates
                    .iter()
                    .map(|template| self.row(template, &bindings, busy, window, cx))
                    .collect::<Vec<_>>(),
            );
        let sidebar = div()
            .flex_none()
            .w(px(LIST_WIDTH))
            .flex()
            .flex_col()
            .gap(px(LIST_COLUMN_GAP))
            .pr(px(LIST_COLUMN_PAD_RIGHT))
            .border_r_1()
            .border_color(theme::color(palette.border_dialog))
            .child(new_template)
            .child(list)
            .child(
                div()
                    .mt_auto()
                    .debug_selector(|| "templates-count".into())
                    .child(small(count_text(templates.len()))),
            );
        // `section.templates-editor`: the open template, or what to do to open one.
        let weak = cx.weak_entity();
        let editor = match &mut self.editor {
            Some(editor) => editor.render(busy, &bindings, self.catalog.as_deref(), &weak, cx),
            None => div()
                .id("templates-editor")
                .role(Role::Region)
                .aria_label("Template editor")
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .debug_selector(|| "templates-empty".into())
                        .text_color(theme::color(palette.fg_1))
                        .child("Choose a template to edit, or make a new one."),
                )
                .into_any_element(),
        };
        let panes = div()
            .flex()
            .gap(px(PANES_GAP))
            .min_h(px(PANES_MIN_HEIGHT))
            .child(sidebar)
            .child(editor);

        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(CONTENT_GAP))
            .child(small(help_text()));
        if let Some(hint) = self.hint {
            body = body.child(
                div()
                    .id("templates-hint")
                    .debug_selector(|| "templates-hint".into())
                    .role(Role::Status)
                    .child(small(hint.text())),
            );
        }
        body = body.child(panes);
        if let Some(refused) = refused {
            // A refused mark or key: the helm's words, or why the bindings could not be read, under the panes, until
            // the next preference write.
            body = body.child(
                div()
                    .debug_selector(|| "templates-write-refused".into())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child(refused),
            );
        }
        if let Some(status) = self
            .editor
            .as_ref()
            .and_then(Editor::status)
            .or(self.status.as_ref())
        {
            // The status line (`.templates-error[role=status]`): why a save, a delete or an Undo was refused, in the
            // app's or the helm's words, until the next one or another template opens.
            body = body.child(
                div()
                    .id("templates-editor-status")
                    .debug_selector(|| "templates-editor-status".into())
                    .role(Role::Status)
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child(status.clone()),
            );
        }
        if let Some(error) = unavailable {
            // `.templates-error`: the helm's own words, in the warning color, below both panes.
            body = body.child(
                div()
                    .debug_selector(|| "templates-unavailable".into())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child(
                        HostSentence::new("templates-unavailable")
                            .text("Templates unavailable: ")
                            .host(&HostText::from_host(error)),
                    ),
            );
        }
        if let Some(undo) = &self.undo {
            // `.templates-undo-notice`: "Deleted <name>." and Undo, on the raised surface, while Undo is offered.
            let weak = cx.weak_entity();
            body = body.child(
                div()
                    .id("templates-undo-notice")
                    .debug_selector(|| "templates-undo-notice".into())
                    .role(Role::Status)
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(10.))
                    .p(px(10.))
                    .rounded(metrics::RADIUS_SM)
                    .bg(theme::color(palette.bg_1))
                    .child(
                        HostSentence::new("templates-undo-text")
                            .text("Deleted ")
                            .host(&HostText::from_host(undo.template.name.clone()))
                            .text("."),
                    )
                    .child(
                        Button::new(
                            "templates-undo",
                            "Undo",
                            HoverHelp::tooltip(
                                "Restore the deleted template if its old name is still free",
                            ),
                            Role::Button,
                        )
                        .tier(Tier::Neutral)
                        .busy(busy)
                        .on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |dialog, cx| dialog.restore(window, cx));
                        }),
                    ),
            );
        }

        let writing = self.writing(cx);
        DialogFrame::new("templates", "Templates", body, Vec::new())
            .summary(SUMMARY)
            .tile(
                svg()
                    .path(Icon::Templates.path())
                    .size(px(TILE_GLYPH))
                    .text_color(theme::color(palette.fg_0)),
            )
            .width(px(WIDTH))
            .close(
                CLOSE_LABEL,
                HoverHelp::tooltip(CLOSE_HELP),
                // Held while a write is in flight, as the dismissal hook holds Escape (the old dialog disabled it).
                writing,
                dismiss(weak),
            )
    }
}
