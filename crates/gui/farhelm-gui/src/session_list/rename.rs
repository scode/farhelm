//! The naming editor changes a session title, changes a group name or
//! creates one group for a selection. All three share the draft and focus
//! rules. The helm validates the name for its subject.
//!
//! SPEC.md, "Lifecycle operations": "A list rename opens in a modal editor owned by the list rather than by the
//! row-actions popup." The editor is a dialog of the layer manager, centered over a veil, not a popover anchored to a
//! row, so a listing that reorders, filters or fails under it never moves or closes its field. Ported from the old
//! app's `rename.rs` (`RenameForm`, `RenameDialog`) and the editor ownership in `list/view.rs` at reference commit
//! `24bc0580`.
//!
//! Here's the TLDR of its rules:
//!
//! - **The draft.** The field is a [`DraftInput`] seeded from the session's raw title ([`HostText::raw`]), so saving it
//!   untouched sends back the helm's own bytes, hidden characters included. Until the user edits it, the draft follows
//!   a rename the helm pushes (the command line, an agent); once edited, a push never overwrites it (SPEC.md, "Things
//!   changing underneath": "A refresh never changes what the user is typing"). It does not follow while a save is in
//!   flight; an untouched draft whose save the helm refused takes up the current title again. The current title above
//!   the field always follows, escaped.
//! - **Saving.** Save, or Enter in the field, sends exactly what the field holds: no trimming, no emptiness check, no
//!   screening ([`command`]). The field keeps a pasted line feed ([`TextInputState::new_keeping_line_breaks`]),
//!   so a title the supervisor refuses reaches the supervisor, and its refusal is what the user reads. A refusal keeps
//!   the draft and shows the helm's words, escaped, under the field; a success closes the editor. While a save is in
//!   flight the field is read-only, Cancel and Close are disabled, Escape does nothing (the layer manager asks the
//!   editor's dismissal hook), and Save draws busy and ignores presses (the old app's busy rules; SPEC.md: "Close uses
//!   the same cancellation and busy-state rules as Cancel"). Save is busy rather than disabled because a disabled
//!   button gives up focus, and the old app kept a clicked Save focused.
//! - **Focus.** It starts in the title field, and Tab follows Close, the field, Cancel, Save (the dialog frame draws
//!   them in that order). Closing it, however it closes, moves focus only if focus is still inside it: that is the
//!   layer manager's rule, and it is also the rule decided for a successful save
//!   ([`crate::operations::Settled::user_moved_on`]). Opened from a row's menu, focus then goes back to what held it
//!   when the editor opened (the menu's button). Opened with the shortcut, it goes to the open session's terminal,
//!   whatever held focus when the shortcut was pressed (SPEC.md, "Desktop window chrome": "Closing that
//!   shortcut-opened dialog returns focus to the terminal"; [`ReturnTo::Home`]).
//! - **The session going away.** The editor has no layer subject, so nothing but the user closes it. When a complete
//!   listing no longer has the session ([`session::listing_proves_gone`]) it says so and disables Save, keeping the
//!   draft for the user to copy; a filtered, truncated or failed listing proves nothing and leaves Save enabled.
//! - **One request per session.** It does not open while the session has work in flight in the operation store (a
//!   Delete, say), as the old app refused it (`on_rename_start`'s `busy_now`), nor while the YOLO question its Replace
//!   met is open (`super::replace_yolo`); and a Save while other work started on the session is refused by the store.
//! - **A session the list does not show.** The editor reads the session from the store by key, never from the list,
//!   and the helm keeps the open session in the store while a filter or the listing cap leaves it out of the list
//!   (`super::filters`, "The query names the open session"). So the shortcut renames the open session "including when
//!   its row is filtered out" (SPEC.md, "Desktop window chrome"), seeded from its current title, and the editor follows
//!   its title there as for a listed one. Naming mode is the one case that opens without a store row: a just-created
//!   session can be missing from the store at its success, so it starts from the create reply's title instead.
//!
//! The old app's optimistic rename overlay is not ported: the UI channel pushes every change before the reply that
//! caused it (SPEC_impl.md, "The UI channel"), so the store already shows the new title when the save succeeds.
//!
//! **Naming mode.** A quick launch whose success opened its new session, New never having been shown, opens this editor
//! over it to name it ([`open_naming`]; SPEC.md, "Quick launch uses the same editor in naming mode after creation,
//! without holding up agent startup"). Only that mode starts with the whole name selected, so typing replaces it, and
//! treats a save of the name the session has now as keeping it, closing with no request: the user who presses Enter on
//! the created name accepted it, and ordinary Rename sends even an unchanged title so the supervisor judges it. A
//! changed name is an ordinary rename. However it closes, focus goes to the new session's terminal ([`ReturnTo::Home`],
//! as for the shortcut), which takes no input while the editor is up, since the editor is a modal dialog over it.

