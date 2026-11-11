//! Provider presentation has one exhaustive home. Shared controls operate on
//! ownership and phase; they never guess billing or resource names from a label.

use dioxus::prelude::*;
use farhelm_proto::{LaunchTarget, Place, ProviderKind};

/// Static wording and decorative geometry for a disposable-machine provider.
#[derive(Clone, Copy)]
pub(crate) struct ProviderUi(pub(crate) ProviderKind);

#[warn(clippy::wildcard_enum_match_arm)]
impl ProviderUi {
    /// Resource words describe the provider, never a supervisor implementation.
    pub(crate) fn noun(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "sandbox",
            ProviderKind::Podman => "container",
        }
    }
    /// The hidden locality word accompanies a decorative host-sized drawing.
    pub(crate) fn locality_label(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Sandbox",
            ProviderKind::Podman => "Container",
        }
    }
    /// Runs-on labels name new resource creation rather than an existing host.
    pub(crate) fn segment(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "New sandbox",
            ProviderKind::Podman => "New container",
        }
    }
    /// Launch help explains file locality and the provider's ongoing cost.
    pub(crate) fn launch_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "A new Tensorlake sandbox for this session. It bills while running. Suspend stops CPU and memory billing; snapshot storage still bills. Delete removes the whole sandbox and its files. After Farhelm disconnects, Tensorlake suspends it after 30 minutes without activity."
            }
            ProviderKind::Podman => {
                "A new container for this session, built from the recipe on its container host. The checkout and files live inside it. Suspend freezes processes while keeping memory in use; Delete removes the whole container and its files."
            }
        }
    }
    /// Stopping one agent leaves the environment and unrelated processes alive.
    pub(crate) fn stop_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "The sandbox keeps running and billing until you suspend or delete it."
            }
            ProviderKind::Podman => {
                "The container keeps running until you suspend or delete it. Suspending freezes its processes; its memory stays in use on its host."
            }
        }
    }
    /// Whole-resource consent includes work missing from the session cache.
    pub(crate) fn delete_consequence(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Delete the whole sandbox? Every session, running process, file, checkout, unpushed change and snapshot in it will be permanently removed. This cannot be undone."
            }
            ProviderKind::Podman => {
                "Delete the whole container? Every session, running process, file, checkout and unpushed change in it will be permanently removed. This cannot be undone."
            }
        }
    }
    /// Session Delete names the resource it will destroy with the session.
    pub(crate) fn delete_label(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Delete session and sandbox",
            ProviderKind::Podman => "Delete session and container",
        }
    }
    /// Host management uses the same whole-resource deletion authority.
    pub(crate) fn host_delete_label(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Delete sandbox",
            ProviderKind::Podman => "Delete container",
        }
    }
    /// The removal affordance explains what the provider will remove.
    pub(crate) fn host_delete_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Terminate this Tensorlake sandbox and delete its snapshots"
            }
            ProviderKind::Podman => {
                "Delete this container, all its sessions, running processes and files"
            }
        }
    }
    /// Suspension preserves processes while retaining provider-specific costs.
    pub(crate) fn suspend_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Freeze the sandbox; its processes continue on Resume. Only snapshot storage bills while suspended."
            }
            ProviderKind::Podman => {
                "Freeze the container; its processes continue on Resume. Its memory stays in use on the container host."
            }
        }
    }
    /// A single-provider recipe explains the artifact its build retains.
    /// Snapshot language must not imply cloud billing for a container image.
    pub(crate) fn recipe_build_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Farhelm then adds Claude Code and Codex with their official installers and takes a snapshot."
            }
            ProviderKind::Podman => {
                "Farhelm then adds Claude Code and Codex with their official installers and saves an image on the container host for launches there."
            }
        }
    }
    /// A suspended card describes the current state, rather than repeating
    /// the Suspend button's instruction. Each provider owns its cost caveat.
    pub(crate) fn suspended_explanation(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "This session's sandbox is suspended. Its processes are preserved, including the agent's running command, and continue when you resume it. Only snapshot storage bills while suspended."
            }
            ProviderKind::Podman => {
                "This session's container is suspended. Its processes are preserved, including the agent's running command, and continue when you resume it. Its memory stays in use on the container host."
            }
        }
    }
    /// Attribution comes from the recorded completion, never an inferred idle
    /// timeout. An external observation explains policy without claiming cause.
    pub(crate) fn suspended_detail(
        self,
        observed: i64,
        by_farhelm: Option<i64>,
        failed: Option<&crate::peer::PeerText>,
        session_owned: bool,
    ) -> Vec<crate::peer::DetailPart> {
        use crate::peer::DetailPart;
        let mut detail = Vec::new();
        if let Some(reason) = failed {
            // Tensorlake hosts keep the wording they had before containers
            // shared this detail: the provider's own diagnosis, named.
            detail.extend([
                DetailPart::text(match self.0 {
                    ProviderKind::Tensorlake => "Tensorlake could not resume the sandbox: ",
                    ProviderKind::Podman => "Resume failed: ",
                }),
                DetailPart::peer(reason),
                DetailPart::text(". "),
            ]);
        }
        let at = by_farhelm.unwrap_or(observed);
        let attribution = match self.0 {
            ProviderKind::Tensorlake => {
                if by_farhelm.is_some() {
                    "Suspended from Farhelm"
                } else {
                    "Suspended outside Farhelm, noticed"
                }
            }
            ProviderKind::Podman => {
                // Podman bindings do not persist Farhelm's pause completion.
                // A missing timestamp therefore establishes no outside actor.
                if by_farhelm.is_some() {
                    "Suspended from Farhelm"
                } else {
                    "Suspension noticed"
                }
            }
        };
        detail.push(DetailPart::text(format!(
            "{attribution} at {}.",
            crate::activity::utc_stamp(at / 1000)
        )));
        match self.0 {
            ProviderKind::Tensorlake => {
                if by_farhelm.is_none() {
                    detail.push(DetailPart::text(if session_owned {
                        " Tensorlake suspends this sandbox after 30 minutes without a connection from Farhelm."
                    } else {
                        " On Usage Credits, Tensorlake suspends a sandbox after 24 hours without a connection."
                    }));
                }
            }
            ProviderKind::Podman => {}
        }
        detail
    }
    /// A host row while Farhelm waits for its suspend or resume request to be
    /// confirmed. Tensorlake hosts keep the wording they had before
    /// containers shared host rows; a container's request goes to Podman on
    /// its container host.
    pub(crate) fn lifecycle_request_detail(self, verb: &str, requested_at: i64) -> String {
        let asked = match self.0 {
            ProviderKind::Tensorlake => "Tensorlake",
            ProviderKind::Podman => "the container host",
        };
        format!(
            "Asked {asked} to {verb} at {}; waiting for confirmation",
            crate::activity::utc_stamp(requested_at / 1000)
        )
    }
    /// Confirmation preserves the distinction between ending the selected agent
    /// and freezing every other process in the same environment.
    pub(crate) fn suspend_question(self, stop: bool) -> String {
        let noun = self.noun();
        if stop {
            format!(
                "Stop this session's agent and close its terminal tabs, then suspend the whole {noun}? Every other process freezes and continues on Resume. The stopped agent and closed tabs do not continue. "
            )
        } else {
            format!(
                "Suspend the whole {noun}? Every process in it freezes, including any other sessions, and continues on Resume. "
            )
        }
    }
    /// Every suspension question identifies the actual resource being frozen.
    pub(crate) fn suspend_title(self, stop: bool) -> String {
        format!(
            "{} {}?",
            if stop { "Stop and suspend" } else { "Suspend" },
            self.noun()
        )
    }
    /// Cancelling suspension keeps both selected and unrelated programs running.
    pub(crate) fn suspend_cancel(self, stop: bool) -> String {
        if stop {
            format!(
                "Cancel: keep the agent, its terminal tabs and the {} running",
                self.noun()
            )
        } else {
            format!("Cancel: keep the {} running", self.noun())
        }
    }
    /// The action's scope includes other sessions, even when their cache is stale.
    pub(crate) fn suspend_action(self, stop: bool) -> String {
        if stop {
            format!(
                "Stop this session's agent and close its terminal tabs, then suspend the whole {}",
                self.noun()
            )
        } else {
            format!(
                "Suspend the whole {}, freezing every process in it until Resume",
                self.noun()
            )
        }
    }
    /// Starting a stopped environment cannot promise preservation of processes.
    pub(crate) fn resume_help(self, stopped: bool) -> &'static str {
        if stopped {
            self.starting_help()
        } else {
            "Resume this environment; preserved processes continue"
        }
    }
    /// A confirmed deletion names the provider resource independently of a title.
    pub(crate) fn resource_label(self, name: &str) -> String {
        format!(" {}: {}", self.noun(), crate::peer::display_peer(name))
    }
    /// Starting a stopped resource cannot imply that its old processes survived.
    pub(crate) fn starting_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Starting the sandbox; this view reconnects when it is ready."
            }
            ProviderKind::Podman => {
                "Starting the container again; its sessions return as interrupted, with Restart available."
            }
        }
    }
    /// Provider geometry carries locality, independent of the ordinary host mark.
    pub(crate) fn mark(self, session_row: bool) -> Element {
        match self.0 {
            ProviderKind::Tensorlake => rsx! {
                crate::icons::SandboxMark { session_row }
            },
            ProviderKind::Podman => rsx! {
                ContainerMark { session_row }
            },
        }
    }
    /// Provider-specific prerequisites stay separate from the shared base rule.
    pub(crate) fn recipe_refusal(
        self,
        recipe: &farhelm_proto::recipes::RecipeView,
        settings: &farhelm_proto::tensorlake::Settings,
    ) -> Option<String> {
        if !recipe.base.supports(self.0) {
            return Some(format!(
                "This recipe's base cannot run as a {}",
                self.noun()
            ));
        }
        match self.0 {
            ProviderKind::Tensorlake => crate::sandbox_sessions::recipe_refusal(recipe, settings),
            ProviderKind::Podman => None,
        }
    }
}

