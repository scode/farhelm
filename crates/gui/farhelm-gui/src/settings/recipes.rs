//! Settings → Recipes: the helm's one shared catalog of recipes, one row per recipe with its summary and its snapshot
//! build, and the build action (Build snapshot, Rebuild snapshot, Retry).
//!
//! SPEC.md, "Topology" (the Recipes paragraphs: "The list shows each recipe's name, supported providers, base, package
//! count, tunnel count and build status", "Saving a recipe never builds one, because a build bills", "A list refresh
//! that fails keeps the rows shown, says they may be out of date, and offers **Retry loading**"); SPEC_impl.md,
//! "Tensorlake recipes", "Tensorlake snapshot builds" and "What is pushed, and how changes are found". Ported from the
//! old app's `RecipeList` and its status lines (`tensorlake/recipes.rs`) and `RecipesSettings`
//! (`tensorlake_settings.rs`), with their words and the `.recipe-*` rules in `app.css`, all at reference commit
//! `24bc0580`. Here's the TLDR of its rules:
//!
//! - **The rows are the pushed catalog.** The helm pushes the recipe catalog as the `recipes` collection, the entries
//!   `GET /api/tensorlake/recipes` serves, and re-reads it after every change it announces (each build step among them)
//!   and after each of this app's commands. The rows draw it straight from the store, so a running build's step moves
//!   on screen and its end shows while the section stays shown, as in the old list, which read the catalog again on
//!   every change the helm announced. Until the helm pushed the catalog (decided by the migration's orchestrator on
//!   2026-11-15), this section read it each time it was shown and after an accepted build, and a row kept the step it
//!   last read; nothing reads it now. A collection the store has not installed is "Loading recipes…". A collection the
//!   helm could not read (`Availability::Unavailable`) keeps the rows it last pushed, says they may be out of date, and
//!   offers Retry loading; with no rows it shows the helm's reason alone, never "No recipes yet.".
//! - **Retry loading is any command.** The GUI has no read of its own left to repeat, but the helm re-runs every pushed
//!   read after each command it answers, so Retry loading sends the section's one remaining read, the account's, and
//!   the re-read after it either brings the catalog back or leaves it unavailable with the new reason.
//! - **The account is still read.** Whether a Tensorlake API key is saved (`GET /api/tensorlake`) gates Build and is
//!   not pushed, so the section reads it each time it is shown in an opening of the dialog, as before. Until its first
//!   answer, Build is unavailable without the no-key note, so the rows, which draw from the store at once, never claim
//!   for a frame that no key is saved.
//! - **What a row says.** The name, a one-line summary (size, base, package count, tunnel count), and the recipe's
//!   built snapshot: when it was built and with which agents, warnings when the base, the packages or a smaller disk
//!   have moved on since, a running build's step ("step 5 of 9") with the running pulse, a failed build's step and
//!   reason, an interrupted build, cleanup still pending, and a base the snapshot's provider cannot run
//!   ([`snapshot_status`]). The agents' versions and a failure's reason come from the build sandbox, so they are drawn
//!   as host text; the rest is the user's own recipe or the app's words.
//! - **Builds are accepted work.** Build snapshot, Rebuild snapshot and Retry send the same request, which starts a
//!   build or joins the running one, through the operation store under the recipe's key ([`OpKey::recipe`]). The helm
//!   answers 202 once the build runs on its own task, and pushes the recipe building before that reply. A refusal (no
//!   API key, no SSH key, a recipe being deleted) stays under the row, in the helm's words, until the next build of
//!   that recipe starts or the pushed catalog shows the recipe building (the old row's rule, [`settle_refusals`]), so a
//!   build started elsewhere since does not bring an old refusal back once it ends. The request needs a saved API key,
//!   so without one the button is unavailable and the row says so, as the old one did. The work is [`Work::Save`]: the
//!   store treats it like an editor's save (busy only on its own key, never superseded), and a kind of its own would
//!   need an arm in other areas' exhaustive matches over work for no difference in behavior. The key is shared with
//!   the recipe's editor, whose own refusals it clears ([`OpKey::recipe`]).
//! - **The pulse only while shown.** The running pulse is held only while a row shows a running build, the window is
//!   active and the section is the one shown: the dialog keeps the section's state while another section shows, and
//!   tells it to let the pulse go then ([`Recipes::hidden`]), since a held pulse keeps the ticker waking for nothing.
//!
//! NOTE: The push does not follow the clock. What a recipe has prepared reads stale once it passes its 24-hour
//! freshness window only when the helm next re-reads the catalog, on its next announced change or command, not when the
//! window passes. Nothing here re-reads for it: the old list did not either (it re-read on the helm's change
//! notices, which ageing does not send, and said readiness was "when last checked" for that reason), and the helm
//! decides freshness again whenever a launch uses the artifact. Decided by the migration's settings-and-hosts area on
//! 2026-11-15, porting the old app's behavior as the migration's orchestrator asked.
//!
//! NOTE: Not ported yet, each waiting for the change that brings it (`docs/gpui/parity/settings-and-hosts.md`):
//! New recipe and each row's Edit, which open the recipe editor, are not drawn until the editor exists; a row's
//! prepared-snapshot line and its per-place container lines with their Rebuild come next.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::Place;
use farhelm_proto::recipes::{BuildStep, BuildView, RecipeBase, RecipeView};
use farhelm_proto::ui_channel::{Availability, Collection};
use gpui::{
    AnyElement, App, Context, Div, ElementId, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Role, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::choices;
use crate::components::{Button, HoverHelp, Tier, pulse_opacity};
use crate::environment_ui::ProviderUi;
use crate::host_text::{HostSentence, HostText};
use crate::operations::{Command, OpKey, Work};
use crate::session;
use crate::theme::{self, Palette, metrics};
use crate::ticker;

/// `.tensorlake-recipes`' gap between the section's parts.
const PART_GAP: f32 = 8.;
/// `.recipe-list`'s gap between rows.
const ROW_GAP: f32 = 6.;
/// `.recipe-row`'s padding, top and bottom then left and right, and its gap between the row's text and its controls.
const ROW_PAD: (f32, f32) = (8., 10.);
const ROW_INNER_GAP: f32 = 10.;
/// `.recipe-row-main`'s and `.recipe-row-snapshot`'s gap between lines.
const LINE_GAP: f32 = 2.;
/// `.recipe-row-status`'s gap between the running dot and its words.
const DOT_GAP: f32 = 6.;
/// `.delete-progress-dot`'s size.
const DOT: f32 = 6.;
/// `.recipe-row-actions`' gap between controls and its margin above.
const ACTIONS_GAP: f32 = 8.;
const ACTIONS_MARGIN: f32 = 4.;

/// The section's help, the old words.
const HELP: &str = "A recipe describes a new sandbox or container: its software, credentials and tunnels back to your \
                    side. The base determines which providers can run it. Size applies to Tensorlake sandboxes.";

/// The note beside an unavailable build button. The old app's said "Save an API key above to build snapshots.", which
/// was already inaccurate there: its key forms were in Tensorlake and Credentials, both listed below Recipes. Here the
/// key form is in Credentials, so the note names that section instead (`docs/gpui/parity/settings-and-hosts.md`,
/// "Deviations").
const NO_KEY: &str = "Save an API key in Settings → Credentials to build snapshots.";

/// The place whose artifacts a [`RecipeView`]'s `snapshot`, `build` and `prepared` fields describe: the helm reports
/// the Tensorlake account's built snapshot there, and every other place's artifacts in `GET /api/recipe-places`. Named
/// once so the row asks this place's provider whether the base can run there, and takes its words from the provider
/// wording module, rather than naming a provider itself (AGENTS.md, "Code rules").
const SNAPSHOT_PLACE: Place = Place::TensorlakeAccount;

/// `POST /api/tensorlake/recipes/<name>/build`: start the recipe's snapshot build or join the one running, what Build,
/// Rebuild and Retry all send. Named in a wordless refusal by its method and route: a recipe's name is the user's own,
/// not an internal id, so it is shown.
fn build_command(name: &str) -> Result<Command, PathError> {
    let path = ApiPath::api(&["tensorlake", "recipes", name, "build"])?;
    let shown = format!("POST {}", path.as_str());
    Ok(Command::new(Work::Save, Method::Post, path, None, shown))
}

// ---- What a row says -----------------------------------------------------------------------------------------------

/// The one-line summary under a recipe's name: size, base, package count, then the tunnel count only when there are
/// tunnels, so choosing a recipe shows what its next build will contain. The old `recipe_summary`'s words.
pub(super) fn summary(view: &RecipeView) -> String {
    let plural = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    let mut parts = vec![
        format!("{} CPU", view.size.cpus),
        format!("{} GB", view.size.memory_gb),
        format!("{} GB", view.size.disk_gb),
    ];
    parts.push(match &view.base {
        RecipeBase::ManagedUbuntu => "base: Tensorlake Ubuntu".into(),
        RecipeBase::TensorlakeImage { image } => format!("base: your image {image}"),
        RecipeBase::TensorlakeSnapshot { snapshot_id } => {
            format!("base: your snapshot {snapshot_id}")
        }
        RecipeBase::ContainerImage { reference, user } => {
            format!("base: container image {reference} as {user} (Podman only)")
        }
    });
    parts.push(plural(view.packages.len(), "package", "packages"));
    if !view.tunnels.is_empty() {
        parts.push(plural(view.tunnels.len(), "tunnel", "tunnels"));
    }
    parts.join(" · ")
}

/// A status line's tone, the old `.recipe-row-status-*` classes: what color its words take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    /// The ordinary quiet line (`fg-1`).
    Plain,
    /// Something the user may want to act on, such as a stale snapshot (`warn-bright`).
    Warning,
    /// A failed build (`danger`).
    Error,
    /// Background housekeeping the user need not act on (`fg-2`).
    Muted,
}

