//! The session launcher's dialog: what the user sees of a [`LaunchDraft`] and how their input reaches it.
//!
//! SPEC.md, "Creation" (the launcher layout paragraph and the launch composer bullet) and "Keyboard focus". Ported from
//! the old app's `list/create_form.rs` and `launch_controls.rs`, and the `.create-session-form` and `.launch-composer-*`
//! rules in its stylesheet, at reference commit `24bc0580`. Here's the TLDR:
//!
//! - **The frame.** The shared dialog frame, 960 pixels wide, with the old header (the Launch tile, a title and a
//!   summary naming the mode, and Close), a scrolling body, and a pinned footer: a summary of what Launch would start,
//!   then Cancel, Reset choices (Agent tab) and the main action, with what a press of it has to say pinned above them.
//! - **The body**, in reading and Tab order: the destination rows (Folder, Name), the Agent and Command tabs, and the
//!   chosen tab's fields. The other tab's fields are not drawn, but its draft is kept (SPEC.md: "switching tabs keeps
//!   each tab's draft"). The Agent tab's harness, effort and permission choices are drawn by [`choices`].
//! - **Input to edits.** Every field's change becomes one of the draft's edit methods; nothing here decides a launch
//!   rule. A field the draft changes on its own (Reset choices clearing the wrapper prefix) is put back in step after
//!   the edit.
//! - **The main action.** Launch (Replace, Restart for those modes) is drawn retired while [`LaunchDraft::refusal`]
//!   names a reason, and the reason is its hover text; a press, or Enter on the dialog, a single-line field or a
//!   choice, shows the reason beside it in the bordered alert, "Not launched." first ("Restart error." in Restart
//!   with). A press of a complete New or Clone launches it ([`super::launch`]): the dialog closes, and the same
//!   launcher comes back with the helm's words in that alert if the helm refuses. Its accessible name keeps the
//!   operation and its target, as the old one did.
//! - **Reads.** The model catalog is read when the launcher opens (`GET /api/launch-catalog`), and again on Retry
//!   after a failed read. Hosts, sessions and the remembered permission are read from the store while drawing, and the
//!   remembered wrapper prefixes when an agent type is chosen.
//!
//! NOTE: Not built yet, each with the piece that brings it: sending a Restart with or Replace with (until then a press
//! of a complete draft there does nothing); marking the folder field when the helm refuses the folder; Restart with's
//! markers for changed settings; the composer search; recent setups, Browse and the folder
//! check, with its warning on the folder field; managed checkouts, with their folder and name placeholders; the escaped
//! display of a prompt, wrapper prefix, Custom flags, seeded model, command line or resume command holding invisible
//! characters, with Clear wrapper prefix, Clear agent command and Clear resume command; and keyboard scrolling of an overflowing notice (the old region took focus once it overflowed;
//! today's refusals are one or two lines). The model field is `model_field`, the host picker `host_picker`.

use std::cell::Cell;
use std::rc::Rc;

use farhelm_helm_client::ApiPath;
use farhelm_proto::launcher::LaunchCatalogModel;
use farhelm_proto::ui_channel::{
    HostStateView, HostView, PREFERENCES_KEY, Preferences, SessionRow,
};
use farhelm_proto::{LaunchHarness, LaunchPermission, SessionLaunch, split_wrapper_prefix};
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, ElementId, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px, svg,
};

use super::draft::{Field, LaunchDraft, Mode, Tab, harness_label, remembered_permission};
use super::launch;
use super::request::{Target, capitalized};
use super::words::agent_note;
use crate::assets::Icon;
use crate::components::{
    self, Button, Checkbox, DialogFrame, HoverHelp, RadioGroup, RadioOption, Role, Select,
    SelectOption, TabOption, TabStrip, TextInput, TextInputEvent, TextInputState, Tier,
};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity::{display_name, phase};
use crate::layers::{self, Dialog, DialogPlacement, LayerId, MainAction};
use crate::permission::words::{effort_label, permission_display};
use crate::store::HelmStore;
use crate::theme::{self, metrics};
use crate::{operations, session};

mod choices;
mod host_picker;
mod model_field;
pub(in crate::launcher) mod yolo_question;

/// `.launch-composer-backdrop .create-session-form`: `width: min(960px, 100%)`.
const WIDTH: f32 = 960.;
/// `.launch-composer-row`: the label column (`grid-template-columns: 104px minmax(0, 1fr)`), and the gaps between
/// rows and between a label and its control.
const LABEL_COLUMN: f32 = 104.;
const ROW_GAP: f32 = 12.;
const LABEL_GAP: f32 = 12.;
/// `.launch-composer-columns`: the destination block and the choices are 20 pixels apart.
const SECTION_GAP: f32 = 20.;
/// `.create-session-form .dialog-tile .action-icon`: the header tile's glyph size.
const TILE_ICON: f32 = 22.;
/// The Command tab's agent type select's value for no declared agent; every other value is an agent's label, which are
/// unique.
const NO_AGENT: &str = "none";
/// The initial prompt starts three rows high, as the old `<textarea rows=3>`.
const PROMPT_ROWS: usize = 3;

/// Open the launcher on `draft`, on top of everything in `window`. `mark` is the [`Mark`] taken in the handler of the
/// user's action that asked for it; nothing opens if the user moved on since (the layer manager's rule) or no UI
/// session is connected. Returns the launcher's layer.
///
/// Focus starts on the first field, Restart with's on Cancel (SPEC.md: "Ordinary launches still start focus on search;
/// Restart starts on Cancel"; the search box comes later, and until then its place is the first field's). Closing
/// returns focus to whatever held it when the launcher opened.
pub fn open(draft: LaunchDraft, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    open_noted(draft, None, mark, window, cx)
}

/// [`open`], with `host_note` said under the Host row until the user picks a host: why an opening did not keep the host
/// its source names (Clone's, as the old launcher's `.create-session-host-note` said it). Something else an opening
/// has to say (a banner, a refusal) belongs beside the note, as another argument here.
pub(in crate::launcher) fn open_noted(
    draft: LaunchDraft,
    host_note: Option<&'static str>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    let store = session::current_store(cx)?;
    let launcher = cx.new(|cx| {
        let mut launcher = Launcher::new(draft, &store, window, cx);
        launcher.host_picker.note = host_note;
        launcher
    });
    let layer = show(&launcher, mark, window, cx)?;
    read_catalog(launcher.downgrade(), cx);
    #[cfg(test)]
    cx.set_global(super::view_tests::Opened(launcher.downgrade()));
    Some(layer)
}

