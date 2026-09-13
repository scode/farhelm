# README hero screenshot: requirements

This directory is the design of the screenshot at the top of the README, and the requirements that govern how it is
produced. The capture code lives under `e2e/readme-hero/` and the two commands under `scripts/`; this file is what they
are built to satisfy.

NOTE: This is not a visual regression fixture. Nothing in CI looks at the image, compares it, or fails when it drifts.
It exists so the README shows the real product at whatever version is checked out, and so an agent can refresh it
without inventing anything.

## The one rule

The design is fixed and the pixels are not. `scenario.json5` and `transcript.txt` say what the picture contains, and
they change only when the maintainer asks for a different picture. Everything else about the image, the chrome, the
layout, the colours, the exact shape of a status badge, is whatever the UI does at the version being captured, and it is
refreshed by re-running the capture, never by editing the image. An agent asked to "refresh the README screenshot" reads
this directory, runs the two commands below, and commits the one-line README change. It does not touch the scenario or
the transcript unless the request is about the design.

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

Two row fields are rewritten in transit, and only those two. A fresh stack cannot honestly show a session that was last
active an hour ago, and the maintainer chose to have the capture intercept the session listing and set each row's
last-activity stamp from the scenario rather than add a clock seam to the supervisor. The working directory is the
other: a session's directory has to exist on the capturing machine, and the only directories that can be created there
are under the run's temporary state, whose path is exactly the kind of machine detail the picture must not show. So each
session runs in a scratch directory and the listing is rewritten to show the directory the scenario names. Nothing else
in that reply is altered. Because the list orders by recent activity, the ages also decide the row order, so the
scenario lists sessions in the order they should appear with ages that agree.

## Where the image lives and how it gets there

The PNG is never committed on main. It lives on the `readme-assets` branch as a single root commit holding a single
file, replaced wholesale on every refresh, and the README references it by a raw URL pinned to that commit's hash. That
keeps main's history free of image blobs, keeps the branch itself from accumulating any, and keeps a checkout at an old
tag pointing at the image that matched it for as long as GitHub retains the unreferenced commit.

Publishing is scripted end to end and never done by hand, because a force-push is the one operation in this whole
arrangement that can destroy something if the wrong ref is named. `scripts/publish-readme-hero.sh` is the only thing
that knows the branch name and the only thing that pushes it. It works in a throwaway clone-free repository, names the
fully qualified ref explicitly, reads the remote from the checkout's `origin`, refuses anything that is not a PNG of the
scenario's declared size, and rewrites the README between its markers. An agent that finds itself typing `git push` with
that branch name has left the procedure and should stop.

## Refreshing the image

Prerequisites are the ones the browser suite has (a cargo build, a dx web build, Playwright with Chromium installed
under `e2e/`) plus passwordless ssh to `localhost` and to `$USER@localhost` on the capturing machine, and ImageMagick's
`identify` for the publish check.

- `scripts/readme-screenshot.sh` builds what is missing, stages the fleet, captures, and prints the PNG path under
  `target/readme-hero/`. Look at it.
- `scripts/publish-readme-hero.sh <png>` pushes it and rewrites the README's URL. `--dry-run` first if in doubt.
- Commit the README change like any other change.

## Changing the design

Edit `scenario.json5` or `transcript.txt`, run the two commands, and commit the scenario change together with the
README's URL change so the design and the picture move in one commit. The scenario's own comments say what each field
does and which values the capture can actually deliver.