impl Tone {
    /// The ink this tone draws its words in.
    fn ink(self, palette: &Palette) -> Hsla {
        theme::color(match self {
            Tone::Plain => palette.fg_1,
            Tone::Warning => palette.warn_bright,
            Tone::Error => palette.danger,
            Tone::Muted => palette.fg_2,
        })
    }
}

/// One piece of a status line's words: the app's own, or text that came from a build sandbox (an agent's version, a
/// failure's reason), which reaches the screen only escaped.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Words {
    App(String),
    Host(HostText),
}

/// One line of a row's snapshot status: its tone and its words, in order, and whether it is a running build's line,
/// which carries the running pulse's dot before its words.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Line {
    pub(super) tone: Tone,
    pub(super) words: Vec<Words>,
    pub(super) running: bool,
}

impl Line {
    /// A line of the app's own words only.
    fn app(tone: Tone, text: impl Into<String>) -> Line {
        Line {
            tone,
            words: vec![Words::App(text.into())],
            running: false,
        }
    }

    /// The line's text as written, host text unescaped: what tests compare.
    #[cfg(test)]
    pub(super) fn text(&self) -> String {
        self.words
            .iter()
            .map(|words| match words {
                Words::App(text) => text.as_str(),
                Words::Host(text) => text.raw(),
            })
            .collect()
    }

