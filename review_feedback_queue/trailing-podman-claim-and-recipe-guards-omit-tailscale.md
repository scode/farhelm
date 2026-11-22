# Container place claim and recipe state SQL guards omit the host Tailscale flag

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

Switching an SSH container host's Tailscale setting while a recipe image build is starting can let that build commit
ownership and inject environment proxy settings collected over the old SSH route onto the host after its route mode
changed.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/store/podman.rs:79-99`.

Bucket note: 5-way corroborated (`g43/l4` suggested `high` bordering `highest`, `g45/f5` flagged borderline `highest`,
`g35/l4`+`g36/l1`+`g37/f1` suggested `other`); placed in `high` while surfacing the trust-boundary caveat if unpinned
SSH proxy values carry into a newly Tailscale-pinned recipe image.

SPEC_impl.md:6143-6144 specifies that an ordinary SSH row's `tailscale` bit "travels with the destination, through the
supervisor handshake's identity write guard, the container place check, and confirmation of a provisioning plan." Commit
74fa1951 updated `HelmStore::claim_podman_place` (`crates/farhelm-helm/src/store/podman.rs:398,414,424`) to fence on
`tailscale IS ?7` and include `host.tailscale` in twin `DialedAs`, but missed two other container-place fences:

1. `has_podman_place_claim` (`crates/farhelm-helm/src/store/podman.rs:79-99`, also backing
   `HelmStore::owns_podman_place` at `store/podman.rs:456-466`) checks
   `WHERE host.id = ?1 AND host.entry_uuid = ?2 AND host.destination IS ?3 AND claim.machine_id = ?4 AND claim.uid = ?5`,
   omitting `AND host.tailscale IS ?6`.
2. `HelmStore::write_recipe_state` (`crates/farhelm-helm/src/tensorlake/build.rs:345-359`, invoked via
   `update_recipe_place_state` from `Podman::edit_at` at `crates/farhelm-helm/src/environment/podman/build.rs:578`)
   queries `kind, tensorlake_sandbox, environment_provider, entry_uuid, destination` and checks
   `kind.can_host_environments() && kind == place.kind && identity == place.entry_uuid && destination == place.destination`,
   omitting `tailscale == place.tailscale`. Route in shipped code: During a recipe build on an SSH place with no
   existing containers or recorded recipe artifacts (`Podman::prepare_images` -> `Podman::software`,
   `crates/farhelm-helm/src/environment/podman/build.rs:162-196`), `self.place(place)` snapshots `(expected, _, ready)`
   (including `ready.identity` and `ready.proxy` read over the old SSH mode, e.g., `tailscale = false`). Before
   `self.helper` calls `self.edit_at(name, &expected, Some(identity), ...)` (`build.rs:643`),
   `POST /api/hosts/{id}/tailscale` (`crates/farhelm-helm/src/hosts.rs:981-1039`) is not blocked by
   `podman::refuse_referenced_place` (no container row or `needs_cleanup` recipe record exists yet) and takes no
   recipe-build lock, so it commits `tailscale = true` and deletes `podman_place_claims`. For `claim = None`
   (`Podman::edit`, `build.rs:548-561`), `write_recipe_state` succeeds unconditionally across the mode change. For
   `claim = Some(identity)` (`Podman::helper`, `build.rs:643`), if a probe on the new `tailscale = true` route re-claims
   the same `(machine_id, uid)` (the normal case when enabling Tailscale for an existing host), both
   `write_recipe_state` and `has_podman_place_claim` succeed even though `expected.tailscale != hosts.tailscale`. The
   build then records helper ownership under the Tailscale-mode host and writes `ready.proxy` observed over the old
   unpinned SSH route into the new container image (`build.rs:196`). Bucket caveat: This could cross into `highest` if a
   user enables Tailscale mode to stop trusting an unpinned/MITM'd SSH path while a build started on that path carries
   attacker-supplied `ready.proxy` settings into the Tailscale-verified recipe image (requiring the attacker to match
   the host's world-readable `/etc/machine-id` + UID and race the mode switch + re-probe). Fix direction: Add
   `AND host.tailscale IS ?6` (bound to `row.tailscale`) to `has_podman_place_claim` in
   `crates/farhelm-helm/src/store/podman.rs:84-98`, and select and compare `tailscale == place.tailscale` in
   `write_recipe_state` at `crates/farhelm-helm/src/tensorlake/build.rs:346-352`. Add tests proving `owns_podman_place`
   and `update_recipe_place_state` reject a stale `HostRow` after a `set_ssh_tailscale` toggle even if the same
   `(machine_id, uid)` is re-claimed.

Reviewer metadata (g43-tailscale-store-and-access/l4): suggested bucket: high (could be highest — see security boundary
caveat below); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/farhelm-helm/src/tensorlake/build.rs:346` (g35-podman-identity-probe/l4):

