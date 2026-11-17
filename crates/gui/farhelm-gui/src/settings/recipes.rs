//! Settings → Recipes: the helm's one shared catalog of recipes, one row per recipe with its summary and its snapshot
//! build, the build action (Build snapshot, Rebuild snapshot, Retry), and New recipe and each row's Edit, which open the
//! recipe editor in place of the list.
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
//!   read after each command it answers, so Retry loading reads the Tensorlake account again, and the re-read after it
//!   either brings the catalog back or leaves it unavailable with the new reason.
//! - **Whether a key is saved comes from the shared account.** Whether a Tensorlake API key is saved gates Build and is
//!   not pushed. Settings reads it once per opening into the account Credentials and Tensorlake share
//!   ([`super::tensorlake_key::TensorlakeAccount`]), and Recipes reads that account as it draws, so a key saved or
//!   cleared in either form shows here at once. Until the account's read answers, Build is unavailable without the
//!   no-key note, so the rows, which draw from the store at once, never claim for a frame that no key is saved; a
//!   failed read counts as no key, as the old list treated a key it could not read.
//! - **What a row says.** The name, a one-line summary (size, base, package count, tunnel count), and the recipe's
//!   built snapshot: when it was built and with which agents, warnings when the base, the packages or a smaller disk
//!   have moved on since, a running build's step ("step 5 of 9") with the running pulse, a failed build's step and
//!   reason, an interrupted build, cleanup still pending, and a base the snapshot's provider cannot run
//!   ([`snapshot_status`]). The agents' versions and a failure's reason come from the build sandbox, so they are drawn
//!   as host text; the rest is the user's own recipe or the app's words. Those lines and the prepared line report the
//!   Tensorlake account, so each block starts with that place's name ([`at_snapshot_place`]), as a container place's
//!   line starts with its host's.
//! - **Builds are accepted work.** Build snapshot, Rebuild snapshot and Retry send the same request, which starts a
//!   build or joins the running one, through the operation store under the recipe's key ([`OpKey::recipe`]). The helm
//!   answers 202 once the build runs on its own task, and pushes the recipe building before that reply. A refusal (no
//!   API key, no SSH key, a recipe being deleted) stays under the row, in the helm's words, until the next build of
//!   that recipe starts or the pushed state shows the recipe start building (the old row's rule,
//!   [`settle_refusals`]), so a build started elsewhere since does not bring an old refusal back once it ends. The
//!   request needs a saved API key, so without one the button is unavailable and the row says so, as the old one did.
//!   The work is [`Work::Save`]: the store treats it like an editor's save (busy only on its own key, never
//!   superseded), and a kind of its own would need an arm in other areas' exhaustive matches over work for no
//!   difference in behavior. The key is shared with the recipe's editor, whose own refusals it clears
//!   ([`OpKey::recipe`]).
//! - **Preparation and places.** Under the built snapshot, a row says whether the snapshot new hosts and sessions start
//!   from is ready ([`prepared`]), and above it, how the recipe stands at each container place, with a Rebuild per place
//!   ([`places`]), from the second pushed collection, `recipe_places`.
//! - **One editor in place.** New recipe and a row's Edit replace the list with an [`Editor`] (its own module docs
//!   have its rules), with focus on its Back; Back, Cancel or a done save close it and put focus on New recipe. A
//!   created or changed recipe needs no read: the helm pushes the catalog's change before it answers the save. A reply
//!   closes only the editor that sent it, never one opened since. The editor's answers are drawn by the dialog, pinned
//!   with its footer ([`Editor::actions`]).
//! - **The pulse and the clock only while shown.** The running pulse is held only while a row shows a running build or
//!   preparation, the window is active and the section is the one shown; the ticker's 30-second clock, which moves the
//!   prepared line's ages on, only while a row shows one and the section is shown. The dialog keeps the section's state
//!   while another section shows, and tells it to let both go then ([`Recipes::hidden`]), since a held handle keeps the
//!   ticker waking for nothing.
//!
//! NOTE: The push does not follow the clock. A prepared snapshot reads stale once it passes its 24-hour freshness
//! window only when the helm next re-reads the catalog, on its next announced change or command, not when the window
//! passes; until then the prepared line says it was ready "when last checked", with an age the clock moves on. Nothing
//! here re-reads for it: the old list did not either (it re-read on the helm's change notices, which ageing does not
//! send, and qualified readiness as last checked for that reason), and the helm decides freshness again whenever a
//! launch uses the snapshot. Decided by the migration's settings-and-hosts area on 2026-11-15, porting the old app's
//! behavior as the migration's orchestrator asked.