    /// The line as a sentence under `id`, its host pieces escaped.
    fn sentence(&self, id: String) -> HostSentence {
        self.words.iter().fold(
            HostSentence::new(ElementId::Name(id.into())),
            |sentence, words| match words {
                Words::App(text) => sentence.text(text.clone()),
                Words::Host(text) => sentence.host(text),
            },
        )
    }
}

/// The build a row offers. All three send the same request ([`build_command`]); the name says which state the user
/// is recovering from or building on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuildAction {
    Build,
    Rebuild,
    Retry,
}

impl BuildAction {
    /// The button's words.
    fn label(self) -> &'static str {
        match self {
            BuildAction::Build => "Build snapshot",
            BuildAction::Rebuild => "Rebuild snapshot",
            BuildAction::Retry => "Retry",
        }
    }

    /// The button's hover help, the old words: it says before a click that a build bills and that a rebuild replaces
    /// the snapshot.
    fn help(self) -> &'static str {
        match self {
            BuildAction::Build => {
                "Build this recipe's snapshot in a temporary Tensorlake sandbox, which bills while it runs"
            }
            BuildAction::Rebuild => {
                "Build a fresh snapshot from this recipe, then delete the old one; existing hosts keep running"
            }
            BuildAction::Retry => "Start the snapshot build again",
        }
    }
}

/// What a row says about its recipe's built snapshot, and what it offers.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SnapshotStatus {
    /// The lines in order; a running build's line pulses ([`Line::running`]), and then the row offers no build.
    pub(super) lines: Vec<Line>,
    pub(super) action: Option<BuildAction>,
}

impl SnapshotStatus {
    /// Whether a line is a running build's, what tests ask instead of reading the lines' flags one by one.
    #[cfg(test)]
    pub(super) fn running(&self) -> bool {
        self.lines.iter().any(|line| line.running)
    }
}

