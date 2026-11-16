//! One recipe's editor, drawn in place of the Recipes list: Back, Name, Size, the Software base cards with each base's
//! fields and the requirements note, Ubuntu packages and Tunnels; and its answers, Delete, Cancel and Save, which
//! Settings pins at the bottom of its dialog rather than letting them scroll with the fields.
//!
//! SPEC.md, "Topology" (the Recipes paragraphs: "The editor replaces the list in place"; "In Settings the editor opens
//! at its top, including Back, with field labels smaller than its section captions. Its actions and leave/delete
//! questions and refusal messages stay visible at the bottom of the scrolling panel, above the unchanged Close footer.
//! Switching Settings sections hides those actions with the editor"). Ported from the old app's `RecipeEditor`
//! (`tensorlake/recipe_editor.rs` at reference commit `24bc0580`), with its words and the `.recipe-editor` rules in
//! `app.css`, and shaped after this app's Credentials editor (`settings/credentials/editor.rs`), whose in-place pattern
//! it shares: a view of its own that its section holds, closed through the section by its entity id, so a late reply
//! never closes an editor opened since. Here's the TLDR of its rules:
//!
//! - **The draft is read from the form.** The fields are plain inputs, and the whole recipe is one draft
//!   ([`draft::Draft`], the old model), built from the inputs whenever it is needed: to tell whether anything changed,
//!   whether an error still applies, and to save. Nothing mirrors keystrokes; an edit only redraws. The draft the
//!   editor opened with is its baseline, and the form has unsaved changes exactly when the two differ.
//! - **The credential choices are chosen by name.** The GitHub token and each agent's mode, endpoint and login come
//!   from the helm's credential catalog, read once as the editor opens ([`credentials`] has their rules). The editor
//!   loads every one of them from the recipe it opens, so Edit then Save sends back exactly what it loaded, the
//!   choices the user did not touch included.
//! - **Save is the recipe's own work.** It checks a new name and the whole draft first, with the helm's shared rules,
//!   and says why it sent nothing; otherwise it creates (`POST /api/tensorlake/recipes`) or updates (`PUT
//!   /api/tensorlake/recipes/<name>`) through the operation store under [`OpKey::recipe`], the key the row's build uses,
//!   with every field read-only and Back and Cancel unavailable meanwhile, as in the old editor. A done save closes the
//!   editor, or finishes the leave whose question it answered or still asks; the list already shows the change, which
//!   the helm pushed before it answered. A refused one keeps the draft with the helm's words, shown while the draft is
//!   as it was sent, so the next edit drops them as the old `oninput` did.
//! - **A refusal shown here leaves the key.** The key's one refusal record is what the row draws under its build
//!   button, so the editor forgets every refusal it shows itself ([`crate::operations::Operations::forget`]); otherwise
//!   a refused save would come back under the row as the last build's refusal once the editor closed. A refusal that
//!   lands after the editor is gone (Settings closed while the save was out) has nowhere else to show, so it stays on
//!   the key and the row shows it, in the helm's words, until the recipe's next build or save starts.
//! - **The answers are a separate element.** [`Editor::actions`] builds the error line, the busy line and the buttons
//!   for whichever dialog hosts the editor to place: Settings passes them to its frame's pinned notices only while
//!   Recipes shows an open editor, so switching sections hides them, and the Close row stays as it is. Add host's
//!   Tensorlake sandbox hosts the same editor for its "New recipe…" and pins the answers in place of its own footer.
//! - **Two hosts, one editor.** Where the editor is open ([`Host`]) decides the little that differs, as the old
//!   editor's `in_add_host` did: in Settings → Recipes, Back above the title, where focus starts; in Add host, focus
//!   starting in Name and no Back, while Add host's Escape and header close ask before closing Add host as Settings'
//!   do. Each host decides where a departure goes.
//! - **Leaving asks first, and Delete confirms.** Back, Cancel and closing the hosting dialog ask "Save these changes
//!   before leaving?" while the form has unsaved changes, and Delete asks before it deletes; [`leaving`] has their
//!   rules.

mod credentials;
mod leaving;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::recipes::{CreateRecipe, RecipeSave, RecipeView, check_recipe_name};
use gpui::{
    AnyElement, App, Context, Entity, EntityId, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, WeakEntity, Window, div, px,
};

use super::Recipes;
use super::draft::{Base, Draft, TunnelDraft};
use crate::components::{
    Button, HoverHelp, RadioGroup, RadioOption, TextInput, TextInputState, Tier,
};
use crate::environment_ui;
use crate::focus::Mark;
use crate::host_text::HostText;
use crate::operations::{Command, NotStarted, OpKey, Outcome, Work};
use crate::prompt::Inline;
use crate::session;
use crate::settings::Refused;
use crate::settings::choices::{self, FIELD_HEIGHT, LABEL_GAP};
use crate::theme::{self, metrics};
use credentials::{Agent, Choices};
pub(crate) use leaving::Departure;
use std::rc::Rc;

