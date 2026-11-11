//! Container settings display provider-owned cached probes and explicit cleanup.
//! Closing Settings drops drafts and confirmations, never an admitted deletion.
use crate::{ApiBase, api, peer::display_peer};
use dioxus::prelude::*;
use std::{cell::Cell, rc::Rc};

/// Refresh cached place availability and discover leftovers through the helm.
/// Reads retain their errors; only explicit confirmation can remove a resource.
#[component]
pub(crate) fn ContainersSettings() -> Element {
    let base = use_context::<ApiBase>().0;
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let dropped = mounted.clone();
    use_drop(move || dropped.set(false));
    // Probe failures and resource-discovery failures are independent. Keep
    // host readiness visible when one provider enumeration is unavailable.
    let (places, refresh_places) = crate::environment_ui::use_environment_places();
    let mut catalog =
        use_signal(|| None::<Result<Vec<farhelm_proto::ContainerLeftoverView>, String>>);
    let reader = use_signal(crate::reader::SurfaceReader::default);
    let read_base = base.clone();
    let reload = use_callback(move |trigger| {
        let base = read_base.clone();
        crate::reader::request_read(reader, trigger, move || {
            let base = base.clone();
            async move {
                let result = api::fetch_container_leftovers(&base).await;
                let answered = result.is_ok();
                catalog.set(Some(result));
                answered
            }
        });
    });
    use_hook(move || reload.call(crate::reader::Trigger::Explicit));
    crate::feed::use_feed_reader(move || reload.call(crate::reader::Trigger::Notice));
    use_future(move || async move {
        loop {
            crate::feed::fallback_sleep().await;
            if crate::feed::fallback_polls_now() {
                reload.call(crate::reader::Trigger::Scheduled);
            }
        }
    });
    let mut confirm = crate::ops::use_confirm_slot::<farhelm_proto::ContainerLeftoverView, ()>();
    let mut busy = use_signal(|| false);
    let mut action_error = use_signal(|| None::<String>);
    let answer = catalog.read().clone();
    let place_answer = places.read().clone();
    rsx! {
        p { class: "host-settings-help",
            "Containers run on your Linux hosts with rootless Podman, cgroup v2 and the systemd cgroup manager. Suspend keeps their memory in use."
        }
        button {
            r#type: "button",
            class: "btn btn-neutral",
            disabled: busy(),
            "data-tooltip": "Refresh Podman status and containers not on this helm",
            onclick: move |_| {
                refresh_places.call(());
                reload.call(crate::reader::Trigger::Explicit);
            },
            "Refresh"
        }
        section { aria_label: "Container hosts",
            match place_answer {
                None => rsx! {
                    p { role: "status", "Reading container hosts…" }
                },
                Some(Err(reason)) => rsx! {
                    p { role: "alert", "{display_peer(&reason)}" }
                },
                Some(Ok(places)) => rsx! {
                    for place in places.iter().filter(|place| place.place.container_host().is_some()) {
                        div { class: "recipe-row",
                            span { "{display_peer(&place.name)}" }
                            span { role: "status",
                                {
                                    place
                                        .reason
                                        .as_ref()
                                        .map(|reason| display_peer(reason))
                                        .unwrap_or_else(|| "Ready".into())
                                }
                            }
                            if place.refreshing {
                                span { " · Refreshing…" }
                            }
                        }
                    }
                },
            }
        }
        section { aria_label: "Containers not on this helm",
            h4 { "Containers not on this helm" }
            p { class: "host-settings-help",
                "These containers have no completed session on this helm. Interrupted launch resources remain here for explicit cleanup. Nothing is deleted automatically."
            }
            match answer {
                None => rsx! {
                    p { role: "status", "Reading container leftovers…" }
                },
                Some(Err(reason)) => rsx! {
                    p { role: "alert", class: "checkout-root-error", "{display_peer(&reason)}" }
                },
                Some(Ok(leftovers)) => rsx! {
                    if leftovers.is_empty() {
                        p { role: "status", "None found" }
                    }
                    for row in leftovers {
                        div { class: "tensorlake-orphan",
                            span { "{display_peer(&row.name)}" }
                            button {
                                r#type: "button",
                                class: "btn btn-neutral",
                                disabled: busy(),
                                "data-tooltip": "Delete this container and all its processes and files",
                                onclick: move |_| {
                                    action_error.set(None);
                                    confirm.open(row.clone(), ());
                                },
                                "Delete"
                            }
                        }
                    }
                },
            }
        }
        if let Some(shown) = confirm.current() {
            div {
                class: "orphan-delete-confirm",
                role: "alertdialog",
                aria_label: "Delete container?",
                p {
                    "Delete {display_peer(&shown.key().name)}? Every session, process, file and unpushed change in it will be permanently removed."
                }
                button {
                    r#type: "button",
                    class: "btn btn-neutral",
                    disabled: busy(),
                    "data-tooltip": "Cancel: keep this container",
                    onclick: {
                        let shown = shown.clone();
                        move |_| {
                            confirm.take(&shown);
                        }
                    },
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "btn btn-danger",
                    disabled: busy(),
                    "data-tooltip": "Delete the whole container and its files",
                    onkeydown: crate::guard_confirmation_enter,
                    onclick: {
                        let shown = shown.clone();
                        let base = base.clone();
                        let mounted = mounted.clone();
                        move |_| {
                            let mounted = mounted.clone();
                            if busy() || confirm.take(&shown).is_none() {
                                return;
                            }
                            busy.set(true);
                            let sending = api::delete_container_leftover(&base, shown.key());
                            dioxus::core::spawn_forever(async move {
                                let result = sending.await;
                                if !mounted.get() {
                                    return;
                                }
                                busy.set(false);
                                match result {
                                    Ok(()) => reload.call(crate::reader::Trigger::Explicit),
                                    Err(reason) => action_error.set(Some(reason)),
                                }
                            });
                        }
                    },
                    "Delete container"
                }
            }
        }
        if let Some(reason) = action_error() {
            p { role: "alert", class: "checkout-root-error", "{display_peer(&reason)}" }
        }
    }
}

