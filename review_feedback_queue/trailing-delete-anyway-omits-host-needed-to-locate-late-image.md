# Delete anyway omits the host needed to locate a late image

Reviewed commit: f2b3b5b9c4be17779dbdce517c1a7af04c9f4eaa

## TLDR

With several container hosts, even a warning for exactly one capture gives the user no location for the cleanup it says
might be needed.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/recipes/editor/leaving.rs:253`.

The observation is stored under a real place as its tag (environment/recipes.rs:39–71.
Environment/podman/build.rs:769–775), but tensorlake/recipes.rs:709–720 returns a bare marker and generic words, and the
editor substitutes a warning that names the recipe but only says “a container host” (leaving.rs:250–259,333–341). On
confirmed absence build.rs:777–812 removes the tag and the place record when cleanup succeeds.
Tensorlake/recipes.rs:699–705 deletes the recipe and sends generic success. The existing Settings → Containers recovery
surface returns ContainerLeftoverView from container discovery (podman.rs:825–886), so that surface cannot later give
the omitted location for an image. SPEC.md:509–511 describes the warning about cleanup at the container host, and
:5328–5335 gives Settings a list specifically for containers. The product does not explicitly require displaying the
host name or say that a late image itself will be listed. This is a source-supported diagnostic/usability gap, separate
from f1’s extra-tag consent and f2’s separate cleanup error. Carry the already-known place into the warning or retain a
discoverable cleanup reference. Bucket: other. The conditional later image is a generated artifact. Confidence:
confirmed omission by source, no late appearance observed. Possible coverage:
an_unconfirmed_recipe_capture_requires_delete_anyway checks the generic risk sentence and button labels, not cleanup
location. Both revisions have the same warning and success path, and podman.rs:1484–1509 limits discovery to container
listing and inspection. The tip keeps the product warning at SPEC.md:509–511 and describes the container-list-only
surface at :5340–5347.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recipes/f15`.