/// Whether the helm reports `view`'s snapshot build running: the one test for it, which the row's status, the pulse and
/// the settling of a refusal ([`settle_refusals`]) all ask.
fn building(view: &RecipeView) -> bool {
    matches!(view.build, BuildView::Building { .. })
}

/// A UTC date and minute, `2026-11-03 14:02 UTC`: the old row's stamp. Labelled UTC because the helm's epoch seconds
/// are not the viewer's local time, and the GUI carries no time zone database.
fn utc_minute(secs: i64) -> String {
    let (year, month, day) = farhelm_proto::time::civil_from_days(secs.div_euclid(86_400));
    let time = secs.rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60
    )
}

/// "step 5 of 9".
fn step_of(step: BuildStep) -> String {
    format!("step {} of {}", step.number(), BuildStep::COUNT)
}

/// The status of a recipe's built snapshot, from its view alone: the old `snapshot_status`, line for line.
///
/// A built snapshot stays listed while a rebuild runs or after one failed, because hosts keep starting from it until a
/// new one replaces it. A running or interrupted build, and anything that still awaits deletion, show even after a base
/// change. A save never replaces the current snapshot, so a changed base or package list warns until a successful build
/// records it; packages count as unchanged in any order. A base the snapshot's provider cannot run says so and offers
/// no build, since the helm would refuse it.
pub(super) fn snapshot_status(view: &RecipeView) -> SnapshotStatus {
    let mut lines = Vec::new();
    if let Some(snapshot) = &view.snapshot {
        let mut built = vec![Words::App(format!(
            "Snapshot built {}",
            utc_minute(snapshot.built_at)
        ))];
        for version in [&snapshot.claude_version, &snapshot.codex_version]
            .into_iter()
            .flatten()
        {
            built.push(Words::App(" · ".into()));
            built.push(Words::Host(HostText::from_host(version.clone())));
        }
        lines.push(Line {
            tone: Tone::Plain,
            words: built,
            running: false,
        });
        if view.base != snapshot.base {
            lines.push(Line::app(
                Tone::Warning,
                "Base changed since this snapshot was built; rebuild it to apply the change",
            ));
        }
        let mut wanted = view.packages.clone();
        let mut had = snapshot.packages.clone();
        wanted.sort();
        had.sort();
        if wanted != had {
            lines.push(Line::app(
                Tone::Warning,
                "Packages changed since this snapshot was built; rebuild it to apply them",
            ));
        }
        if view.size.disk_gb < snapshot.disk_gb {
            lines.push(Line::app(
                Tone::Warning,
                format!(
                    "The disk is now smaller than this snapshot's {} GB; rebuild before adding hosts",
                    snapshot.disk_gb
                ),
            ));
        }
    }
    let mut action = match &view.build {
        BuildView::Building { step, .. } => {
            lines.push(Line {
                running: true,
                ..Line::app(
                    Tone::Plain,
                    format!("Building snapshot: {} ({})", step.label(), step_of(*step)),
                )
            });
            None
        }
        BuildView::Failed { step, message, .. } => {
            lines.push(Line {
                tone: Tone::Error,
                words: vec![
                    Words::App(format!(
                        "Snapshot build failed while {} ({}): ",
                        step.label(),
                        step_of(*step)
                    )),
                    Words::Host(HostText::from_host(message.clone())),
                ],
                running: false,
            });
            Some(BuildAction::Retry)
        }
        BuildView::Interrupted { .. } => {
            lines.push(Line::app(
                Tone::Warning,
                "The last snapshot build stopped when the helm stopped",
            ));
            Some(BuildAction::Retry)
        }
        BuildView::Idle if view.snapshot.is_some() => Some(BuildAction::Rebuild),
        BuildView::Idle => {
            lines.push(Line::app(Tone::Plain, "No snapshot yet"));
            Some(BuildAction::Build)
        }
    };
    if view.cleanup_pending {
        lines.push(Line::app(
            Tone::Muted,
            "Farhelm could not delete an old build or preparation sandbox or snapshot yet; it tries again at the next \
             build or preparation, whichever left it, and when the recipe is deleted",
        ));
    }
    let provider = SNAPSHOT_PLACE.provider();
    if !view.base.supports(provider) {
        lines.push(Line::app(
            Tone::Warning,
            ProviderUi(provider).recipe_base_unsupported(),
        ));
        action = None;
    }
    SnapshotStatus { lines, action }
}

