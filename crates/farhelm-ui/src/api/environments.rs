//! Environment catalogs and explicit maintenance share the helm's provider
//! services. Writes register before their initiating Settings view can unmount.
use super::*;

/// Opening a launcher schedules probes without waiting for host fan-out.
pub(crate) async fn fetch_environment_places(
    base: &str,
) -> Result<Vec<farhelm_proto::EnvironmentPlaceView>, String> {
    let url = format!("{base}/api/environment-places");
    let response = send_read(client().get(&url)).await?;
    if !response.status().is_success() {
        return Err(read_failure("GET", &url, response).await);
    }
    response.json().await.map_err(|error| error.to_string())
}

/// Reusable user choices do not require the deleted source's supervisor.
pub(crate) async fn fetch_environment_history(
    base: &str,
) -> Result<Vec<LaunchHistoryEntry>, String> {
    let url = format!("{base}/api/environment-launch-history");
    let response = send_read(client().get(&url)).await?;
    if !response.status().is_success() {
        return Err(read_failure("GET", &url, response).await);
    }
    response.json().await.map_err(|error| error.to_string())
}

/// State is public-safe readiness, not an assertion that an image still exists.
pub(crate) async fn fetch_recipe_places(
    base: &str,
) -> Result<Vec<farhelm_proto::RecipePlaceView>, String> {
    let url = format!("{base}/api/recipe-places");
    let response = send_read(client().get(&url)).await?;
    if !response.status().is_success() {
        return Err(read_failure("GET", &url, response).await);
    }
    response.json().await.map_err(|error| error.to_string())
}

/// Register a deliberate rebuild before the Settings component can disappear.
pub(crate) fn rebuild_recipe_at(
    base: &str,
    recipe: &str,
    place: farhelm_proto::Place,
) -> impl Future<Output = Result<(), String>> + use<> {
    let url = format!("{base}/api/recipes/{}/rebuild", encode_path_segment(recipe));
    let sending = send_save(
        client()
            .post(&url)
            .json(&serde_json::json!({"place":place})),
    );
    async move {
        let response = sending.await?;
        if !response.status().is_success() {
            return Err(refusal_text("POST", &url, response).await);
        }
        Ok(())
    }
}

/// A failed discovery must stay an error; an empty list promises no leftovers.
pub(crate) async fn fetch_container_leftovers(
    base: &str,
) -> Result<Vec<farhelm_proto::ContainerLeftoverView>, String> {
    let url = format!("{base}/api/container-orphans");
    let response = send_read(client().get(&url)).await?;
    if !response.status().is_success() {
        return Err(read_failure("GET", &url, response).await);
    }
    response.json().await.map_err(|error| error.to_string())
}

/// Row-backed cleanup reuses ordinary whole-host deletion. The unregistered
/// route rechecks provider ownership; the client never submits execution coordinates.
pub(crate) fn delete_container_leftover(
    base: &str,
    row: &farhelm_proto::ContainerLeftoverView,
) -> impl Future<Output = Result<(), String>> + use<> {
    let (url, request) = if let Some(host) = row.host {
        let url = format!("{base}/api/hosts/{host}");
        let request = client().delete(&url);
        (url, request)
    } else {
        let url = format!("{base}/api/container-orphans");
        let request = client()
            .delete(&url)
            .json(&serde_json::json!({"place":row.place,"id":row.id}));
        (url, request)
    };
    let sending = send_save(request);
    async move {
        let response = sending.await?;
        if !response.status().is_success() {
            return Err(refusal_text("DELETE", &url, response).await);
        }
        Ok(())
    }
}

/// Explicit Refresh uses the existing one-place background probe route. It
/// admits no build or container creation and never waits for a fleet fan-out.
pub(crate) async fn refresh_environment_places(base: &str) -> Result<(), String> {
    for place in fetch_environment_places(base).await? {
        if let Some(host) = place.place.container_host() {
            let url = format!("{base}/api/hosts/{host}/podman-probe");
            let response = send_read(client().post(&url)).await?;
            if !response.status().is_success() {
                return Err(read_failure("POST", &url, response).await);
            }
        }
    }
    Ok(())
}
