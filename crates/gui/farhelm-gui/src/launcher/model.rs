//! The model field's rules, with no GPUI types: which rows its list offers for what has been typed, and what Enter in
//! the field means.
//!
//! Ported from the old app's `launch_composer.rs` at reference commit `24bc0580` (`ModelOption`, `model_options`,
//! `ModelEnterTarget`, `model_enter_target`, `owned_elsewhere_message`), with its tests. The model catalog is the
//! helm's (`GET /api/launch-catalog`); what a chosen model then does to the draft is the draft's
//! ([`super::draft::LaunchDraft::choose_catalog_model`], [`super::draft::LaunchDraft::choose_custom_model`]).
//!
//! Two rules matter most, both SPEC.md's ("Creation"): typing a model never changes a chosen agent type (a typed id
//! that only another agent offers is said, not applied), while picking another agent's model from the full list
//! switches the agent on purpose; and Enter applies only a row the user moved to with the arrows, or what they typed,
//! never the first row by default, which would quietly replace a valid choice.

use farhelm_proto::LaunchHarness;
use farhelm_proto::launcher::LaunchCatalogModel;

use super::draft::harness_label;
use super::words::HARNESS_SEARCH_ORDER;

/// What Enter on a typed id says when no agent type is chosen to own it (the old launcher's words).
pub(crate) const NEEDS_HARNESS: &str = "Choose a harness before a custom model id";

/// One row of the model list, in its keyboard order. Rows of other agent types carry a " (Harness)" suffix rather
/// than sitting under headings, so this flat order is what the arrows walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModelOption {
    /// The agent type's own configured model.
    HarnessDefault,
    /// A catalog model, with the agent type that offers it.
    Model { id: String, harness: LaunchHarness },
    /// Switch between the chosen agent type's models and every agent type's. Offered only while an agent type is
    /// chosen: with none, every model is already listed and the toggle would change nothing.
    ShowAll,
}

/// What Enter in the model field applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModelEnterTarget {
    /// Nothing: an empty field with no row moved to, or a stale row.
    Nothing,
    /// The row moved to, or the catalog row the typed id names.
    Option(ModelOption),
    /// A custom id, exactly as typed, under the chosen agent type.
    Custom { id: String, harness: LaunchHarness },
    /// A typed id no agent type can own yet: [`NEEDS_HARNESS`].
    NeedsHarness(String),
    /// A typed id only other agent types offer, while one is chosen: said ([`owned_elsewhere_message`]), not applied.
    OwnedElsewhere {
        id: String,
        owners: Vec<LaunchHarness>,
    },
}

/// The list's rows for `query` (case-insensitive, matched within model ids): the default first, then the chosen agent
/// type's catalog models (every agent type's with `show_all`, or with none chosen) in the search order and the
/// catalog's own order within each, then the show-all toggle while an agent type is chosen. Nothing for an agent type
/// that offers no model choice, whose field is not drawn.
pub(crate) fn model_options(
    catalog: &[LaunchCatalogModel],
    harness: Option<LaunchHarness>,
    query: &str,
    show_all: bool,
) -> Vec<ModelOption> {
    if harness.is_some_and(|harness| !harness.offers_model()) {
        return Vec::new();
    }
    let folded = query.to_ascii_lowercase();
    let mut options = vec![ModelOption::HarnessDefault];
    for owner in HARNESS_SEARCH_ORDER
        .into_iter()
        .filter(|owner| owner.offers_model())
    {
        if !show_all && harness.is_some_and(|chosen| chosen != owner) {
            continue;
        }
        options.extend(
            catalog
                .iter()
                .filter(|model| {
                    model.latest
                        && model.harness == owner
                        && model.id.to_ascii_lowercase().contains(&folded)
                })
                .map(|model| ModelOption::Model {
                    id: model.id.clone(),
                    harness: model.harness,
                }),
        );
    }
    if harness.is_some() {
        options.push(ModelOption::ShowAll);
    }
    options
}

/// What Enter means, given the rows, the row moved to (`active`), the typed `draft` and the chosen agent type.
///
/// A row moved to wins over the text. Otherwise an empty draft does nothing. With an agent type chosen, the draft is
/// looked up in that type's own catalog spelling ([`LaunchHarness::catalog_model_id`], so a bare OpenCode name is
/// OpenCode's model): an exact match is that catalog row, canonical spelling included; the type's other spelling of a
/// known model keeps the typed text; an id only other types currently suggest is [`ModelEnterTarget::OwnedElsewhere`]; anything
/// else is a custom id, kept byte for byte. With none chosen, a known id names its one owner, and anything else needs
/// an agent type first.
pub(crate) fn model_enter_target(
    options: &[ModelOption],
    active: Option<usize>,
    draft: &str,
    catalog: &[LaunchCatalogModel],
    harness: Option<LaunchHarness>,
) -> ModelEnterTarget {
    // An agent type without a model choice draws no field, but a key from a frame before the switch can still arrive.
    if harness.is_some_and(|harness| !harness.offers_model()) {
        return ModelEnterTarget::Nothing;
    }
    if let Some(index) = active {
        return options
            .get(index)
            .cloned()
            .map(ModelEnterTarget::Option)
            .unwrap_or(ModelEnterTarget::Nothing);
    }
    if draft.trim().is_empty() {
        return ModelEnterTarget::Nothing;
    }
    if let Some(current) = harness {
        let catalog_id = current.catalog_model_id(draft);
        let owners: Vec<&LaunchCatalogModel> = catalog
            .iter()
            .filter(|model| model.id.eq_ignore_ascii_case(&catalog_id))
            .collect();
        return match owners.iter().find(|model| model.harness == current) {
            // The agent type's other spelling of a known model, typed exactly: the typed text is the choice, as a
            // custom id's is, and every catalog check reads it through the same spelling.
            Some(known) if !known.id.eq_ignore_ascii_case(draft) && known.id == *catalog_id => {
                ModelEnterTarget::Custom {
                    id: draft.to_owned(),
                    harness: current,
                }
            }
            // Any other match is the catalog row, its case included: the helm compares spellings exactly.
            Some(known) => ModelEnterTarget::Option(ModelOption::Model {
                id: known.id.clone(),
                harness: current,
            }),
            None if owners.iter().any(|model| model.latest) => ModelEnterTarget::OwnedElsewhere {
                id: draft.to_owned(),
                owners: owners
                    .iter()
                    .filter(|model| model.latest)
                    .map(|model| model.harness)
                    .collect(),
            },
            None => ModelEnterTarget::Custom {
                id: draft.to_owned(),
                harness: current,
            },
        };
    }
    let owners: Vec<&LaunchCatalogModel> = catalog
        .iter()
        .filter(|model| model.id.eq_ignore_ascii_case(draft))
        .collect();
    if let [owner] = owners.as_slice() {
        return ModelEnterTarget::Option(ModelOption::Model {
            id: owner.id.clone(),
            harness: owner.harness,
        });
    }
    ModelEnterTarget::NeedsHarness(draft.to_owned())
}

/// What [`ModelEnterTarget::OwnedElsewhere`] says: which agent types offer the typed model, so the user can pick it
/// there on purpose.
pub(crate) fn owned_elsewhere_message(id: &str, owners: &[LaunchHarness]) -> String {
    let labels = owners
        .iter()
        .map(|owner| harness_label(*owner))
        .collect::<Vec<_>>()
        .join(" or ");
    format!("{id} is offered by {labels}; choose {labels} to use it")
}