/// `.recipe-editor`: the gap between the editor's parts.
const EDITOR_GAP: f32 = 8.;
/// `.recipe-editor .recipe-editor-title`: 18 pixels, bold.
const TITLE_TEXT: f32 = 18.;
/// `div.checkout-root-label > span` in the editor: a block's caption (Size, Software, Tunnels) at 14 pixels, semibold.
const CAPTION_TEXT: f32 = 14.;
/// `.recipe-size`: the three size fields' columns and their gap.
const SIZE_COLUMNS: u16 = 3;
const SIZE_GAP: f32 = 8.;
/// `.recipe-tunnel`: a port field's width (`minmax(0, 6rem)`) and the gap between a row's parts.
const PORT_WIDTH: f32 = 96.;
const TUNNEL_GAP: f32 = 6.;
/// The packages box starts four rows high, as the old `<textarea rows="4">` did.
const PACKAGE_ROWS: usize = 4;
/// The Save button's element id, which [`Refused`] names its busy line after (`recipe-save-busy`).
const SAVE: &str = "recipe-save";
/// The Delete button's element id.
const DELETE: &str = "recipe-delete";

/// The Software cards in the old order: each base's title and what it says about where it runs.
const BASES: [(Base, &str, &str); 4] = [
    (
        Base::TensorlakeUbuntu,
        "Managed Ubuntu",
        "Ubuntu with systemd, for Tensorlake sandboxes and Podman containers.",
    ),
    (
        Base::Image,
        "Your image",
        "Tensorlake only: an image in your project, by name.",
    ),
    (
        Base::Snapshot,
        "Your snapshot",
        "Tensorlake only: a snapshot in your project, by ID.",
    ),
    (
        Base::ContainerImage,
        "Container image",
        "Runs on Podman hosts.",
    ),
];

/// The custom-base note, the old words: what a base Farhelm builds on must provide (SPEC.md: "The custom-base note
/// explains that the base must run systemd as its init system, allow passwordless sudo, be Ubuntu or Debian with apt,
/// and include bash, curl and git").
const REQUIREMENTS: &str = "Farhelm builds on this base. It must boot systemd as its init system, let the sandbox user \
                            use sudo without a password, be Ubuntu or Debian with apt, and have bash, curl and git. The \
                            build checks these first and stops with the one that's missing.";

/// A shared rule's reason as a sentence, its first letter capitalized: the old app's `local_validation_notice`.
fn capitalized(mut reason: String) -> String {
    if let Some(first) = reason.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    reason
}

/// `/api/tensorlake/recipes`, or with `name` the recipe's own route.
fn path(name: Option<&str>) -> Result<ApiPath, PathError> {
    match name {
        Some(name) => ApiPath::api(&["tensorlake", "recipes", name]),
        None => ApiPath::api(&["tensorlake", "recipes"]),
    }
}

/// What Save sends: `POST /api/tensorlake/recipes` with the name and the recipe for a new one, `PUT
/// /api/tensorlake/recipes/<name>` with the bare recipe for a saved one, whose URL is the name's only authority. Named in
/// a wordless refusal by its method and route: a recipe's name is the user's own, not an internal id. Turning either
/// into JSON cannot fail (strings, numbers and enums); if it ever did, the request would go without a body and the helm
/// would refuse it with its own words.
fn save_command(name: &str, saved: bool, recipe: RecipeSave) -> Result<Command, PathError> {
    let (method, path, body) = if saved {
        (
            Method::Put,
            path(Some(name))?,
            serde_json::to_value(recipe).ok(),
        )
    } else {
        let body = CreateRecipe {
            name: name.to_owned(),
            recipe,
        };
        (Method::Post, path(None)?, serde_json::to_value(body).ok())
    };
    let shown = format!("{} {}", method.as_str(), path.as_str());
    Ok(Command::new(Work::Save, method, path, body, shown))
}

/// One text field and the subscription that redraws the editor on each edit; dropping it drops both.
struct Input {
    state: TextInputState,
    _edits: Subscription,
}

impl Input {
    /// `state` holding `text`, redrawing the editor whenever the user edits it, so whatever reads the draft (Save's
    /// look, the error line) follows.
    fn new(
        state: TextInputState,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Editor>,
    ) -> Input {
        state.set_text(text.to_owned(), window, cx);
        let edits = state.subscribe(window, cx, |_, _, _, cx| cx.notify());
        Input {
            state,
            _edits: edits,
        }
    }

    /// A single-line field holding `text`.
    fn line(text: &str, window: &mut Window, cx: &mut Context<Editor>) -> Input {
        Input::new(TextInputState::new(window, cx), text, window, cx)
    }