#[cfg(test)]
mod tests;

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Collections, Method, PathError};
use farhelm_proto::session_requests::UpdateSessionGroup;
use farhelm_proto::ui_channel::{Availability, Collection, SessionRow};
use gpui::{
    AnyWindowHandle, App, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, WeakEntity, Window, div,
    px,
};

use crate::components::{Button, DialogFrame, HoverHelp, Role, TextInput, TextInputState, Tier};
use crate::drafts::DraftInput;
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Dialog, DismissHook, Dismissal, LayerId, MainAction};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::store::HelmStore;
use crate::theme::{self, metrics};
use crate::{local_state, session};

use super::SessionList;
use super::groups;

/// Open the Rename editor for the session the helm files under `key`, on top of everything else in `window`. The one
/// opener: the row menu's Rename item calls it, and so does the shortcut ([`open_for_selected`]).
///
/// `mark` is the [`Mark`] taken in the handler of the user's action (the item's click); nothing opens when the user has
/// moved on since (the layer manager's rule). Focus starts in the title field, and closing the editor returns it to
/// whatever held focus when it opened ([`ReturnTo::Opener`]), so a caller that wants focus back somewhere
/// particular opens it from there (the row menu closes itself first, which leaves focus on its button).
///
/// Returns the editor's layer, or `None` when nothing opened: the window is inert, the session has work in flight (a
/// Delete, say: SPEC.md, "an open lifecycle confirmation or a pending request refuses another lifecycle action"), the
/// store holds no row for the session so there is no title to start from, the session is a retired containerized one
/// ([`super::retired`]: Delete only), or the user moved on. A refusal is silent, as
/// it was in the old app: the session's row already shows the work in flight.
pub fn open(key: &str, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    open_returning(key, mark, Mode::Ordinary(ReturnTo::Opener), window, cx)
}

/// Open the editor in naming mode (the module docs' "Naming mode") for the session a quick launch just created and
/// opened, under `key`: the name selected, an unchanged save closing with no request, focus going to that session's
/// terminal however it closes. The caller has decided the quick launch earned naming (New never shown, its success
/// opened the session, the preference on); `mark` is taken by that caller, and the opening is refused as [`open`]'s is,
/// except that a missing store row does not refuse it (below).
///
/// `created` is the session's title as the create's reply gave it, which the editor starts from while the store has
/// no row for the session yet: a session a filter or the listing cap leaves out of the list reaches the store only
/// once the list has asked the helm for the open session, which is after the success that opened it.
pub(crate) fn open_naming(
    key: &str,
    created: &str,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    open_returning(key, mark, Mode::Naming { created }, window, cx)
}

/// How the editor opens: an ordinary rename, closing back where its [`ReturnTo`] says, or naming mode (the module
/// docs), which always closes back to the new session's terminal. One enum, so naming cannot be asked for with another
/// return target.
enum Mode<'a> {
    /// The row menu's or the shortcut's rename of a session the store holds, closing back where the opener asks.
    Ordinary(ReturnTo),
    /// Naming a session just created, whose title the create's reply gave as `created`.
    Naming { created: &'a str },
}

