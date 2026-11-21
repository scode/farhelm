# README hero screenshot: requirements

This directory is the design of the screenshot at the top of the README, which the docs website's landing page shows
too, and the requirements that govern how it is produced. The staging and the capture live in `scripts/gui-agent.py`
(`start --scenario` and `capture`, described in `docs/gui-agent.md`, "Scenario runs" and "Scenario captures") and the
two commands under `scripts/`; this file is what they are built to satisfy. The fleet staging is shared with the demo
video (`docs/readme-video/SPEC.md`), so what this file says about staging governs both, and a change to it changes both.

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
localhost under distinct destination spellings (reached through a private sshd the run starts, not the machine's own ssh
setup), and given the aliases the scenario names. The sessions run a fake agent that replays `transcript.txt` and then
behaves as the scenario's target status requires. The transcript imitates what a coding agent's turn looks like; it is
invented, not a capture of any real session, and the file says so at the top. No real hostname, path, user name, or
project from the capturing machine may appear in the picture, which is the reason the fleet is staged rather than
photographed.

Statuses are real. Every dot and badge in the list is the supervisor's own classification of a live pane, and the
capture waits for each session to reach the status the scenario asks for before taking the picture. A capture must not
rewrite statuses in transit; if a status will not settle, the fix is in the fake agent's behaviour or the wait, not in
the listing.

Two session fields are pinned to the scenario's values, and only those two of a session's. A fresh stack cannot honestly
show a session that was last active an hour ago, and the maintainer chose to set each session's last-activity stamp from
the scenario rather than add a clock seam to the supervisor. The working directory is the other: a session's directory
has to exist on the capturing machine, and the only directories that can be created there are under the run's temporary
state, whose path is exactly the kind of machine detail the picture must not show. So each session runs in a scratch
directory and the app shows the directory the scenario names.

The browser capture did this by intercepting the helm's session replies on their way to the page. The desktop app has no
such channel to intercept, since it talks to the helm embedded in the same process, so the pinned values go through a
debug-only seam in the app instead (the maintainer's decision of 2026-11-16). The capture writes each session's age and
directory from the scenario into a file, an agent build of the app reads it at startup, and the app rewrites those two
fields of the named sessions as it takes in what its helm pushes, so the list's rows and the open session's details bar
both show them. The seam is compiled only into agent builds, which an optimized build refuses to compile, so no release
build can carry it. The ages count from the app's start, so they read older by however long the capture took to stage
the fleet (seconds, for this scenario).

The same file carries a block of values per scenario host, matched by the host's alias, which the maintainer added to
the seam on 2026-11-18 for the docs screenshots (`docs/docs-shots/SPEC.md`): the launch history behind the launcher's
recent setups and recent folders, the checkout trash, and the answers to a terminal's file checks. An agent build shows
those in place of what each staged host would answer, and never asks the host for its trash or a file, nor sends it a
trash delete. Every scenario host gets the block, an empty one when the scenario stages nothing for it, as this one
does. That matters here too: the helm records each staged session's launch in its scratch directory, so without the
block the launcher's recent folders would show machine paths. Nothing else is altered, statuses included. Separately,
staging creates every `~` folder the scenario names inside the run's private home, which is the home every staged
supervisor expands `~` against, so the launcher's folder check finds them for real.

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

The hero refresh uses `--local` on Linux, like the docs screenshots (`docs/docs-shots/SPEC.md`). This selects local
capture even when `FARHELM_MEDIA_MAC` is set. The run's private headless Mutter renders the scenario's viewport at its
scale of 2. It needs the Linux prerequisites in `docs/gui-agent.md` and uses its own sshd for the staged fleet.

- `scripts/readme-screenshot.sh --local` prints the PNG's path (`target/readme-hero/readme-hero.png` unless `--output`
  says otherwise). The capture refuses a picture that shows a machine path, the account's ssh spelling or the hostname,
  or that is not the scenario's size; then there is no PNG, and the message names the run directory, kept for its logs
  (remove it once read). Look at the picture.
- `scripts/publish-readme-hero.sh <png>` pushes it and rewrites both pages' URL. `--dry-run` first if in doubt.
- Commit the two-page change like any other change.

`--mac <destination>` runs the same capture on a Mac over ssh, on its real screen, which must fit the scenario's window
(`docs/gui-agent.md`, "On a Mac"); the script's header lists what the Mac needs and the environment that names it. It is
not needed for a refresh.

## Changing the design

Edit `scenario.json5` or `transcript.txt`, run the two commands, and commit the scenario change together with the pages'
URL change so the design and the picture move in one commit. The scenario's own comments say what each field does and
which values the capture can actually deliver.
