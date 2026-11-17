# README hero screenshot: requirements

This directory is the design of the screenshot at the top of the README, which the docs website's landing page shows
too, and the requirements that govern how it is produced. The capture code lives under `e2e/readme-hero/` and the two
commands under `scripts/`; this file is what they are built to satisfy. The fleet staging (`e2e/readme-hero/stage.ts`)
is shared with the demo video (`docs/readme-video/SPEC.md`), so what this file says about staging governs both, and a
change to it changes both.

NOTE: This is not a visual regression fixture. Nothing in CI looks at the image, compares it, or fails when it drifts.
It exists so the README shows the real product at whatever version is checked out, and so an agent can refresh it
without inventing anything.

## The one rule

The design is fixed and the pixels are not. `scenario.json5` and `transcript.txt` say what the picture contains, and
they change only when the maintainer asks for a different picture. Everything else about the image, the chrome, the
layout, the colours, the exact shape of a status badge, is whatever the UI does at the version being captured, and it is
refreshed by re-running the capture, never by editing the image. An agent asked to "refresh the README screenshot" reads
this directory, runs the two commands below, and commits the resulting one-line URL change in each page. It does not
touch the scenario or the transcript unless the request is about the design.

## What is real and what is not

Everything visible comes from a real helm, real supervisors, and real terminals, with two deliberate exceptions.

The fleet is staged. The hosts are separate supervisors on the capturing machine, registered with the helm over ssh to
localhost under distinct destination spellings, and given the aliases the scenario names. The sessions run a fake agent
that replays `transcript.txt` and then behaves as the scenario's target status requires. The transcript imitates what a
coding agent's turn looks like; it is invented, not a capture of any real session, and the file says so at the top. No
real hostname, path, user name, or project from the capturing machine may appear in the picture, which is the reason the
fleet is staged rather than photographed.

Statuses are real. Every dot and badge in the list is the supervisor's own classification of a live pane, and the
capture waits for each session to reach the status the scenario asks for before taking the picture. A capture must not
rewrite statuses in transit; if a status will not settle, the fix is in the fake agent's behaviour or the wait, not in
the listing.

Two session fields are pinned to the scenario's values, and only those two. A fresh stack cannot honestly show a session
that was last active an hour ago, and the maintainer chose to set each session's last-activity stamp from the scenario
rather than add a clock seam to the supervisor. The working directory is the other: a session's directory has to exist
on the capturing machine, and the only directories that can be created there are under the run's temporary state, whose
path is exactly the kind of machine detail the picture must not show. So each session runs in a scratch directory and
the app shows the directory the scenario names.

The browser capture did this by intercepting the helm's session replies on their way to the page. The desktop app has no
such channel to intercept, since it talks to its embedded helm in memory, so the pinned values go through a debug-only
seam in the app instead (the maintainer's decision of 2026-11-16). The capture writes each session's age and directory
from the scenario into a file, an agent build of the app reads it at startup, and the app rewrites those two fields of
the named sessions as it takes in what its helm pushes, so the list's rows and the open session's details bar both show
them. The seam is compiled only into agent builds, which an optimized build refuses to compile, so no release build can
carry it. Nothing else is altered, statuses included. The ages count from the app's start, so they read older by however
long the capture took to stage the fleet (seconds, for this scenario).

The pinned stamps do not reorder the list. The desktop app's "Recently active" order is the helm's own: it puts running
and waiting sessions first and orders each group by when its sessions last started work, while the browser capture
re-sorted its rewritten listing by the pinned stamps alone. The scenario lists sessions in the order they should appear
with ages that agree, and the picture shows the app's order wherever the two differ: where the file puts an idle or
ended session above a running or waiting one, and inside each group wherever the staged sessions did not start work in
the file's order.

## Where the image lives and how it gets there

The PNG is never committed on main. It lives on the `readme-assets` branch as a single root commit holding a single
file, replaced wholesale on every refresh, and the README and the website's landing page
(`website/src/content/docs/docs/index.mdx`) both reference it by a raw URL pinned to that commit's hash. That keeps
main's history free of image blobs, keeps the two pages showing the same picture without a second copy of it, keeps the
branch itself from accumulating any, and keeps a checkout at an old tag pointing at the image that matched it for as
long as GitHub retains the unreferenced commit.

Publishing is scripted end to end and never done by hand, because a force-push is the one operation in this whole
arrangement that can destroy something if the wrong ref is named. `scripts/publish-readme-hero.sh` is the only thing
that knows the branch name and the only thing that pushes it. It works in a throwaway clone-free repository, names the
fully qualified ref explicitly, reads the remote from the checkout's `origin`, refuses anything that is not a PNG of the
scenario's declared size, and rewrites the URL between each page's markers: an HTML comment pair in the README, an MDX
comment pair on the website, since MDX rejects HTML comments. It checks both pairs before pushing and rewrites both or
neither, so the pages cannot end up on different images. An agent that finds itself typing `git push` with that branch
name has left the procedure and should stop.

## Refreshing the image

Prerequisites are the ones the browser suite has (a cargo build, a dx web build, Playwright with Chromium installed
under `e2e/`) plus passwordless ssh to `localhost` and to `$USER@localhost` on the capturing machine, and ImageMagick's
`identify` for the publish check.

- `scripts/readme-screenshot.sh` builds what is missing, stages the fleet, captures, and prints the PNG path under
  `target/readme-hero/`. Look at it.
- `scripts/publish-readme-hero.sh <png>` pushes it and rewrites both pages' URL. `--dry-run` first if in doubt.
- Commit the two-page change like any other change.

## Changing the design

Edit `scenario.json5` or `transcript.txt`, run the two commands, and commit the scenario change together with the pages'
URL change so the design and the picture move in one commit. The scenario's own comments say what each field does and
which values the capture can actually deliver.