// ---- The pushed catalog --------------------------------------------------------------------------------------------

/// The recipe catalog as the store holds it now, which the section draws and its refusals are settled against.
struct Catalog {
    /// The recipes in the helm's order: the last ones pushed, kept while the helm cannot read the catalog.
    rows: Vec<RecipeView>,
    /// The helm's reason, while its last read of the catalog failed (`Availability::Unavailable`).
    unavailable: Option<HostText>,
}

/// The pushed catalog, or `None` before a snapshot installed it (and while the window is inert, with no store).
fn catalog(cx: &App) -> Option<Catalog> {
    let store = session::current_store(cx)?;
    let collections = store.read(cx).collections();
    let meta = collections.meta(Collection::Recipes)?;
    Some(Catalog {
        rows: collections
            .ordered::<RecipeView>()
            .into_iter()
            .cloned()
            .collect(),
        unavailable: match &meta.availability {
            Availability::Available => None,
            Availability::Unavailable { error } => Some(HostText::from_host(error.as_str())),
        },
    })
}

/// Forget the last refused build of every recipe the pushed catalog shows building. The build the refusal was about has
/// been asked for since, here or elsewhere, so its words would only come back stale once that build ends (the old
/// row's rule: a refusal stays "until the next click or a re-read shows the build running"). A recipe shown idle keeps
/// its refusal.
///
/// The section calls it whenever the pushed state changes, and once as it is made ([`session::observe_current`]), so
/// reopening Settings drops a refusal whose recipe started building while the dialog was closed. This is not the
/// refusal's own completion, which is where `forget` is usually called. It is safe because the helm pushes the changes a
/// command causes before that command's reply: a press whose request is still out has nothing on its key to drop
/// (`forget` leaves work in flight alone), even when the push showing its own build arrives first, and a refusal it
/// settles with lands after that push and stays until a later one shows the recipe building. A reset retires the old
/// store with its records, and the observation moves to the new store.
fn settle_refusals(cx: &mut App) {
    let (Some(catalog), Some(operations)) = (catalog(cx), session::current_operations(cx)) else {
        return;
    };
    let keys: Vec<OpKey> = catalog
        .rows
        .iter()
        .filter(|recipe| building(recipe))
        .map(|recipe| OpKey::recipe(&recipe.name))
        .collect();
    operations.update(cx, |operations, cx| {
        for key in &keys {
            operations.forget(key, cx);
        }
    });
}

// ---- The section ---------------------------------------------------------------------------------------------------

/// The Recipes section, a view of its own that Settings makes the first time the section shows in an opening and keeps
/// until the dialog closes (see the module docs).
pub(super) struct Recipes {
    /// Whether the helm has a Tensorlake API key saved, as the last account read said: `None` until one answers, and
    /// `Some(false)` after a failed one, as the old list treated a key it could not read. Kept while a later read is
    /// out, so showing the section again does not flicker Build.
    api_key_set: Option<bool>,
    /// The running pulse, held while the section is shown, the window is active and a row shows a build running; let
    /// go while another section shows ([`Recipes::hidden`]).
    pulse: Option<ticker::Pulsing>,
    /// The subscription that settles refusals whenever the pushed state changes ([`settle_refusals`]). Drawing needs
    /// none: the rows read the store while they draw, so GPUI redraws them on every push.
    _pushed: session::ObservingCurrent,
}

