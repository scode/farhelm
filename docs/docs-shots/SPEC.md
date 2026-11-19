# Docs screenshots: requirements

The docs screenshot runner captures regions of the real GPUI desktop app against a staged fleet. The engine is
`scripts/docs_shots.py`, page modules live under `scripts/docs-shots/`, and the commands are
`scripts/docs-screenshots.sh` and `scripts/publish-docs-shots.sh`. Pages show a shot with the `Screenshot` component in
`website/src/components/`.

NOTE: Use the release-notes module for new release shots. CI leaves capture and image review to the docs and release
procedures.

## Why screenshots, and who owns them

The docs use real screenshots to show where to click, annotated with red callouts and arrows
(`website/EDITORIAL_RULES.md`, "Rules from feedback"). Shots belong to their pages and change with them, annotation
wording included: an agent writing a page writes its shots' callouts too. The scenario here likewise belongs to the docs
and may grow when a shot needs something the fleet lacks, as long as existing shots still show what their pages say.

The annotations are drawn onto the picture after capture (`scripts/docs_callouts.py`), from the bridge's boxes of the
elements they point at, in the old browser overlay's look and with its placement rules, so the desktop app's pictures
match the shots the browser UI had. A callout box that would cover an earlier one moves out of its way. Only the
annotations are drawn: every other pixel is the capture's.

## What is real and what is not

The fleet staging is the README hero's (`docs/readme-hero/SPEC.md`): real helm, supervisors and terminals, with fake
agents replaying invented transcripts. Statuses are the supervisor's real readings. The app pins the scenario's ages and
directories on the staged sessions so the run's private paths do not enter the picture. `gui-agent.py start --scenario`
owns that staging and its private sshd; nothing uses a live installation.

The docs engine also stores the scenario's launch templates through the helm's real `PUT /api/templates/<name>` API. A
template's host key becomes that host's install identity from `GET /api/hosts`. No shot saves or deletes a template.
Other scenario declarations for pages not yet ported are not staged by this engine.

Before each shot the engine presses Escape until no layer remains, giving up after six presses, and hides the Hosts
panel, which is not a layer. The page's steps then reach the state to photograph; a shot whose steps start, replace or
delete a session fails, since the helm's session list is compared before and after. Every shot uses the scenario's
viewport and dark appearance, in one shared app run. The harness's `capture --keep-focus` parks the pointer, waits for
idle, checks for machine details before and after the screenshot, crops the Linux window frame and checks the full
picture's size. The Linux engine asks its private Mutter for the scenario's scale so GPUI renders the 1600x1500-point
viewport as 3200x3000 pixels. Capture leaves focus alone. Only a passing capture is cropped to the union of the page's
region boxes, padded by 16 logical pixels, clamped to the window and scaled to image pixels. A failed shot writes no
output and the engine continues to the next; `stop` runs at the end even after a failure.

## Where the images live

PNGs are never committed on main. Captures go to the gitignored `website/public/docs-shots-local/<page>/<shot>.png`,
where the docs preview can show them. A failed run prints its run directory, retained with its full capture and failure
messages.

The existing publisher builds a snapshot, a root commit holding every shot the docs reference, and points
`refs/docs-assets/keep` at a keeper whose parents are the new snapshot, snapshots replaced less than six weeks ago, and
the snapshot main's manifest pins, whatever its age. `website/src/data/docs-shots.json` pins the snapshot, records
`captured_from` and `captured_at`, and records each shot's size. Pages load images from that snapshot's raw GitHub URL.

This keeps clones small (the ref is outside `refs/heads/`), lets old images become unreachable, and preserves images for
recent release tags for six weeks after their snapshot was replaced. The publisher reads main's manifest from the remote
too, so a checkout behind main cannot prune its images. It pushes with a lease and writes the manifest only after GitHub
serves the snapshot. Only `scripts/publish-docs-shots.sh` pushes the ref; never push it by hand or resolve a manifest
conflict by hand-editing it.

## Regenerated in bulk

Publishing requires every page's shots from one complete capture against one main commit. Published release shots are
carried over unchanged. A complete run writes `capture.json` beside the shots, naming the commit and each PNG's
checksum. `--only <page>` deletes that record, as does a full run whose app code (`crates/`, `Cargo.toml`, `Cargo.lock`)
differs from main or which used `--no-build`. Such a full run still captures for preview but exits 3 and explains why.
The harness asks cargo where it builds, so `CARGO_TARGET_DIR` does not affect this decision.

