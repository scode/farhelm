//! A recipe row's per-place lines: how the recipe stands at each place other than the Tensorlake account (each container
//! host, today), with a Rebuild per place.
//!
//! SPEC.md, "Topology": "The list shows each recipe's name, supported providers, base, package count, tunnel count and
//! build status per place, with Rebuild per place"; SPEC_impl.md, "Shared recipes and prepared artifacts" ("Settings →
//! Recipes owns the shared catalog/editor and per-place readiness and Rebuild"; "Rebuild is a required provider
//! operation that returns after admission or joining"). Ported from the old app's `RecipePlaces`
//! (`containers_settings.rs`) and its reads in `api/environments.rs`, at reference commit `24bc0580`. Here's the TLDR:
//!
//! - **The lines are the pushed places.** The helm pushes the `recipe_places` collection, the entries
//!   `GET /api/recipe-places` serves. The helm uses local records and cached checks to hide unready places with no
//!   recipe history or work when the base can run on more than one provider. Its read starts no check and the helm re-reads
//!   it after every change it announces and after each command. Every row draws its lines from the store, so a place's
//!   build moves on screen while the section stays shown. Before the push (decided by the migration's orchestrator on
//!   2026-11-15) the section read the places whenever it was shown, after an accepted Rebuild, and when a pushed host's
//!   finished Podman check changed; nothing reads them now. A collection the store has not installed makes every row
//!   say "Reading per-place builds…", and one the helm could not read puts its reason in every row, as a failed read
//!   did in the old rows. Unlike the catalog, whose rows stay when the helm cannot read it, an unreadable places
//!   collection drops every line for its reason, though the store still holds the last entries: that is deliberate, as
//!   the old rows showed only the failed read's reason, and it offers no Retry loading of its own. Only places whose
//!   provider has per-place words get a line ([`crate::environment_ui::ProviderUi::recipe_place`]): the Tensorlake
//!   account's place is reported by the row's built-snapshot and prepared-snapshot lines instead.
//! - **A place's words follow its host's check through the push.** A place's reason comes from its host's Podman
//!   check. The helm announces a check's start and its end (`PodmanPlaces::announce` in the helm's
//!   `environment/podman_probe.rs`), and every announced change re-runs every pushed read, the places' included, whose
//!   reasons come from that same check cache. So a finished check reaches the lines with the next push, and the section
//!   watches no hosts for it, as it did while the places were read.
//! - **A place's name is the host's.** The old rows took it from `GET /api/environment-places`, whose read also asked
//!   the helm to check every container host that was never checked. Here it comes from the pushed hosts collection,
//!   by the place's host id, through the one rule every host name follows ([`crate::hosts::identity::display_name`]),
//!   so naming a place starts no check, and a place reads as its host does everywhere else in the app (the local host
//!   as "Local (this machine)", where the old row said "this machine"). A place whose host is no longer registered
//!   says "Unavailable place", as the old row did for a place it could not name.
//! - **Checks, once per opening.** As the old section did on opening, the section asks the helm, once per opening of
//!   Settings, to check every host that can hold containers (`POST /api/hosts/<id>/podman-probe`, which the helm
//!   answers at once and runs in the background), so a place does not say "this host needs a Podman check" for good.
//!   The checks' outcomes reach the lines through the push (above).
//! - **Rebuild is accepted work** on the recipe's own key, like the row's snapshot build ([`super`], "Builds are
//!   accepted work"): `POST /api/recipes/<name>/rebuild` with the place, answered 202 once the provider admitted or
//!   joined the work, by which time the helm has pushed the place building. Every Rebuild of the recipe is busy while
//!   one is out. A place that is building, unavailable or incompatible with the recipe's base disables its button.
//!   The reason appears in the hover help even when the line has a stored build failure to show. A refusal shows under the
//!   row, where the key's refusal shows. The words of each state are the place's provider's
//!   ([`crate::environment_ui::ProviderUi::recipe_place`]).

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::RecipePlaceView;
use farhelm_proto::recipes::{RecipeBase, RecipeView};
use farhelm_proto::ui_channel::{Availability, Collection, HostView};
use gpui::{
    App, Context, Div, ElementId, InteractiveElement as _, ParentElement as _, Role, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::{Recipes, choices};
use crate::components::{Button, HoverHelp, Tier};
use crate::environment_ui::ProviderUi;
use crate::host_text::{HostSentence, HostText};
use crate::local_time::{self, Zone};
use crate::operations::{Command, OpKey, Work};
use crate::session;
use crate::theme::{self, metrics};

/// `.recipe-row-status`'s gap between a place's words and its Rebuild.
const PLACE_GAP: f32 = 6.;

/// `POST /api/recipes/<name>/rebuild` with `place` in its body: rebuild the recipe at that place, or join the work
/// running there. Named in a wordless refusal by its route, the recipe's name being the user's own.
fn rebuild_command(name: &str, place: farhelm_proto::Place) -> Result<Command, PathError> {
    let path = ApiPath::api(&["recipes", name, "rebuild"])?;
    let shown = format!("POST {}", path.as_str());
    // Turning a place into JSON cannot fail (a tagged enum of an integer); if it ever did, the request would go without
    // a body and the helm would refuse it with its own words.
    let body = serde_json::to_value(place)
        .ok()
        .map(|place| serde_json::json!({ "place": place }));
    Ok(Command::new(Work::Save, Method::Post, path, body, shown))
}

/// The pushed places as the store holds them now, taken once per draw and shared by every row.
pub(super) enum Places {
    /// No snapshot installed the collection yet (or the window is inert): every row says it is reading.
    Unknown,
    /// The helm could not read the places: every row shows its reason in place of its lines, as a failed read did in
    /// the old rows.
    Unreadable(HostText),
    /// Every recipe's places, in the helm's order.
    Listed(Vec<RecipePlaceView>),
}

/// The pushed places now ([`Places`]).
pub(super) fn places(cx: &App) -> Places {
    let Some(store) = session::current_store(cx) else {
        return Places::Unknown;
    };
    let collections = store.read(cx).collections();
    match collections
        .meta(Collection::RecipePlaces)
        .map(|meta| &meta.availability)
    {
        None => Places::Unknown,
        Some(Availability::Unavailable { error }) => {
            Places::Unreadable(HostText::from_host(error.as_str()))
        }
        Some(Availability::Available) => Places::Listed(
            collections
                .ordered::<RecipePlaceView>()
                .into_iter()
                .cloned()
                .collect(),
        ),
    }
}

/// The places with a line that the pushed places show building, each as its recipe and its host's id, which settle the
/// recipe's last refusal when one starts, as its snapshot starting to build does ([`super::settle_refusals`]). A place's
/// preparation counts, since the pushed place does not tell it from a build. Only places that get a line count: the
/// Tensorlake account's `building` is also true while a preparation runs, which a launch elsewhere starts and which says
/// nothing about a refused Build snapshot. `None` while the places are not known (not pushed yet, or unreadable): which
/// of them builds is unknown then, not none of them.
pub(super) fn building_places(cx: &App) -> Option<Vec<(String, i64)>> {
    match places(cx) {
        Places::Listed(places) => Some(
            places
                .into_iter()
                .filter(|place| place.building && has_line(place))
                .filter_map(|place| Some((place.recipe, place.place.host()?)))
                .collect(),
        ),
        Places::Unknown | Places::Unreadable(_) => None,
    }
}

impl Recipes {
    /// Rebuild `name` at `place` under the recipe's key. Does nothing while the recipe already has work in flight, when
    /// every Rebuild of it is drawn busy. An accepted rebuild needs nothing more here: the helm has pushed the place
    /// building by the time it replies. A refusal is the store's to keep on the key, where the row shows it.
    fn rebuild(&mut self, name: &str, place: farhelm_proto::Place, cx: &mut Context<Self>) {
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let command = match rebuild_command(name, place) {
            Ok(command) => command,
            Err(error) => {
                tracing::debug!(?error, "recipe rebuild not built");
                return;
            }
        };
        let started = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::recipe(name), command, cx, |_, _| {})
        });
        if let Err(not_started) = started {
            tracing::debug!(name, ?not_started, "recipe rebuild not started");
        }
        cx.notify();
    }

    /// The per-place part of `recipe`'s row from the pushed `places`: "Reading per-place builds…" until the store has
    /// them, the helm's reason while it cannot read them, or a line per place whose provider has per-place words, in
    /// the helm's order. `in_flight` says whether the recipe's key has work out, which makes every Rebuild of it busy;
    /// `zone` is the viewer's, for the times the lines give.
    pub(super) fn place_lines(
        &self,
        recipe: &RecipeView,
        places: &Places,
        in_flight: bool,
        zone: Zone,
        cx: &Context<Self>,
    ) -> Vec<Stateful<Div>> {
        let palette = theme::palette(cx);
        let name = recipe.name.as_str();
        match places {
            Places::Unknown => vec![
                choices::outcome_line(
                    format!("recipe-{name}-places-reading"),
                    Role::Status,
                    theme::color(palette.fg_1),
                )
                .aria_label("Reading per-place builds…")
                .child("Reading per-place builds…"),
            ],
            Places::Unreadable(reason) => vec![choices::failure_outcome(
                format!("recipe-{name}-places-error"),
                reason,
                cx,
            )],
            Places::Listed(places) => places
                .iter()
                .filter(|place| place.recipe == name)
                .filter_map(|place| self.place_line(place, &recipe.base, in_flight, zone, cx))
                .collect(),
        }
    }

    /// One place's line (the old `.recipe-row-status`): the place's name, its state in the provider's words or the
    /// helm's reason, the running step, when it was prepared or built (the viewer's local time), pending cleanup, and
    /// its Rebuild. `None` for a place whose provider has no per-place words, or that names no host to call it by and to
    /// tell it apart (no provider with per-place words has such a place).
    fn place_line(
        &self,
        place: &RecipePlaceView,
        base: &RecipeBase,
        in_flight: bool,
        zone: Zone,
        cx: &Context<Self>,
    ) -> Option<Stateful<Div>> {
        let palette = theme::palette(cx);
        let kind = place.place.provider();
        let provider = ProviderUi(kind);
        let words = provider.recipe_place()?;
        // The host id names the line's elements: unique among a recipe's places, and stable across reads.
        let at = place.place.host()?;
        let id = format!("recipe-{}-place-{at}", place.recipe);
        let name = place_name(at, cx);
        let mut sentence = HostSentence::new(ElementId::Name(id.clone().into()))
            .host(&name)
            .text(" · ");
        sentence = match place.reason.as_ref().or(place.unavailable_reason.as_ref()) {
            // The helm's reason (a host's Podman check, a failed build), which may quote a host.
            Some(reason) => sentence.host(&HostText::from_host(reason.clone())),
            None if place.building => sentence.text(words.building),
            None if place.ready => sentence.text(words.ready),
            None => sentence.text(words.not_built),
        };
        if let Some(step) = &place.step {
            sentence = sentence
                .text(" · ")
                .host(&HostText::from_host(step.clone()));
        }
        if let Some(at) = place.prepared_at {
            sentence = sentence.text(format!(" · Prepared {}", local_time::stamp(at, zone)));
        } else if let Some(at) = place.built_at {
            sentence = sentence.text(format!(" · Built {}", local_time::stamp(at, zone)));
        }
        if place.cleanup_pending {
            sentence = sentence.text(format!(" · {}", words.cleanup_pending));
        }
        let weak = cx.weak_entity();
        let (recipe, target) = (place.recipe.clone(), place.place);
        let rebuild_name = HostSentence::new(ElementId::Name(format!("{id}-rebuild-name").into()))
            .text(format!("Rebuild {} at ", place.recipe))
            .host(&name)
            .accessible_name();
        // A stored place can outlive the base that built there. It still
        // explains the old build, while Rebuild must use the current base.
        let base_supported = base.supports(kind);
        let rebuild_help = place.unavailable_reason.as_ref().map_or_else(
            || {
                HoverHelp::tooltip(if base_supported {
                    words.rebuild_help
                } else {
                    provider.recipe_base_unsupported()
                })
            },
            |reason| {
                HoverHelp::sentence(
                    HostSentence::new(ElementId::Name(format!("{id}-rebuild-help").into()))
                        .host(&HostText::from_host(reason.clone())),
                )
            },
        );
        let rebuild = Button::new(
            format!("recipe-rebuild-{}-{at}", place.recipe),
            "Rebuild",
            rebuild_help.beside_list(false),
            Role::Button,
        )
        .accessible_name(rebuild_name)
        .tier(Tier::Neutral)
        .disabled(place.building || place.unavailable_reason.is_some() || !base_supported)
        .busy(in_flight)
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |section, cx| section.rebuild(&recipe, target, cx));
        });
        let line = div()
            .id(ElementId::Name(id.into()))
            .role(Role::Status)
            .aria_label(sentence.accessible_name())
            .flex()
            .items_center()
            .gap(px(PLACE_GAP))
            .min_w_0()
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_1))
            .child(div().flex_1().min_w_0().child(sentence))
            .child(div().flex_none().child(rebuild));
        Some(line)
    }
}

/// Whether `place` gets a line in its recipe's row: its provider has per-place words
/// ([`crate::environment_ui::ProviderUi::recipe_place`]).
fn has_line(place: &RecipePlaceView) -> bool {
    ProviderUi(place.place.provider()).recipe_place().is_some()
}

/// The name of the place's host `host` as the pushed hosts collection has it, or "Unavailable place" for a host that
/// is not registered any more. Host text, as every host name is.
fn place_name(host: i64, cx: &Context<Recipes>) -> HostText {
    session::current_store(cx)
        .and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<HostView>(&HostView::key(host))
                .map(crate::hosts::identity::display_name)
        })
        .unwrap_or_else(|| HostText::from_host("Unavailable place"))
}