impl Recipes {
    /// The section, reading the account now.
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let recipes = Recipes {
            api_key_set: None,
            pulse: None,
            _pushed: session::observe_current(cx, |_: &mut Recipes, cx| settle_refusals(cx)),
        };
        recipes.read_account(cx);
        recipes
    }

    /// Read whether a Tensorlake API key is saved, which is not pushed; the dialog calls it each time it shows the
    /// section again, and Retry loading sends it for the re-read every command brings (the module docs).
    pub(super) fn read_account(&self, cx: &mut Context<Self>) {
        let view = cx.weak_entity();
        choices::read_route(
            ApiPath::api(&["tensorlake"]),
            "GET /api/tensorlake",
            cx,
            move |answer: Result<farhelm_proto::tensorlake::Settings, HostText>, cx| {
                let _ = view.update(cx, |view, cx| {
                    view.api_key_set = Some(answer.is_ok_and(|settings| settings.api_key_set));
                    cx.notify();
                });
            },
        );
    }

    /// Another section is shown in its place: let the running pulse go. The dialog keeps this view, undrawn, until it
    /// closes, and only drawing reconsiders the pulse, so without this a row pushed as building would keep the ticker
    /// waking ten times a second for a section nobody sees. Showing the section again draws it, which takes the pulse
    /// again if a row still shows a build.
    pub(super) fn hidden(&mut self) {
        self.pulse = None;
    }

    /// Build, Rebuild or Retry for the recipe `name`: start or join its snapshot build under the recipe's key. Does
    /// nothing unless an API key is known to be saved, or while the recipe already has work in flight, when the button
    /// is drawn unavailable or busy. An accepted build needs nothing more here: the helm has pushed the recipe building
    /// by the time it replies. A refusal is the store's to keep on the key, where the row shows it.
    fn build(&mut self, name: &str, cx: &mut Context<Self>) {
        if self.api_key_set != Some(true) {
            return;
        }
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let command = match build_command(name) {
            Ok(command) => command,
            Err(error) => {
                tracing::debug!(?error, "recipe build not built");
                return;
            }
        };
        let started = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::recipe(name), command, cx, |_, _| {})
        });
        if let Err(not_started) = started {
            tracing::debug!(name, ?not_started, "recipe build not started");
        }
        cx.notify();
    }

    /// One recipe's row (`.recipe-row`): its name, summary and snapshot status, the build it offers, and the helm's
    /// words for its last refused build.
    fn row(&self, view: &RecipeView, pulse: f32, cx: &Context<Self>) -> Stateful<Div> {
        let palette = theme::palette(cx);
        let name = view.name.clone();
        let status = snapshot_status(view);
        let key = OpKey::recipe(&name);
        let (in_flight, refusal) = session::current_operations(cx)
            .map(|operations| {
                let operations = operations.read(cx);
                (
                    operations.in_flight(&key).is_some(),
                    operations
                        .failure(&key)
                        .map(|failure| failure.message.clone()),
                )
            })
            .unwrap_or_default();
        // Each line's sentence is made once: drawn in its line, and named in the region's label below.
        let sentences: Vec<HostSentence> = status
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| line.sentence(format!("recipe-{name}-snapshot-{index}")))
            .collect();
        let accessible: Vec<String> = sentences
            .iter()
            .map(|sentence| sentence.accessible_name().to_string())
            .collect();
        let lines = status.lines.iter().zip(sentences).map(|(line, sentence)| {
            div()
                .flex()
                .items_center()
                .gap(px(DOT_GAP))
                .min_w_0()
                .text_size(metrics::TEXT_SM)
                .text_color(line.tone.ink(palette))
                .children(line.running.then(|| {
                    div()
                        .flex_none()
                        .size(px(DOT))
                        .rounded_full()
                        .bg(theme::color(palette.accent))
                        .opacity(pulse)
                }))
                .child(sentence)
        });
        // `.recipe-row-snapshot`: one status region per recipe, named for it, so assistive technology reads the whole
        // status as one.
        let snapshot = div()
            .id(ElementId::Name(format!("recipe-{name}-snapshot").into()))
            .role(Role::Status)
            .aria_label(SharedString::from(format!(
                "Snapshot of {name}: {}",
                accessible.join(". ")
            )))
            .flex()
            .flex_col()
            .gap(px(LINE_GAP))
            .min_w_0()
            .children(lines);
        let actions = status.action.map(|action| {
            let weak = cx.weak_entity();
            let label = format!("{} for {name}", action.label());
            let building = name.clone();
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(ACTIONS_GAP))
                .mt(px(ACTIONS_MARGIN))
                .child(
                    Button::new(
                        format!("recipe-build-{name}"),
                        action.label(),
                        HoverHelp::tooltip(action.help()),
                        Role::Button,
                    )
                    .accessible_name(label)
                    .tier(Tier::Neutral)
                    .disabled(self.api_key_set != Some(true))
                    .busy(in_flight)
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |section, cx| section.build(&building, cx));
                    }),
                )
                .children((self.api_key_set == Some(false)).then(|| {
                    div()
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(palette.fg_2))
                        .child(NO_KEY)
                }))
        });
        // The refusal of the last build of this recipe, until its next build starts or the pushed catalog shows it
        // building ([`settle_refusals`]).
        let refused = refusal.map(|reason| {
            choices::failure_outcome(format!("recipe-{name}-build-error"), &reason, cx)
        });
        div()
            .id(ElementId::Name(format!("recipe-{name}").into()))
            .role(Role::ListItem)
            .aria_label(SharedString::from(name.clone()))
            .flex()
            .items_center()
            .gap(px(ROW_INNER_GAP))
            .py(px(ROW_PAD.0))
            .px(px(ROW_PAD.1))
            .rounded(metrics::RADIUS_SM)
            .bg(theme::color(palette.bg_1))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(LINE_GAP))
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .font_family(crate::assets::TERMINAL_FONT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::color(palette.fg_0))
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .text_size(metrics::TEXT_SM)
                            .text_color(theme::color(palette.fg_1))
                            .child(summary(view)),
                    )
                    .child(snapshot)
                    .children(actions)
                    .children(refused),
            )
    }

    /// The rows, the empty line or the loading line, whichever the pushed `catalog` calls for. A catalog the helm could
    /// not read with no rows to keep draws none of them: its reason, drawn below, is the whole story.
    fn listing(
        &self,
        catalog: Option<&Catalog>,
        pulse: f32,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        match catalog {
            None => Some(
                choices::help("Loading recipes…", cx)
                    .id("recipes-loading")
                    .role(Role::Status)
                    .into_any_element(),
            ),
            Some(catalog) if catalog.rows.is_empty() && catalog.unavailable.is_some() => None,
            Some(catalog) if catalog.rows.is_empty() => Some(
                choices::help("No recipes yet.", cx)
                    .id("recipes-empty")
                    .into_any_element(),
            ),
            Some(catalog) => Some(
                div()
                    .id("recipes-list")
                    .role(Role::List)
                    .aria_label("Recipes")
                    .flex()
                    .flex_col()
                    .gap(px(ROW_GAP))
                    .children(catalog.rows.iter().map(|view| self.row(view, pulse, cx)))
                    .into_any_element(),
            ),
        }
    }

    /// Why the helm could not read the catalog, worded for whether rows are still shown, and Retry loading under it.
    fn read_failure(&self, catalog: Option<&Catalog>, cx: &Context<Self>) -> Vec<AnyElement> {
        let Some((reason, kept)) = catalog
            .and_then(|catalog| Some((catalog.unavailable.as_ref()?, !catalog.rows.is_empty())))
        else {
            return Vec::new();
        };
        let sentence = HostSentence::new("recipes-load-error");
        let sentence = if kept {
            sentence
                .text("Could not refresh the recipes, so this list may be out of date: ")
                .host(reason)
        } else {
            sentence.host(reason)
        };
        let line = choices::outcome_line(
            "recipes-load-error",
            Role::Alert,
            theme::color(theme::palette(cx).danger),
        )
        .aria_label(sentence.accessible_name())
        .child(sentence);
        let weak = cx.weak_entity();
        let retry = Button::new(
            "recipes-retry",
            "Retry loading",
            HoverHelp::tooltip("Read the recipes again"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |view, cx| view.read_account(cx));
        });
        vec![
            line.into_any_element(),
            div().flex().child(retry).into_any_element(),
        ]
    }
}

