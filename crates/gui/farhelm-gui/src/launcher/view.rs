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
//!   each tab's draft").
//! - **Input to edits.** Every field's change becomes one of the draft's edit methods; nothing here decides a launch
//!   rule. A field the draft changes on its own (Reset choices clearing the wrapper prefix) is put back in step after
//!   the edit.
//! - **The main action.** Launch (Replace, Restart for those modes) is drawn retired while [`LaunchDraft::refusal`]
//!   names a reason, and the reason is its hover text; a press, or Enter on the dialog, a single-line field or a
//!   choice, shows the reason beside it in the bordered alert, "Not launched." first ("Restart error." in Restart
//!   with). Its accessible name keeps the operation and its target, as the old one did.
//! - **Reads.** The model catalog is read once when the launcher opens (`GET /api/launch-catalog`); hosts, sessions and
//!   the remembered permission are read from the store while drawing.
//!
//! NOTE: Not built yet, each with the piece that brings it: the host picker and the model field (its list on the
//! non-focusing popover), with the host's phase in the footer; the agent type, effort and permission choices, with
//! custom flags; the Command tab's declared agent type; sending the launch (until then a press of a complete draft does
//! nothing); the composer search; recent setups, Browse and the folder check, with its warning on the folder field;
//! managed checkouts, with their folder and name placeholders; the escaped read-only display of a prompt or wrapper
//! prefix holding invisible characters, with its Clear wrapper prefix; and keyboard scrolling of an overflowing notice
//! (the old region took focus once it overflowed; today's refusals are one or two lines).

use std::rc::Rc;

use farhelm_helm_client::ApiPath;
use farhelm_proto::launcher::LaunchCatalogModel;
use farhelm_proto::ui_channel::{HostView, PREFERENCES_KEY, Preferences, SessionRow};
use farhelm_proto::{
    LaunchEffort, LaunchHarness, LaunchPermission, SessionLaunch, split_wrapper_prefix,
};
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window, div, px, svg,
};

use super::draft::{
    Field, LaunchDraft, Mode, Tab, harness_label, permission_name, remembered_permission,
};
use super::request::{Target, capitalized};
use crate::assets::Icon;
use crate::components::{
    self, Button, Checkbox, DialogFrame, HoverHelp, RadioGroup, RadioOption, Role, TabOption,
    TabStrip, TextInput, TextInputEvent, TextInputState, Tier,
};
use crate::focus::{Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Dialog, DialogPlacement, LayerId, MainAction};
use crate::session_list::host_rail::display_name;
use crate::store::HelmStore;
use crate::theme::{self, metrics};
use crate::{operations, session};

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
    let store = session::current_store(cx)?;
    let launcher = cx.new(|cx| Launcher::new(draft, &store, window, cx));
    let start = launcher.read(cx).start_focus(cx);
    let weak = launcher.downgrade();
    // Enter on the dialog itself (after a click on its empty space) presses the main action.
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
            // Escape and the header Close discard the draft, as Cancel does.
            on_dismiss: None,
            placement: DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    launcher.update(cx, |launcher, _| launcher.layer = Some(layer));
    read_catalog(launcher.downgrade(), cx);
    #[cfg(test)]
    cx.set_global(super::view_tests::Opened(launcher.downgrade()));
    Some(layer)
}

