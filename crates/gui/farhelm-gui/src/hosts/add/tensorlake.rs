//! Add host's Tensorlake sandbox: the body Add host shows once its host kind is "Tensorlake sandbox", with the Name and
//! Recipe of the sandbox to create.
//!
//! SPEC.md, "Topology" (adding a Tensorlake host). Ported from the old app's `TensorlakeAddDialog`
//! (`tensorlake/add_host.rs` at reference commit `24bc0580`) and the `.add-tensorlake-*` rules in `app.css`. Here's the
//! TLDR of its rules:
//!
//! - **The account is read when the form is made.** The form is made each time the Tensorlake sandbox kind is chosen,
//!   and dropped when SSH is chosen again, as the old dialog remounted it. `GET /api/tensorlake` says whether an API
//!   key is saved; the account is not a pushed collection, so it is read rather than followed, as Settings →
//!   Tensorlake reads it once per opening. Until it answers the body says "Loading Tensorlake settings…"; a failed
//!   read, or one that could not be sent, shows why with Retry, which reads again.
//! - **No key, no form.** Without a saved key the body says so and offers "Open Settings → Tensorlake", which opens
//!   Settings at Tensorlake over Add host. Closing Settings returns focus to that button, and that return reads the
//!   account again, so a key saved meanwhile brings up the form; the old dialog read its account again when Settings
//!   closed too. The no-key line stays up while that read is out, so focus stays on the button until the answer; an
//!   answer with a key moves focus to Name, as the old dialog's remount did, unless the user moved on since Settings
//!   closed. Focus reaching the button any other way reads nothing.
//! - **Name and Recipe.** Name is the host's name in the rail and on session rows. Recipe chooses from the pushed
//!   recipe catalog, which it follows while open, and shows the chosen recipe's summary below, the words Settings →
//!   Recipes shows for it ([`crate::settings::recipe_summary`]). A chosen recipe that leaves the catalog stays chosen
//!   as "<name> (unavailable)" rather than falling back to another, as the old select kept its draft. Recipe names are
//!   the user's own words, drawn as Settings → Recipes draws them.
//! - **New recipe… opens the recipe editor here.** The Recipe choice's last option opens Settings → Recipes' editor
//!   ([`crate::settings::RecipeEditor`]) in place of the fields, with focus in its Name, its answers in Add host's
//!   footer in place of Cancel and Continue (pinned there, so they never scroll away with the editor, which is taller
//!   than the dialog), and the kind choice hidden until it closes. Its Cancel, or a done Save, brings the fields back
//!   with focus on Recipe, and a done Save chooses the new recipe. Closing Add host while the editor holds unsaved
//!   changes asks "Save these changes before leaving?" over Add host. Its Save or Discard then closes Add
//!   host. The old dialog swapped its body and footer for the editor's the same way (`in_add_host`).
//! - **Continue asks the helm for a creation plan.** Continue, or Enter in Name, is unavailable until both Name and
//!   Recipe are set; Enter with either missing says which. It refuses while a session launch is in flight, as Add
//!   does, and otherwise sends `POST /api/hosts/probe` for the name (trimmed) and recipe under Add host's key, reading
//!   "Checking…" meanwhile with the fields read-only. The helm's plan is free and local: nothing is created yet. A
//!   refusal, or a reply that is not a Tensorlake plan, shows in Add host's error line with the fields kept.
//! - **The plan is confirmed on the prompt.** Add host closes and the creation confirmation opens in its place,
//!   through the Continue's mark, as an SSH plan's setup confirmation does; its answers, Back and what a refusal does
//!   are `setup`'s (`crate::hosts::setup`'s module docs). Once the user moved on since Continue, the plan
//!   is dropped and the error line says to press Continue again. With "Create Tensorlake sandboxes without asking" on
//!   in Settings → Tensorlake (the account's `create_without_asking`, never the SSH setup preference), the plan is
//!   submitted as it arrives instead, with Add host closed first, as an SSH plan is under "Set up new hosts without
//!   asking"; a session launch started while the probe was out refuses it as it refuses Create sandbox. A reopened
//!   Add host (Back, or a refused creation) starts on this form with the name and recipe as they were, the account as
//!   it was read, and focus in Name.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::provisioning::{PlanSteps, ProbeReply};
use farhelm_proto::recipes::{RecipeSize, RecipeView};
use farhelm_proto::tensorlake::Settings;
use serde::Deserialize as _;
use serde::de::IgnoredAny;
use serde_json::{Value, json};
use std::rc::Rc;