/// `POST /api/sessions/{id}/rename` with `title` exactly as given: no trimming, no emptiness check, no screening of
/// control characters. The supervisor owns every rule about what a title may hold, and its refusal is the text the user
/// acts on; an empty title is a legal rename. Ported from the old app's `api::rename_session`.
pub(crate) fn command(session: &str, title: &str) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Rename,
        Method::Post,
        ApiPath::api(&["sessions", session, "rename"])?,
        Some(serde_json::json!({ "title": title })),
        "POST /api/sessions/[session]/rename",
    ))
}

/// [`open`] or [`open_naming`], as `mode` says.
fn open_returning(
    key: &str,
    mark: &Mark,
    mode: Mode<'_>,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let operations = session::current_operations(cx)?;
    if let Some(work) = operations.read(cx).in_flight(&OpKey::session(key)) {
        tracing::debug!(?work, "rename not opened: the session has work in flight");
        return None;
    }
    if super::replace_yolo::open(key, cx) {
        tracing::debug!("rename not opened: the session's YOLO question is open");
        return None;
    }
    let row = store.read(cx).collections().get::<SessionRow>(key);
    // A retired containerized session supports Delete only; its menu offers no Rename, and the shortcut must not
    // reach one either (`super::retired`).
    if row.is_some_and(super::retired::is) {
        tracing::debug!("rename not opened: the session is a retired containerized session");
        return None;
    }
    let stored = row.map(|row| row.info.title.clone());
    let (title, returns_to, naming) = match mode {
        Mode::Ordinary(returns_to) => (stored?, returns_to, false),
        Mode::Naming { created } => (
            stored.unwrap_or_else(|| created.to_owned()),
            ReturnTo::Home,
            true,
        ),
    };
    open_editor(
        Subject::Session {
            key: key.to_owned(),
            naming,
        },
        &title,
        returns_to,
        mark,
        window,
        cx,
    )
}

/// Open the same editor for a named group or for one new group containing
/// the selected sessions. The helm owns group-name validation.
pub(super) fn group(
    id: Option<i64>,
    sessions: Vec<String>,
    marked: Option<WeakEntity<SessionList>>,
    returns_to: ReturnTo,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let subject = id.map_or_else(|| Subject::NewGroup { sessions, marked }, Subject::Group);
    let operations = session::current_operations(cx)?;
    if operations.read(cx).in_flight(&subject.key()).is_some() {
        return None;
    }
    let store = session::current_store(cx)?;
    let title = subject.title(store.read(cx).collections())?;
    open_editor(subject, &title, returns_to, mark, window, cx)
}