    /// A single-line field holding `text`, showing `placeholder` while empty: a tunnel row's, whose fields have no
    /// label above them, as the old row's had none, and an agent endpoint's base URL, with the old example URL.
    fn placeholder(
        text: &str,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Editor>,
    ) -> Input {
        let input = Input::line(text, window, cx);
        input.state.set_placeholder(placeholder, window, cx);
        input
    }

    fn text(&self, cx: &App) -> String {
        self.state.text(cx).to_string()
    }
}

/// One tunnel row's three fields, under a number of its own that keys its element ids, so they follow the row and not
/// its position when an earlier row is removed.
struct TunnelRow {
    key: u64,
    sandbox_port: Input,
    host: Input,
    port: Input,
}

impl TunnelRow {
    fn new(key: u64, tunnel: &TunnelDraft, window: &mut Window, cx: &mut Context<Editor>) -> Self {
        TunnelRow {
            key,
            sandbox_port: Input::placeholder(&tunnel.sandbox_port, "Sandbox port", window, cx),
            host: Input::placeholder(&tunnel.host, "Host", window, cx),
            port: Input::placeholder(&tunnel.port, "Port", window, cx),
        }
    }

    fn draft(&self, cx: &App) -> TunnelDraft {
        TunnelDraft {
            sandbox_port: self.sandbox_port.text(cx),
            host: self.host.text(cx),
            port: self.port.text(cx),
        }
    }
}

/// Where an editor is open (the module docs, "Two hosts, one editor"): what decides the parts that differ between its
/// hosting dialogs, and where its departures go.
pub(in crate::settings) enum Host {
    /// Settings → Recipes, in place of the list: Back above the title, where focus starts. Departures go to the
    /// section ([`Recipes::depart`]).
    Recipes(WeakEntity<Recipes>),
    /// Add host's Tensorlake sandbox, in place of its fields, for "New recipe…": no Back, and focus starting in Name.
    /// Departures go to the callback.
    AddHost(Departed),
}

impl Host {
    /// Whether the editor draws Back above its title: in Settings, where it returns to the list; not in Add host,
    /// where the editor's Cancel is the way back to the form, as the old `in_add_host` editor had no Back.
    fn has_back(&self) -> bool {
        match self {
            Host::Recipes(_) => true,
            Host::AddHost(_) => false,
        }
    }
}

/// What an editor hosted by Add host calls when it leaves: with its entity id, so a late reply never moves an editor
/// opened since; where it goes; the recipe's name once it is saved (a done create included), which Add host chooses;
/// the press the departure answers; and the window, when a press is at hand rather than a reply.
pub(crate) type Departed =
    Rc<dyn Fn(EntityId, Departure, Option<String>, &Mark, Option<&mut Window>, &mut App)>;

/// One recipe's editor (see the module docs). A view of its own, dropped by Back, Cancel, a done save, or the dialog
/// closing; a save it started goes on in the operation store without it.
pub(crate) struct Editor {
    /// Where the editor is open, and where its departures go.
    host: Host,
    /// The saved recipe's name, or `None` for a new recipe.
    saved: Option<String>,
    /// The draft as the editor opened it: what "unsaved changes" is measured against.
    baseline: Draft,
    name: Input,
    cpus: Input,
    memory_gb: Input,
    disk_gb: Input,
    base: Base,
    image: Input,
    snapshot_id: Input,
    container_reference: Input,
    container_user: Input,
    packages: Input,
    tunnels: Vec<TunnelRow>,
    /// The key the next added tunnel row takes.
    next_tunnel: u64,
    /// The GitHub token and each agent's choices, and the credential catalog they are chosen from.
    choices: Choices,
    /// Why the last Save sent nothing or was refused, with the draft as it stood then; shown while the draft is still
    /// that ([`Editor::error`]).
    error: Option<(HostText, Draft)>,
    /// Whether this editor's save is out.
    saving: bool,
    /// Whether this editor's Delete is out.
    deleting: bool,
    /// The leave question, while it is up in the answers' place ([`leaving`]), with the departure it asks about: a
    /// done save finishes that departure while the question is still open, unanswered.
    leave_question: Option<(Departure, Inline<Departure>)>,
    /// The delete confirmation, while it is up in the answers' place, frozen on the recipe's name.
    delete_question: Option<Inline<String>>,
    /// Where the leave question's Save goes once the save it sent is done, and the press that answered it.
    leaving: Option<(Departure, Mark)>,
    /// Whether the last Save sent nothing because the recipe's key already had work in flight (a build, say).
    refused: Refused,
    /// Back's focus handle, where focus starts when the editor opens ([`Editor::start`]).
    back: FocusHandle,
}

impl Editor {
    /// A new recipe's editor for Add host's Tensorlake sandbox, whose departures go to `departed` (the module docs).
    pub(crate) fn for_add_host(
        departed: Departed,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(Host::AddHost(departed), None, window, cx)
    }