`SPEC_impl.md:6143-6144` requires: "An ordinary SSH row stores a false-by-default `tailscale` bit. It travels with the
destination, through the supervisor handshake's identity write guard, the container place check, and confirmation of a
provisioning plan." `HelmStore::claim_podman_place` (`crates/farhelm-helm/src/store/podman.rs:394-400`) enforces this
with `AND tailscale IS ?7`. However, both `has_podman_place_claim` (`crates/farhelm-helm/src/store/podman.rs:79-99`) and
`HelmStore::write_recipe_state` (`crates/farhelm-helm/src/tensorlake/build.rs:346-359`) omit `hosts.tailscale`:

- `store/podman.rs:87`:
  `WHERE host.id = ?1 AND host.entry_uuid = ?2 AND host.destination IS ?3 AND claim.machine_id = ?4 AND claim.uid = ?5`
  (no `AND host.tailscale IS ?6`).
- `tensorlake/build.rs:347,351`: selects only `kind, tensorlake_sandbox, environment_provider, entry_uuid, destination`
  and checks `kind == place.kind && identity == place.entry_uuid && destination == place.destination` (no
  `&& tailscale == place.tailscale`). In `crates/farhelm-helm/src/environment/podman/build.rs:162-185, 643`,
  `software()` captures `(expected, _, ready)` from `self.place(place)` and later calls `helper()` ->
  `edit_at(name, &expected, Some(identity), ...)`. If `set_tailscale` toggles `hosts.tailscale` in between (succeeding
  because no recipe reference exists yet) and a fresh probe on the new transport mode re-claims the same
  `(machine_id, uid)`, both SQL guards pass despite `expected.tailscale != hosts.tailscale`, recording helper ownership
  and using the pre-toggle `ready.proxy` at `build.rs:196`. Fix direction: Include `AND host.tailscale IS ?6` in
  `has_podman_place_claim` and select/compare `tailscale == place.tailscale` in `write_recipe_state`.