pub(super) mod draft;
mod editor;
mod places;
pub(super) mod prepared;

pub(in crate::settings) use editor::Host;
pub(crate) use editor::{Departed, Departure, Editor};

use std::collections::HashSet;

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::Place;
use farhelm_proto::recipes::{BuildStep, BuildView, PrepareView, RecipeBase, RecipeView};
use farhelm_proto::ui_channel::{Availability, Collection};
use gpui::{
    AnyElement, AnyWindowHandle, App, AppContext as _, Context, Div, ElementId, Entity, EntityId,
    FocusHandle, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};

use super::choices;
use super::tensorlake_key::TensorlakeAccount;
use crate::components::{Button, HoverHelp, Tier, pulse_opacity};
use crate::environment_ui::ProviderUi;
use crate::focus::{self, Mark};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, LayerId};
use crate::local_time::{self, Zone};
use crate::operations::{Command, OpKey, Work};
use crate::session;
use crate::session_list::age;
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
/// tunnels, so choosing a recipe shows what its next build will contain. The old `recipe_summary`'s words, except that
/// the managed base is "Managed Ubuntu", as the editor's card and SPEC.md name it: the old "Tensorlake Ubuntu" dated
/// from before the base ran on Podman too, and read as Tensorlake-only on a recipe used for containers.
pub(crate) fn summary(view: &RecipeView) -> String {
    let plural = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    let mut parts = vec![
        format!("{} CPU", view.size.cpus),
        format!("{} GB", view.size.memory_gb),
        format!("{} GB", view.size.disk_gb),
    ];
    parts.push(match &view.base {
        RecipeBase::ManagedUbuntu => "base: Managed Ubuntu".into(),
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
    /// A prepared snapshot the helm found fresh at its last read (`ok`).
    Positive,
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
            Tone::Positive => palette.ok,
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

/// Whether the helm reports a preparation of `view`'s snapshot running: the test the pulse asks, the same state that
/// makes the prepared line the running one ([`prepared::prepared_status`]).
fn preparing(view: &RecipeView) -> bool {
    matches!(view.preparation, PrepareView::Preparing { .. })
}

/// One status line of a row, `line` drawn as `sentence` in its tone's ink at the rows' 12 pixels, with the running
/// pulse's dot at opacity `pulse` before its words when it is a running build's or preparation's line.
fn status_line(line: &Line, sentence: HostSentence, pulse: f32, cx: &App) -> Div {
    let palette = theme::palette(cx);
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
}

/// Start `line` with the name of the place its words report ([`SNAPSHOT_PLACE`], "Tensorlake · No snapshot yet"), as
/// every container place's line starts with its host's name. Applied as the row draws, to the first line of the
/// built-snapshot status and to the prepared line, each the start of its own status region; the lines' own rules
/// ([`snapshot_status`], [`prepared::prepared_status`]) stay the old ones, and their tests read them without it.
fn at_snapshot_place(line: &mut Line) {
    if let Some(place) = ProviderUi(SNAPSHOT_PLACE.provider()).account_place() {
        line.words.insert(0, Words::App(format!("{place} · ")));
    }
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
/// no build, since the helm would refuse it. When the snapshot was built is written to the minute in `zone`, the
/// viewer's local time, as the old row wrote it in UTC.
pub(super) fn snapshot_status(view: &RecipeView, zone: Zone) -> SnapshotStatus {
    let mut lines = Vec::new();
    if let Some(snapshot) = &view.snapshot {
        let mut built = vec![Words::App(format!(
            "Snapshot built {}",
            local_time::minute(snapshot.built_at, zone)
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

/// Forget the last refused build or rebuild of every recipe one of whose sources the pushed state newly shows building:
/// its snapshot in the catalog, or one of its places that has a line ([`places::building_places`]). `seen` holds the
/// sources shown building last time, which this replaces; a source is the recipe and the place's host id, or `None`
/// for the snapshot. The build the refusal was about has been asked for since, here or elsewhere, so its words would
/// only come back stale once that build ends (the old row's rule: a refusal stays "until the next click or a re-read
/// shows the build running"). A recipe shown idle keeps its refusal, and so does one whose sources were shown building
/// all along, except at an opening's first settling, which counts every source shown building as started.
///
/// It settles on a source starting to build, not on its being shown building, because it runs on every push of
/// anything, and a build announces each of its steps. The key is the recipe's, shared by its snapshot build and every
/// place's Rebuild, so it cannot tell places apart: a Rebuild refused at one place while another place of the recipe
/// already builds lands after the push that shows that build, and settling on the build being shown would drop the
/// refusal at the next unrelated push, within a second, before anyone could read it. Each source counts on its own, so
/// the snapshot or another place starting while one place builds still settles; and a source that starts settles a
/// refused Rebuild at another place, or a refused Build snapshot. That is accepted: the row shows one refusal per
/// recipe, as the old row did, and the user's next press of any of them clears it anyway. A container place's
/// preparation counts as building too, since the pushed place does not tell it from a build. While the places are not
/// known (unreadable, say), the places last seen building are carried forward, so the push that makes them readable
/// again does not count a place that kept building as starting.
///
/// The section calls it whenever the pushed state changes, and once as it is made with nothing seen yet
/// ([`session::observe_current`]), so reopening Settings drops a refusal whose recipe started building while the
/// dialog was closed. This is not the refusal's own completion, which is where `forget` is usually called. It is safe
/// because the helm pushes the changes a command causes before that command's reply: a press whose request is still
/// out has nothing on its key to drop (`forget` leaves work in flight alone), even when the push showing its own build
/// arrives first, and a refusal it settles with lands after that push and stays until the recipe next starts to build.
/// A reset retires the old store with its records, and the observation moves to the new store.
fn settle_refusals(seen: &mut HashSet<Source>, cx: &mut App) {
    let (Some(catalog), Some(operations)) = (catalog(cx), session::current_operations(cx)) else {
        return;
    };
    let mut building: HashSet<Source> = catalog
        .rows
        .iter()
        .filter(|recipe| building(recipe))
        .map(|recipe| (recipe.name.clone(), None))
        .collect();
    match places::building_places(cx) {
        Some(places) => building.extend(
            places
                .into_iter()
                .map(|(recipe, host)| (recipe, Some(host))),
        ),
        None => building.extend(seen.iter().filter(|(_, host)| host.is_some()).cloned()),
    }
    let started: HashSet<OpKey> = building
        .difference(seen)
        .map(|(name, _)| OpKey::recipe(name))
        .collect();
    *seen = building;
    operations.update(cx, |operations, cx| {
        for key in &started {
            operations.forget(key, cx);
        }
    });
}

// ---- The section ---------------------------------------------------------------------------------------------------

/// What can build for a recipe, as [`settle_refusals`] tells starts apart: the recipe's name, and the host id of its
/// container place, or `None` for its snapshot.
type Source = (String, Option<i64>);

/// The Recipes section, a view of its own that Settings makes the first time the section shows in an opening and keeps
/// until the dialog closes (see the module docs).
pub(super) struct Recipes {
    /// The opening's Tensorlake account, shared with Credentials and Tensorlake, which says whether a key is saved
    /// ([`Recipes::api_key_set`]).
    account: Entity<TensorlakeAccount>,
    /// The running pulse, held while the section is shown, the window is active and a row shows a build or a
    /// preparation running; let go while another section shows ([`Recipes::hidden`]).
    pulse: Option<ticker::Pulsing>,
    /// The ticker's 30-second clock, held while the section is shown and a row's prepared line shows an age
    /// ([`prepared::shows_age`]); let go while another section shows.
    clock: Option<ticker::Clock>,
    /// The sources the pushed state showed building when refusals were last settled, so only a source that starts to
    /// build settles its recipe's refusal ([`settle_refusals`]). A change-detection memo, never drawn; empty until the
    /// first settling.
    building: HashSet<Source>,
    /// The subscription that settles refusals whenever the pushed state changes ([`settle_refusals`]). Drawing needs
    /// none: the rows read the store while they draw, so GPUI redraws them on every push.
    _pushed: session::ObservingCurrent,
    /// The open editor, drawn in place of the list.
    editor: Option<Entity<Editor>>,
    /// New recipe's focus handle, where focus goes when an editor closes.
    new_recipe: FocusHandle,
    /// The window the section is drawn in, for moving focus when a reply closes the editor: a reply has no window at
    /// hand.
    window: AnyWindowHandle,
    /// The hosting dialog's layer, which an editor's departure out of the dialog closes ([`Recipes::depart`]).
    layer: Option<LayerId>,
}

impl Recipes {
    /// The section, inside the dialog on `layer`, reading whether a key is saved from the opening's `account`, and
    /// asking the helm to check every container host once.
    pub(super) fn new(
        layer: Option<LayerId>,
        account: Entity<TensorlakeAccount>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let recipes = Recipes {
            account,
            pulse: None,
            clock: None,
            building: HashSet::new(),
            _pushed: session::observe_current(cx, |recipes: &mut Recipes, cx| {
                settle_refusals(&mut recipes.building, cx)
            }),
            editor: None,
            new_recipe: cx.focus_handle().tab_stop(true),
            window: window.window_handle(),
            layer,
        };
        // Once per opening, as the old section did when Settings opened, check every host that can hold containers;
        // the outcomes reach the places through the push (the places module docs). A refusal changes nothing shown
        // here: the place keeps the reason of its last check, which says what is wrong.
        let hosts = super::containers::container_host_ids(cx);
        super::check_container_hosts(hosts, cx, |host, reason, _| {
            tracing::debug!(host, ?reason, "a Podman check was not asked");
        });
        recipes
    }

    /// Whether the helm has a Tensorlake API key saved, as the opening's account says: `None` until its read answers,
    /// and `Some(false)` while the read has failed (the module docs).
    fn api_key_set(&self, cx: &App) -> Option<bool> {
        let account = self.account.read(cx);
        account.key_set().or(account.unreadable().map(|_| false))
    }

    /// Another section is shown in its place: let the running pulse and the clock go. The dialog keeps this view,
    /// undrawn, until it closes, and only drawing reconsiders them, so without this a row pushed as building would keep
    /// the ticker waking ten times a second for a section nobody sees. Showing the section again draws it, which takes
    /// them again if a row still needs them.
    pub(super) fn hidden(&mut self) {
        self.pulse = None;
        self.clock = None;
    }

    /// The open editor, if any: what Settings asks to pin its answers ([`Editor::actions`]) while Recipes is shown.
    pub(super) fn editor(&self) -> Option<Entity<Editor>> {
        self.editor.clone()
    }

    /// Open an editor in place of the list, for `existing` or a new recipe, with focus on its Back once it is drawn,
    /// unless the user moves on before. New recipe and a row's Edit call it, and so does the editor's golden scene.
    pub(super) fn open_editor(
        &mut self,
        existing: Option<&RecipeView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mark = Mark::take(cx);
        let parent = cx.weak_entity();
        let editor = cx.new(|cx| Editor::new(Host::Recipes(parent), existing, window, cx));
        let target = editor.read(cx).start(cx);
        self.editor = Some(editor);
        focus::focus_in_layer(&target, &mark, window, cx);
        cx.notify();
    }

    /// The editor `editor` (by its entity id) is leaving for `departure`: back to the list ([`Recipes::close_editor`]),
    /// or out of the dialog, which closes it. Nothing if `editor` is no longer the open editor, so a late reply never
    /// moves an editor opened since. `window` is as [`Recipes::close_editor`] takes it.
    pub(super) fn depart(
        &mut self,
        editor: EntityId,
        departure: Departure,
        mark: &Mark,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        match departure {
            Departure::Back => self.close_editor(editor, mark, window, cx),
            Departure::Close => {
                if self.editor.as_ref().map(Entity::entity_id) != Some(editor) {
                    return;
                }
                self.editor = None;
                let Some(layer) = self.layer else {
                    return;
                };
                // Closed outright, not dismissed: the editor has answered for itself, and the dismissal hook would ask
                // it again.
                match window {
                    Some(window) => layers::close(layer, window, cx),
                    None => {
                        let _ = self
                            .window
                            .update(cx, |_, window, cx| layers::close(layer, window, cx));
                    }
                }
            }
        }
    }

    /// The editor `editor` (by its entity id) is done: Back or Cancel with nothing unsaved, Discard, or a done save or
    /// delete. If `editor` is still the open editor it closes, with focus on New recipe unless the user moved on since
    /// `mark`, the press that closed it or started the work. Nothing is read: a done save's or delete's change reached
    /// the store, pushed, before its answer. `window` is the window when the press itself closes the editor; a reply has
    /// none at hand, and then the window the section is drawn in is updated for the move.
    fn close_editor(
        &mut self,
        editor: EntityId,
        mark: &Mark,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.editor.as_ref().map(Entity::entity_id) != Some(editor) {
            return;
        }
        self.editor = None;
        // New recipe is drawn again by the next frame; the coordinator keeps the move until then.
        let target = self.new_recipe.clone();
        match window {
            Some(window) => focus::focus_in_layer(&target, mark, window, cx),
            None => {
                let mark = *mark;
                let _ = self.window.update(cx, |_, window, cx| {
                    focus::focus_in_layer(&target, &mark, window, cx)
                });
            }
        }
        cx.notify();
    }

    /// Whether the section holds the running pulse and the ticker's clock now, for tests of when it lets them go: a
    /// held 30-second clock fires too rarely for a test to tell it from none by its ticks.
    #[cfg(test)]
    pub(super) fn holds_time(&self) -> (bool, bool) {
        (self.pulse.is_some(), self.clock.is_some())
    }

    /// Build, Rebuild or Retry for the recipe `name`: start or join its snapshot build under the recipe's key. Does
    /// nothing unless an API key is known to be saved, or while the recipe already has work in flight, when the button
    /// is drawn unavailable or busy. An accepted build needs nothing more here: the helm has pushed the recipe building
    /// by the time it replies. A refusal is the store's to keep on the key, where the row shows it.
    fn build(&mut self, name: &str, cx: &mut Context<Self>) {
        if self.api_key_set(cx) != Some(true) {
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

    /// One recipe's row (`.recipe-row`): its name, summary, per-place lines from the pushed `places`, snapshot status,
    /// prepared-snapshot line, the build it offers, and the helm's words for its last refused build or rebuild.
    /// `pulse` is the running pulse's opacity now, `now` the viewer's time in seconds since the epoch, for the prepared
    /// line's ages, and `zone` the viewer's, for the times it gives.
    fn row(
        &self,
        view: &RecipeView,
        places: &places::Places,
        pulse: f32,
        now: Option<i64>,
        zone: Zone,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let palette = theme::palette(cx);
        let name = view.name.clone();
        let mut status = snapshot_status(view, zone);
        if let Some(first) = status.lines.first_mut() {
            at_snapshot_place(first);
        }
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
        let lines = status
            .lines
            .iter()
            .zip(sentences)
            .map(|(line, sentence)| status_line(line, sentence, pulse, cx));
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
        // `.recipe-row-prepared`: preparation is a status region of its own, apart from the build, as in the old row.
        let mut prepared = prepared::prepared_status(view, now, zone);
        at_snapshot_place(&mut prepared);
        let prepared_sentence = prepared.sentence(format!("recipe-{name}-prepared-line"));
        let prepared = div()
            .id(ElementId::Name(format!("recipe-{name}-prepared").into()))
            .role(Role::Status)
            .aria_label(SharedString::from(format!(
                "Preparation of {name}: {}",
                prepared_sentence.accessible_name()
            )))
            .min_w_0()
            .child(status_line(&prepared, prepared_sentence, pulse, cx));
        let key_set = self.api_key_set(cx);
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
                    .disabled(key_set != Some(true))
                    .busy(in_flight)
                    .on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |section, cx| section.build(&building, cx));
                    }),
                )
                .children((key_set == Some(false)).then(|| {
                    div()
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(palette.fg_2))
                        .child(NO_KEY)
                }))
        });
        // The refusal of the last build or rebuild of this recipe, until its next one starts or the pushed state shows
        // it start building ([`settle_refusals`]); also a refusal of its save or delete that the editor did not show
        // itself, such as a delete whose cleanup carries on past the helm's bound (the editor's `leaving` module).
        let refused = refusal.map(|reason| {
            choices::failure_outcome(format!("recipe-{name}-build-error"), &reason, cx)
        });
        let edit = {
            let weak = cx.weak_entity();
            let editing = view.clone();
            Button::new(
                format!("recipe-edit-{name}"),
                "Edit",
                HoverHelp::tooltip("Edit this recipe"),
                Role::Button,
            )
            .accessible_name(format!("Edit {name}"))
            .tier(Tier::Neutral)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |section, cx| {
                    section.open_editor(Some(&editing), window, cx)
                });
            })
        };
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
                    .children(self.place_lines(&name, places, in_flight, zone, cx))
                    .child(snapshot)
                    .child(prepared)
                    .children(actions)
                    .children(refused),
            )
            .child(edit)
    }

    /// The rows, the empty line or the loading line, whichever the pushed `catalog` calls for. A catalog the helm could
    /// not read with no rows to keep draws none of them: its reason, drawn below, is the whole story. `zone` is the
    /// viewer's, read once for the draw.
    fn listing(
        &self,
        catalog: Option<&Catalog>,
        places: &places::Places,
        pulse: f32,
        now: Option<i64>,
        zone: Zone,
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
                    .children(
                        catalog
                            .rows
                            .iter()
                            .map(|view| self.row(view, places, pulse, now, zone, cx)),
                    )
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
        let account = self.account.clone();
        let retry = choices::retry_loading(
            "recipes-retry",
            "Retry loading",
            "Read the recipes again",
            move |cx| account.update(cx, |account, cx| account.read_again(cx)),
        );
        choices::failure_with_retry(line, retry).into()
    }
}

