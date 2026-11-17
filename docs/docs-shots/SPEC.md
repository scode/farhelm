# Docs screenshots: requirements

The docs screenshot runner captures regions of the real GPUI desktop app against a staged fleet. The engine is
`scripts/docs_shots.py`, page modules live under `scripts/docs-shots/`, and the commands are
`scripts/docs-screenshots.sh` and `scripts/publish-docs-shots.sh`. Pages show a shot with the `Screenshot` component in
`website/src/components/`.

NOTE: Capture is local only until the Mac path lands with the first published page. Only Launch templates has been
ported, with two unannotated crops; no page is published yet. Linux runs exercise staging and capture checks, then stop
at the size check because headless Mutter renders at scale 1 and the scenario requires scale 2. This is not a visual
regression fixture: nothing in CI captures or compares the images.

## Why screenshots, and who owns them

The docs use real screenshots to show where to click (`website/EDITORIAL_RULES.md`, "Rules from feedback"). Shots belong
to their pages and change with them. The scenario here likewise belongs to the docs and may grow when a shot needs
something the fleet lacks, as long as existing shots still show what their pages say. The current runner adds no
annotation overlay.

## What is real and what is not

The fleet staging is the README hero's (`docs/readme-hero/SPEC.md`): real helm, supervisors and terminals, with fake
agents replaying invented transcripts. Statuses are the supervisor's real readings. The app pins the scenario's ages and
directories on the staged sessions so the run's private paths do not enter the picture. `gui-agent.py start --scenario`
owns that staging and its private sshd; nothing uses a live installation.

The docs engine also stores the scenario's launch templates through the helm's real `PUT /api/templates/<name>` API. A
template's host key becomes that host's install identity from `GET /api/hosts`. No shot saves or deletes a template.
Other scenario declarations for pages not yet ported are not staged by this engine.

Before each shot the engine presses Escape until no layer remains, giving up after six presses. The page's steps then
reach the state to photograph. Every shot uses the scenario's viewport and dark appearance, in one shared app run. The
harness's `capture --keep-focus` parks the pointer, waits for idle, checks for machine details before and after the
screenshot, crops the Linux window frame and checks the full picture's size. It leaves the shot's focus alone. Only a
passing capture is cropped to the union of the page's region boxes, padded by 16 logical pixels, clamped to the window
and scaled to image pixels. A failed shot writes no output and the engine continues to the next; `stop` runs at the end
even after a failure.

## Where the images live

PNGs are never committed on main. Local captures go to the gitignored
`website/public/docs-shots-local/<page>/<shot>.png`, where the docs preview can show them. A failed run prints its run
directory, retained with its full capture and failure messages.

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
capture. This preserves one identifiable UI version across the published set. Publishing remains out of scope until the
Mac path lands.

## Release shots are frozen

A release shot is one listed by `website/src/release-notes/*.json` and recorded as published in the manifest. It stays
byte-identical in later publishes, copied from the pinned snapshot; recapturing from today's UI would misrepresent the
release. A missing pinned image makes the publisher refuse rather than drop it.

The command retains validation of `--recapture-release-shot <name>` but refuses the capture: no page module captures
release shots yet. Deliberate corrections and first captures for new releases need that support before they can run.

## Adding or changing a shot

Create `scripts/docs-shots/<page>.py`, import `Shot` with `from docs_shots import Shot`, and define `SHOTS`, a list of
`Shot(name, steps, regions, park=None)` values. `name` is lowercase kebab case and unique within the page. `steps` is a
function taking the engine's bridge facade (`click`, `fill`, `press`, `wait`, `wait_text`); `regions` is a non-empty
list of stable `id:` targets. The engine refuses missing SHOTS, bad or duplicate names, and empty or non-id regions
before starting the app. Use the element IDs in the GPUI source, and wait for the state the shot needs rather than
adding delays. Steps never launch a session or change a setting. The optional `park` is an `id:` target for a static
spot that has no hover or tooltip reaction and stays clear of overlays; capture hovers it directly instead of searching.

Run `scripts/docs-screenshots.sh --local --only <page>` to develop that page. The engine resets layers before its steps,
then captures and crops each shot. Inspect every successful image for the intended controls and machine details. A
one-page capture cannot be published. Pages reference a shot with `<Screenshot name="<page>/<shot>" alt="…" />` in MDX;
the alt text explains what the image shows.

## Refreshing

Until the Mac capture path lands, use local runs to port and check page modules; a full published refresh is
unavailable. When it becomes available, the bulk publishing contract above still applies: read UI changes since the
manifest's `captured_from`, update the affected modules and page wording, capture every page, inspect the pictures, then
publish and commit the manifest with the page changes. Published release material stays frozen.

## Prerequisites

The GPUI harness's build and runtime tools are listed in `docs/gui-agent.md`. Linux needs headless Mutter and lavapipe;
staged hosts use the run's private OpenSSH server, without passwordless ssh to the machine itself. The publisher also
needs `file`, `curl` and GitHub access; its self-test needs ImageMagick's `convert` and no network.