/// Draw `launcher` as the launcher's dialog, on top of everything in `window`: a new launcher from [`open`], or one a
/// submit hid, brought back with its draft (`launch`). Refused, returning `None`, as [`open`] is: if the user moved on
/// since `mark`, or with no UI session. The layer holds the launcher from then on, and closing it drops the launcher
/// unless something else holds it too.
pub(super) fn show(
    launcher: &Entity<Launcher>,
    mark: &Mark,
    window: &mut Window,
    cx: &mut App,
) -> Option<LayerId> {
    session::current_store(cx)?;
    let start = launcher.read(cx).start_focus(cx);
    let weak = launcher.downgrade();
    // Enter on the dialog itself (after a click on its empty space) presses the main action.
    let dismissing = weak.clone();
    let main_action: MainAction = Rc::new(move |window, cx| {
        let _ = weak.update(cx, |launcher, cx| launcher.press(window, cx));
    });
    let layer = layers::open_dialog(
        Dialog {
            view: launcher.clone().into(),
            container: cx.focus_handle(),
            start,
            subject: None,
            main_action: Some(main_action),
            // SPEC.md, "Opening and closing": "A primary click on empty space keeps focus on the control in use in the
            // session launcher, Restart with".
            keeps_focus_on_empty_click: true,
            returns_to: ReturnTo::Opener,
            // Escape discards the draft, as Cancel and the header Close do, except while the YOLO question's host
            // write is out (Cancel and Close refuse then too, in `Launcher::cancel`).
            on_dismiss: Some(Rc::new(move |_, cx| {
                if dismissing
                    .upgrade()
                    .is_some_and(|launcher| launcher.read(cx).yolo_in_flight(cx))
                {
                    layers::Dismissal::Stay
                } else {
                    layers::Dismissal::Close
                }
            })),
            placement: DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    launcher.update(cx, |launcher, cx| {
        launcher.layer = Some(layer);
        // A launch the helm refused for YOLO comes back with its question, asked now that it can open in place.
        launcher.ask_pending_yolo(mark, window, cx);
    });
    Some(layer)
}

/// Read the release's model catalog for `launcher`. A failed read leaves it unread, which refuses no choice (the
/// draft's rule for an unread catalog), and says why beside a Retry, which reads it again (the old launcher's
/// "Model catalog unavailable", so a failed read never makes the user reopen the launcher and lose its draft).
fn read_catalog(launcher: WeakEntity<Launcher>, cx: &mut App) {
    let Ok(path) = ApiPath::api(&["launch-catalog"]) else {
        return;
    };
    operations::read::<Vec<LaunchCatalogModel>>(
        path,
        "GET /api/launch-catalog",
        cx,
        move |answer, cx| {
            let _ = launcher.update(cx, |launcher, cx| {
                match answer {
                    Ok(catalog) => launcher.catalog = Some(catalog),
                    Err(error) => launcher.catalog_error = Some(error),
                }
                cx.notify();
            });
        },
    );
}

/// The open launcher. See the module docs.
pub(crate) struct Launcher {
    pub(super) draft: LaunchDraft,
    /// The store, read while drawing for the hosts, the source session and the preferences.
    store: Entity<HelmStore>,
    /// The model catalog once read; `None` until then, or after a failed read.
    catalog: Option<Vec<LaunchCatalogModel>>,
    /// Why the last catalog read failed, until a retry starts.
    catalog_error: Option<HostText>,
    /// What the last agent-choice edit cleared or refused, said under the agent choices until the next one.
    notice: Option<String>,
    /// Whether the main action was pressed since the draft last changed, which shows its reason beside it.
    pressed: bool,
    /// This launcher's last launch, once the helm refused it: its key, under which the operation store keeps the
    /// refusal's words (read from the store when drawing, never copied here), and whether the reply says the create may
    /// still happen, which the store does not keep. Shown beside the main action until the draft changes or Launch is
    /// pressed again (`launch`).
    refused: Option<launch::RefusedLaunch>,
    /// Whether the keystroke being handled is a held Enter, which never launches. See where it is made, in `new`.
    pub(super) held_enter: Rc<Cell<bool>>,
    pub(super) folder: TextInputState,
    pub(super) name: TextInputState,
    conversation: TextInputState,
    pub(super) wrapper: TextInputState,
    pub(super) prompt: TextInputState,
    /// The Custom permission flags, as typed.
    pub(super) flags: TextInputState,
    /// The model field's text: the chosen model while closed, what is typed while its list is open.
    pub(super) model_input: TextInputState,
    /// The model field's list and what the user is doing in it.
    model_field: model_field::ModelField,
    /// The host picker's list, highlight and typeahead.
    host_picker: host_picker::HostPicker,
    /// The host button's focus handle; leaving it closes the host list.
    pub(super) host_focus: gpui::FocusHandle,
    command: TextInputState,
    resume: TextInputState,
    /// Cancel's focus handle, where Restart with starts.
    pub(super) cancel_focus: gpui::FocusHandle,
    layer: Option<LayerId>,
    /// The YOLO question for a launch the helm refused because its host asks first, from the refusal until it is
    /// answered or the draft changes (`yolo_question`).
    yolo: Option<yolo_question::YoloState>,
    _fields: Vec<Subscription>,
}

/// Which text field an event came from.
#[derive(Clone, Copy)]
enum Input {
    Folder,
    Name,
    Conversation,
    Wrapper,
    Prompt,
    Flags,
    Command,
    Resume,
}

impl Launcher {
    fn new(
        draft: LaunchDraft,
        store: &Entity<HelmStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = |text: &str, window: &mut Window, cx: &mut Context<Self>| {
            let state = TextInputState::new(window, cx);
            state.set_text(text.to_owned(), window, cx);
            state
        };
        let folder = field(draft.folder.value(), window, cx);
        let name = field(draft.name.value(), window, cx);
        let conversation = field(&draft.conversation, window, cx);
        // The mode never changes while the launcher is open, so neither does this placeholder.
        let required =
            matches!(&draft.mode, Mode::RestartWith(restart) if restart.conversation_required);
        conversation.set_placeholder(
            if required {
                "Required for this restart"
            } else {
                "Optional, this start only"
            },
            window,
            cx,
        );
        let wrapper = field(&draft.agent.wrapper, window, cx);
        wrapper.set_placeholder("with-endpoint --profile work --", window, cx);
        let prompt = TextInputState::new_growing(PROMPT_ROWS, window, cx);
        prompt.set_text(draft.prompt.clone(), window, cx);
        let flags = field(&draft.agent.permission_flags, window, cx);
        let command = field(&draft.command.command, window, cx);
        let resume = field(&draft.command.resume, window, cx);
        // The model field is not a plain text edit: what is typed filters its list, and Enter, focus and blur drive it
        // (`model_field`), so it has its own subscriptions.
        let model_input = field(
            draft
                .agent
                .model
                .as_deref()
                .unwrap_or(model_field::HARNESS_DEFAULT),
            window,
            cx,
        );
        let model_handle = model_input.focus_handle(cx);
        let model_events = model_input.subscribe(
            window,
            cx,
            |launcher: &mut Self, event, window, cx| match event {
                TextInputEvent::Changed => launcher.model_typed(window, cx),
                TextInputEvent::Submitted { .. } => launcher.model_enter(window, cx),
            },
        );
        // GPUI runs focus listeners inside the frame's focus phase, where a redraw request is dropped (`focus.rs` works
        // around the same trap for its own listener), so opening and closing the list there would show only on some
        // later frame. Each is deferred to just after, where it redraws at once.
        let model_focus = cx.on_focus(&model_handle, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.model_focused(window, cx)
            });
        });
        let model_blur = cx.on_blur(&model_handle, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.model_blurred(window, cx)
            });
        });
        // The host list closes when the host button loses focus (the old picker's blur), deferred for the same reason.
        // Tab stops, as `focus::control_handle` makes every control's: the launcher owns these two because it focuses
        // or watches them (Restart with starts on Cancel; leaving the host button closes its list).
        let host_focus = cx.focus_handle().tab_stop(true);
        let host_blur = cx.on_blur(&host_focus, window, |_, window, cx| {
            cx.defer_in(window, |launcher, window, cx| {
                launcher.close_host_list(window, cx);
                cx.notify();
            });
        });
        let mut fields: Vec<Subscription> =
            [
                (&folder, Input::Folder),
                (&name, Input::Name),
                (&conversation, Input::Conversation),
                (&wrapper, Input::Wrapper),
                (&prompt, Input::Prompt),
                (&flags, Input::Flags),
                (&command, Input::Command),
                (&resume, Input::Resume),
            ]
            .into_iter()
            .map(|(state, which)| {
                let input = state.clone();
                state.subscribe(window, cx, move |launcher: &mut Self, event, window, cx| {
                    match event {
                        TextInputEvent::Changed => {
                            launcher.edited(which, input.text(cx).to_string(), cx)
                        }
                        TextInputEvent::Submitted { .. } => {
                            if !launcher.held_enter.get() {
                                launcher.press(window, cx);
                            }
                        }
                    }
                })
            })
            .collect();
        // SPEC.md: "A held Enter never launches". A single-line field reports Submitted on every auto-repeat of Enter
        // (its `held` says which; the launcher still keeps its own note of it, below), so this notes, before each
        // keystroke is dispatched, whether it is a held Enter; the fields' Submitted and the model field's Enter read
        // it while that keystroke is handled. Buttons, the choices and the dialog's own Enter already ignore a held
        // key.
        let held_enter = Rc::new(Cell::new(false));
        let noting = held_enter.clone();
        let held_watch = cx.intercept_keystrokes(move |event, _, _| {
            noting.set(
                event
                    .key_down
                    .as_ref()
                    .is_some_and(|down| down.is_held && down.keystroke.key == "enter"),
            );
        });
        fields.extend([model_events, model_focus, model_blur, host_blur, held_watch]);
        Launcher {
            draft,
            store: store.clone(),
            catalog: None,
            catalog_error: None,
            notice: None,
            pressed: false,
            refused: None,
            held_enter,
            folder,
            name,
            conversation,
            wrapper,
            prompt,
            flags,
            model_input,
            model_field: model_field::ModelField::default(),
            host_picker: host_picker::HostPicker::default(),
            host_focus,
            command,
            resume,
            cancel_focus: cx.focus_handle().tab_stop(true),
            layer: None,
            yolo: None,
            _fields: fields,
        }
    }

    /// Where focus starts: Cancel in Restart with, the first field otherwise.
    fn start_focus(&self, cx: &App) -> gpui::FocusHandle {
        match self.draft.mode {
            Mode::RestartWith(_) => self.cancel_focus.clone(),
            Mode::New | Mode::Clone | Mode::ReplaceWith { .. } => self.folder.focus_handle(cx),
        }
    }

    /// The user changed field `which` to `text`: the draft's edit for it. Any edit takes the shown reason away until the
    /// next press, so the user is not told about a draft they already changed.
    fn edited(&mut self, which: Input, text: String, cx: &mut Context<Self>) {
        match which {
            Input::Folder => self.draft.edit_folder(text),
            Input::Name => self.draft.edit_name(text),
            Input::Conversation => self.draft.edit_conversation(text),
            Input::Wrapper => self.draft.edit_wrapper(text),
            Input::Prompt => self.draft.edit_prompt(text),
            Input::Flags => self.draft.edit_permission_flags(text),
            Input::Command => self.draft.edit_command(text),
            Input::Resume => self.draft.edit_command_resume(text),
        }
        self.changed(cx);
    }

    /// The draft changed: take the shown reason away until the next press, so the user is never told about a draft
    /// they already changed, and redraw. Every edit, whatever control made it, ends here.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.pressed = false;
        self.refused = None;
        // A YOLO question asks about the launch that was sent; once the draft changes, it would answer for another.
        self.yolo = None;
        cx.notify();
    }

    /// The helm refused this launcher's launch (`refused`): show the refusal the operation store keeps under its key
    /// beside the main action when the launcher is next drawn.
    pub(super) fn refused(&mut self, refused: launch::RefusedLaunch, cx: &mut Context<Self>) {
        self.pressed = false;
        self.refused = Some(refused);
        cx.notify();
    }

    /// The helm's words refusing this launcher's last launch, as the operation store keeps them, while they are shown.
    fn refusal_words(&self, cx: &App) -> Option<HostText> {
        let key = &self.refused.as_ref()?.key;
        let operations = session::current_operations(cx)?;
        operations
            .read(cx)
            .failure(key)
            .map(|failure| failure.message.clone())
    }

    /// Put the text fields the draft can change on its own back in step with it (after Reset choices, say). A field
    /// already showing the draft's value is left alone, so its caret and selection stay where they were.
    fn sync_fields(&self, window: &mut Window, cx: &mut Context<Self>) {
        let pairs = [
            (&self.wrapper, self.draft.agent.wrapper.clone()),
            (&self.conversation, self.draft.conversation.clone()),
            (&self.folder, self.draft.folder.value().clone()),
            (&self.prompt, self.draft.prompt.clone()),
            (&self.flags, self.draft.agent.permission_flags.clone()),
        ];
        for (state, value) in pairs {
            if state.text(cx).as_ref() != value {
                state.set_text(value, window, cx);
            }
        }
        // The model field shows the chosen model only while its list is closed; open, it holds what is being typed.
        if !self.model_list_open(window, cx) {
            let shown = self.model_display();
            if self.model_input.text(cx).as_ref() != shown {
                self.model_input.set_text(shown, window, cx);
            }
        }
    }

    /// The hosts as the store holds them, the catalog as read, for the draft's launch check.
    fn hosts(&self, cx: &App) -> Vec<HostView> {
        self.store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .cloned()
            .collect()
    }

    /// The source session's stored launch as the store holds it now, for Restart with's check.
    fn live_launch(&self, cx: &App) -> Option<SessionLaunch> {
        let Mode::RestartWith(restart) = &self.draft.mode else {
            return None;
        };
        self.store
            .read(cx)
            .collections()
            .get::<SessionRow>(&restart.source)
            .map(|row| row.info.launch.clone())
    }

    /// Why the draft cannot launch now, if it cannot. A host change in flight comes before every other reason, in every
    /// mode: the draft may be complete, and it is the window, not the draft, that is not ready.
    ///
    /// Restart with also refuses while the model field holds a model not yet applied (typed into its open list, or
    /// said under it as not applicable), after the host's own reasons and before the draft's others, as the old
    /// launcher did: restarting then would quietly keep the session's old model while the field shows another.
    fn refusal(&self, cx: &App) -> Option<String> {
        if crate::hosts::host_change_in_flight(cx) {
            return Some(launch::HOST_CHANGE_IN_FLIGHT.to_owned());
        }
        let hosts = self.hosts(cx);
        if matches!(self.draft.mode, Mode::RestartWith(_)) && self.model_pending(cx) {
            return self
                .draft
                .host_refusal(&hosts)
                .or_else(|| Some("Choose a model before restarting".to_owned()));
        }
        let target = Target {
            hosts: &hosts,
            catalog: self.catalog.as_deref(),
        };
        self.draft.refusal(target, self.live_launch(cx).as_ref())
    }

    /// The main action was pressed (the button, or Enter on the dialog, a single-line field or a choice): launch, or
    /// show why it cannot.
    ///
    /// A complete New or Clone sends its create ([`launch::start`]) and hides: the layer closes, and the launcher waits
    /// for the reply out of sight. The press is the user's answer the reply's moved-on check counts from, so this runs
    /// in the handler of that press, never deferred.
    ///
    /// NOTE: Restart with and Replace with send nothing yet; they come with the pieces that port them, and until then a
    /// press of a complete draft there does nothing.
    pub(super) fn press(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // While the YOLO question's host write is out, its answer owns the launch: a press now would send a create
        // without consent beside the one the answer sends with it.
        if self.yolo_in_flight(cx) {
            return;
        }
        self.submit(false, Mark::take(cx), window, cx);
    }

    /// [`Launcher::press`], with `confirm_yolo` saying whether the user consented to a YOLO launch on a host that asks
    /// first (the YOLO question's answers, `yolo_question`); a plain press never consents. `mark` is the answer the
    /// reply's moved-on check counts from ([`launch::start`]).
    fn submit(
        &mut self,
        confirm_yolo: bool,
        mark: Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refused = None;
        self.yolo = None;
        let creates = matches!(self.draft.mode, Mode::New | Mode::Clone);
        let hosts = self.hosts(cx);
        let body = (creates && self.refusal(cx).is_none())
            .then(|| {
                self.draft.create_body(Target {
                    hosts: &hosts,
                    catalog: self.catalog.as_deref(),
                })
            })
            .flatten();
        // The progress pill's host, captured now for the request's life: the registry's display name, without the
        // footer's phase. A body exists only for a host the refusal check found among `hosts`.
        let host = body.as_ref().and_then(|body| {
            hosts
                .iter()
                .find(|host| Some(host.id) == body.host)
                .map(display_name)
        });
        let (Some(mut body), Some(host)) = (body, host) else {
            self.pressed = true;
            cx.notify();
            return;
        };
        body.confirm_yolo = confirm_yolo;
        if launch::start(cx.entity(), &body, host, mark, window, cx)
            && let Some(layer) = self.layer.take()
        {
            layers::close(layer, window, cx);
        }
    }

    /// Reset choices: the agent choices back to New's, with the remembered permission; in Restart with, the source's
    /// own launch.
    fn reset_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let remembered = self
            .store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| {
                remembered_permission(preferences.remembered_permissions.as_deref())
            });
        self.draft.reset_choices(remembered);
        self.notice = None;
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Retry: read the catalog again, its failure cleared first so the line goes while the read is out and comes back
    /// only if it fails again.
    fn retry_catalog(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.catalog_error = None;
        read_catalog(cx.weak_entity(), cx);
        cx.notify();
    }

    /// Clear initial prompt: the prompt emptied, as the user could by selecting it all and deleting.
    fn clear_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.edit_prompt(String::new());
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Cancel and Close: close the launcher, discarding its draft.
    ///
    /// Refused while the YOLO question's host write is out, as Escape is (the dialog's dismissal hook): closing then
    /// would let the write change the host's setting with nothing left to say whether it did.
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.yolo_in_flight(cx) {
            return;
        }
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------------------------------

