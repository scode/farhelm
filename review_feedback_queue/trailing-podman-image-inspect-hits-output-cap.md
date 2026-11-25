# Recipe image retirement still reads Podman's full image report

Reviewed commit: de6aaf3ae254c51cecd06feba47cdfcd58b6c379

## TLDR

Farhelm fixed container cleanup so a container with a lot of metadata no longer overflows the output limit on its
ownership check. Superseded recipe images are checked the same way before deletion, but that check still reads Podman's
whole image report. An image built on a base with many labels, a long environment or long layer history can make that
report exceed the limit, and then Farhelm can neither retire the old image nor finish the cleanup that depends on it
(Rebuild's image retirement and recipe Delete). The user has to remove the image by hand.

## Details

The commit projected `container inspect` through `--format` (`CONTAINER_INSPECTION`, `podman.rs:32-43`) because
`require` output passes the 64 KiB `MAX_CHILD_STREAM_BYTES` cap (`provisioning/backend.rs:106`).
`retire_images_unlocked` (`crates/farhelm-helm/src/environment/podman/build.rs:821-835`, ~824 at the tip) still runs
`podman image inspect -- <id>` unprojected and reads only `[0].Config.Labels.farhelm.helm`, `farhelm.recipe` and
`[0].Id`. A committed recipe image inherits its base's Config (Env, Labels) and carries History and RootFS.Layers, so
its report grows with the base's metadata and layer count. Over the cap, `require` fails, the loop returns an error, and
later retirements and the caller's work stop; recipe Delete's image cleanup (SPEC.md "Container sessions": "Recipe
deletion cleans up the place's owned images") cannot complete. The base check at `build.rs:984-1000` has the same
unprojected `image inspect`, so a large base would make a recipe build refuse with an output-limit error.

Fix: project both image inspections with `--format` (Id plus the specific `farhelm.*` labels via `index .Config.Labels`)
and update the image arms of the fakes in `failure_tests.rs`/`tests.rs`.

Suggested bucket: `other` (cleanup can be blocked and leave an image behind; no user work is lost). Confidence: likely.
Unverified after investigation: that a real committed recipe image's report exceeds 64 KiB (the original triage record
noted typical container output of a few to about 20 KB). Related: `trailing-podman-leftover-listing-hits-output-cap.md`
(same cap, different command and consequence).

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g06-podman-cleanup-checkout-names` record `f1`.