Reviewer metadata (g35-podman-identity-probe/l4): suggested bucket: other (cannot be in highest: requires toggling
Tailscale mode in the narrow window between `self.place` and `edit_at` plus a fresh probe on the new mode claiming the
exact same `(machine_id, uid)`; at most carries pre-toggle `ready.proxy` into the helper script on a host the user
already trusted before the toggle).; confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`, `crates/farhelm-helm/src/store/podman.rs:79-99`
(g36-podman-identity-store-build/l1):

SPEC_impl.md ('One route for transport and provisioning', lines 6143-6144) specifies that an SSH host's `tailscale` bit
'travels with the destination, through the supervisor handshake's identity write guard, the container place check, and
confirmation of a provisioning plan.' `HelmStore::claim_podman_place` (`crates/farhelm-helm/src/store/podman.rs:398`)
enforces `AND tailscale IS ?7`, and `access::base_route` (`crates/farhelm-helm/src/access.rs:442-450`) includes
`row.ssh_access()?` in `BaseRoute`. However:

1. `has_podman_place_claim` (`crates/farhelm-helm/src/store/podman.rs:79-99`) checks only
   `host.id = ?1 AND host.entry_uuid = ?2 AND host.destination IS ?3 AND claim.machine_id = ?4 AND claim.uid = ?5`,
   omitting `AND host.tailscale IS ?6`.
2. `HelmStore::write_recipe_state` (`crates/farhelm-helm/src/tensorlake/build.rs:346-358`, called by
   `update_recipe_place_state` / `Podman::edit_at`) selects and checks only `kind`, `entry_uuid`, and `destination`,
   omitting `tailscale`. Route (source-verified): if an unused SSH container host toggles Tailscale mode via
   `set_tailscale` -> `PodmanPlaces::set_ssh_tailscale` (`hosts.rs:981-1039`, `store.rs:8144-8183`, which deletes the
   claim and flips `hosts.tailscale` while keeping `id`, `entry_uuid`, and `destination`) between `Podman::software` or
   `Podman::prepare_images_owned` reading `(expected, _, ready)` (`build.rs:162`, `build.rs:365`) and `Podman::helper`
   calling `edit_at(name, &expected, Some(identity), ...)` (`build.rs:643`), and a new probe on the toggled route
   reclaims the same `(machine_id, uid)`, both `write_recipe_state` and `has_podman_place_claim` accept the stale
   `expected` row. `software()` then continues with `ready.proxy` observed under the old SSH mode while later commands
   re-resolve the new mode. For `edit_at(..., None, ...)` calls, the place check accepts a stale `HostRow` even without
   a re-probe. Fix direction: add `AND host.tailscale IS ?6` (bound to `row.tailscale`) in `has_podman_place_claim`, and
   select/compare `tailscale` (`tailscale == place.tailscale`) in `write_recipe_state` (`tensorlake/build.rs:346-352`).
   Add a test case to `first_helper_write_refuses_removed_retargeted_or_replaced_probe_place`
   (`environment/podman/tests.rs:1264`) for a Tailscale mode toggle followed by reclaim.

Reviewer metadata (g36-podman-identity-store-build/l1): suggested bucket: other (could NOT be in highest: cannot cross
to a different machine or Unix account because machine_id and uid are still matched in podman_place_claims and
VERIFY_PLACE_AND_EXEC); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`, `crates/farhelm-helm/src/store/podman.rs:79`
(g37-podman-retarget-probe-store/f1):

SPEC_impl.md:6143-6144 states that an SSH row's `tailscale` bit "travels with the destination, through the supervisor
handshake's identity write guard, the container place check, and confirmation of a provisioning plan." Commit 74fa1951b
updated `HelmStore::claim_podman_place` (`crates/farhelm-helm/src/store/podman.rs:397-398`) to fence on
`destination IS ?3 AND tailscale IS ?7`, but `has_podman_place_claim` (`crates/farhelm-helm/src/store/podman.rs:79-99`)
still checks only
`host.id = ?1 AND host.entry_uuid = ?2 AND host.destination IS ?3 AND claim.machine_id = ?4 AND claim.uid = ?5`,
omitting `AND host.tailscale IS ?6`. Its caller `HelmStore::write_recipe_state`
(`crates/farhelm-helm/src/tensorlake/build.rs:346-358`) likewise checks
`kind == place.kind && identity == place.entry_uuid && destination == place.destination` without comparing `tailscale`.
If a host's `tailscale` bit is toggled after `Podman::place` snapshots `expected` in `Podman::software` or
`Podman::prepare_images_owned` (`crates/farhelm-helm/src/environment/podman/build.rs:162, 365`) and a fresh probe
re-claims the same `(machine_id, uid)` under the new mode before `helper()` calls `edit_at`, both SQL guards succeed
instead of rejecting the stale pre-edit snapshot. Fix: add `AND host.tailscale IS ?6` (binding `row.tailscale`) in
`has_podman_place_claim` and compare `tailscale == place.tailscale` in `write_recipe_state`, with a unit test covering a
`tailscale` toggle followed by re-claim of the same `(machine_id, uid)`.

Reviewer metadata (g37-podman-retarget-probe-store/f1): suggested bucket: other (not highest: the bypass only passes
when the new Tailscale/non-Tailscale route still claims the exact same machine_id and uid, and subsequent Podman
commands re-resolve the current row's BaseRoute from the store, so no cross-machine data loss or credential misrouting
is established); confidence: confirmed; possible cover: none (interface overlap with g43-tailscale-store-and-access /
commit 74fa1951bac92f5a78757100534498c458d86806).