use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Entity, EntityId, FocusHandle,
    IntoElement, ParentElement as _, Render, Role, SharedString, Styled as _, Subscription,
    WeakEntity, Window, div, px,
};

use super::{AddHostDialog, BODY_GAP, FIELD_HEIGHT, Problem, SandboxSeed, error_line, field, help};
use crate::components::{Button, HoverHelp, Select, SelectOption, TextInput, TextInputState, Tier};
use crate::drafts::DraftInput;
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::{self, setup};
use crate::operations::{Command, OpKey, Outcome, Settled, Work};
use crate::session::{self, ObservingCurrent};
use crate::settings::{self, RecipeDeparted, RecipeDeparture, RecipeEditor, Section};
use crate::theme;

/// The route the account is read from, as a failed read names it: here, and by a sandbox's rerun, which reads whether
/// to ask (`crate::hosts::setup`).
pub(in crate::hosts) const ACCOUNT_ROUTE: &str = "GET /api/tensorlake";

/// What the body says when the account read could not be sent at all.
const NOT_SENT: &str =
    "Farhelm is not connected to its helm, so the Tensorlake settings could not be read";

/// The Recipe choice's option that opens the recipe editor rather than choosing a recipe. No recipe can be named this:
/// the helm's rule allows no underscore in a recipe name (`farhelm_proto::recipes::check_recipe_name`).
const NEW_RECIPE: &str = "__new__";

/// What Continue says on hover: where it leads.
const CONTINUE_HELP: &str =
    "Continue: show the sandbox's size and billing before anything is created";

/// What the error line says when the plan arrived after the user moved on since Continue, so no confirmation opens.
pub(super) const MOVED_ON: &str = "Press Continue again to see the sandbox's size and billing.";

/// Where the account read stands.
enum Account {
    /// The read is out, the first one or a Retry.
    Reading,
    /// The read failed, in the helm's words or the app's for an answer it could not read.
    Failed(HostText),
    /// The account as the helm answered.
    Read(Settings),
}

/// The Tensorlake sandbox body of one choice of the kind: made each time the user chooses Tensorlake sandbox, or as a
/// reopened Add host starts on it, and dropped, with its draft, when SSH is chosen again (`Body`).
pub(super) struct TensorlakeForm {
    account: Account,
    /// Add host's key, which the probe runs under and the creation continues under.
    key: OpKey,
    /// The control that opened Add host, which Back and a refusal reopen it for.
    opener: FocusHandle,
    /// The error line: why Continue refused or what the probe answered, or why a creation was refused. Cleared when a
    /// probe starts.
    problem: Option<HostSentence>,
    /// The mark taken at the last Continue: the confirmation opens only through it.
    probe_mark: Mark,
    /// The host's name, as typed.
    name: DraftInput,
    /// The chosen recipe's name, or `None` before a choice.
    recipe: Option<SharedString>,
    /// "Open Settings → Tensorlake"'s focus handle: Settings returns focus to it when it closes.
    open_settings: FocusHandle,
    /// Set when "Open Settings → Tensorlake" opened Settings, and taken when focus comes back to the button, which is
    /// how the form learns that Settings closed and reads the account again (the module docs).
    settings_open: bool,
    /// The moment Settings closed, while the read it started is out: an answer with a key moves focus to Name through
    /// it (the module docs).
    returned: Option<Mark>,
    /// The window the form is drawn in, where an answer moves focus and a close the editor answers for closes Add host.
    window: AnyWindowHandle,
    /// The recipe editor, while "New recipe…" has it open in place of the fields.
    editor: Option<Entity<RecipeEditor>>,
    /// The Recipe choice's focus handle, where focus goes when the editor closes.
    recipe_choice: FocusHandle,
    /// The Add host dialog the form is drawn in: a close the editor answers for (its leave question's Save or Discard)
    /// closes it.
    dialog: WeakEntity<AddHostDialog>,
    _settings_closed: Subscription,
    /// Redraws the recipe choice when the pushed catalog changes.
    _store: ObservingCurrent,
}