impl Render for Recipes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        // Read from the store while drawing, so every push that changes the catalog redraws the rows.
        let catalog = catalog(cx);
        // Hold the pulse only while a row shows a running build and someone can see the window: the ticker's rule,
        // which keeps the tick loop from firing for a window nobody looks at. Drawing means the section is shown; the
        // dialog lets the pulse go when it is not ([`Recipes::hidden`]).
        let running = catalog
            .iter()
            .flat_map(|catalog| &catalog.rows)
            .any(building);
        let pulses = running && ticker::is_active(window, cx);
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let pulse = pulse_opacity(
            self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx)),
            palette.status_pulse_min,
        );
        div()
            .id("recipes")
            .role(Role::Region)
            .aria_label("Recipes")
            .flex()
            .flex_col()
            .gap(px(PART_GAP))
            .w_full()
            .min_w_0()
            .child(choices::help(HELP, cx))
            .children(self.listing(catalog.as_ref(), pulse, cx))
            .children(self.read_failure(catalog.as_ref(), cx))
    }
}

/// Bridge between the module's pure status and the tests that check it without a window.
#[cfg(test)]
pub(super) fn line_texts(status: &SnapshotStatus) -> Vec<(Tone, String)> {
    status
        .lines
        .iter()
        .map(|line| (line.tone, line.text()))
        .collect()
}
