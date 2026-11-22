# A failed Rebuild or preparation on a later-removed container host makes the recipe permanently undeletable

Reviewed commit: 018264e2af62fd7fc94d71773390f80cc6907443

## TLDR

If a recipe rebuild fails on a container host that does not yet hold any images for that recipe (for example because the
host is offline or unusable), and you later remove or replace that container host in Farhelm, deleting the recipe
afterwards always fails and the recipe can never be deleted.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman/build.rs:286`.

Commit 018264e2 moved `self.0.places.fresh(&self.0.store, place).await?` inside `Podman::prepare_images`
(`crates/farhelm-helm/src/environment/podman/build.rs:286-293`), AFTER `self.edit(name, place, ...)` clears
`last_failure` and `last_failure_from_rebuild`. Because `edit_at`
(`crates/farhelm-helm/src/environment/podman/build.rs:579`) calls `state.podman.entry(place).or_default()`, a Rebuild on
an offline or unusable host with no prior artifacts inserts a `PodmanRecipeState` entry into `state.podman` containing
only `last_failure` / `last_failure_from_rebuild` (written again at lines 313-320). (A similar failure-only entry can
also be left if a lazy preparation fails before committing any image, e.g. a failed base pull.) Because that entry has
no images, helpers, or start timestamps, `PodmanRecipeState::needs_cleanup()`
(`crates/farhelm-helm/src/environment/recipes.rs:80-88`) returns `false`. Consequently, `remove_host_row`
(`crates/farhelm-helm/src/store.rs:8652`) -> `referenced_place_reason`
(`crates/farhelm-helm/src/store/podman.rs:360-366`) allows removing that container host from the `hosts` table while
leaving the `place` key in `tensorlake_recipes.state_json`. When the user later deletes the recipe
(`DELETE /api/tensorlake/recipes/{name}` -> `delete_recipe_owned` at `crates/farhelm-helm/src/tensorlake/recipes.rs:689`
-> `Podman::clean_images` at `crates/farhelm-helm/src/environment/podman/build.rs:928-941`), `clean_images` iterates
every `(place, _)` key in `state.podman` and unconditionally calls `self.edit(name, place, ...)` at line 932.
`self.edit` (`crates/farhelm-helm/src/environment/podman/build.rs:552-559`) requires `place` to still exist in `hosts`
and fails with `"the container place was removed before its state could be recorded"`. That aborts recipe deletion
before `delete_tensorlake_recipe` runs. Turn-2 verification: `hosts.id` is `INTEGER PRIMARY KEY AUTOINCREMENT`
(`crates/farhelm-helm/src/store.rs:2925`), so replacing/re-adding the removed host can never reuse its `HostId` and
cannot recover the recipe. Retargeting a host in place (`retarget` -> `update_ssh_destination_changed`) is also
permitted when `needs_cleanup() == false`, but does not block recipe deletion because the host row still exists for
`self.edit` (it only leaves stale `last_failure` text on the retargeted host until the next build). Fix direction: in
`Podman::clean_images`, if the place entry has `!place_state.needs_cleanup()` (no images, helpers, start markers, or
retired images), remove the key via `update_tensorlake_recipe_state` without calling `self.edit(name, place, ...)`, and
consider avoiding creating a new `state.podman` entry before `places.fresh()` succeeds. Verify with a unit test that
triggers a failed Rebuild on an unusable/offline place, deletes the host row, and confirms `clean_images` and
`delete_tensorlake_recipe` succeed.

Reviewer metadata (g34-podman-recipe-rebuild/l6): suggested bucket: high (could not be highest: no security or data-loss
impact, but permanently prevents deleting a recipe); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g34-podman-recipe-rebuild/l6`.
