//! The Rename editor: a modal dialog that changes one session's title, opened from a row's menu or with the Rename
//! shortcut.
//!
//! SPEC.md, "Lifecycle operations": "A list rename opens in a modal editor owned by the list rather than by the
//! row-actions popup." The editor is a dialog of the layer manager, centered over a veil, not a popover anchored to a
//! row, so a listing that reorders, filters or fails under it never moves or closes its field. Ported from the old
//! app's `rename.rs` (`RenameForm`, `RenameDialog`) and the editor ownership in `list/view.rs` at reference commit
//! `24bc0580`.
//!
//! Here's the TLDR of its rules:
//!
//! - **The draft.** The field holds a [`Draft`] seeded from the session's raw title ([`HostText::raw`]), so saving it
//!   untouched sends back the helm's own bytes, hidden characters included. Until the user edits it, the draft follows
//!   a rename the helm pushes (another client, an agent); once edited, a push never overwrites it (SPEC.md, "Things
//!   changing underneath": "A refresh never changes what the user is typing"). It does not follow while a save is in
//!   flight; an untouched draft whose save the helm refused takes up the current title again. The current title above
//!   the field always follows, escaped.
//! - **Saving.** Save, or Enter in the field, sends exactly what the field holds: no trimming, no emptiness check, no
//!   screening ([`command`]). The field keeps a pasted line feed ([`TextInputState::new_keeping_line_breaks`]),
//!   so a title the supervisor refuses reaches the supervisor, and its refusal is what the user reads. A refusal keeps
//!   the draft and shows the helm's words, escaped, under the field; a success closes the editor. While a save is in
//!   flight the field is read-only, Cancel and Close are disabled, Escape does nothing (the layer manager asks the
//!   editor's dismiss guard), and Save is ignored (the old app's busy rules; SPEC.md: "Close uses the same cancellation
//!   and busy-state rules as Cancel"). Save keeps its enabled look meanwhile, because a disabled button gives up focus
//!   and the button component has no busy look.
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
//!   Delete, say), as the old app refused it (`on_rename_start`'s `busy_now`); and a Save while other work started on
//!   the session is refused by the store.
//!
//! The old app's optimistic rename overlay is not ported: the UI channel pushes every change before the reply that
//! caused it (SPEC_impl.md, "The UI channel"), so the store already shows the new title when the save succeeds.
//!
//! NOTE: Not ported yet. Quick launch's naming mode of this editor (SPEC.md: "Quick launch uses the same editor in
//! naming mode") comes with quick launch. A selected session the store does not list (past the listing cap, or filtered
//! out once filters exist) cannot be renamed yet, although SPEC.md has the shortcut work "including when its row is
//! filtered out": the store holds no title to seed the draft from, so the shortcut does nothing for it.

#[cfg(test)]
mod tests;

use std::rc::Rc;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyWindowHandle, App, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Subscription, WeakEntity, Window, div,
    px,
};