impl Render for Recipes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        // Read from the store while drawing, so every push that changes the catalog or the places redraws the rows.
        let catalog = catalog(cx);
        let places = places::places(cx);
        // Hold the pulse only while a row shows a running build or preparation and someone can see the window: the
        // ticker's rule, which keeps the tick loop from firing for a window nobody looks at. Hold the clock while a
        // row shows an age, whatever the window's activity, as every age's view does. Drawing means the section is
        // shown; the dialog lets both go when it is not ([`Recipes::hidden`]).
        // The open editor replaces the rows, so then no row is shown to pulse or age.
        let shown = if self.editor.is_none() {
            catalog
                .as_ref()
                .map_or(&[][..], |catalog| &catalog.rows[..])
        } else {
            &[]
        };
        let running = shown.iter().any(|view| building(view) || preparing(view));
        let pulses = running && ticker::is_active(window, cx);
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let ages = shown.iter().any(prepared::shows_age);
        if ages != self.clock.is_some() {
            self.clock = ages.then(|| ticker::clock(cx));
        }
        let now = age::seconds(ticker::wall_now(cx));
        let zone = local_time::zone(cx);
        let pulse = pulse_opacity(
            self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx)),
            palette.status_pulse_min,
        );
        let section = div()
            .id("recipes")
            .role(Role::Region)
            .aria_label("Recipes")
            .flex()
            .flex_col()
            .gap(px(PART_GAP))
            .w_full()
            .min_w_0();
        if let Some(editor) = &self.editor {
            return section.child(editor.clone());
        }
        let weak = cx.weak_entity();
        let new_recipe = Button::new(
            "recipe-new",
            "New recipe",
            HoverHelp::tooltip("Write a new recipe"),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .focus(&self.new_recipe)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |section, cx| section.open_editor(None, window, cx));
        });
        section
            .child(choices::help(HELP, cx))
            .children(self.listing(catalog.as_ref(), &places, pulse, now, zone, cx))
            .children(self.read_failure(catalog.as_ref(), cx))
            // `.recipe-new`: below the list and any failure, as wide as its words.
            .child(div().flex().child(new_recipe))
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