/// A click handler that runs `act` on the launcher, if it is still open.
fn on_launcher(
    launcher: WeakEntity<Launcher>,
    act: fn(&mut Launcher, &mut Window, &mut Context<Launcher>),
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let _ = launcher.update(cx, |launcher, cx| act(launcher, window, cx));
    }
}

/// One labelled row: the label in the left column, its control beside it (`.launch-composer-row`). `selector` names the
/// row for tests, which find a field by where it is drawn.
fn row(
    selector: &'static str,
    label: &'static str,
    control: impl IntoElement,
    palette: &theme::Palette,
) -> Div {
    div()
        .debug_selector(move || selector.into())
        .flex()
        .items_start()
        .gap(px(LABEL_GAP))
        .min_w_0()
        .child(
            div()
                .flex_none()
                .w(px(LABEL_COLUMN))
                // Level with the control's text: a field's padding above it.
                .pt(px(8.))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(label),
        )
        .child(div().flex_1().min_w_0().child(control))
}

/// A quiet line of help under a control, in the control's column.
///
/// It is a labelled node named by its words, since plain text is not in the accessibility tree and a screen reader
/// would otherwise never hear it. Its id is its words: no two help lines of one launcher say the same thing.
fn help_line(text: impl Into<SharedString>, palette: &theme::Palette) -> Stateful<Div> {
    let text: SharedString = text.into();
    div()
        .id(ElementId::Name(text.clone()))
        .role(Role::Label)
        .aria_label(text.clone())
        .pl(px(LABEL_COLUMN + LABEL_GAP))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(text)
}