use crate::components::{
    Button, DialogFrame, HoverHelp, Role, TextInput, TextInputEvent, TextInputState, Tier,
};
use crate::drafts::{self, Draft};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Dialog, DismissGuard, LayerId, MainAction};
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::store::HelmStore;
use crate::theme::{self, metrics};
use crate::{local_state, session};

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
/// store does not list the session so there is no title to start from, or the user moved on. A refusal is silent, as
/// it was in the old app: the session's row already shows the work in flight.
pub fn open(key: &str, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    open_returning(key, mark, ReturnTo::Opener, window, cx)
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

/// [`open`], with closing returning focus where `returns_to` says.
fn open_returning(
    key: &str,
    mark: &Mark,
    returns_to: ReturnTo,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let operations = session::current_operations(cx)?;
    if let Some(work) = operations.read(cx).in_flight(&OpKey::session(key)) {
        tracing::debug!(?work, "rename not opened: the session has work in flight");
        return None;
    }
    let title = store
        .read(cx)
        .collections()
        .get::<SessionRow>(key)?
        .info
        .title
        .clone();
    let editor = cx.new(|cx| RenameEditor::new(key.to_owned(), &title, &store, window, cx));
    let start = editor.read(cx).input.focus_handle(cx);
    let weak = editor.downgrade();
    // Enter on the dialog itself (after a click on its empty space) saves, unless Save is unavailable.
    let main_action: MainAction = Rc::new(move |window, cx| {
        let _ = weak.update(cx, |editor, cx| editor.save(window, cx));
    });
    // Escape follows Cancel's busy rule: it closes nothing while a save is in flight, wherever focus is in the dialog.
    let weak = editor.downgrade();
    let may_dismiss: DismissGuard = Rc::new(move |cx| {
        weak.upgrade()
            .is_none_or(|editor| !editor.read(cx).draft.is_saving())
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
            may_dismiss: Some(may_dismiss),
        },
        mark,
        window,
        cx,
    )?;
    editor.update(cx, |editor, _| editor.layer = Some(layer));
    #[cfg(test)]
    cx.set_global(tests::Opened(editor.downgrade()));
    Some(layer)
}

/// The Rename shortcut ([`crate::actions::RenameSession`]): open the editor for the open session, which in this app is
/// the selected one (the module docs' NOTE says which selected session it cannot rename yet). With no session selected,
/// or while any dialog or menu is open (SPEC.md: "A visible modal keeps keyboard ownership"), it does nothing. The main
/// view calls it for the action wherever focus is, the terminal included, and the action is taken either way, as
/// `ONCE_PER_PRESS` requires.
pub(crate) fn open_for_selected(window: &mut Window, cx: &mut App) {
    // Today a key pressed inside a layer never reaches the main view's handler, since focus is trapped in the layer
    // and layers dispatch from the overlay. A layer that leaves focus on the page (the NOTE on `layers::trap`'s
    // suggestion lists) would let it through, and this keeps the modal rule then.
    if layers::any_open(window, cx) {
        return;
    }
    let Some(key) = local_state::selected(cx) else {
        return;
    };
    let mark = Mark::take(cx);
    let _ = open_returning(&key, &mark, ReturnTo::Home, window, cx);
}

/// The open editor: which session it renames, its draft and field, what it shows about the session, and its layer.
pub(crate) struct RenameEditor {
    /// The session's key. Fixed for the editor's life: the editor renames this session or nothing.
    key: String,
    /// The title the store holds now, shown escaped above the field. Kept when the session drops out of the listing.
    current: HostText,
    /// What the user is saving: the raw title until they edit the field, their text after. The field mirrors it.
    draft: Draft<String>,
    /// The title field, one row that keeps pasted line feeds.
    input: TextInputState,
    /// Whether a complete listing has proved the session gone, which disables Save.
    unavailable: bool,
    /// Set right after the layer opens; taken when the editor closes itself, so it closes once.
    layer: Option<LayerId>,
    /// The field's events and the store's pushes.
    _subscriptions: [Subscription; 2],
}

impl RenameEditor {
    /// An editor over `key`, its draft and field holding `title` (raw), following `store` while it lives. The store is
    /// the current UI session's; a reset closes every dialog (`layers::close_all`), so the editor never outlives it.
    fn new(
        key: String,
        title: &str,
        store: &Entity<HelmStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = TextInputState::new_keeping_line_breaks(window, cx);
        input.set_text(title.to_owned(), window, cx);
        let events = input.subscribe(window, cx, |editor, event, window, cx| match event {
            TextInputEvent::Changed => editor.edited(window, cx),
            TextInputEvent::Submitted => editor.save(window, cx),
        });
        let pushes = cx.observe_in(store, window, |editor, store, window, cx| {
            editor.pushed(&store, window, cx)
        });
        RenameEditor {
            key,
            current: HostText::from_host(title),
            draft: Draft::load(title.to_owned()),
            input,
            unavailable: false,
            layer: None,
            _subscriptions: [events, pushes],
        }
    }

    /// The user changed the field: mirror it into the draft. While a save is in flight the field is read-only, so an
    /// edit arrives then only if it was queued before the field became read-only; the draft refuses it
    /// ([`Draft::edit`]) and the field is put back to the value being saved. That moves the caret to the start, which
    /// is why read-only is the main guard and this only the backstop.
    fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.text(cx).to_string();
        if !self.draft.edit(text) {
            self.input.set_text(self.draft.value().clone(), window, cx);
        }
        cx.notify();
    }

    /// The store changed: follow the session's title where the draft allows, and recompute whether the listing proves
    /// the session gone. A session missing from a listing that proves nothing keeps the last title it showed.
    fn pushed(&mut self, store: &Entity<HelmStore>, window: &mut Window, cx: &mut Context<Self>) {
        let collections = store.read(cx).collections();
        self.unavailable = session::listing_proves_gone(collections, &self.key);
        let title = collections
            .get::<SessionRow>(&self.key)
            .map(|row| row.info.title.clone());
        if let Some(title) = title {
            self.current = HostText::from_host(title.as_str());
            if self.draft.follow(title) {
                // `set_text` raises no change event, so following never marks the draft touched.
                self.input.set_text(self.draft.value().clone(), window, cx);
            }
        }
        cx.notify();
    }

    /// Save the draft as the session's title, exactly as it stands. Does nothing while Save is unavailable or a save is
    /// in flight. A success closes the editor; a refusal is kept in the draft, which the editor draws.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.unavailable || self.draft.is_saving() {
            return;
        }
        let key = self.key.clone();
        let handle = window.window_handle();
        let started = drafts::save(
            self,
            |editor| &mut editor.draft,
            &OpKey::session(&key),
            |title| command(&key, title),
            cx,
            move |editor, settled, cx| {
                editor.input.set_read_only(false, cx);
                match settled.outcome {
                    Outcome::Done(_) => editor.close_later(handle, cx),
                    Outcome::Refused(_) => editor.follow_after_refusal(handle, cx),
                }
            },
        );
        match started {
            // The text on screen must stay the text sent, and read-only keeps the caret and undo history where they
            // are. `edited` still undoes an edit that was queued before this took effect.
            Ok(()) => self.input.set_read_only(true, cx),
            // Another operation started on the session meanwhile, or the window went inert; the draft is untouched.
            Err(not_started) => tracing::debug!(?not_started, "rename not sent"),
        }
    }

    /// A save the helm refused has settled. An untouched draft did not follow pushes while it was saving, so it takes
    /// up the current title now: saving it again must not send back a title someone else has since replaced. A touched
    /// draft keeps what the user typed. The field is set once the completion has returned, since it has no window.
    fn follow_after_refusal(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        if !self.draft.follow(self.current.raw().to_owned()) {
            return;
        }
        let input = self.input.clone();
        let value = self.draft.value().clone();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| input.set_text(value, window, cx));
        });
    }

    /// Cancel or Close: close the editor, keeping the session's title. Refused while a save is in flight.
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.is_saving() {
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
        let busy = self.draft.is_saving();
        let weak = cx.weak_entity();
        // `.rename-body`: the current title, the field and what the editor has to say, 10 pixels apart.
        let body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                // `.rename-current-title`: one line, cut with an ellipsis, in the quieter text color.
                host_text::element(&self.current)
                    .min_w_0()
                    .truncate()
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1)),
            )
            .child(
                // The selector is for tests, which find the field by where it is drawn.
                div()
                    .debug_selector(|| "rename-field".into())
                    .child(TextInput::new(
                        "rename-title",
                        &self.input,
                        "New session title",
                        HoverHelp::tooltip("The session's new title; Enter saves it"),
                    )),
            )
            .children(self.unavailable.then(|| {
                div()
                    .debug_selector(|| "rename-unavailable".into())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child("This session is no longer available; copy the draft or Cancel")
            }))
            .children(self.draft.refusal().map(|refusal| {
                // The helm's words may quote the refused title, so they are host text: escaped, as one labelled node.
                div()
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.danger))
                    .child(HostSentence::new("rename-refusal").host(refusal))
            }));
        let cancel = Button::new(
            "rename-cancel",
            "Cancel",
            HoverHelp::tooltip("Cancel: keep the current title"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(busy)
        .on_click(on_editor(weak.clone(), RenameEditor::cancel));
        let save = Button::new(
            "rename-save",
            "Save",
            HoverHelp::tooltip("Save the new title"),
            Role::Button,
        )
        .tier(Tier::Primary)
        // Not disabled while a save is in flight: a disabled button drops out of focus, and the old app kept a
        // clicked Save focused (`aria-disabled` only) so focus stayed on the action the user chose. `save` ignores it
        // meanwhile.
        .disabled(self.unavailable)
        .on_click(on_editor(weak.clone(), RenameEditor::save));
        DialogFrame::new("rename", "Rename session", body, vec![cancel, save])
            .summary("Change this session's title")
            .close(
                "Close rename",
                HoverHelp::tooltip("Close rename; keep the current title"),
                busy,
                on_editor(weak, RenameEditor::cancel),
            )
    }
}
