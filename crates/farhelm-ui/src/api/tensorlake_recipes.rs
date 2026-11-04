//! Client calls for the helm's Tensorlake recipes (`/api/tensorlake/recipes`).
//!
//! A child module of `api` so it can use the parent's private request
//! helpers, and kept in its own file so the recipe calls do not interleave
//! with the rest of the API surface. Writes go through `send_save`, the
//! explicit-configuration path, so a save outlives the component that started
//! it; every refusal comes back as the helm's own text, which the editor shows
//! as is. Saves carry replacement tokens only; nothing here ever receives one.

use super::{client, encode_path_segment, read_failure, refusal_text, send, send_read, send_save};
use farhelm_proto::tensorlake::recipes::{CreateRecipe, RecipeSave, RecipeView, RecipesView};

/// Reserved-name sandboxes with no owner on this helm. Errors must stay
/// visible because an empty list otherwise falsely promises no leftovers.
pub(crate) async fn fetch_orphan_sandboxes(
    base: &str,
) -> Result<Vec<farhelm_proto::tensorlake::OrphanSandbox>, String> {
    let url = format!("{base}/api/tensorlake/orphans");
    let response = send_read(client().get(&url)).await?;
    if !response.status().is_success() {
        return Err(read_failure("GET", &url, response).await);
    }
    response
        .json()
        .await
        .map_err(|_| "Could not read Tensorlake sandboxes".to_string())
}

/// Start the explicitly confirmed delete before its component can close.
pub(crate) fn delete_orphan_sandbox(
    base: &str,
    id: &str,
) -> impl Future<Output = Result<(), String>> + use<> {
    let url = format!("{base}/api/tensorlake/orphans/{}", encode_path_segment(id));
    let sending = send_save(client().delete(&url));
    async move {
        let response = sending.await?;
        if !response.status().is_success() {
            return Err(refusal_text("DELETE", &url, response).await);
        }
        Ok(())
    }
}

/// Every recipe on the helm, by name. A failure is the read's own error text,
/// so the list can offer a retry rather than show an empty catalog.
pub(crate) async fn fetch_recipes(base: &str) -> Result<Vec<RecipeView>, String> {
    let url = format!("{base}/api/tensorlake/recipes");
    let response = send_read(client().get(&url)).await?;
    if !response.status().is_success() {
        return Err(read_failure("GET", &url, response).await);
    }
    response
        .json::<RecipesView>()
        .await
        .map(|body| body.recipes)
        .map_err(|_| "Could not read Tensorlake recipes".to_string())
}

/// Create a recipe. The helm refuses an existing name (409) rather than
/// overwriting it. The request is registered before this returns, so the
/// caller may spawn the future after its component is gone.
pub(crate) fn create_recipe(
    base: &str,
    create: CreateRecipe,
) -> impl Future<Output = Result<RecipeView, String>> + use<> {
    let url = format!("{base}/api/tensorlake/recipes");
    let sending = send_save(client().post(&url).json(&create));
    async move {
        let response = sending.await?;
        if !response.status().is_success() {
            return Err(refusal_text("POST", &url, response).await);
        }
        response
            .json()
            .await
            .map_err(|_| "Could not read the saved recipe".to_string())
    }
}

/// Replace the existing recipe `name`'s definition. The helm refuses a
/// missing name (404) rather than creating it.
pub(crate) fn update_recipe(
    base: &str,
    name: &str,
    save: RecipeSave,
) -> impl Future<Output = Result<RecipeView, String>> + use<> {
    let url = format!(
        "{base}/api/tensorlake/recipes/{}",
        encode_path_segment(name)
    );
    let sending = send_save(client().put(&url).json(&save));
    async move {
        let response = sending.await?;
        if !response.status().is_success() {
            return Err(refusal_text("PUT", &url, response).await);
        }
        response
            .json()
            .await
            .map_err(|_| "Could not read the saved recipe".to_string())
    }
}

/// Delete the recipe `name`. A missing reply is reported as an error even
/// though the delete may have committed; the list re-reads either way.
pub(crate) fn delete_recipe(
    base: &str,
    name: &str,
) -> impl Future<Output = Result<(), String>> + use<> {
    let url = format!(
        "{base}/api/tensorlake/recipes/{}",
        encode_path_segment(name)
    );
    let sending = send_save(client().delete(&url));
    async move {
        let response = sending.await?;
        if !response.status().is_success() {
            return Err(refusal_text("DELETE", &url, response).await);
        }
        Ok(())
    }
}

/// Start building the recipe `name`'s snapshot, or join the build already
/// running; Build, Rebuild and Retry all send this. The helm answers as soon
/// as the build is running; its progress then arrives through the
/// invalidation feed and [`fetch_recipes`]. A refusal (no API key, no ssh
/// key, a recipe that reuses an image) comes back as the helm's own text.
pub(crate) async fn build_recipe(base: &str, name: &str) -> Result<(), String> {
    let url = format!(
        "{base}/api/tensorlake/recipes/{}/build",
        encode_path_segment(name)
    );
    let response = send(client().post(&url)).await?;
    if !response.status().is_success() {
        return Err(refusal_text("POST", &url, response).await);
    }
    Ok(())
}