    /// An editor for `existing`, or for a new recipe with the old starting values, open where `host` says.
    pub(super) fn new(
        host: Host,
        existing: Option<&RecipeView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let baseline = existing.map_or_else(Draft::new_recipe, Draft::from_view);
        let name = Input::line(&baseline.name, window, cx);
        if existing.is_some() {
            // SPEC.md: the name "never changes"; renaming means creating a new recipe.
            name.state.set_read_only(true, cx);
        }
        let tunnels: Vec<TunnelRow> = baseline
            .tunnels
            .iter()
            .zip(0..)
            .map(|(tunnel, key)| TunnelRow::new(key, tunnel, window, cx))
            .collect();
        let mut editor = Editor {
            host,
            saved: existing.map(|view| view.name.clone()),
            choices: Choices::new(&baseline, window, cx),
            cpus: Input::line(&baseline.cpus, window, cx),
            memory_gb: Input::line(&baseline.memory_gb, window, cx),
            disk_gb: Input::line(&baseline.disk_gb, window, cx),
            base: baseline.base,
            image: Input::line(&baseline.image, window, cx),
            snapshot_id: Input::line(&baseline.snapshot_id, window, cx),
            container_reference: Input::line(&baseline.container_reference, window, cx),
            container_user: Input::line(&baseline.container_user, window, cx),
            packages: Input::new(
                TextInputState::new_growing(PACKAGE_ROWS, window, cx),
                &baseline.packages,
                window,
                cx,
            ),
            next_tunnel: tunnels.len() as u64,
            tunnels,
            name,
            baseline,
            error: None,
            saving: false,
            deleting: false,
            leave_question: None,
            delete_question: None,
            leaving: None,
            refused: Refused::default(),
            back: cx.focus_handle().tab_stop(true),
        };
        editor.read_credentials(cx);
        editor
    }

    /// Where focus starts when the editor opens. In Settings, Back, at the editor's top, so moving focus there brings
    /// the top into view, Back included, even when the list it replaces was scrolled (SPEC.md: "In Settings the editor
    /// opens at its top, including Back"); the old editor focused its title, a heading made focusable, and headings are
    /// not controls here, so Back, the control right above it, takes its place. In Add host, Name, as the old editor
    /// focused there.
    pub(crate) fn start(&self, cx: &App) -> FocusHandle {
        match self.host {
            Host::Recipes(_) => self.back.clone(),
            Host::AddHost(_) => self.name.state.focus_handle(cx),
        }
    }

    /// The draft the form holds now: every field's text, the chosen base, the tunnel rows and the credential choices.
    ///
    /// Every field is listed here, with no struct update from the baseline, so a field added to [`Draft`] fails to
    /// compile until the form reads it, rather than quietly coming back as it was loaded.
    pub(in crate::settings) fn draft(&self, cx: &App) -> Draft {
        let claude = self.choices.drafted(Agent::Claude, cx);
        let codex = self.choices.drafted(Agent::Codex, cx);
        Draft {
            name: self.name.text(cx),
            cpus: self.cpus.text(cx),
            memory_gb: self.memory_gb.text(cx),
            disk_gb: self.disk_gb.text(cx),
            base: self.base,
            packages: self.packages.text(cx),
            image: self.image.text(cx),
            snapshot_id: self.snapshot_id.text(cx),
            container_reference: self.container_reference.text(cx),
            container_user: self.container_user.text(cx),
            claude: claude.endpoint,
            codex: codex.endpoint,
            claude_mode: claude.mode,
            claude_login_credential: claude.login,
            codex_mode: codex.mode,
            codex_login_credential: codex.login,
            github_token_credential: self.choices.github(),
            tunnels: self.tunnels.iter().map(|row| row.draft(cx)).collect(),
        }
    }

    /// The text field drawn under element id `id`, for tests that move focus into it by keyboard: GPUI Kit's fields carry
    /// no element id of their own, so a test asks the field's state whether it holds focus.
    #[cfg(test)]
    pub(in crate::settings) fn field(&self, id: &str) -> Option<&TextInputState> {
        let input = match id {
            "recipe-name" => &self.name,
            "recipe-cpus" => &self.cpus,
            "recipe-memory" => &self.memory_gb,
            "recipe-disk" => &self.disk_gb,
            "recipe-image" => &self.image,
            "recipe-snapshot-id" => &self.snapshot_id,
            "recipe-container-reference" => &self.container_reference,
            "recipe-container-user" => &self.container_user,
            "recipe-packages" => &self.packages,
            "recipe-claude-url" => self.choices.url(Agent::Claude),
            "recipe-codex-url" => self.choices.url(Agent::Codex),
            _ => return None,
        };
        Some(&input.state)
    }

    /// Whether the form differs from what the editor opened with.
    fn dirty(&self, cx: &App) -> bool {
        self.draft(cx) != self.baseline
    }