/// A section's heading (`.launch-composer-section-label`). Elsewhere the old app drew a rule after it, but the
/// launcher's stylesheet took that away (`::after { content: none }`), so none is drawn here.
fn section_label(text: &'static str, palette: &theme::Palette) -> Div {
    div()
        .text_size(metrics::TEXT_MD)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(palette.fg_0))
        .child(text)
}

/// What Launch would start, in words, as the footer and the main action's name share them.
struct TargetWords {
    /// Whether the Command tab is chosen, which the words open with.
    command: bool,
    /// The agent type, if one is named.
    harness: Option<&'static str>,
    /// The host's name (see [`Launcher::target`]).
    host: HostText,
    folder: HostText,
}

impl TargetWords {
    /// The words as one sentence, the host and folder escaped: "Command · Codex · host · folder".
    fn sentence(&self, id: &'static str) -> HostSentence {
        let mut sentence = HostSentence::new(id);
        if self.command {
            sentence = sentence.text("Command · ");
        }
        if let Some(harness) = self.harness {
            sentence = sentence.text(format!("{harness} · "));
        }
        sentence.host(&self.host).text(" · ").host(&self.folder)
    }
}

/// The selector's name in the agent's own vocabulary (old `conversation_label`): a literal session file for Pi and
/// OMP, the agent's own identifier for the rest. None of these names implies Farhelm looks the conversation up.
const fn conversation_label(harness: LaunchHarness) -> &'static str {
    match harness {
        LaunchHarness::Pi | LaunchHarness::Omp => "Session file",
        LaunchHarness::Goose | LaunchHarness::OpenCode => "Session ID",
        LaunchHarness::Cursor => "Chat ID",
        LaunchHarness::Claude
        | LaunchHarness::Codex
        | LaunchHarness::Muse
        | LaunchHarness::Grok => "Conversation ID",
    }
}