/// Per-place build status reuses the shared recipe row and existing build engine.
/// A rebuild request survives leaving the editor; failures remain beside its place.
#[component]
pub(crate) fn RecipePlaces(
    recipe: String,
    states: Option<Result<Vec<farhelm_proto::RecipePlaceView>, String>>,
    places: Vec<farhelm_proto::EnvironmentPlaceView>,
    on_refresh: EventHandler<()>,
) -> Element {
    let base = use_context::<ApiBase>().0;
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let dropped = mounted.clone();
    use_drop(move || dropped.set(false));
    // The mutation registers before spawning; only the mounted row owns its
    // completion paint. Closing Settings cannot cancel admitted build work.
    let mut pending = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let answer = states;
    let place_names = places;
    rsx! {
        match answer {
            None => rsx! {
                span { role: "status", "Reading per-place builds…" }
            },
            Some(Err(reason)) => rsx! {
                span { role: "alert", "{display_peer(&reason)}" }
            },
            Some(Ok(rows)) => rsx! {
                for row in rows.into_iter()
                    .filter(|row| row.recipe == recipe && row.place.container_host().is_some())
                {
                    div { class: "recipe-row-status",
                        span {
                            {
                                format!(
                                    "{} · {}",
                                    place_names
                                        .iter()
                                        .find(|place| place.place == row.place)
                                        .map(|place| display_peer(&place.name))
                                        .unwrap_or_else(|| "Unavailable place".into()),
                                    row
                                        .reason
                                        .clone()
                                        .unwrap_or_else(|| {
                                            if row.building {
                                                "Preparing…".into()
                                            } else if row.ready {
                                                "Ready".into()
                                            } else {
                                                "Not built; builds on first launch".into()
                                            }
                                        }),
                                )
                            }
                        }
                        if let Some(step) = &row.step {
                            span { role: "status", " · {display_peer(step)}" }
                        }
                        // Both public build timestamps and utc_stamp use epoch seconds.
                        if let Some(at) = row.prepared_at {
                            span { " · Prepared {crate::activity::utc_stamp(at)}" }
                        } else if let Some(at) = row.built_at {
                            span { " · Built {crate::activity::utc_stamp(at)}" }
                        }
                        if row.cleanup_pending {
                            span { " · Previous image cleanup is unfinished" }
                        }
                        button {
                            r#type: "button",
                            class: "btn btn-neutral",
                            disabled: pending() || row.building,
                            "data-tooltip": "Rebuild this recipe at this place; existing environments retain their files",
                            onclick: {
                                let base = base.clone();
                                let recipe = recipe.clone();
                                let mounted = mounted.clone();
                                move |_| {
                                    let mounted = mounted.clone();
                                    pending.set(true);
                                    error.set(None);
                                    let sending = api::rebuild_recipe_at(&base, &recipe, row.place);
                                    dioxus::core::spawn_forever(async move {
                                        let result = sending.await;
                                        if !mounted.get() {
                                            return;
                                        }
                                        pending.set(false);
                                        match result {
                                            Ok(()) => on_refresh.call(()),
                                            Err(reason) => error.set(Some(reason)),
                                        }
                                    });
                                }
                            },
                            "Rebuild"
                        }
                    }
                }
            },
        }
        if let Some(reason) = error() {
            p { role: "alert", "{display_peer(&reason)}" }
        }
    }
}
