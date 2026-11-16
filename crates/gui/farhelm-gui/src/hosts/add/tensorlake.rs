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
//!   changes asks the editor's "Save these changes before leaving?" first, and its Save or Discard then closes Add
//!   host. The old dialog swapped its body and footer for the editor's the same way (`in_add_host`).
//! - **Continue is unavailable here.** Continue leads to the creation confirmation and its billing callout, which are
//!   not drawn yet, so the button shows but cannot be pressed, and Enter in Name does nothing.

use farhelm_helm_client::ApiPath;
use farhelm_proto::recipes::RecipeView;
use farhelm_proto::tensorlake::Settings;
use std::rc::Rc;

use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Entity, EntityId, FocusHandle,
    IntoElement, ParentElement as _, Render, Role, SharedString, Styled as _, Subscription,
    WeakEntity, Window, div, px,
};

use super::{AddHostDialog, BODY_GAP, FIELD_HEIGHT, field, help};
use crate::components::{Button, HoverHelp, Select, SelectOption, TextInput, TextInputState, Tier};
use crate::drafts::DraftInput;
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::session::{self, ObservingCurrent};
use crate::settings::{self, RecipeDeparted, RecipeDeparture, RecipeEditor, Section};
use crate::theme;

/// The route the account is read from, as a failed read names it.
const ACCOUNT_ROUTE: &str = "GET /api/tensorlake";

/// What the body says when the account read could not be sent at all.
const NOT_SENT: &str =
    "Farhelm is not connected to its helm, so the Tensorlake settings could not be read";

/// The Recipe choice's option that opens the recipe editor rather than choosing a recipe. No recipe can be named this:
/// the helm's rule allows no underscore in a recipe name (`farhelm_proto::recipes::check_recipe_name`).
const NEW_RECIPE: &str = "__new__";

/// What Continue says on hover: where it leads.
const CONTINUE_HELP: &str =
    "Continue: show the sandbox's size and billing before anything is created";

/// Where the account read stands.
enum Account {
    /// The read is out, the first one or a Retry.
    Reading,
    /// The read failed, in the helm's words or the app's for an answer it could not read.
    Failed(HostText),
    /// The account as the helm answered.
    Read(Settings),
}

/// The Tensorlake sandbox body of one choice of the kind: made each time the user chooses Tensorlake sandbox and
/// dropped, with its draft, when SSH is chosen again (`Body`).
pub(super) struct TensorlakeForm {
    account: Account,
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
    /// The form for a new choice of the Tensorlake sandbox kind in `dialog`: empty, with the account read started.
    pub(super) fn new(
        dialog: WeakEntity<AddHostDialog>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = DraftInput::new(
            "",
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
            account: Account::Reading,
            name,
            recipe: None,
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
        form.read_account(cx);
        form
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
        let had_key = matches!(&self.account, Account::Read(account) if account.api_key_set);
        self.account = match read {
            Ok(settings) => Account::Read(settings),
            Err(words) => Account::Failed(words),
        };
        let has_key = matches!(&self.account, Account::Read(account) if account.api_key_set);
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
    fn open_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            RecipeDeparture::Close => {
                let handle = self.window;
                let _ = self
                    .dialog
                    .update(cx, |dialog, cx| dialog.close_later(None, handle, cx));
            }
        }
    }

    /// Continue, from the button or Enter in Name: unavailable in this build (the module docs).
    pub(super) fn proceed(&mut self, _: &mut Window, _: &mut Context<Self>) {}

    /// The name as typed, for the tests to read back a kept draft.
    #[cfg(test)]
    pub(super) fn name_typed(&self) -> String {
        self.name.draft().value().clone()
    }

    /// Whether Continue can be pressed: never in this build (the module docs).
    fn can_continue(&self) -> bool {
        false
    }

    /// Continue, for Add host's footer in the place of Add.
    pub(super) fn continue_button(&self, form: WeakEntity<Self>) -> Button {
        Button::new(
            "add-tensorlake-continue",
            "Continue",
            HoverHelp::tooltip(CONTINUE_HELP),
            Role::Button,
        )
        .tier(Tier::Primary)
        .disabled(!self.can_continue())
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

impl Render for TensorlakeForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let parts = match &self.account {
            Account::Reading => vec![help("Loading Tensorlake settings…", palette)],
            Account::Failed(words) => {
                let form = cx.weak_entity();
                vec![
                    super::super::problem_line(
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