impl Launcher {
    /// The mode's title and summary for the header.
    fn heading(&self) -> (&'static str, &'static str) {
        match self.draft.mode {
            Mode::RestartWith(_) => (
                "Restart session",
                "Resume this session with different settings",
            ),
            Mode::ReplaceWith { .. } => (
                "Replace session",
                "Start a new session, then delete the source",
            ),
            Mode::New | Mode::Clone => ("New session", "Start an agent or a command on any host"),
        }
    }

    /// The main action's visible word: Launch, Replace or Restart.
    fn verb(&self) -> &'static str {
        match self.draft.mode {
            Mode::RestartWith(_) => "Restart",
            Mode::ReplaceWith { .. } => "Replace",
            Mode::New | Mode::Clone => "Launch",
        }
    }

    /// What the draft would start, in the old footer's words: "Command" on the Command tab, the agent type (on the Agent
    /// tab, or a Restart with command's declared one), the host's display name and the folder. The host and folder are
    /// host-supplied text (a Clone's folder comes from its source), so they are only shown escaped.
    ///
    /// Restart with names the source session's own host name, as the old footer did. Otherwise a host that is not
    /// chosen, or that the store no longer lists, is "Unavailable host", the old footer's words.
    ///
    /// A registered host that is not connected carries its phase in parentheses ("host1 (Unreachable, retrying)"), as
    /// the old footer and host list did.
    fn target(&self, cx: &App) -> TargetWords {
        let harness = match self.draft.tab {
            Tab::Agent => self.draft.agent.harness,
            Tab::Command if matches!(self.draft.mode, Mode::RestartWith(_)) => {
                self.draft.command.agent
            }
            Tab::Command => None,
        };
        let collections = self.store.read(cx).collections();
        let source = match &self.draft.mode {
            Mode::RestartWith(restart) => collections
                .get::<SessionRow>(&restart.source)
                .map(|row| HostText::from_host(row.host_name.as_str())),
            Mode::New | Mode::Clone | Mode::ReplaceWith { .. } => None,
        };
        let registered = self.draft.host.and_then(|id| {
            collections
                .ordered::<HostView>()
                .into_iter()
                .find(|host| host.id == id)
                .map(host_name_with_phase)
        });
        let host = source
            .or(registered)
            .unwrap_or_else(|| HostText::from_host("Unavailable host"));
        TargetWords {
            command: self.draft.tab == Tab::Command,
            harness: harness.map(harness_label),
            host,
            folder: HostText::from_host(self.draft.folder.value().as_str()),
        }
    }

    /// The footer's summary (`.launch-composer-footer-summary`): what Launch would start, and on the Agent tab the
    /// choices it would carry, each cut to one line. Each line is one labelled node holding its full text, since a cut
    /// line's drawn words are not all on screen (SPEC.md: "Long summaries may truncate visually; their full text remains
    /// accessible"), and the target line's full text is its hover help too, as the old one's tooltip was.
    fn summary(&self, target: &TargetWords, window: &mut Window, cx: &mut App) -> AnyElement {
        let palette = theme::palette(cx);
        let full = target
            .sentence("launcher-summary-target-name")
            .accessible_name();
        let mut line = div()
            .id("launcher-summary-target")
            .role(Role::Label)
            .aria_label(full.clone())
            .flex()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap();
        if target.command {
            line = line.child("Command · ");
        }
        if let Some(harness) = target.harness {
            line = line.child(format!("{harness} · "));
        }
        line = line
            .child(host_text::element(&target.host).flex_none())
            .child(" · ");
        line = line.child(
            host_text::element(&target.folder)
                .font_family(crate::assets::TERMINAL_FONT)
                .truncate(),
        );
        let line = components::with_hover_help(
            line,
            &SharedString::from("launcher-summary-target"),
            &HoverHelp::sentence(target.sentence("launcher-summary-target-help")),
            false,
            window,
            cx,
        );
        let mut summary = div()
            .debug_selector(|| "launcher-summary".into())
            .flex()
            .flex_col()
            .min_w_0()
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_1))
            .child(line);
        if self.draft.tab == Tab::Agent {
            summary = summary.child(self.choices(palette));
        }
        summary.into_any_element()
    }

    /// The Agent tab's line of choices (`.launch-composer-summary`): model, effort, permissions with YOLO in the danger
    /// color (SPEC.md: "permission danger stays red"), the line count of a prompt this launch would send, and a stored
    /// container the launch still carries. The model is host text, from a stored launch.
    fn choices(&self, palette: &theme::Palette) -> AnyElement {
        let agent = &self.draft.agent;
        let model = agent.model.as_deref().map(HostText::from_host);
        let permission = agent.harness.map_or(agent.permissions, |harness| {
            harness.request_permission(agent.permissions)
        });
        let tail = format!(" · Effort: {} · Permissions: ", effort_label(agent.effort));
        let mut name = HostSentence::new("launcher-summary-choices-name").text("Model: ");
        name = match &model {
            Some(model) => name.host(model),
            None => name.text("Default"),
        };
        let mut rest = String::new();
        if let Some(lines) = self
            .draft
            .active_prompt()
            .map(|prompt| prompt.split('\n').count())
        {
            rest.push_str(&format!(
                " · Prompt: {lines} line{}",
                if lines == 1 { "" } else { "s" }
            ));
        }
        if agent.container.is_some() {
            rest.push_str(" · retired container");
        }
        let shown_permission = permission_display(permission);
        name = name.text(format!("{tail}{shown_permission}{rest}"));
        let mut line = div()
            .id("launcher-summary-choices")
            .role(Role::Label)
            .aria_label(name.accessible_name())
            .flex()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .child("Model: ");
        line = match &model {
            Some(model) => line.child(host_text::element(model).flex_none()),
            None => line.child("Default"),
        };
        line = line.child(tail);
        line = if permission == Some(LaunchPermission::Yolo) {
            line.child(
                div()
                    .text_color(theme::color(palette.danger))
                    .child(shown_permission),
            )
        } else {
            line.child(shown_permission)
        };
        line.child(rest).into_any_element()
    }

    /// The main action's accessible name: the operation and its target, as the old one had ("Launch Codex · Local (this
    /// machine) · ~/src"), so a screen reader hears what a press would start, not only "Launch".
    fn action_name(&self, target: &TargetWords) -> SharedString {
        let sentence = target.sentence("launcher-action-name");
        format!("{} {}", self.verb(), sentence.accessible_name()).into()
    }

    /// The Agent tab's fields that exist so far, in the old order: the harness, effort and permission choices (with a
    /// failed catalog read said under the harness row), the conversation selector and the initial prompt once an agent
    /// that offers them is chosen (the prompt not in Restart with, which holds it), the Custom flags while Custom is
    /// chosen, the wrapper prefix, what the last choice changed, and a note for an agent type that lacks something.
    fn agent_fields(
        &self,
        model: Option<Vec<AnyElement>>,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut fields = Vec::new();
        let harness = self.draft.agent.harness;
        let mut choices = self.choice_rows(palette, cx);
        // Under the harness row, where the old launcher had the catalog's failure and then the model, or first when
        // Restart with holds the agent type and draws no harness row.
        let at = usize::from(!self.draft.holds(Field::AgentType));
        let mut after_harness = Vec::new();
        if let Some(error) = &self.catalog_error {
            after_harness.push(self.catalog_error_line(error, palette, cx));
        }
        after_harness.extend(model.into_iter().flatten());
        choices.splice(at..at, after_harness);
        fields.extend(choices);
        if let Some(harness) = harness.filter(|harness| harness.offers_named_conversation()) {
            let required = matches!(&self.draft.mode, Mode::RestartWith(restart) if restart.conversation_required);
            let label = conversation_label(harness);
            fields.push(
                row(
                    "launcher-conversation-row",
                    label,
                    TextInput::new(
                        "launcher-conversation",
                        &self.conversation,
                        label,
                        HoverHelp::tooltip(format!(
                            "{label}: start {} on this conversation, this start only",
                            harness_label(harness)
                        )),
                    ),
                    palette,
                )
                .into_any_element(),
            );
            if required {
                fields.push(
                    help_line(
                        "Farhelm has no captured conversation to resume. Enter its ID or session file to restart here.",
                        palette,
                    )
                    .into_any_element(),
                );
            }
            if !harness.captures_named_conversation() {
                fields.push(
                    help_line(
                        "Farhelm will not capture this conversation after a named start, so Restart will not be \
                         available.",
                        palette,
                    )
                    .into_any_element(),
                );
            }
        }
        let prompt_offered = harness
            .filter(|harness| harness.supports_initial_prompt(false))
            .filter(|_| !self.draft.holds(Field::InitialPrompt));
        if let Some(harness) = prompt_offered {
            fields.push(
                row(
                    "launcher-prompt-row",
                    "Initial prompt",
                    TextInput::new(
                        "launcher-prompt",
                        &self.prompt,
                        "Initial prompt",
                        HoverHelp::tooltip("Initial prompt: sent once when this session starts"),
                    ),
                    palette,
                )
                .into_any_element(),
            );
            fields.push(
                help_line(
                    format!(
                        "Sent once, as {}'s first message when it starts. Restart, Clone and Replace do not send it \
                         again.",
                        harness_label(harness)
                    ),
                    palette,
                )
                .into_any_element(),
            );
            if !self.draft.prompt.is_empty() {
                fields.push(
                    div()
                        .pl(px(LABEL_COLUMN + LABEL_GAP))
                        .child(
                            Button::new(
                                "launcher-clear-prompt",
                                "Clear initial prompt",
                                HoverHelp::tooltip("Clear initial prompt: remove this first message from the draft"),
                                Role::Button,
                            )
                            .tier(Tier::Neutral)
                            .on_click(on_launcher(cx.weak_entity(), Launcher::clear_prompt)),
                        )
                        .into_any_element(),
                );
            }
        }
        if self.draft.agent.permissions == Some(LaunchPermission::Custom) {
            fields.push(self.flags_editor(harness, palette));
        }
        fields.push(
            row(
                "launcher-wrapper-row",
                "Wrapper prefix",
                TextInput::new(
                    "launcher-wrapper",
                    &self.wrapper,
                    "Wrapper prefix",
                    HoverHelp::tooltip(
                        "Wrapper prefix: Bash text before Farhelm's agent command; quotes and expansions keep their \
                         shell meaning; no placeholders",
                    ),
                )
                .monospace(),
                palette,
            )
            .into_any_element(),
        );
        fields.push(
            help_line(
                "Runs through Bash before the agent command; blank means none.",
                palette,
            )
            .into_any_element(),
        );
        // Said as soon as the prefix stops parsing, not only after a press (`launch-composer-choice-error`), so the
        // user sees which field Launch's reason is about while still typing in it.
        let wrapper_error = (!self.draft.agent.wrapper.is_empty())
            .then(|| split_wrapper_prefix(&self.draft.agent.wrapper).err())
            .flatten();
        if let Some(error) = wrapper_error {
            // Its words as its name too: plain text children are not in the accessibility tree.
            let words = capitalized(&error.to_string());
            fields.push(
                div()
                    .id("launcher-wrapper-error")
                    .role(Role::Status)
                    .aria_label(words.clone())
                    .pl(px(LABEL_COLUMN + LABEL_GAP))
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child(words)
                    .into_any_element(),
            );
        }
        if let Some(notice) = &self.notice {
            fields.push(
                choices::status_line("launcher-choice-notice", notice.clone(), palette)
                    .into_any_element(),
            );
        }
        if let Some(note) = harness.and_then(agent_note) {
            fields.push(help_line(note, palette).into_any_element());
        }
        fields
    }

    /// "Model catalog unavailable", with the read's own words, beside Retry. The words are a live region of their
    /// own and the button beside it, so a screen reader announces the failure rather than the control.
    fn catalog_error_line(
        &self,
        error: &HostText,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = HostSentence::new("launcher-catalog-error-name")
            .text("Model catalog unavailable: ")
            .host(error)
            .accessible_name();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.warn))
            .child(
                div()
                    .id("launcher-catalog-error")
                    .role(Role::Status)
                    .aria_label(name)
                    .flex()
                    .min_w_0()
                    .child("Model catalog unavailable: ")
                    .child(host_text::element(error)),
            )
            .child(
                Button::new(
                    "launcher-catalog-retry",
                    "Retry",
                    HoverHelp::tooltip("Retry: ask the host for its model list again"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .on_click(on_launcher(cx.weak_entity(), Launcher::retry_catalog)),
            )
            .into_any_element()
    }

    /// The Command tab's fields: the command line, the YOLO assertion, and Resume with its command when an agent type
    /// is declared.
    fn command_fields(&self, palette: &theme::Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let weak = cx.weak_entity();
        let mut fields = vec![
            row(
                "launcher-command-row",
                "Agent command",
                TextInput::new(
                    "launcher-command",
                    &self.command,
                    "Agent command",
                    HoverHelp::tooltip(
                        "Agent command: the command line Farhelm runs through Bash, as written",
                    ),
                )
                .monospace(),
                palette,
            )
            .into_any_element(),
        ];
        let (on_yolo, on_enter) = (weak.clone(), weak.clone());
        fields.push(
            row(
                "launcher-yolo-row",
                "Runs in YOLO mode",
                RadioGroup::new(
                    "launcher-yolo",
                    "Runs in YOLO mode",
                    HoverHelp::tooltip("Whether this command bypasses its safety checks; Farhelm believes your answer"),
                    Role::RadioGroup,
                )
                .radios()
                .option(RadioOption::new(
                    "Yes (YOLO)",
                    HoverHelp::tooltip(
                        "Yes: you assert this command bypasses human, classifier and sandbox safety checks; Farhelm \
                         marks it YOLO and does not check",
                    ),
                    true,
                ))
                .option(RadioOption::new(
                    "No",
                    HoverHelp::tooltip(
                        "No: you assert this command retains a safety check that may refuse or ask you; Farhelm does \
                         not check",
                    ),
                    false,
                ))
                .chosen(self.draft.command.yolo)
                .on_change(move |yolo, _, cx| {
                    let _ = on_yolo.update(cx, |launcher, cx| {
                        launcher.draft.choose_command_yolo(*yolo);
                        launcher.changed(cx);
                    });
                })
                .on_enter(move |window, cx| {
                    let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
                }),
                palette,
            )
            .into_any_element(),
        );
        // The declared agent type (old `.launch-command-agent`): which agent the command runs, if any, so Farhelm can
        // track and resume it. Restart with holds it, so there it is not drawn; the footer still names it.
        if !self.draft.holds(Field::AgentType) {
            let (on_agent, on_agent_enter) = (weak.clone(), weak.clone());
            let options = std::iter::once(SelectOption::new(
                NO_AGENT,
                "None",
                HoverHelp::tooltip(
                    "None: declare no agent type; Farhelm does not track or resume the command's agent",
                ),
            ))
            .chain(LaunchHarness::ALL.iter().map(|harness| {
                let label = harness_label(*harness);
                SelectOption::new(
                    label,
                    label,
                    HoverHelp::tooltip(format!("{label}: the command runs {label}")),
                )
            }));
            fields.push(
                row(
                    "launcher-command-agent-row",
                    "Agent type",
                    Select::new(
                        "launcher-command-agent",
                        "Agent type",
                        HoverHelp::tooltip(
                            "The agent this command runs, if any, so Farhelm can track and resume it",
                        ),
                        Role::ComboBox,
                    )
                    .options(options)
                    .chosen(self.draft.command.agent.map_or(NO_AGENT, harness_label))
                    .on_choose(move |value, _, cx| {
                        let agent = LaunchHarness::ALL
                            .iter()
                            .copied()
                            .find(|harness| harness_label(*harness) == value.as_ref());
                        let _ = on_agent.update(cx, |launcher, cx| {
                            launcher.draft.choose_command_agent(agent);
                            launcher.changed(cx);
                        });
                    })
                    .on_enter(move |window, cx| {
                        let _ = on_agent_enter.update(cx, |launcher, cx| launcher.press(window, cx));
                    }),
                    palette,
                )
                .into_any_element(),
            );
        }
        if self.draft.command.agent.is_some() {
            fields.push(
                help_line(
                    "Put {farhelm_args} in the command where Farhelm adds its own arguments.",
                    palette,
                )
                .into_any_element(),
            );
            if !matches!(self.draft.mode, Mode::RestartWith(_)) {
                let (on_resume, on_resume_enter) = (weak.clone(), weak.clone());
                fields.push(
                    div()
                        .pl(px(LABEL_COLUMN + LABEL_GAP))
                        .child(
                            Checkbox::new(
                                "launcher-resume",
                                "Resume",
                                HoverHelp::tooltip(
                                    "Resume: give the command a way to pick its conversation back up after a restart",
                                ),
                                Role::CheckBox,
                            )
                            .checked(self.draft.command.resume_on)
                            .on_toggle(move |on, _, cx| {
                                let _ = on_resume.update(cx, |launcher, cx| {
                                    launcher.draft.choose_command_resume(on);
                                    launcher.changed(cx);
                                });
                            })
                            .on_enter(move |window, cx| {
                                let _ =
                                    on_resume_enter.update(cx, |launcher, cx| launcher.press(window, cx));
                            }),
                        )
                        .into_any_element(),
                );
            }
            if self.draft.command.resume_on {
                fields.push(
                    row(
                        "launcher-resume-command-row",
                        "Resume command",
                        TextInput::new(
                            "launcher-resume-command",
                            &self.resume,
                            "Resume command",
                            HoverHelp::tooltip(
                                "Resume command: how Farhelm restarts this command on its conversation; needs \
                                 {conversation} and {farhelm_args}",
                            ),
                        )
                        .monospace(),
                        palette,
                    )
                    .into_any_element(),
                );
            }
        }
        fields
    }
}