/// One dialog lifecycle for names from any list item. A layer belongs to
/// its editor even when the list moves or hides the item beneath it.
fn open_editor(
    subject: Subject,
    title: &str,
    returns_to: ReturnTo,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let editor = cx.new(|cx| RenameEditor::new(subject, title, &store, window, cx));
    let start = editor.read(cx).title.input().focus_handle(cx);
    let weak = editor.downgrade();
    // Enter on the dialog itself (after a click on its empty space) saves, unless Save is unavailable.
    let main_action: MainAction = Rc::new(move |window, cx| {
        let _ = weak.update(cx, |editor, cx| editor.save(window, cx));
    });
    // Escape follows Cancel's busy rule: it closes nothing while a save is in flight, wherever focus is in the dialog.
    let weak = editor.downgrade();
    let on_dismiss: DismissHook = Rc::new(move |_, cx| {
        let saving = weak
            .upgrade()
            .is_some_and(|editor| editor.read(cx).title.draft().is_saving());
        if saving {
            Dismissal::Stay
        } else {
            Dismissal::Close
        }
    });
    let layer = layers::open_dialog(
        Dialog {
            view: editor.clone().into(),
            container: cx.focus_handle(),
            start,
            // None on purpose: the editor never closes when its session goes, it disables Save (module docs).
            subject: None,
            main_action: Some(main_action),
            // SPEC.md, "Opening and closing": Rename does not have the keep-focus-on-empty-click rule yet.
            keeps_focus_on_empty_click: false,
            returns_to,
            on_dismiss: Some(on_dismiss),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    editor.update(cx, |editor, _| editor.layer = Some(layer));
    #[cfg(test)]
    layers::record_opened(&editor, cx);
    Some(layer)
}

/// The Rename shortcut ([`crate::actions::RenameSession`]): open the editor for the open session, which in this app is
/// the selected one, listed or not (the module docs' "A session the list does not show"). With no session selected,
/// or while a dialog or menu holds the keyboard (SPEC.md: "A visible modal keeps keyboard ownership"; a popover that
/// never takes focus, such as a terminal link's destination display, is not one), it does nothing. The main
/// view calls it for the action wherever focus is, the terminal included, and the action is taken either way, as
/// `ONCE_PER_PRESS` requires.
pub(crate) fn open_for_selected(window: &mut Window, cx: &mut App) {
    // A key pressed inside a layer reaches the main view's handler too, since `root::wrap` draws the overlay inside the
    // main view's ground; this is what keeps the modal rule there.
    if layers::focus_layer_open(window, cx) {
        return;
    }
    let Some(key) = local_state::selected(cx) else {
        return;
    };
    let mark = Mark::take(cx);
    let _ = open_returning(&key, &mark, Mode::Ordinary(ReturnTo::Home), window, cx);
}

/// Model the subject separately from the dialog so session-only naming and
/// absence rules cannot affect a group rename or creation.
#[derive(Clone)]
enum Subject {
    Session {
        key: String,
        naming: bool,
    },
    Group(i64),
    NewGroup {
        sessions: Vec<String>,
        marked: Option<WeakEntity<SessionList>>,
    },
}

impl Subject {
    /// A save shares the subject's operation key with its other edits.
    fn key(&self) -> OpKey {
        match self {
            Self::Session { key, .. } => OpKey::session(key),
            Self::Group(id) => OpKey::session_group(*id),
            Self::NewGroup { .. } => OpKey::new_session_group(),
        }
    }

    /// Follow pushed names until the user types. Creation starts blank.
    fn title(&self, collections: &Collections) -> Option<String> {
        match self {
            Self::Session { key, .. } => collections
                .get::<SessionRow>(key)
                .map(|row| row.info.title.clone()),
            Self::Group(id) => groups::roster(collections)
                .iter()
                .find(|group| group.id == *id)
                .map(|group| group.name.clone()),
            Self::NewGroup { .. } => Some(String::new()),
        }
    }

    /// The group roster is complete even under a filter. A failed read
    /// does not prove disappearance. New group creation validates its targets.
    fn unavailable(&self, collections: &Collections, has_title: bool) -> bool {
        match self {
            Self::Session { key, .. } => session::listing_proves_gone(collections, key),
            Self::Group(_) => {
                !has_title
                    && collections
                        .meta(Collection::Sessions)
                        .is_some_and(|meta| meta.availability == Availability::Available)
            }
            Self::NewGroup { .. } => false,
        }
    }

    /// Send the exact draft so the subject's writer owns validation.
    fn command(&self, draft: &str) -> Result<Command, PathError> {
        match self {
            Self::Session { key, .. } => command(key, draft),
            Self::Group(id) => groups::update(
                *id,
                UpdateSessionGroup {
                    name: Some(draft.to_owned()),
                    ..Default::default()
                },
            ),
            Self::NewGroup { sessions, .. } => groups::create(draft, sessions),
        }
    }

    /// Quick launch's follow-up naming keeps its untouched title locally.
    fn naming_session(&self) -> bool {
        matches!(self, Self::Session { naming: true, .. })
    }
}

/// The open naming dialog tracks one subject and keeps the draft even if
/// the subject disappears while the user is typing.
pub(crate) struct RenameEditor {
    subject: Subject,
    /// The title the store holds now, shown escaped above the field. Kept when the session drops out of the listing.
    current: HostText,
    /// The title field (one row that keeps pasted line feeds) over its draft: the raw title until the user edits it,
    /// their text after.
    title: DraftInput,
    /// Whether a complete listing has proved the session gone, which disables Save.
    unavailable: bool,
    /// Set right after the layer opens; taken when the editor closes itself, so it closes once.
    layer: Option<LayerId>,
    /// The store's pushes.
    _pushes: Subscription,
}

impl RenameEditor {
    /// An editor over `key`, its draft and field holding `title` (raw), following `store` while it lives, in naming
    /// mode with the whole title selected when `naming` is set. The store is the current UI session's; a reset closes
    /// every dialog (`layers::close_all`), so the editor never outlives it.
    fn new(
        subject: Subject,
        title: &str,
        store: &Entity<HelmStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = TextInputState::new_keeping_line_breaks(window, cx);
        let title_field = DraftInput::new(
            title,
            input,
            |editor: &mut Self| &mut editor.title,
            Self::save,
            window,
            cx,
        );
        if subject.naming_session() {
            title_field.input().select_all(window, cx);
        }
        let pushes = cx.observe_in(store, window, |editor, store, window, cx| {
            editor.pushed(&store, window, cx)
        });
        RenameEditor {
            subject,
            current: HostText::from_host(title),
            title: title_field,
            unavailable: false,
            layer: None,
            _pushes: pushes,
        }
    }

    /// The store changed: follow the session's title where the draft allows, and recompute whether the listing proves
    /// the session gone. A session missing from a listing that proves nothing keeps the last title it showed.
    fn pushed(&mut self, store: &Entity<HelmStore>, window: &mut Window, cx: &mut Context<Self>) {
        let collections = store.read(cx).collections();
        let title = self.subject.title(collections);
        self.unavailable = self.subject.unavailable(collections, title.is_some());
        if let Some(title) = title {
            self.current = HostText::from_host(title.as_str());
            self.title.follow(&title, window, cx);
        }
        cx.notify();
    }

    /// Save the draft as the session's title, exactly as it stands. Does nothing while Save is unavailable or a save is
    /// in flight. A success closes the editor; a refusal is kept in the draft, which the editor draws. In naming mode a
    /// draft that is the session's current title closes the editor with nothing sent.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.unavailable || self.title.draft().is_saving() {
            return;
        }
        // Naming keeps the name the session has now, byte for byte, without asking the supervisor to set it again.
        if self.subject.naming_session()
            && self.title.input().text(cx).as_ref() == self.current.raw()
        {
            return self.cancel(window, cx);
        }
        let subject = self.subject.clone();
        let handle = window.window_handle();
        let started = DraftInput::save(
            self,
            |editor| &mut editor.title,
            &subject.key(),
            |title| subject.command(title),
            window,
            cx,
            move |editor, settled, cx| {
                if let Outcome::Done(_) = settled.outcome {
                    editor.close_later(handle, cx);
                }
            },
        );
        match started {
            Ok(()) => {
                // A marked metadata choice clears its marks when it sends
                // the write. Cancel before dispatch leaves them available.
                if let Subject::NewGroup {
                    marked: Some(list), ..
                } = &self.subject
                {
                    let _ = list.update(cx, |list, cx| list.clear_marks(cx));
                }
            }
            Err(not_started) => tracing::debug!(?not_started, "name not sent"),
        }
    }

    /// Cancel: close the editor, keeping the session's title. Refused while a save is in flight. (Close dismisses
    /// the editor instead, through the layer manager, so its dismissal hook decides as it does for Escape.)
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.title.draft().is_saving() {
            return;
        }
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
    }

    /// Close the editor from a save's completion, which has no window at hand: once the completion has returned, in
    /// `window` if it is still open.
    fn close_later(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        let Some(layer) = self.layer.take() else {
            return;
        };
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| layers::close(layer, window, cx));
        });
    }
}