    /// The error to show: the last one, while the draft is still what it was when it came.
    fn error(&self, cx: &App) -> Option<&HostText> {
        self.error
            .as_ref()
            .filter(|(_, draft)| *draft == self.draft(cx))
            .map(|(words, _)| words)
    }

    /// Every text field, for locking them all while a save is out.
    fn inputs(&self) -> impl Iterator<Item = &Input> {
        [
            &self.name,
            &self.cpus,
            &self.memory_gb,
            &self.disk_gb,
            &self.image,
            &self.snapshot_id,
            &self.container_reference,
            &self.container_user,
            &self.packages,
        ]
        .into_iter()
        .chain(
            self.tunnels
                .iter()
                .flat_map(|row| [&row.sandbox_port, &row.host, &row.port]),
        )
        .chain(self.choices.inputs())
    }

    /// Mark the save out or settled, locking the form while it is out ([`Editor::lock`]).
    fn set_saving(&mut self, saving: bool, cx: &mut App) {
        self.saving = saving;
        self.lock(cx);
    }

    /// Every field read-only while a save or a delete is out, as the old editor's `readonly: locked`, and editable
    /// again after, except a saved recipe's name, which never is.
    fn lock(&self, cx: &mut App) {
        let busy = self.busy();
        for input in self.inputs() {
            input.state.set_read_only(busy, cx);
        }
        self.name
            .state
            .set_read_only(busy || self.saved.is_some(), cx);
    }

    /// Save (see the module docs): check, then create or update under the recipe's key, and say whether it was sent.
    /// Does nothing while a save or delete is out, when Save is drawn busy.
    fn save(&mut self, cx: &mut Context<Self>) -> bool {
        if self.busy() {
            return false;
        }
        let draft = self.draft(cx);
        let name = match &self.saved {
            Some(saved) => saved.clone(),
            None => draft.name.trim().to_owned(),
        };
        if self.saved.is_none()
            && let Err(reason) = check_recipe_name(&name)
        {
            self.fail(capitalized(reason.to_owned()), draft, cx);
            return false;
        }
        let recipe = match draft.to_save() {
            Ok(recipe) => recipe,
            Err(reason) => {
                self.fail(capitalized(reason), draft, cx);
                return false;
            }
        };
        let command = match save_command(&name, self.saved.is_some(), recipe) {
            Ok(command) => command,
            Err(error) => {
                tracing::debug!(?error, "recipe save not built");
                return false;
            }
        };
        let Some(operations) = session::current_operations(cx) else {
            return false;
        };
        let key = OpKey::recipe(&name);
        let mark = Mark::take(cx);
        let editor = cx.weak_entity();
        let sent = draft;
        let forgetting = key.clone();
        let created = name.clone();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, move |settled, cx| {
                let refusal = match &settled.outcome {
                    Outcome::Refused(refusal) => refusal.words.clone(),
                    Outcome::Done(_) | Outcome::Accepted(_) => {
                        let _ = editor.update(cx, |editor, cx| {
                            // The recipe is saved now: the departure that follows carries its name, which is how
                            // Add host learns which recipe to choose (Settings' editor is dropped by the same
                            // departure). Recorded before `set_saving` locks the fields again, so a saved recipe's
                            // name is never editable.
                            editor.saved = Some(created);
                            editor.set_saving(false, cx);
                            editor.saved(&mark, cx);
                        });
                        return;
                    }
                };
                let shown = editor
                    .update(cx, |editor, cx| {
                        editor.set_saving(false, cx);
                        // A refused save keeps the editor open, so a leave its question asked for does not happen. A
                        // question still open was asked while the save was out, without Discard; it goes, and the
                        // ordinary answers come back under the helm's words, as after any refused leave.
                        editor.leaving = None;
                        editor.leave_question = None;
                        editor.error = Some((refusal, sent));
                        cx.notify();
                    })
                    .is_ok();
                // Shown here, so it leaves the key the row's build shares (the module docs).
                if shown && let Some(operations) = session::current_operations(cx) {
                    operations.update(cx, |operations, cx| operations.forget(&forgetting, cx));
                }
            })
        });
        let sent = started.is_ok();
        self.started(started, cx);
        sent
    }

    /// Record `words` as why Save sent nothing, against `draft`, the form as it stood.
    fn fail(&mut self, words: String, draft: Draft, cx: &mut Context<Self>) {
        self.error = Some((HostText::from_host(words), draft));
        cx.notify();
    }

    /// After a Save was asked: a started one locks the form and drops the last error; a busy key says so ([`Refused`]).
    fn started(&mut self, started: Result<(), NotStarted>, cx: &mut Context<Self>) {
        self.refused.record(SAVE, &started);
        match started {
            Ok(()) => {
                self.error = None;
                self.set_saving(true, cx);
            }
            Err(not_started) => tracing::debug!(?not_started, "recipe not saved"),
        }
        cx.notify();
    }

    /// A Software card was chosen.
    fn choose_base(&mut self, base: Base, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.base = base;
        cx.notify();
    }

    /// Add tunnel: a new empty row at the end.
    fn add_tunnel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let row = TunnelRow::new(self.next_tunnel, &TunnelDraft::default(), window, cx);
        self.next_tunnel += 1;
        self.tunnels.push(row);
        cx.notify();
    }

    /// A row's Remove: drop the row `key` and its text.
    fn remove_tunnel(&mut self, key: u64, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.tunnels.retain(|row| row.key != key);
        cx.notify();
    }

    /// The editor's answers for its hosting dialog to place (the module docs): the error, the busy line, then Delete
    /// at the left and Cancel and Save at the right, or, while one is up, the leave question or the delete
    /// confirmation in their place, as the old questions replaced the ordinary actions. Read from the editor as it is
    /// now, so the host draws them in the same frame as the fields.
    pub(crate) fn actions(editor: &Entity<Editor>, cx: &App) -> AnyElement {
        let this = editor.read(cx);
        let weak = editor.downgrade();
        let busy = this.busy();
        let question: Option<AnyElement> = match (&this.leave_question, &this.delete_question) {
            (Some((_, leave)), _) if leave.is_open(cx) => Some(leave.clone().into_any_element()),
            (_, Some(delete)) if delete.is_open(cx) => Some(delete.clone().into_any_element()),
            _ => None,
        };
        let row = question.unwrap_or_else(|| {
            let delete = this.saved.is_some().then(|| {
                let weak = weak.clone();
                Button::new(
                    DELETE,
                    "Delete",
                    HoverHelp::tooltip("Delete this recipe"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .disabled(this.saving)
                .busy(this.deleting)
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |editor, cx| editor.ask_delete(window, cx));
                })
            });
            let cancel = {
                let weak = weak.clone();
                Button::new(
                    "recipe-cancel",
                    "Cancel",
                    HoverHelp::tooltip("Go back without saving; unsaved changes ask first"),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .disabled(busy)
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |editor, cx| editor.leave(window, cx));
                })
            };
            let save = {
                let weak = weak.clone();
                Button::new(
                    SAVE,
                    if this.saving { "Saving…" } else { "Save" },
                    HoverHelp::tooltip("Save this recipe"),
                    Role::Button,
                )
                .accessible_name("Save")
                .tier(Tier::Primary)
                // The old Save was `aria-disabled` while a saved recipe had no unsaved changes, and still saved when
                // pressed.
                .refused(this.saved.is_some() && !this.dirty(cx))
                // Busy, not disabled, while saving, so a pressed Save keeps focus (SPEC_impl.md, "Drafts"); disabled
                // while a delete is out, which has the busy look on Delete.
                .busy(this.saving)
                .disabled(this.deleting)
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |editor, cx| {
                        editor.save(cx);
                    });
                })
            };
            // `.recipe-editor-actions`: Delete pushed to the start (`margin-right: auto`), the rest at the end.
            div()
                .flex()
                .flex_wrap()
                .justify_end()
                .gap(px(8.))
                .children(delete.map(|delete| div().mr_auto().child(delete)))
                .child(cancel)
                .child(save)
                .into_any_element()
        });
        div()
            .id("recipe-editor-actions")
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .children(
                this.error(cx)
                    .map(|reason| choices::failure_outcome("recipe-editor-error", reason, cx)),
            )
            .children(choices::busy_line(&this.refused, SAVE, cx))
            .child(row)
            .into_any_element()
    }
}