impl Render for Launcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let refusal = self.refusal(cx);
        let target = self.target(cx);
        // While the YOLO question's host write is out, the launcher holds still (`yolo_question`).
        let holding = self.yolo_in_flight(cx);
        let restart = matches!(self.draft.mode, Mode::RestartWith(_));

        // A held destination (Restart with's) is shown, not edited: its folder as read-only text, its name not at all
        // (SPEC.md, "Restart with": held fields' "editing controls are hidden", and "The held agent type, host and
        // folder stay visible").
        let folder: AnyElement = if self.draft.holds(Field::Destination) {
            div()
                .pt(px(8.))
                .font_family(crate::assets::TERMINAL_FONT)
                .text_size(metrics::TEXT_MD)
                .child(
                    HostSentence::new("launcher-folder-held")
                        .host(&HostText::from_host(self.draft.folder.value().as_str())),
                )
                .into_any_element()
        } else {
            TextInput::new(
                "launcher-folder",
                &self.folder,
                "Folder",
                HoverHelp::tooltip(
                    "Folder: an existing directory on the host, absolute or starting with ~",
                ),
            )
            .monospace()
            .into_any_element()
        };
        let name = (!self.draft.holds(Field::Name)).then(|| {
            row(
                "launcher-name-row",
                "Name (optional)",
                TextInput::new(
                    "launcher-name",
                    &self.name,
                    "Name (optional)",
                    HoverHelp::tooltip("Name: the session's title; left empty, Farhelm names it"),
                ),
                palette,
            )
        });
        let host_row = self.host_row(palette, window, cx);
        let destination =
            div()
                .flex()
                .flex_col()
                .gap(px(ROW_GAP))
                .child(section_label("Destination", palette))
                .child(host_row)
                .children(self.host_picker.note.map(|note| {
                    choices::status_line("launcher-host-note", note.to_owned(), palette)
                }))
                .child(row("launcher-folder-row", "Folder", folder, palette))
                .children(name);
        let (on_tab, on_tab_enter) = (weak.clone(), weak.clone());
        let tabs = (!self.draft.holds(Field::LaunchKind)).then(|| {
            TabStrip::new(
                "launcher-tabs",
                "Launch kind",
                HoverHelp::tooltip("Launch kind: an agent with its choices, or a command line"),
            )
            .tab(TabOption::new(
                "Agent",
                HoverHelp::tooltip(
                    "Agent: start a supported agent with its model, effort and permissions",
                ),
                Tab::Agent,
            ))
            .tab(TabOption::new(
                "Command",
                HoverHelp::tooltip("Command: run a command line you type, as written"),
                Tab::Command,
            ))
            .chosen(self.draft.tab)
            .on_choose(move |tab, _, cx| {
                let _ = on_tab.update(cx, |launcher, cx| {
                    launcher.draft.choose_tab(*tab);
                    launcher.changed(cx);
                });
            })
            .on_enter(move |window, cx| {
                let _ = on_tab_enter.update(cx, |launcher, cx| launcher.press(window, cx));
            })
        });
        let fields = match self.draft.tab {
            Tab::Agent => {
                let model = self.model_row(palette, window, cx);
                self.agent_fields(model, palette, cx)
            }
            Tab::Command => self.command_fields(palette, cx),
        };
        let choices = div()
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .children(tabs)
            .children(fields);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(destination)
            .child(choices);

        // What a press has to say: the draft's own reason after a press it refused, or the helm's words refusing the
        // last launch. The helm's words are host text and are drawn escaped, as their own element.
        let lead = if restart {
            "Restart error."
        } else {
            "Not launched."
        };
        let alert = match (&refusal, &self.refusal_words(cx)) {
            (Some(reason), _) if self.pressed => {
                Some(HostSentence::new("launcher-refusal-words").text(format!("{lead} {reason}")))
            }
            // The helm's timeout reply is not a refusal: the create may still happen, and its words say so.
            (_, Some(words))
                if self
                    .refused
                    .as_ref()
                    .is_some_and(|refused| refused.may_still_start) =>
            {
                Some(HostSentence::new("launcher-refusal-words").host(words))
            }
            (_, Some(words)) => Some(
                HostSentence::new("launcher-refusal-words")
                    .text(format!("{lead} "))
                    .host(words),
            ),
            _ => None,
        };
        let notices = alert.map(|sentence| {
            let text = sentence.accessible_name();
            // `.launch-composer-refusal-alert`: a bordered alert with a warning sign, the lead before the reason, in
            // the warning colors (red is kept for danger in the launcher).
            div()
                .id("launcher-refusal")
                .debug_selector(|| "launcher-refusal".into())
                .role(Role::Alert)
                .aria_label(text)
                .flex()
                .items_start()
                .gap(px(8.))
                .py(px(8.))
                .px(px(10.))
                .border_1()
                .border_color(theme::color(palette.warn_border))
                .rounded(metrics::RADIUS_MD)
                .bg(theme::color(palette.warn_bg_dim))
                .text_size(metrics::TEXT_MD)
                .text_color(theme::color(palette.warn_bright))
                .child(div().flex_none().child("⚠"))
                .child(div().min_w_0().child(sentence))
        });

        let mut answers = vec![
            Button::new(
                "launcher-cancel",
                "Cancel",
                HoverHelp::tooltip("Cancel: close without starting anything"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .focus(&self.cancel_focus)
            // Drawn busy while the YOLO question's host write is out, when it refuses (`Launcher::cancel`).
            .busy(holding)
            .on_click(on_launcher(weak.clone(), Launcher::cancel)),
        ];
        if self.draft.tab == Tab::Agent {
            answers.push(
                Button::new(
                    "launcher-reset",
                    "Reset choices",
                    HoverHelp::tooltip(if restart {
                        "Reset choices: restore the session's stored settings and clear the conversation selector"
                    } else {
                        "Reset choices: clear the agent, model and effort; remember permissions and the next agent's \
                         wrapper prefix"
                    }),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .on_click(on_launcher(weak.clone(), Launcher::reset_choices)),
            );
        }
        let verb = self.verb();
        let help = refusal.clone().unwrap_or_else(|| match self.draft.mode {
            Mode::RestartWith(_) => "Restart: relaunch this session with these settings".to_owned(),
            Mode::ReplaceWith { .. } => {
                "Replace: start this session, then delete the old one and its state".to_owned()
            }
            Mode::New | Mode::Clone => "Launch: start the session".to_owned(),
        });
        answers.push(
            Button::new(
                "launcher-launch",
                self.action_name(&target),
                HoverHelp::tooltip(help),
                Role::Button,
            )
            .content(verb)
            .tier(Tier::Primary)
            .refused(refusal.is_some())
            // Busy while the YOLO question's host write is out, when a press does nothing (`Launcher::press`).
            .busy(holding)
            .on_click(on_launcher(weak.clone(), Launcher::press)),
        );
        let (title, summary) = self.heading();
        let mut frame = DialogFrame::new("launcher", title, body, answers)
            .summary(summary)
            // `.create-session-form .dialog-tile .action-icon`: the launch glyph at 22 pixels.
            .tile(
                svg()
                    .path(Icon::Launch.path())
                    .size(px(TILE_ICON))
                    .text_color(theme::color(palette.fg_0)),
            )
            .width(px(WIDTH))
            .flat_margin_when_wide()
            .footer_lead(self.summary(&target, window, cx))
            .close(
                "Close session launcher",
                HoverHelp::tooltip("Close: discard this session setup"),
                holding,
                on_launcher(weak.clone(), Launcher::cancel),
            );
        // The YOLO question, while it is up, takes the notices' place: it is what a press of Launch has to say.
        if let Some(question) = self.yolo_inline() {
            frame = frame.notices(question.clone());
        } else if let Some(notices) = notices {
            frame = frame.notices(notices);
        }
        frame
    }
}

/// A registered host's name in the launcher (the footer, the host picker's button and list): its display name, with its phase in parentheses while it is not connected
/// (old `HostOption::label`). The phase is the app's own words; carried in host text, escaping them changes nothing.
pub(super) fn host_name_with_phase(host: &HostView) -> HostText {
    let name = display_name(host);
    if matches!(host.state, HostStateView::Connected { .. }) {
        name
    } else {
        HostText::from_host(format!("{} ({})", name.raw(), phase::words(&host.state)))
    }
}