/// The place supplies its host label; recipe text remains raw until rendering.
pub(crate) fn target_label(
    target: &LaunchTarget,
    places: &[farhelm_proto::EnvironmentPlaceView],
) -> String {
    let noun = ProviderUi(target.provider()).noun();
    match target.place() {
        Place::TensorlakeAccount => format!("{} {noun}", target.recipe()),
        Place::PodmanHost(host) => {
            let name = places
                .iter()
                .find(|place| place.place == target.place())
                .map(|place| place.name.clone())
                .unwrap_or_else(|| format!("host {host}"));
            format!("{} {noun} on {name}", target.recipe())
        }
    }
}

/// An original shipping-box silhouette, distinct from the sandbox cube. The
/// 16-unit stroke box follows host geometry: a long enclosure and two ribs,
/// weight 1.3. It names containment without borrowing a provider's brand mark.
#[component]
fn ContainerMark(#[props(default)] session_row: bool) -> Element {
    rsx! {
        svg {
            width: "12",
            height: "12",
            stroke_linejoin: "round",
            "data-tooltip-anchor": if session_row { "session-row" },
            view_box: "0 0 16 16",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            "data-glyph": "container",
            "data-tooltip": "Container: runs in its own disposable container on one of your hosts",
            "aria-hidden": "true",
            rect {
                x: "1.5",
                y: "3.5",
                width: "13",
                height: "9",
                rx: "1",
            }
            path { d: "M5.5 3.5v9M10.5 3.5v9M1.5 6h13" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Pause retains container memory; cloud billing claims must not leak into
    /// container confirmations when the common session controls are reused.
    /// Without persisted completion attribution, an observation cannot claim
    /// that someone outside Farhelm paused the container.
    #[test]
    fn container_consequences_keep_memory_and_files_explicit() {
        let ui = ProviderUi(ProviderKind::Podman);
        assert!(ui.suspend_help().contains("memory stays"));
        assert!(ui.suspended_explanation().contains("memory stays"));
        assert!(!ui.suspended_explanation().contains("bills"));
        assert!(
            ui.suspended_explanation()
                .starts_with("This session's container is suspended.")
        );
        let detail = crate::peer::detail_text(&ui.suspended_detail(0, None, None, true));
        assert!(detail.starts_with("Suspension noticed at"));
        assert!(!detail.contains("outside Farhelm"));
        assert!(!ui.stop_help().contains("billing"));
        assert!(ui.delete_consequence().contains("Every session"));
        assert!(!ui.delete_consequence().contains("snapshot"));
    }

    /// Generalizing the lifecycle controls must keep Tensorlake's state
    /// description and idle policy separate from the action button's help.
    /// An observation time cannot attribute an external suspension to Farhelm.
    #[test]
    fn tensorlake_suspension_keeps_state_and_attribution_wording() {
        let ui = ProviderUi(ProviderKind::Tensorlake);
        assert_eq!(
            ui.suspended_explanation(),
            "This session's sandbox is suspended. Its processes are preserved, including the agent's running command, and continue when you resume it. Only snapshot storage bills while suspended."
        );
        assert_ne!(ui.suspended_explanation(), ui.suspend_help());
        let at = 1_700_000_000_000;
        let stamp = crate::activity::utc_stamp(at / 1000);
        let own = crate::peer::detail_text(&ui.suspended_detail(at + 1000, Some(at), None, true));
        assert_eq!(own, format!("Suspended from Farhelm at {stamp}."));
        let external = crate::peer::detail_text(&ui.suspended_detail(at, None, None, true));
        assert_eq!(
            external,
            format!(
                "Suspended outside Farhelm, noticed at {stamp}. Tensorlake suspends this sandbox after 30 minutes without a connection from Farhelm."
            )
        );
        let host = crate::peer::detail_text(&ui.suspended_detail(at, None, None, false));
        assert!(host.contains("24 hours without a connection"));
        assert!(!host.contains("30 minutes"));
        let failed = crate::peer::PeerText::from("snapshot lost");
        let failed = crate::peer::detail_text(&ui.suspended_detail(at, None, Some(&failed), true));
        assert!(
            failed.starts_with("Tensorlake could not resume the sandbox: snapshot lost. "),
            "{failed}"
        );
        assert!(
            ui.lifecycle_request_detail("suspend", at)
                .starts_with("Asked Tensorlake to suspend at "),
        );
    }
}

/// The launcher and template editor share placement semantics and disabled
/// reasons. Existing-host authority remains in the caller's own selector.
/// An active segment emits no placement edit, preserving the caller's explicit
/// host or recipe/place choice. Defaults are chosen only when entering another mode.
#[component]
pub(crate) fn PlacementPicker(
    target: Option<LaunchTarget>,
    recipes: Vec<farhelm_proto::recipes::RecipeView>,
    places: Vec<farhelm_proto::EnvironmentPlaceView>,
    #[props(default)] builds: Vec<farhelm_proto::RecipePlaceView>,
    settings: Option<farhelm_proto::tensorlake::Settings>,
    #[props(default)] last_container_host: Option<i64>,
    #[props(default)] disabled: bool,
    #[props(default)] authoring: bool,
    on_target: EventHandler<Option<LaunchTarget>>,
    children: Element,
) -> Element {
    let mode = target
        .as_ref()
        .map(|target| match target.provider() {
            ProviderKind::Tensorlake => 2,
            ProviderKind::Podman => 1,
        })
        .unwrap_or(0);
    let containers = places.iter().any(|place| {
        (authoring || place.reason.is_none())
            && match place.place {
                Place::PodmanHost(_) => true,
                Place::TensorlakeAccount => false,
            }
    });
    let sandboxes = authoring
        || settings
            .as_ref()
            .is_some_and(|settings| settings.api_key_set);
    let current_recipe = target
        .as_ref()
        .map(|target| target.recipe().to_string())
        .unwrap_or_default();
    let current_place = target.as_ref().map(LaunchTarget::place);
    let default_container = last_container_host
        .and_then(|host| {
            places
                .iter()
                .find(|place| {
                    place.place == Place::PodmanHost(host) && (authoring || place.reason.is_none())
                })
                .map(|place| place.place)
        })
        .or_else(|| {
            places
                .iter()
                .find(|place| {
                    (authoring || place.reason.is_none()) && place.place.container_host().is_some()
                })
                .map(|place| place.place)
        });
    let container_recipe = recipes
        .iter()
        .find(|recipe| recipe.base.supports(ProviderKind::Podman))
        .map(|recipe| recipe.name.clone())
        .unwrap_or_default();
    let sandbox_recipe = recipes
        .iter()
        .find(|recipe| {
            recipe.base.supports(ProviderKind::Tensorlake)
                && (authoring || recipe.snapshot.is_some())
        })
        .map(|recipe| recipe.name.clone())
        .unwrap_or_default();
    let recipe_target = target.clone();
    let place_recipe = current_recipe.clone();
    let provider = target.as_ref().map(|target| ProviderUi(target.provider()));
    let selected_place =
        current_place.and_then(|selected| places.iter().find(|place| place.place == selected));
    let build = builds
        .iter()
        .find(|build| Some(build.place) == current_place && build.recipe == current_recipe);
    rsx! {
        if containers || sandboxes || mode != 0 {
            div {
                class: "launch-placement",
                role: "group",
                aria_label: "Runs on",
                span { "Runs on" }
                div { class: "segmented-control",
                    button {
                        r#type: "button",
                        disabled,
                        aria_pressed: mode == 0,
                        "data-tooltip": "Launch on one of your existing hosts",
                        onclick: move |_| {
                            if mode == 0 {
                                return;
                            }
                            on_target.call(None);
                        },
                        "Existing host"
                    }
                    if containers || mode == 1 {
                        button {
                            r#type: "button",
                            disabled: disabled || !containers,
                            aria_pressed: mode == 1,
                            "data-tooltip": "Create a disposable container from a recipe on one of your hosts",
                            onclick: move |_| {
                                if mode == 1 {
                                    return;
                                }
                                if let Some(place) = default_container {
                                    on_target
                                        .call(
                                            Some(LaunchTarget::Environment {
                                                recipe: container_recipe.clone(),
                                                place,
                                            }),
                                        );
                                }
                            },
                            {ProviderUi(ProviderKind::Podman).segment()}
                        }
                    }
                    if sandboxes || mode == 2 {
                        button {
                            r#type: "button",
                            disabled: disabled || !sandboxes,
                            aria_pressed: mode == 2,
                            "data-tooltip": "Create a disposable Tensorlake sandbox from a built recipe",
                            onclick: move |_| {
                                if mode == 2 {
                                    return;
                                }
                                on_target
                                    .call(
                                        Some(LaunchTarget::Environment {
                                            recipe: sandbox_recipe.clone(),
                                            place: Place::TensorlakeAccount,
                                        }),
                                    );
                            },
                            {ProviderUi(ProviderKind::Tensorlake).segment()}
                        }
                    }
                }
            }
        }
        if let Some(provider) = provider {
            label { class: "checkout-root-label",
                span { "Recipe" }
                select {
                    aria_label: "Recipe",
                    value: current_recipe.clone(),
                    disabled,
                    "data-tooltip": "Recipe: the software and credentials for the new environment",
                    onchange: move |event| {
                        if let Some(target) = &recipe_target {
                            on_target
                                .call(
                                    Some(LaunchTarget::Environment {
                                        recipe: event.value(),
                                        place: target.place(),
                                    }),
                                );
                        }
                    },
                    // Restored targets may name a non-first option. Options
                    // mount after select.value is applied, so their selected
                    // property must preserve the same choice the draft submits.
                    if !recipes.iter().any(|recipe| recipe.name == current_recipe) {
                        option { value: current_recipe.clone(), selected: true, "Unavailable recipe (retained)" }
                    }
                    for recipe in &recipes {
                        {
                            let refusal = if !recipe.base.supports(provider.0) {
                                Some(format!("This recipe base cannot run as a {}", provider.noun()))
                            } else if authoring {
                                None
                            } else {
                                settings
                                    .as_ref()
                                    .and_then(|settings| provider.recipe_refusal(recipe, settings))
                            };
                            rsx! {
                                option { key: "{recipe.name}", value: recipe.name.clone(), selected: recipe.name == current_recipe, disabled: refusal.is_some(),
                                    {
                                        format!(
                                            "{}{}",
                                            crate::peer::display_peer(&recipe.name),
                                            refusal.as_ref().map(|reason| format!(" · {reason}")).unwrap_or_default(),
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if mode == 1 {
                label { class: "checkout-root-label",
                    span { "Container host" }
                    select {
                        aria_label: "Container host",
                        disabled,
                        value: current_place.and_then(Place::host).map(|id| id.to_string()).unwrap_or_default(),
                        "data-tooltip": "Container host: rootless Podman creates the container on this machine",
                        onchange: move |event| {
                            if let Ok(host) = event.value().parse::<i64>() {
                                on_target
                                    .call(
                                        Some(LaunchTarget::Environment {
                                            recipe: place_recipe.clone(),
                                            place: Place::PodmanHost(host),
                                        }),
                                    );
                            }
                        },
                        if current_place.is_some() && selected_place.is_none() {
                            option { value: current_place.and_then(Place::host).map(|host| host.to_string()).unwrap_or_default(), selected: true,
                                "Unavailable host (retained)"
                            }
                        }
                        for place in places.iter().filter(|place| place.place.container_host().is_some()) {
                            option {
                                key: "{place.place.container_host().unwrap()}",
                                value: place.place.container_host().unwrap().to_string(),
                                selected: Some(place.place) == current_place,
                                disabled: !authoring && place.reason.is_some(),
                                {
                                    format!(
                                        "{}{}",
                                        crate::peer::display_peer(&place.name),
                                        place
                                            .reason
                                            .as_ref()
                                            .map(|reason| format!(" · {reason}"))
                                            .unwrap_or_default(),
                                    )
                                }
                            }
                        }
                    }
                }
                if let Some(place) = selected_place {
                    p { class: "host-settings-help", role: "status",
                        if let Some(reason) = &place.reason {
                            "{crate::peer::display_peer(reason)}"
                        } else if let Some(build) = build {
                            if let Some(reason) = &build.reason {
                                "{crate::peer::display_peer(reason)}"
                            } else if build.building {
                                "Preparing…"
                            } else if build.ready {
                                "Ready on {crate::peer::display_peer(&place.name)}"
                            } else {
                                "Builds on first launch here (a few minutes)"
                            }
                        } else {
                            "Checking build status…"
                        }
                    }
                }
            }
        } else {
            {children}
        }
    }
}

/// Retain the latest place response for loading, failure and availability views.
type PlacesAnswer = Signal<Option<Result<Vec<farhelm_proto::EnvironmentPlaceView>, String>>>;

/// Place reads use the ordinary surface reader's single-flight admission and
/// retry ladder. Probes publish only to their cache, so an in-flight probe owes
/// another cached read at the shared cadence even with a healthy fleet feed.
/// Opening requests background refresh once. After it settles, feed/fallback
/// owns cached reads; only an explicit Refresh repeats the host probe.
pub(crate) fn use_environment_places() -> (PlacesAnswer, Callback<()>) {
    let base = use_context::<crate::ApiBase>().0;
    let reader = use_signal(crate::reader::SurfaceReader::default);
    let mut answer = use_signal(|| None);
    let request = use_callback(move |(trigger, refresh): (crate::reader::Trigger, bool)| {
        let base = base.clone();
        let mut refresh = refresh;
        crate::reader::request_read(reader, trigger, move || {
            let base = base.clone();
            let refresh = std::mem::take(&mut refresh);
            async move {
                let result = async {
                    if refresh {
                        crate::api::refresh_environment_places(&base).await?;
                    }
                    crate::api::fetch_environment_places(&base).await
                }
                .await;
                let success = result.is_ok();
                answer.set(Some(result));
                success
            }
        });
    });
    use_hook(move || request.call((crate::reader::Trigger::Explicit, true)));
    crate::feed::use_feed_reader(move || request.call((crate::reader::Trigger::Notice, false)));
    use_future(move || async move {
        loop {
            crate::feed::fallback_sleep().await;
            let probing = answer
                .peek()
                .as_ref()
                .and_then(|answer| answer.as_ref().ok())
                .is_some_and(|places| places.iter().any(|place| place.refreshing));
            if probing || crate::feed::fallback_polls_now() {
                request.call((crate::reader::Trigger::Scheduled, false));
            }
        }
    });
    let refresh = use_callback(move |()| request.call((crate::reader::Trigger::Explicit, true)));
    (answer, refresh)
}
