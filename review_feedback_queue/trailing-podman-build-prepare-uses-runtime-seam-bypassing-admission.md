# Podman image build and preparation stages use the runtime probe seam instead of fresh admission

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When a container host's ordinary Farhelm connection becomes refused (duplicate, changed, or unverified installation)
after a launch or rebuild begins, later build and preparation stages still start new image-building containers on that
host instead of stopping new work.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman/build.rs:162`.

SPEC.md ('Container sessions', lines 5289-5290) states: 'A Farhelm connection refusal for a duplicate, changed or
unverified installation also prevents new work through that host. Such an admission refusal still allows existing
containers to use the place's command route.' SPEC_impl.md ('Podman places, creation and images', lines 6245-6247)
states: 'Fresh preparation requires a current place claim and respects the ordinary host actor's duplicate, mismatch and
unverified identity refusals. Runtime capabilities for existing containers remain separate from admission.' In
`crates/farhelm-helm/src/environment/podman/build.rs`, both `Podman::software` (line 162) and
`Podman::prepare_images_owned` (line 365) acquire `(expected, _, ready)` by calling `self.place(place).await?`
(`crates/farhelm-helm/src/environment/podman.rs:223-238`), which calls `self.0.places.runtime(&self.0.store, host)`
(`podman_probe.rs:630-671`). `PodmanPlaces::runtime` is explicitly the existing-container runtime seam: it returns any
cached `Ok(ReadyPlace)` for the route while ignoring `FRESH_FOR` (300s), `CachedProbe.admission_refusal`, and
`ordinary_identity_refusal` (`Duplicate`, `IdentityMismatch`, `IdentityUnverified`). Two concrete source-verified paths
allow new helper containers to start on a refused host:

1. Multi-stage build/prepare (`build.rs:162`, `build.rs:365`): `software()` installs packages, Claude Code, and Codex
   (routinely exceeding `FRESH_FOR`). If the host's ordinary actor enters `Duplicate`, `IdentityMismatch`, or
   `IdentityUnverified` (or a refresh runs and records `admission_refusal = Some(CLAIM_NEEDS_REFRESH)` per
   `podman_probe.rs:824`, which leaves the prior row in `podman_place_claims` intact), `prepare_images_owned` at line
   365 calls `self.place(place)` (`runtime()`), ignores the refusal, passes `has_podman_place_claim` in `edit_at`,
   starts the `prepare` helper, and publishes the prepared image.
2. Lazy preparation loop (`podman.rs:1214-1262` and `build.rs:291-293`): `Podman::ensure_prepared` calls
   `places.fresh()` once at `podman.rs:1214` before `loop { ... }`, and `prepare_images` (`build.rs:291`) only calls
   `places.fresh()` when `rebuild == true`. If `ensure_prepared` waits on `lock_podman_use` or waits for another job in
   the loop and then starts `prepare_images(..., rebuild = false)` after the host became refused (while
   `podman_place_claims` still holds the old claim), neither `prepare_images` nor `software()` rechecks admission before
   pulling bases and launching helpers. Fix direction: enforce admission (e.g., `places.fresh()` /
   `confirm_cached_admission` paired with the current `HostRow`) inside `prepare_images` unconditionally (not only
   `if rebuild`) and before starting new `build` / `prepare` helpers in `software()` and `prepare_images_owned()`.

Reviewer metadata (g36-podman-identity-store-build/l2): suggested bucket: other (could NOT be in highest at 66ad3aaa:
VERIFY_PLACE_AND_EXEC in PlaceTarget::claimed still pins pull/run/commit to the claimed machine_id and uid, and image
build/prepare sends no user credentials; established consequence is running new image-build/prepare work on a host whose
ordinary Farhelm connection is refused as Duplicate, IdentityMismatch, or IdentityUnverified, or whose probe is stale);
confidence: confirmed; possible cover: none.

Additional source record at `86be6c582492cddb7413e5a176d84bb29eb76da1`,
`crates/farhelm-helm/src/environment/podman/build.rs:162-167` (g38-podman-retarget-build-tests/l5):

In `crates/farhelm-helm/src/environment/podman/build.rs:162-167` (`Podman::software`) and
`crates/farhelm-helm/src/environment/podman/build.rs:365-370` (`Podman::prepare_images_owned`), new helper creation
obtains `ready` via `self.place(place).await?` (`podman.rs:223-238`), which calls
`self.0.places.runtime(&self.0.store, place)`. Unlike `PodmanPlaces::fresh` (`podman_probe.rs:675-715`),
`PodmanPlaces::runtime` (`podman_probe.rs:630-671`) ignores `FRESH_FOR` (300s), ignores `probe.admission_refusal`, and
ignores `ordinary_identity_refusal` (`HostState::Duplicate`, `IdentityMismatch`, `IdentityUnverified`). While
`rebuild_recipe` calls `places.fresh` once at `build.rs:292` and `ensure_prepared` calls `places.fresh` once before its
loop at `podman.rs:1214`: (1) `software()` installs apt packages, Claude Code, and Codex (`IMAGE_WORK = 1800s`),
routinely exceeding `FRESH_FOR` (300s), after which `prepare_images_owned` at `build.rs:365` launches the `prepare`
helper using `self.place(place)` without rechecking `FRESH_FOR` or `ordinary_identity_refusal`; (2)
`prepare_images(..., rebuild = false)` skips `places.fresh` (`if rebuild` at `build.rs:291`), so any second loop
iteration of `ensure_prepared` (`podman.rs:1215-1262`, e.g. after waiting on a joined job or lock) and any
`ordinary_identity_refusal` arriving between `ensure_prepared:1214` and `software:162` is never checked before starting
helpers; (3) `helper()` -> `edit_at` (`tensorlake/build.rs:353-358`) checks only `has_podman_place_claim`, and when the
host is on the same `(machine_id, uid)` with a `Duplicate`, `IdentityMismatch` (reinstalled supervisor), or
`IdentityUnverified` ordinary state, both `has_podman_place_claim` and `VERIFY_PLACE_AND_EXEC` pass. This violates
`SPEC.md:5289-5290` ("A Farhelm connection refusal for a duplicate, changed or unverified installation also prevents new
work through that host") and `SPEC_impl.md:6245-6247` ("Fresh preparation requires a current place claim and respects
the ordinary host actor’s duplicate, mismatch and unverified identity refusals"). Fix direction: gate new helper
creation in `software()` and `prepare_images_owned()` (and/or inside `ensure_prepared`’s loop / `prepare_images` before
starting helpers) through `places.fresh` (or an admission-checked helper on `Podman` that enforces
`ordinary_identity_refusal` and `FRESH_FOR`). Independently identified by `g36/l2`.

Reviewer metadata (g38-podman-retarget-build-tests/l5): suggested bucket: other (cannot be in highest:
`VERIFY_PLACE_AND_EXEC` and `has_podman_place_claim` still prevent targeting a different `(machine_id, uid)` store;
bypassing ordinary-supervisor identity refusals or `FRESH_FOR` starts a recipe helper on a same-machine/UID duplicate or
reinstalled-supervisor host without losing user data, credentials, or user-owned work); confidence: confirmed; possible
cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g36-podman-identity-store-build/l2`,
`g38-podman-retarget-build-tests/l5`.