The publisher refuses a partial record, a referenced shot the run did not produce, or a shot whose bytes changed after
capture. This preserves one identifiable UI version across the published set.

## Release shots are frozen

A release shot is frozen when `website/src/release-notes/*.json` lists it and `website/src/data/docs-shots.json` records
it. The capture skips it even if its steps are still declared. The publisher carries its bytes over from the pinned
snapshot and refuses a publish if the image is missing. Today's UI may no longer match the old release.

For a deliberate correction, make sure the shot has steps in `scripts/docs-shots/release-notes.py`, then run
`scripts/docs-screenshots.sh --local --recapture-release-shot release-notes/v<X-Y-Z>-<what>`. The full run records the
exception in `capture.json`. Add `--only release-notes` to preview a correction before the full run.

## Adding or changing a shot

Create `scripts/docs-shots/<page>.py`, import `Shot` (and `Callout`, `Highlight`) with
`from docs_shots import Shot, Callout, Highlight`, and define `SHOTS`, a list of
`Shot(name, steps, regions, park=None, annotations=[], max_width=None, max_height=None)` values. `name` is lowercase
kebab case and unique within the page. `steps` is a function taking the engine's bridge facade (`click`, `fill`,
`type_into_focused`, `press`, `wait`, `wait_text`, `hover`, `scroll_into_view`); `regions` is a non-empty list of stable
`id:` targets. The engine refuses missing SHOTS, bad or duplicate names, empty or non-id regions, and malformed
annotations before starting the app. Use the element IDs in the GPUI source (`.id(...)`; a `debug_selector` name is not
a target the bridge can see), and wait for the state the shot needs rather than adding delays. Steps never launch a
session or change a setting. The optional `park` is an `id:` target for a static spot that has no hover or tooltip
reaction and stays clear of overlays; capture hovers it directly instead of searching.

A target may name a staged session or host by its scenario name, `{session:<title>}` or `{host:<key>}`, wherever the
facade, a region, a park spot or an annotation takes one: the app's ids for rows and rail entries carry the helm's
session key or host id, which every run mints anew (`id:session-item-{session:fix flaky attach test}`).

`annotations` lists what is drawn on the picture: `Callout(target, text, side="auto", dx=0, dy=0)` puts a box of `text`
beside the target on `side` (`left`, `right`, `top`, `bottom`, or `auto` for the side with most room), shifted by `dx`
and `dy` logical pixels, with an arrow to the target; `Highlight(target, pad=3)` rings it. Say as much as the picture
can carry in the callouts, per the editorial rules. The crop always grows to keep every annotation whole. `max_width`
and `max_height` cap the crop in logical pixels, keeping its top left, for a region much larger than what matters in it,
such as the terminal or the session list, which run to the window's foot.

Run `scripts/docs-screenshots.sh --local --only <page>` to develop that page. The engine resets layers before its steps,
then captures and crops each shot. Inspect every successful image for the intended controls and machine details. A
one-page capture cannot be published. Pages reference a shot with `<Screenshot name="<page>/<shot>" alt="…" />` in MDX;
the alt text explains what the image shows.

For release notes, add a `Shot` named `v<X-Y-Z>-<what>` to `scripts/docs-shots/release-notes.py`. List its full name,
`release-notes/v<X-Y-Z>-<what>`, in `website/src/release-notes/vX.Y.Z.json`. For example, `v1-2-3-new-control` goes in
`v1.2.3.json`. The listing also supplies the entry's PR number and alt text. Draft with `--only release-notes`, then run
the full capture for publication. Add invented hosts, sessions or templates to `docs/docs-shots/scenario.json5` as
needed, preserving the existing shots. For state the engine cannot stage yet, add the scenario fields and seed them
before the shot steps run. Follow `stage_templates` in `scripts/docs_shots.py`, which calls the private helm's API.

## Refreshing

Run `scripts/docs-screenshots.sh --local` on Linux for the full 2x capture and its `capture.json`. Use `--only <page>`
to work on one page.

For a bulk refresh, read UI changes since the manifest's `captured_from`, update the affected modules and page wording,
capture every page, inspect the pictures, then publish and commit the manifest with the page changes. Published release
material stays frozen.

## Prerequisites

The GPUI harness's build and runtime tools are listed in `docs/gui-agent.md`. Linux needs headless Mutter and lavapipe;
staged hosts use the run's private OpenSSH server, without passwordless ssh to the machine itself. Drawing annotations
needs `rsvg-convert` (librsvg). The publisher also needs `file`, `curl` and GitHub access; its self-test needs
ImageMagick's `convert` and no network.