impl TensorlakeForm {
    /// The form for `dialog`, opened by `opener`, running under its `key`: empty with the account read started for a
    /// new choice of the kind, or as `seed` left it, without a read, for a reopened Add host.
    pub(super) fn new(
        dialog: WeakEntity<AddHostDialog>,
        key: OpKey,
        opener: FocusHandle,
        seed: Option<SandboxSeed>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (typed, recipe, account, problem) = match seed {
            Some(seed) => (
                seed.name,
                Some(SharedString::from(seed.recipe)),
                Some(seed.account),
                seed.problem,
            ),
            None => (String::new(), None, None, None),
        };
        let read = account.is_none();
        let name = DraftInput::new(
            &typed,
            TextInputState::new(window, cx),
            |form: &mut Self| &mut form.name,
            Self::proceed,
            window,
            cx,
        );
        let open_settings = cx.focus_handle().tab_stop(true);
        let settings_closed = cx.on_focus(&open_settings, window, |form, _, cx| {
            if std::mem::take(&mut form.settings_open) {
                // Settings closing is the user's last action, which a later focus move is judged against.
                form.returned = Some(Mark::take(cx));
                form.read_account(cx);
            }
        });
        let mut form = TensorlakeForm {
            account: account.map_or(Account::Reading, Account::Read),
            key,
            opener,
            problem,
            probe_mark: Mark::take(cx),
            name,
            recipe,
            open_settings,
            settings_open: false,
            returned: None,
            window: window.window_handle(),
            editor: None,
            recipe_choice: cx.focus_handle().tab_stop(true),
            dialog,
            _settings_closed: settings_closed,
            _store: session::observe_current(cx, |_, cx| cx.notify()),
        };
        if read {
            form.read_account(cx);
        }
        form
    }

    /// The account as read, once it is.
    fn settings(&self) -> Option<&Settings> {
        match &self.account {
            Account::Read(account) => Some(account),
            Account::Reading | Account::Failed(_) => None,
        }
    }

    /// Whether the account says an API key is saved, which the form needs before anything else.
    fn key_saved(&self) -> bool {
        self.settings().is_some_and(|account| account.api_key_set)
    }

    /// Name's focus handle, where a reopened Add host starts.
    pub(super) fn name_handle(&self, cx: &App) -> FocusHandle {
        self.name.input().focus_handle(cx)
    }

    /// Read the account. The first read and a Retry show "Loading Tensorlake settings…" until it answers; a read again
    /// after Settings closed keeps the account it had on screen meanwhile, so focus stays on "Open Settings →
    /// Tensorlake", where closing Settings put it, rather than falling to the next control when the button would give
    /// way to the loading line, and its answer with a key moves focus to Name (the module docs). A read that cannot be
    /// sent (no UI session, which the dialog does not outlive in practice) shows why, with Retry, rather than loading
    /// for good.
    fn read_account(&mut self, cx: &mut Context<Self>) {
        // A constant path: it always builds.
        let Ok(path) = ApiPath::api(&["tensorlake"]) else {
            return;
        };
        let form = cx.weak_entity();
        let sent = crate::operations::read::<Settings>(path, ACCOUNT_ROUTE, cx, move |read, cx| {
            let _ = form.update(cx, |form, cx| form.account_read(read, cx));
        });
        let on_screen = matches!(self.account, Account::Read(_));
        if sent {
            if !on_screen {
                self.account = Account::Reading;
            }
        } else {
            // Nothing will answer: keep an account on screen as it is, or say why there is none, with Retry.
            self.returned = None;
            if !on_screen {
                self.account = Account::Failed(HostText::from_host(NOT_SENT));
            }
        }
        cx.notify();
    }