Additional source record at `74fa1951bac92f5a78757100534498c458d86806`, `crates/farhelm-helm/src/store/podman.rs:79-99`
(g45-tailscale-provisioning-podman/f5):

`SPEC_impl.md` ("One route for transport and provisioning", lines 6143-6144) states that an SSH row's `tailscale` bit
"travels with the destination, through the supervisor handshake's identity write guard, the container place check, and
confirmation of a provisioning plan." While `HelmStore::claim_podman_place`
(`crates/farhelm-helm/src/store/podman.rs:397-398`) was updated to fence on `AND tailscale IS ?7`, two companion SQL
guards for Podman place ownership were missed:

1. `has_podman_place_claim` (`crates/farhelm-helm/src/store/podman.rs:79-99`, called by `HelmStore::owns_podman_place`
   at `podman_probe.rs:711` and by `HelmStore::write_recipe_state` at `tensorlake/build.rs:355`) filters only
   `WHERE host.id = ?1 AND host.entry_uuid = ?2 AND host.destination IS ?3 AND claim.machine_id = ?4 AND claim.uid = ?5`,
   omitting `AND host.tailscale IS ?6`.
2. `HelmStore::write_recipe_state` (`crates/farhelm-helm/src/tensorlake/build.rs:345-359`, called by `Podman::edit` /
   `Podman::edit_at` at `environment/podman/build.rs:560-584` and `Podman::helper` at `environment/podman/build.rs:643`)
   selects only `kind, tensorlake_sandbox, environment_provider, entry_uuid, destination` and ensures
   `kind.can_host_environments() && kind == place.kind && identity == place.entry_uuid && destination == place.destination`,
   omitting `tailscale == place.tailscale`. Route in shipped code: A recipe build/preparation observes
   `(expected: HostRow { tailscale: false }, ready: ReadyPlace { proxy, identity })` via `PodmanPlaces::fresh`
   (`environment/podman/build.rs`). Before `helper()` calls `edit_at(name, expected, Some(identity), ...)`, the user
   toggles Tailscale mode ON (`set_tailscale`, which clears `podman_place_claims` and sets `tailscale = 1`) and a fresh
   probe on the Tailscale route re-claims the same `(machine_id, uid)` (same machine now reached via Tailscale).
   `write_recipe_state` and `has_podman_place_claim` both pass despite `hosts.tailscale != expected.tailscale`,
   committing helper ownership (which immediately blocks future mode edits via `refuse_referenced_place`) and then
   passing the pre-toggle `ready.proxy` values into the preparer container at `environment/podman/build.rs:400`
   (`self.proxies(place, &id, &user, &ready.proxy)`). Likewise, `Podman::edit` (`claim = None`) fails to detect a
   `tailscale` change between reading `expected` (`build.rs:552`) and committing `write_recipe_state`. Fix: include
   `AND host.tailscale IS ?6` in `has_podman_place_claim` (`store/podman.rs:87-96`) and select and compare
   `tailscale == place.tailscale` in `write_recipe_state` (`tensorlake/build.rs:346-352`), with unit tests verifying
   both guards reject a toggled `tailscale` bit even when `(machine_id, uid)` is re-claimed.

Reviewer metadata (g45-tailscale-provisioning-podman/f5): suggested bucket: other (borderline highest: if a
pre-Tailscale unpinned SSH probe's cached `ready.proxy` values crossing into a newly Tailscale-pinned host's recipe
preparer container is counted as a trust-boundary crossing, this could be `highest`; surfacing explicitly per Priority
buckets); confidence: confirmed; possible cover: none (cross-group interface with g43-tailscale-store-and-access in
`crates/farhelm-helm/src/store/podman.rs:79-99` and `crates/farhelm-helm/src/tensorlake/build.rs:345-359`).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g43-tailscale-store-and-access/l4`,
`g35-podman-identity-probe/l4`, `g36-podman-identity-store-build/l1`, `g37-podman-retarget-probe-store/f1`,
`g45-tailscale-provisioning-podman/f5`.