/// A block's caption (`div.checkout-root-label > span`): Size, Software, Tunnels.
fn caption(text: &'static str, cx: &App) -> gpui::Div {
    div()
        .text_size(px(CAPTION_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::color(theme::palette(cx).fg_0))
        .child(text)
}

/// A field's label (`label.checkout-root-label > span`), at 13 pixels in the secondary ink: smaller than the blocks'
/// captions, as SPEC.md asks.
fn label(text: &'static str, cx: &App) -> gpui::Div {
    div()
        .text_size(metrics::TEXT_MD)
        .text_color(theme::color(theme::palette(cx).fg_1))
        .child(text)
}

/// A labelled single-line field, as wide as its column: `text` over `input`, drawn under `id`'s debug selector so tests
/// and the test bridge find it, monospace when `mono` (names, images and hosts, as the old `font-mono` fields).
fn field(
    text: &'static str,
    id: impl Into<SharedString>,
    input: &Input,
    help: &'static str,
    mono: bool,
    cx: &App,
) -> gpui::Div {
    let id: SharedString = id.into();
    let input = TextInput::new(id.clone(), &input.state, text, HoverHelp::tooltip(help))
        .height(px(FIELD_HEIGHT));
    let input = if mono { input.monospace() } else { input };
    div()
        .flex()
        .flex_col()
        .gap(px(LABEL_GAP))
        .w_full()
        .min_w_0()
        .child(label(text, cx))
        .child(
            div()
                .w_full()
                .debug_selector(move || id.to_string())
                .child(input),
        )
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        // Every control is unavailable while a save or a delete is out, as the old editor's `locked`.
        let busy = self.busy();
        let title = match &self.saved {
            Some(name) => format!("Recipe: {name}"),
            None => "New recipe".to_owned(),
        };

        let back = {
            let weak = weak.clone();
            Button::new(
                "recipe-back",
                "‹ Recipes",
                HoverHelp::tooltip("Back to the recipe list; unsaved changes ask before leaving"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .focus(&self.back)
            .disabled(busy)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |editor, cx| editor.leave(window, cx));
            })
        };
        let heading = div()
            .id("recipe-editor-title")
            .debug_selector(|| "recipe-editor-title".to_owned())
            .role(Role::Heading)
            .aria_label(title.clone())
            .text_size(px(TITLE_TEXT))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::color(palette.fg_0))
            .child(title.clone());

        // `.recipe-size`: CPUs, memory and disk side by side, each with its Usage Credits hint under it.
        let size_field =
            |text: &'static str, id: &'static str, input: &Input, hint: &'static str| {
                field(text, id, input, "A whole number", false, cx).child(
                    div()
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(palette.fg_1))
                        .child(hint),
                )
            };
        let size = div()
            .grid()
            .grid_cols(SIZE_COLUMNS)
            .gap(px(SIZE_GAP))
            .child(size_field(
                "CPUs",
                "recipe-cpus",
                &self.cpus,
                "1–4 on Usage Credits",
            ))
            .child(size_field(
                "Memory, GB",
                "recipe-memory",
                &self.memory_gb,
                "1–8 per CPU on Usage Credits",
            ))
            .child(size_field(
                "Disk, GB",
                "recipe-disk",
                &self.disk_gb,
                "10–100 on Usage Credits",
            ));

        let cards = {
            let weak = weak.clone();
            BASES
                .into_iter()
                .fold(
                    RadioGroup::new(
                        "recipe-base",
                        "Software",
                        HoverHelp::tooltip("What Farhelm builds this recipe's environments on"),
                        Role::Group,
                    )
                    .cards(),
                    |group, (base, title, help)| {
                        // The description is the card's accessible description; the hover help repeats it, as the
                        // Terminal section's described choices do.
                        group.option(
                            RadioOption::new(title, HoverHelp::tooltip(help), base)
                                .description(help),
                        )
                    },
                )
                .chosen(Some(self.base))
                .disabled(busy)
                .on_change(move |&base, _, cx| {
                    let _ = weak.update(cx, |editor, cx| editor.choose_base(base, cx));
                })
        };
        // The chosen base's own fields, then the custom-base note under any base but managed Ubuntu.
        let mut base_fields: Vec<AnyElement> = match self.base {
            Base::TensorlakeUbuntu => Vec::new(),
            Base::Image => vec![
                field(
                    "Image",
                    "recipe-image",
                    &self.image,
                    "A Tensorlake image's name",
                    true,
                    cx,
                )
                .into_any_element(),
                choices::help(
                    "A Tensorlake image in your project; `tl sbx image ls` lists them.",
                    cx,
                )
                .into_any_element(),
            ],
            Base::Snapshot => vec![
                field(
                    "Snapshot ID",
                    "recipe-snapshot-id",
                    &self.snapshot_id,
                    "A Tensorlake snapshot's ID",
                    true,
                    cx,
                )
                .into_any_element(),
                choices::help(
                    "A Tensorlake snapshot in your project; `tl sbx checkpoint ls` lists them.",
                    cx,
                )
                .into_any_element(),
            ],
            Base::ContainerImage => vec![
                field(
                    "Container image",
                    "recipe-container-reference",
                    &self.container_reference,
                    "The container image's full reference",
                    true,
                    cx,
                )
                .into_any_element(),
                field(
                    "Container login user",
                    "recipe-container-user",
                    &self.container_user,
                    "The account sessions log in as inside the container",
                    true,
                    cx,
                )
                .into_any_element(),
            ],
        };
        if self.base != Base::TensorlakeUbuntu {
            base_fields.push(
                choices::help(REQUIREMENTS, cx)
                    .id("recipe-base-requirements")
                    .debug_selector(|| "recipe-base-requirements".to_owned())
                    .role(Role::Note)
                    .aria_label(REQUIREMENTS)
                    .into_any_element(),
            );
        }

        let packages = div()
            .flex()
            .flex_col()
            .gap(px(LABEL_GAP))
            .w_full()
            .min_w_0()
            .child(label("Ubuntu packages", cx))
            .child(
                div()
                    .w_full()
                    .debug_selector(|| "recipe-packages".to_owned())
                    .child(
                        TextInput::new(
                            "recipe-packages",
                            &self.packages.state,
                            "Ubuntu packages",
                            HoverHelp::tooltip(
                                "Ubuntu packages to install, separated by spaces or new lines",
                            ),
                        )
                        .monospace(),
                    ),
            );

        let tunnels = self.tunnels.iter().enumerate().map(|(index, row)| {
            let number = index + 1;
            let key = row.key;
            let weak = weak.clone();
            let port = |id: String, label: &'static str, input: &Input| {
                div().w(px(PORT_WIDTH)).flex_none().child(
                    TextInput::new(
                        id,
                        &input.state,
                        label,
                        HoverHelp::tooltip("A port from 1 to 65535"),
                    )
                    .height(px(FIELD_HEIGHT))
                    .monospace(),
                )
            };
            let arrow = |text: &'static str| {
                div()
                    .flex_none()
                    .text_color(theme::color(palette.fg_1))
                    .child(text)
            };
            div()
                .id(SharedString::from(format!("recipe-tunnel-{key}")))
                .role(Role::Group)
                .aria_label(SharedString::from(format!("Tunnel {number}")))
                .flex()
                .items_center()
                .gap(px(TUNNEL_GAP))
                .w_full()
                .min_w_0()
                .child(port(
                    format!("recipe-tunnel-{key}-sandbox-port"),
                    "Sandbox port",
                    &row.sandbox_port,
                ))
                .child(arrow("→"))
                .child(
                    div().flex_1().min_w_0().child(
                        TextInput::new(
                            format!("recipe-tunnel-{key}-host"),
                            &row.host.state,
                            "Host",
                            HoverHelp::tooltip(
                                "The host to reach, as seen from this helm's machine",
                            ),
                        )
                        .height(px(FIELD_HEIGHT))
                        .monospace(),
                    ),
                )
                .child(arrow(":"))
                .child(port(format!("recipe-tunnel-{key}-port"), "Port", &row.port))
                .child(
                    Button::new(
                        format!("recipe-tunnel-{key}-remove"),
                        "Remove",
                        HoverHelp::tooltip("Remove this tunnel from the recipe"),
                        Role::Button,
                    )
                    .accessible_name(format!("Remove tunnel {number}"))
                    .tier(Tier::Neutral)
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |editor, cx| editor.remove_tunnel(key, cx));
                    }),
                )
        });
        let add_tunnel = Button::new(
            "recipe-add-tunnel",
            "Add tunnel",
            HoverHelp::tooltip(
                "Add a port in the sandbox that reaches a host and port on this helm's side",
            ),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(busy)
        .on_click({
            let weak = weak.clone();
            move |_, window, cx| {
                let _ = weak.update(cx, |editor, cx| editor.add_tunnel(window, cx));
            }
        });

        div()
            .id("recipe-editor")
            .role(Role::Region)
            .aria_label(title)
            .flex()
            .flex_col()
            .gap(px(EDITOR_GAP))
            .w_full()
            .min_w_0()
            .children(self.host.has_back().then(|| div().flex().child(back)))
            .child(heading)
            .child(choices::help(
                "Software and credentials for new container and sandbox sessions.",
                cx,
            ))
            .child(field(
                "Name",
                "recipe-name",
                &self.name,
                "The recipe's name; it can't be changed later",
                true,
                cx,
            ))
            .child(choices::help(
                "Can't be changed later. Hosts refer to their recipe by name. Lowercase letters, digits and hyphens.",
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(LABEL_GAP))
                    .child(caption("Size (Tensorlake sandboxes only)", cx))
                    .child(size),
            )
            .child(choices::help("Tensorlake decides which sizes your plan allows.", cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(LABEL_GAP))
                    .child(caption("Software", cx))
                    .child(cards),
            )
            .children(base_fields)
            .child(packages)
            .child(choices::help(
                "Optional. Separated by spaces or new lines. Changes take effect at the next recipe build.",
                cx,
            ))
            // The shared catalog must explain both artifacts without implying that a container build creates a billed
            // Tensorlake snapshot.
            .child(choices::help(
                environment_ui::recipe_build_help(&self.draft(cx).recipe_base()),
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(LABEL_GAP))
                    .child(caption("Tunnels to your side", cx))
                    .children(tunnels)
                    .child(div().flex().child(add_tunnel)),
            )
            .child(choices::help(
                "Programs in the sandbox reach 127.0.0.1 at the sandbox port, and Farhelm carries the connection to \
                 the host and port as seen from this helm's machine. Anything in the sandbox, agents included, can use \
                 a tunnel.",
                cx,
            ))
            .children(self.credential_blocks(cx))
            .children(self.saved.is_some().then(|| {
                choices::help(
                    "Size applies to Tensorlake sandboxes created after you save. Base and package changes take effect \
                     at the next recipe build. Tunnels, endpoints and credential selections are read when a host \
                     connects or is set up, so those changes reach existing hosts at their next connection.",
                    cx,
                )
            }))
    }
}
