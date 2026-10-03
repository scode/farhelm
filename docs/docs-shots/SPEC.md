# Docs screenshots: requirements

This directory is the design of the annotated screenshots the docs website shows, and the requirements that govern how
they are produced. The capture code lives under `e2e/docs-shots/` (one spec per docs page, one test per shot), the
commands are `scripts/docs-screenshots.sh` and `scripts/publish-docs-shots.sh`, and pages show a shot with the
`Screenshot` component in `website/src/components/`. The fleet staging is the README hero's
(`docs/readme-hero/SPEC.md`), so what that file says about staging governs these shots too, except where this file lists
an addition.

NOTE: This is not a visual regression fixture. Nothing in CI captures the images, compares them, or fails when they
drift. They exist so the docs show the real product at whatever version was last captured, and so an agent can refresh
them without inventing anything.

## Why screenshots, and who owns them

The docs show where to click with a real screenshot, annotated with red arrows and short labels, instead of narrating
the clicks (`website/EDITORIAL_RULES.md`, "Rules from feedback"). The shots belong to the pages: an agent writing a page
writes its shots, including the annotation wording, and changes them with the page. That differs from the README hero,
whose design only the maintainer changes. The scenario here is likewise the docs' own and may grow when a shot needs
something the fleet lacks, as long as the existing shots still show what their pages say.

## What is real and what is not

Everything visible comes from a real helm, real supervisors, and real terminals, with the hero's two exceptions (the
staged fleet, and each row's age and folder rewritten in transit) and three more, all of them in transit, all of them
from `scenario.json5`, and no others:

- **Launch history.** The session launcher's recent setups come from launches it remembers. A fresh stack remembers
  none, and staged sessions are not launches made from the launcher, so the history reply carries the scenario's
  `launch_history` instead. The model catalog the launcher checks those setups against is the real one.
- **Recent folders.** The folders offered under the launcher's folder field come from the same reply, from the
  scenario's `folders`.
- **Host destinations.** Host settings and details show each host's ssh destination. The staged remotes are self-ssh
  spellings on the capturing machine, and one of them names its account, which must never be photographed, so every host
  reply shows each remote's `shown_ssh` instead.

Statuses are never rewritten, as for the hero. Before writing each PNG the capture also checks the page's visible text
and form values for the capturing account's name, its home directory, and the run's temporary state, and fails if any
appears: the rewrites are supposed to keep those out, and the check is what proves they did.

## Where the images live

The PNGs are never committed on main. Each publish builds a snapshot, a root commit holding every shot the docs pages
reference, and points `refs/docs-assets/keep` at a keeper commit whose parents are the new snapshot and every earlier
snapshot replaced less than six weeks ago, plus the one main's manifest pins, whatever its age.
`website/src/data/docs-shots.json` pins the newest snapshot, records which main commit its capture ran on
(`captured_from`) and when (`captured_at`), and records each shot's size. Pages load each image from that snapshot's raw
GitHub URL.

The maintainer chose that shape for three reasons:

- **Old images are garbage.** A snapshot that falls out of the six-week window is reachable from nothing, so GitHub may
  collect it, which is the point: images nobody needs any more should not accumulate anywhere.
- **Recent tags keep working.** A tag pins whichever snapshot was current when it was cut, and that snapshot stopped
  being current only when the next one was published. Counting the six weeks from that replacement, not from the
  snapshot's own publish date, is what keeps the docs at any release tag from the last six weeks showing their images
  when screenshots are republished rarely.
- **Clones stay small.** The ref is outside `refs/heads/`, so clones never fetch it: `jj git fetch` and a default
  `git fetch` only pull branches. A branch would have put every retained snapshot into every checkout.

Publishing is scripted end to end. `scripts/publish-docs-shots.sh` is the only thing that knows the ref and the only
thing that pushes it. It pushes with a lease on the keeper it read, so a concurrent publish fails instead of being
overwritten, and it writes the manifest only after GitHub serves the new snapshot. Two changes that both moved the
manifest conflict on it; resolve that by rebasing onto the newer one and capturing and publishing again, never by
editing the manifest. An agent that finds itself typing a push to that ref has left the procedure and should stop.

## Regenerated in bulk

Every publish is a complete regeneration: all pages' shots from one run of `scripts/docs-screenshots.sh`, captured
against one main commit. The capture writes a record of that run (`capture.json` beside the shots) naming the commit and
the checksum of every PNG, and only a complete run that can vouch for showing main's UI writes it. `--only <page>`,
which is for drafting a page, deletes it, and so does a full run whose app code (`crates/`, `Cargo.toml`, `Cargo.lock`)
differs from main, one with `--no-build`, or one whose builds go to a `CARGO_TARGET_DIR` outside the checkout; such a
full run still captures for the preview but exits 3, and says so before it starts. The publish script refuses shots
without a complete record, a page referencing a shot the run did not produce, and a shot that changed after the run
recorded it, so a published set never mixes captures from different versions of the UI.

The maintainer asked for that so a refresh can be reasoned about: the manifest says exactly which version of the UI the
published shots show, and the changes since then are a `git log` away.

## Adding or changing a shot

A shot is a test in `e2e/docs-shots/<page>.spec.ts` named after the page. It opens the staged fleet, drives the UI to
the state it shows without launching sessions or changing settings, draws annotations with the docs overlay theme, and
calls `shot(page, "<page>/<shot>", [regions])`. The image is cropped to the regions plus every annotation on screen.

- `scripts/docs-screenshots.sh --only <page>` captures that page's shots into `website/public/docs-shots-local/`, and
  the local docs preview shows them straight away, labelled as local captures. Look at every one: the arrow points at
  what the label says, the label covers nothing the reader needs, and nothing from the capturing machine shows. A
  one-page capture cannot be published.
- The page shows a shot with `<Screenshot name="<page>/<shot>" alt="…" />` (the page must be `.mdx`). The alt text says
  what the shot points at, for readers who cannot see it.
- To publish, run `scripts/docs-screenshots.sh` with no `--only` on a checkout based on the latest main, look at every
  page's shots, then run `scripts/publish-docs-shots.sh`. Commit the manifest change with the page.

## Refreshing

"Refresh the docs screenshots" means, on a checkout of the latest main:

1. Read `captured_from` in `website/src/data/docs-shots.json` and look at what changed in the app since then:
   `git log --oneline <captured_from>..origin/main -- crates/ Cargo.toml Cargo.lock` for the overview (the same paths
   the capture checks; the UI's wording also comes from the shared protocol crate, and a dependency bump in the lockfile
   can change what renders), then the diffs of anything that touches a screen a page shows. Note what a shot or a page
   will need: a renamed control, a new option, a moved menu, changed wording in a dialog. A label that changed in the UI
   can make a page's prose wrong too.
2. Update the shot specs for those changes and run `scripts/docs-screenshots.sh`. When a shot fails because the UI moved
   under it (a selector that no longer matches, a control now behind a menu), rewrite its steps so it shows the same
   thing on the current UI; change its annotation wording only when what it explains changed.
3. Look at every image, and fix any page whose text no longer matches what its shots show.
4. Run `scripts/publish-docs-shots.sh`, and commit the manifest with any page or spec changes.

When nothing in the UI changed since `captured_from`, say so and ask whether to regenerate anyway.

## Prerequisites

The hero's: a cargo build, a dx web build, Playwright with Chromium installed under `e2e/`, and passwordless ssh to the
scenario's remote destinations (`localhost` and `$USER@localhost`). The publish script also needs `file`, `curl`, and
network access to GitHub; its `--self-test` needs ImageMagick's `convert` and no network.