/// Read the release's model catalog for `launcher`, once. A failed read leaves it unread, which refuses no choice
/// (the draft's rule for an unread catalog).
fn read_catalog(launcher: WeakEntity<Launcher>, cx: &mut App) {
    let Ok(path) = ApiPath::api(&["launch-catalog"]) else {
        return;
    };
    operations::read::<Vec<LaunchCatalogModel>>(
        path,
        "GET /api/launch-catalog",
        cx,
        move |answer, cx| {
            let Ok(catalog) = answer else {
                return;
            };
            let _ = launcher.update(cx, |launcher, cx| {
                launcher.catalog = Some(catalog);
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
    /// Whether the main action was pressed since the draft last changed, which shows its reason beside it.
    pressed: bool,
    pub(super) folder: TextInputState,
    name: TextInputState,
    conversation: TextInputState,
    pub(super) wrapper: TextInputState,
    pub(super) prompt: TextInputState,
    command: TextInputState,
    resume: TextInputState,
    /// Cancel's focus handle, where Restart with starts.
    pub(super) cancel_focus: gpui::FocusHandle,
    layer: Option<LayerId>,
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
        let command = field(&draft.command.command, window, cx);
        let resume = field(&draft.command.resume, window, cx);
        let fields =
            [
                (&folder, Input::Folder),
                (&name, Input::Name),
                (&conversation, Input::Conversation),
                (&wrapper, Input::Wrapper),
                (&prompt, Input::Prompt),
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
                        TextInputEvent::Submitted => launcher.press(window, cx),
                    }
                })
            })
            .collect();
        Launcher {
            draft,
            store: store.clone(),
            catalog: None,
            pressed: false,
            folder,
            name,
            conversation,
            wrapper,
            prompt,
            command,
            resume,
            cancel_focus: cx.focus_handle(),
            layer: None,
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
            Input::Command => self.draft.edit_command(text),
            Input::Resume => self.draft.edit_command_resume(text),
        }
        self.changed(cx);
    }

    /// The draft changed: take the shown reason away until the next press, so the user is never told about a draft
    /// they already changed, and redraw. Every edit, whatever control made it, ends here.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.pressed = false;
        cx.notify();
    }

    /// Put the text fields the draft can change on its own back in step with it (after Reset choices, say). A field
    /// already showing the draft's value is left alone, so its caret and selection stay where they were.
    fn sync_fields(&self, window: &mut Window, cx: &mut Context<Self>) {
        let pairs = [
            (&self.wrapper, self.draft.agent.wrapper.clone()),
            (&self.conversation, self.draft.conversation.clone()),
            (&self.folder, self.draft.folder.value().clone()),
            (&self.prompt, self.draft.prompt.clone()),
        ];
        for (state, value) in pairs {
            if state.text(cx).as_ref() != value {
                state.set_text(value, window, cx);
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

    /// Why the draft cannot launch now, if it cannot.
    fn refusal(&self, cx: &App) -> Option<String> {
        let hosts = self.hosts(cx);
        let target = Target {
            hosts: &hosts,
            catalog: self.catalog.as_deref(),
        };
        self.draft.refusal(target, self.live_launch(cx).as_ref())
    }

    /// The main action was pressed (the button, or Enter on the dialog, a single-line field or a choice): show why it
    /// cannot launch, if it cannot.
    ///
    /// NOTE: Sending the launch comes with the piece that ports launching; until then a press of a complete draft does
    /// nothing. That piece must also keep a held Enter from launching (SPEC.md: "A held Enter never launches"): a
    /// single-line field reports Submitted on every auto-repeat, which is harmless only while a press just shows the
    /// reason. The tab strip and the radios already ignore a held Enter.
    fn press(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.pressed = true;
        cx.notify();
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
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Clear initial prompt: the prompt emptied, as the user could by selecting it all and deleting.
    fn clear_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.draft.edit_prompt(String::new());
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// Cancel and Close: close the launcher, discarding its draft.
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
fn help_line(text: impl Into<SharedString>, palette: &theme::Palette) -> Div {
    div()
        .pl(px(LABEL_COLUMN + LABEL_GAP))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(text.into())
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

/// An effort's name as the launcher shows it (old `effort_label`); `None` is the model's default.
const fn effort_label(effort: Option<LaunchEffort>) -> &'static str {
    match effort {
        Some(LaunchEffort::Off) => "Off",
        Some(LaunchEffort::Minimal) => "Minimal",
        Some(LaunchEffort::Low) => "Low",
        Some(LaunchEffort::Medium) => "Medium",
        Some(LaunchEffort::High) => "High",
        Some(LaunchEffort::Xhigh) => "xhigh",
        Some(LaunchEffort::Max) => "Max",
        Some(LaunchEffort::Ultra) => "Ultra",
        None => "Default",
    }
}

/// A permission's name in the footer (old `permission_display_value`). It differs from the escalation banner's
/// [`permission_name`] in two words, as the old app's did: no choice is "Default", and custom flags are "Custom".
const fn permission_display(permission: Option<LaunchPermission>) -> &'static str {
    match permission {
        None => "Default",
        Some(LaunchPermission::Custom) => "Custom",
        Some(_) => permission_name(permission),
    }
}

/// What Launch would start, in words, as the footer and the main action's name share them.
struct TargetWords {
    /// Whether the Command tab is chosen, which the words open with.
    command: bool,
    /// The agent type, if one is named.
    harness: Option<&'static str>,
    /// The chosen host's label; `None` while no registered host is chosen.
    host: Option<HostText>,
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
        if let Some(host) = &self.host {
            sentence = sentence.host(host).text(" · ");
        }
        sentence.host(&self.folder)
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
    /// A chosen host the store no longer lists is "Unavailable host", or in Restart with the source session's own host
    /// name, as the old footer had them.
    ///
    /// NOTE: The host picker adds a host's phase while it is not connected ("host1 (Unreachable, retrying)"), as the
    /// old footer and host list did; the phase words live with the Hosts panel.
    fn target(&self, cx: &App) -> TargetWords {
        let harness = match self.draft.tab {
            Tab::Agent => self.draft.agent.harness,
            Tab::Command if matches!(self.draft.mode, Mode::RestartWith(_)) => {
                self.draft.command.agent
            }
            Tab::Command => None,
        };
        let collections = self.store.read(cx).collections();
        let host = self.draft.host.map(|id| {
            let registered = collections
                .ordered::<HostView>()
                .into_iter()
                .find(|host| host.id == id)
                .map(display_name);
            let source = match &self.draft.mode {
                Mode::RestartWith(restart) => collections
                    .get::<SessionRow>(&restart.source)
                    .map(|row| HostText::from_host(row.host_name.as_str())),
                Mode::New | Mode::Clone | Mode::ReplaceWith { .. } => None,
            };
            registered
                .or(source)
                .unwrap_or_else(|| HostText::from_host("Unavailable host"))
        });
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
        if let Some(host) = &target.host {
            line = line
                .child(host_text::element(host).flex_none())
                .child(" · ");
        }
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

    /// The Agent tab's fields that exist so far: the conversation selector and the initial prompt once an agent that
    /// offers them is chosen (the prompt not in Restart with, which holds it), and the wrapper prefix.
    fn agent_fields(&self, palette: &theme::Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut fields = Vec::new();
        let harness = self.draft.agent.harness;
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
        fields
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
        if self.draft.command.agent.is_some() {
            fields.push(
                help_line(
                    "Put {farhelm_args} in the command where Farhelm adds its own arguments.",
                    palette,
                )
                .into_any_element(),
            );
            if !matches!(self.draft.mode, Mode::RestartWith(_)) {
                let on_resume = weak.clone();
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
        let restart = matches!(self.draft.mode, Mode::RestartWith(_));

        // A held destination (Restart with's) is shown, not edited: its folder as read-only text, its name not at all
        // (SPEC.md, "Restart with": "held fields' editing controls are hidden, while the held agent type, host and
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
        let destination = div()
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .child(section_label("Destination", palette))
            .child(row("launcher-folder-row", "Folder", folder, palette))
            .children(name);
        let (on_tab, on_tab_enter) = (weak.clone(), weak.clone());
        let tabs = (!restart).then(|| {
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
            Tab::Agent => self.agent_fields(palette, cx),
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

        let notices = (self.pressed && refusal.is_some()).then(|| {
            let lead = if restart {
                "Restart error."
            } else {
                "Not launched."
            };
            let text = format!("{lead} {}", refusal.clone().unwrap_or_default());
            // `.launch-composer-refusal-alert`: a bordered alert with a warning sign, the lead before the reason, in
            // the warning colors (red is kept for danger in the launcher).
            div()
                .id("launcher-refusal")
                .debug_selector(|| "launcher-refusal".into())
                .role(Role::Alert)
                .aria_label(text.clone())
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
                .child(div().min_w_0().child(text))
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
            .footer_lead(self.summary(&target, window, cx))
            .close(
                "Close session launcher",
                HoverHelp::tooltip("Close: discard this session setup"),
                false,
                on_launcher(weak.clone(), Launcher::cancel),
            );
        if let Some(notices) = notices {
            frame = frame.notices(notices);
        }
        frame
    }
}