    /// The account read answered: show what it says, and after a return from Settings that brought a key, move focus to
    /// Name through the mark Settings' close took, unless the user moved on since.
    fn account_read(&mut self, read: Result<Settings, HostText>, cx: &mut Context<Self>) {
        let had_key = self.key_saved();
        self.account = match read {
            Ok(settings) => Account::Read(settings),
            Err(words) => Account::Failed(words),
        };
        let has_key = self.key_saved();
        if let Some(mark) = self.returned.take()
            && has_key
            && !had_key
        {
            // Name is drawn by the next frame; the coordinator keeps the move until then.
            let target = self.name.input().focus_handle(cx);
            let _ = self.window.update(cx, |_, window, cx| {
                focus::focus_in_layer(&target, &mark, window, cx)
            });
        }
        cx.notify();
    }

    /// The recipe editor, while "New recipe…" has it open: Add host pins its answers and asks it before closing.
    pub(super) fn editor(&self) -> Option<Entity<RecipeEditor>> {
        self.editor.clone()
    }

    /// "New recipe…": open the recipe editor in place of the fields, with focus in its Name (the module docs).
    pub(super) fn open_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mark = Mark::take(cx);
        let form = cx.weak_entity();
        let departed: RecipeDeparted = Rc::new(move |id, departure, saved, mark, window, cx| {
            let _ = form.update(cx, |form, cx| {
                form.editor_left(id, departure, saved, mark, window, cx)
            });
        });
        let editor = cx.new(|cx| RecipeEditor::for_add_host(departed, window, cx));
        let target = editor.read(cx).start(cx);
        self.editor = Some(editor);
        focus::focus_in_layer(&target, &mark, window, cx);
        cx.notify();
    }

    /// The editor `id` left for `departure` (Cancel or a done Save go back, a leave question asked by closing Add host
    /// closes it), at the press `mark`, with the recipe's name when it is saved. Nothing if `id` is no longer the open
    /// editor, so a late reply never closes an editor opened since. Back chooses the saved recipe and puts focus on
    /// Recipe, unless the user moved on since `mark`.
    fn editor_left(
        &mut self,
        id: EntityId,
        departure: RecipeDeparture,
        saved: Option<String>,
        mark: &Mark,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.editor.as_ref().map(Entity::entity_id) != Some(id) {
            return;
        }
        self.editor = None;
        cx.notify();
        match departure {
            RecipeDeparture::Back => {
                if let Some(name) = saved {
                    self.recipe = Some(name.into());
                }
                // The Recipe choice is drawn again by the next frame; the coordinator keeps the move until then.
                let target = self.recipe_choice.clone();
                match window {
                    Some(window) => focus::focus_in_layer(&target, mark, window, cx),
                    None => {
                        let mark = *mark;
                        let _ = self.window.update(cx, |_, window, cx| {
                            focus::focus_in_layer(&target, &mark, window, cx)
                        });
                    }
                }
            }
            RecipeDeparture::Close => self.close_dialog(None, self.window, cx),
        }
    }

    /// Whether the probe or the creation is out under Add host's key.
    fn busy(&self, cx: &App) -> bool {
        super::key_busy(&self.key, cx)
    }

    /// The name as it is sent: as typed, trimmed.
    fn name(&self) -> String {
        self.name.draft().value().trim().to_owned()
    }

    /// Continue, from the button, Enter in Name or Enter on the dialog: refuse what can be refused here, else ask the
    /// helm for a creation plan (the module docs). Nothing while the probe or the creation is out, while the recipe
    /// editor is open, or before the account says a key is saved.
    pub(super) fn proceed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.key_saved() || self.editor.is_some() || self.busy(cx) {
            return;
        }
        let name = self.name();
        let refused = if name.is_empty() {
            Some("Enter a name for the host".to_owned())
        } else if self.recipe.is_none() {
            Some("Choose a recipe".to_owned())
        } else {
            hosts::launch_in_flight(cx).then(|| format!("Add host: {}", hosts::LAUNCH_IN_FLIGHT))
        };
        if let Some(refused) = refused {
            self.problem = Some(error_line().text(refused));
            cx.notify();
            return;
        }
        let Some(recipe) = self.recipe.clone() else {
            return;
        };
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let command = match probe_command(&name, &recipe) {
            Ok(command) => command,
            Err(error) => {
                tracing::debug!(?error, "Tensorlake probe not built");
                return;
            }
        };
        let handle = window.window_handle();
        self.probe_mark = Mark::take(cx);
        let form = cx.weak_entity();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&self.key, command, cx, move |settled, cx| {
                let _ = form.update(cx, |form, cx| form.planned(settled, handle, cx));
            })
        });
        match started {
            Ok(()) => {
                self.problem = None;
                self.name.input().set_read_only(true, cx);
                cx.notify();
            }
            Err(not_started) => tracing::debug!(?not_started, "Tensorlake probe not sent"),
        }
    }

    /// The probe settled: free the fields, then confirm the plan on the prompt, submit it without asking, or say why
    /// there is none (the module docs).
    fn planned(&mut self, settled: &Settled, window: AnyWindowHandle, cx: &mut Context<Self>) {
        self.name.input().set_read_only(false, cx);
        cx.notify();
        let plan = match read_plan(&settled.outcome) {
            Ok(plan) => plan,
            Err(problem) => {
                self.problem = Some(problem.sentence(error_line()));
                return;
            }
        };
        let (Some(account), Some(recipe)) = (self.settings(), self.recipe.clone()) else {
            return;
        };
        let skips = account.create_without_asking;
        let mark = self.probe_mark;
        let offer = setup::Offer {
            key: self.key.clone(),
            probe_id: plan.probe_id,
            confirmation: plan.confirmation,
            steps: plan.steps,
            target: setup::Target::Sandbox(setup::Sandbox {
                opener: self.opener.clone(),
                name: self.name(),
                recipe: recipe.to_string(),
                account: account.clone(),
                resources: plan.resources,
            }),
            // As an SSH plan's: the answer's own mark replaces it, and without asking the plan goes out on the strength
            // of the Continue.
            mark,
        };
        if skips {
            // The close is queued first, as Add's is on its own path without asking: a submission refused at once (a
            // launch started while the probe was out) reopens Add host on a later turn.
            self.close_dialog(None, window, cx);
            setup::submit(offer, window, cx);
        } else if focus::has_moved_on(&mark, cx) {
            self.problem = Some(error_line().text(MOVED_ON));
        } else {
            self.close_dialog(Some(offer), window, cx);
        }
    }

    /// Close Add host from a completion, then ask `confirm`'s confirmation in its place ([`AddHostDialog::close_later`]).
    fn close_dialog(
        &self,
        confirm: Option<setup::Offer>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let _ = self
            .dialog
            .update(cx, |dialog, cx| dialog.close_later(confirm, window, cx));
    }

    /// The name as typed, for the tests to read back a kept draft.
    #[cfg(test)]
    pub(super) fn name_typed(&self) -> String {
        self.name.draft().value().clone()
    }

    /// Whether Continue can be pressed: both Name and Recipe are set, as the old Continue required.
    fn can_continue(&self) -> bool {
        !self.name().is_empty() && self.recipe.is_some()
    }

    /// Continue, for Add host's footer in the place of Add: "Checking…" and busy while the probe is out.
    pub(super) fn continue_button(&self, form: WeakEntity<Self>, cx: &App) -> Button {
        let busy = self.busy(cx);
        Button::new(
            "add-tensorlake-continue",
            if busy { "Checking…" } else { "Continue" },
            HoverHelp::tooltip(CONTINUE_HELP),
            Role::Button,
        )
        .tier(Tier::Primary)
        .busy(busy)
        .disabled(!busy && !self.can_continue())
        .on_click(move |_, window, cx| {
            let _ = form.update(cx, |form, cx| form.proceed(window, cx));
        })
    }

    /// The recipes the choice offers: the pushed catalog's, in its order. Empty before the catalog arrives and while the
    /// window is inert.
    fn recipes(cx: &App) -> Vec<RecipeView> {
        session::current_store(cx)
            .map(|store| {
                store
                    .read(cx)
                    .collections()
                    .ordered::<RecipeView>()
                    .into_iter()
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The form with a key saved: Name, Recipe and the chosen recipe's summary.
    fn fields(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let palette = theme::palette(cx);
        let recipes = Self::recipes(cx);
        let summary = self.recipe.as_ref().and_then(|chosen| {
            recipes
                .iter()
                .find(|recipe| recipe.name == chosen.as_ref())
                .map(|recipe| HostText::from_host(settings::recipe_summary(recipe)))
        });
        let form = cx.weak_entity();
        let mut recipe = Select::new(
            "add-tensorlake-recipe",
            "Recipe",
            HoverHelp::tooltip("Recipe: the software and size the sandbox is created with"),
            Role::ComboBox,
        )
        .placeholder("Choose a recipe")
        .retained(
            self.recipe
                .as_ref()
                .map(|name| format!("{name} (unavailable)"))
                .unwrap_or_default(),
            HoverHelp::tooltip("This recipe is no longer in the recipe list; choose another"),
        )
        .options(recipes.iter().map(|recipe| {
            SelectOption::new(
                recipe.name.clone(),
                recipe.name.clone(),
                HoverHelp::tooltip(format!(
                    "Create the sandbox from the recipe {}",
                    recipe.name
                )),
            )
        }))
        .options([SelectOption::new(
            NEW_RECIPE,
            "New recipe…",
            HoverHelp::tooltip("New recipe…: make a recipe here, without leaving Add host"),
        )])
        .focus(&self.recipe_choice)
        .disabled(self.busy(cx))
        .on_choose(move |value, window, cx| {
            let value = value.clone();
            let _ = form.update(cx, |form, cx| {
                if value.as_ref() == NEW_RECIPE {
                    form.open_editor(window, cx);
                } else {
                    form.recipe = Some(value);
                    cx.notify();
                }
            });
        });
        if let Some(chosen) = &self.recipe {
            recipe = recipe.chosen(chosen.clone());
        }
        let mut fields = vec![
            field(
                "Name",
                TextInput::new(
                    "add-tensorlake-name",
                    self.name.input(),
                    "Name",
                    HoverHelp::tooltip(
                        "Name: how the host is shown in the rail and on session rows",
                    ),
                )
                .height(px(FIELD_HEIGHT)),
                palette,
            ),
            help(
                "Shown in the rail and on session rows. The sandbox is named farhelm-host-…",
                palette,
            ),
            field("Recipe", recipe, palette),
            help(
                "What the sandbox runs. New recipe… opens the editor here.",
                palette,
            ),
        ];
        // The summary quotes an image or snapshot name the recipe's author typed, so it is host text.
        fields.extend(summary.map(|summary| {
            host_text::element(&summary)
                .text_size(theme::metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .into_any_element()
        }));
        // The helm's words, which may quote the name or the recipe, so they are host text.
        fields.extend(
            self.problem
                .clone()
                .map(|line| hosts::problem_line(line, palette)),
        );
        fields
    }

    /// The body without a key saved: why, and the way to Settings → Tensorlake.
    fn no_key(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let palette = theme::palette(cx);
        let form = cx.weak_entity();
        let opener = self.open_settings.clone();
        vec![
            help(
                "Save a Tensorlake API key in Settings → Tensorlake before adding a sandbox.",
                palette,
            ),
            div()
                .flex()
                .child(
                    Button::new(
                        "add-tensorlake-open-settings",
                        "Open Settings → Tensorlake",
                        HoverHelp::tooltip(
                            "Open Settings at Tensorlake, where the API key is saved",
                        ),
                        Role::Button,
                    )
                    .tier(Tier::Primary)
                    .focus(&self.open_settings)
                    .on_click(move |_, window, cx| {
                        let mark = Mark::take(cx);
                        if settings::open_at(Section::Tensorlake, &opener, &mark, window, cx)
                            .is_some()
                        {
                            let _ = form.update(cx, |form, _| form.settings_open = true);
                        }
                    }),
                )
                .into_any_element(),
        ]
    }
}

/// `POST /api/hosts/probe` for a new sandbox named `name` from the recipe `recipe`: the helm's free, local plan, kept
/// under the one-use id it answers with.
fn probe_command(name: &str, recipe: &str) -> Result<Command, PathError> {
    Ok(Command::new(
        Work::Probe,
        Method::Post,
        ApiPath::api(&["hosts", "probe"])?,
        Some(json!({ "target": { "kind": "tensorlake", "name": name, "recipe": recipe } })),
        "POST /api/hosts/probe",
    ))
}

/// A Tensorlake creation plan as the probe answered it: Continue's for a new sandbox, and a sandbox's rerun's
/// (`crate::hosts::setup::rerun`).
pub(in crate::hosts) struct Plan {
    /// The helm's one-use id for the plan it kept.
    pub(in crate::hosts) probe_id: String,
    /// The size the plan creates the sandbox with.
    pub(in crate::hosts) resources: RecipeSize,
    /// The helm's plan as text, which Copy plan copies.
    pub(in crate::hosts) confirmation: String,
    pub(in crate::hosts) steps: PlanSteps,
}

/// Read a Tensorlake probe's reply: the plan, or why there is none to show. A reply of any other kind is the old app's
/// "did not return a Tensorlake creation plan".
pub(in crate::hosts) fn read_plan(outcome: &Outcome) -> Result<Plan, Problem> {
    let body = match outcome {
        Outcome::Refused(refusal) => return Err(Problem::Helm(refusal.words.clone())),
        Outcome::Done(body) | Outcome::Accepted(body) => body.as_ref().unwrap_or(&Value::Null),
    };
    match ProbeReply::<IgnoredAny>::deserialize(body) {
        Ok(ProbeReply::TensorlakeProvisionable {
            probe_id,
            resources,
            confirmation,
            steps,
            ..
        }) => Ok(Plan {
            probe_id,
            resources,
            confirmation,
            steps: *steps,
        }),
        Ok(ProbeReply::Discovered { .. })
        | Ok(ProbeReply::OwnedElsewhere { .. })
        | Ok(ProbeReply::Manual { .. })
        | Ok(ProbeReply::Provisionable { .. }) => Err(Problem::App(
            "The helm did not return a Tensorlake creation plan",
        )),
        // The decoder's words quote the reply, so they are host text.
        Err(error) => Err(Problem::Unreadable(HostText::from_host(error.to_string()))),
    }
}

impl Render for TensorlakeForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let parts = match &self.account {
            Account::Reading => vec![help("Loading Tensorlake settings…", palette)],
            Account::Failed(words) => {
                let form = cx.weak_entity();
                vec![
                    hosts::problem_line(
                        HostSentence::new("add-tensorlake-account-error").host(words),
                        palette,
                    ),
                    div()
                        .flex()
                        .child(
                            Button::new(
                                "add-tensorlake-account-retry",
                                "Retry",
                                HoverHelp::tooltip("Retry: read the Tensorlake settings again"),
                                Role::Button,
                            )
                            .tier(Tier::Neutral)
                            .on_click(move |_, _, cx| {
                                let _ = form.update(cx, |form, cx| form.read_account(cx));
                            }),
                        )
                        .into_any_element(),
                ]
            }
            Account::Read(account) if !account.api_key_set => self.no_key(cx),
            Account::Read(_) => match self.editor.clone() {
                Some(editor) => vec![editor.into_any_element()],
                None => self.fields(cx),
            },
        };
        div().flex().flex_col().gap(px(BODY_GAP)).children(parts)
    }
}