/// A click handler that runs `act` on the editor, if it is still open.
fn on_editor(
    editor: WeakEntity<RenameEditor>,
    act: fn(&mut RenameEditor, &mut Window, &mut Context<RenameEditor>),
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let _ = editor.update(cx, |editor, cx| act(editor, window, cx));
    }
}

impl Render for RenameEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let busy = self.title.draft().is_saving();
        let weak = cx.weak_entity();
        let (heading, summary, field, help, unavailable) = match self.subject {
            Subject::Session { .. } => (
                "Rename session",
                "Change this session's title",
                "New session title",
                "The session's new title; Enter saves it",
                "This session is no longer available; copy the draft or Cancel",
            ),
            Subject::Group(_) => (
                "Rename group",
                "Change this group's name",
                "New group name",
                "The group's new name. Enter saves it.",
                "This group is no longer available. You can copy the draft or cancel.",
            ),
            Subject::NewGroup { .. } => (
                "New group",
                "Name this group of sessions",
                "Group name",
                "Enter saves the name and creates the group.",
                "",
            ),
        };
        // The buttons' words follow what is being named: a group has a name, not a title, and a new group is created,
        // so Save says Create and Cancel says nothing is made.
        let (save_label, save_help, cancel_help, close_label, close_help) = match self.subject {
            Subject::Session { .. } => (
                "Save",
                "Save the new title",
                "Cancel: keep the current title",
                "Close rename",
                "Close rename; keep the current title",
            ),
            Subject::Group(_) => (
                "Save",
                "Save the new name",
                "Cancel: keep the current name",
                "Close rename",
                "Close rename; keep the current name",
            ),
            Subject::NewGroup { .. } => (
                "Create",
                "Create the group",
                "Cancel: create no group",
                "Close new group",
                "Close new group; create no group",
            ),
        };
        let creating = matches!(self.subject, Subject::NewGroup { .. });
        // `.rename-body`: the current title, the field and what the editor has to say, 10 pixels apart.
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .children((!creating).then(|| {
                // `.rename-current-title`: one line, cut with an ellipsis, in the quieter text color.
                host_text::element(&self.current)
                    .min_w_0()
                    .truncate()
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1))
            }))
            .child(
                // The selector is for tests, which find the field by where it is drawn.
                div()
                    .debug_selector(|| "rename-field".into())
                    .child(TextInput::new(
                        "rename-title",
                        self.title.input(),
                        field,
                        HoverHelp::tooltip(help),
                    )),
            )
            .children(self.unavailable.then(|| {
                div()
                    .debug_selector(|| "rename-unavailable".into())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child(unavailable)
            }))
            .children(self.title.draft().refusal().map(|refusal| {
                // The helm's words may quote the refused title, so they are host text: escaped, as one labelled node.
                div()
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.danger))
                    .child(HostSentence::new("rename-refusal").host(refusal))
            }));
        let cancel = Button::new(
            "rename-cancel",
            "Cancel",
            HoverHelp::tooltip(cancel_help),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(busy)
        .on_click(on_editor(weak.clone(), RenameEditor::cancel));
        let save = Button::new(
            "rename-save",
            save_label,
            HoverHelp::tooltip(save_help),
            Role::Button,
        )
        .tier(Tier::Primary)
        // Busy, not disabled, while a save is in flight: a disabled button drops out of focus, and the old app kept a
        // clicked Save focused (`aria-disabled` only) so focus stayed on the action the user chose.
        .busy(busy)
        .disabled(self.unavailable)
        .on_click(on_editor(weak.clone(), RenameEditor::save));
        DialogFrame::new("rename", heading, body, vec![cancel, save])
            .summary(summary)
            .close(
                close_label,
                HoverHelp::tooltip(close_help),
                busy,
                // A dismissal, so the editor's dismissal hook decides, as it does for Escape. Called from the click,
                // not from inside an update of the editor: the hook reads the editor.
                move |_, window, cx| {
                    let layer = weak.upgrade().and_then(|editor| editor.read(cx).layer);
                    if let Some(layer) = layer {
                        layers::dismiss(layer, window, cx);
                    }
                },
            )
    }
}
