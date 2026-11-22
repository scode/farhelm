# The New container golden settles with its build status still loading

Reviewed commit: 30441792a456f98227643624e65ae76620a5fb64

## TLDR

The new-container golden settles on a loading status because its fixture never supplies the build result.

## Details

**other.** Source inspection.

30441792a456f98227643624e65ae76620a5fb64 crates/gui/farhelm-gui/src/goldens/launcher.rs:39-63,164-182 and
crates/gui/farhelm-gui/src/launcher/view/placement.rs:214-267; 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/goldens/launcher.rs:119-132,246-258,549-568

the selected-container scene can finish staging while saying “Checking build status…”, although the fake has no way to
finish that check. The snapshot adds Development under Recipes, with a usable default host and no RecipePlaces row; the
scene stops staging on the recipe value alone. At the pinned tip goldens/mod.rs:278-303 connects exactly the listed
collections, and crates/farhelm-helm-client/src/fake.rs:84-115,128-161 sends only the supplied snapshot and explicitly
pushed changes; command replies are completions and do not synthesize collection changes. launcher/view.rs:2277-2291
mounts the status next to Container host, and launcher/view/placement.rs:228-255 picks the checking branch for that
usable-host/missing-row combination. goldens/mod.rs:1537-1569 accepts stable pixels after staging stops.
SPEC.md:1010-1017 describes the ready and first-launch note for normal use. The scene caption
(docs/gpui/screenshot-scenes.json:2827-2829) describes a selected container, with no loading qualifier.
launcher/environment_view_tests.rs:356-390 checks the text transition, so the missing coverage is the appearance of the
settled launcher note. goldens/settings.rs:123-135 offers the existing pattern for seeding place rows. Seed this
recipe's place and require the drawn note before finishing, or name the reference as a build-status loading state.
Different from LV-13 or the container deletion warning. The reviewer suggested other (maintainer visualization),
confirmed fixture/renderer branch from source. Actual reference and rendered pixels unverified. ; for the visual gap.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-golden-scenes/LG-08`.
