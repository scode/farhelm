# Screenshot batches: the GPUI app next to the old app

The GPUI port copies the old app's look, so every area compares its screens side by side with the reference pictures of
the old app (the migration's reference set, kept outside the repository: a directory holding `scenes.json` and
`old-app/`). `scripts/gui-screenshot-batch.py` captures what the GPUI app shows from the current tree and writes one
browsable page that puts each capture next to the matching old-app pictures. The batch stays outside the repository:
captured screenshots are never committed on main (`.agents/docs-media.md`).

## Running a batch

- `scripts/gui-screenshot-batch.py all /tmp/farhelm-tmp/<id>/batch1 --reference <reference set>`: copies the foundation
  goldens, then, for each app variant in the scene list and each group of scenes that share `start` options (see "The
  scene list"), starts the app through `scripts/gui-agent.py` (headless under Mutter and lavapipe on Linux; in the
  logged-in GUI session on a Mac, where the app opens a real window, as `docs/gui-agent.md` says), captures the group's
  scenes and stops the app; last it writes `index.html` in the output directory. Only the first app run builds.
- `capture` and `page` are the two halves. Run `capture` where the app builds (a build worker, say) and `page` where the
  reference set is, after copying the output directory over. `--copy-references` copies the old-app pictures next to the
  page, so the directory can be handed on as is.
- `--lock FILE` is passed to every `gui-agent.py start`, for a machine whose one GUI session several runs share (on a
  Mac, `docs/gui-agent.md`, "On a Mac").
- `--touched ID,...` puts the scenes changed since the last batch at the top. The ids are the section ids on the page:
  an app scene's id, `golden-<name>` for a foundation golden, or a reference scene's id. An id that matches nothing is
  reported on the page and on stderr.

The output directory must be outside the checkout (the script refuses a path inside it) and must not hold an earlier
capture: each capture goes into a new directory. Each app run's own directory (its state, logs and the helm credential
the harness writes for seeding) is made in a fresh directory under `/tmp`, never under the output, and is deleted once
the run stopped cleanly; a run that failed is kept for its logs, and the error names it. `capture` and `all` exit
non-zero when any capture failed or any app run did not start or did not stop cleanly, and the page says which.

## What the page shows

- Every reference scene, by its id in the reference `scenes.json`. A scene the GPUI app does not show yet says "not yet
  ported" next to the old app's pictures; a scene the old app could not be captured in says why.
- App scenes: the running app driven by bridge commands, once per app variant (a theme and a window size), each labelled
  with its variant and the real pixel size of the capture. The bridge has no theme or resize command, so each variant is
  its own app run, started with `gui-agent.py start --appearance --window-size` (agent-build test hooks, not the app's
  appearance setting). Those runs skip automatic announcements and upgrade highlights unless a scene asks for
  `--start-dialogs`. Welcome in the empty pane appears without any startup option.
- On Linux, only the app's own drawing. Under Mutter GPUI draws the window's decorations itself, and GPUI Kit frames the
  window with a 20-pixel margin for its shadow and a 1-pixel border, all inside the window's surface, which is what the
  bridge's screenshot captures: a 900x700 window showed the app in 858x658 pixels inside a black band. So a Linux run
  asks for a window 42 pixels larger in each direction, and 34 taller still for the title bar the window draws for
  itself under Mutter, and crops the frame and that bar off, and the label's size is the app's. A capture without that
  frame (a GPUI Kit release that changed it, say) is shown uncropped with a "capture failed" note rather than cropped
  blindly. A Mac capture is not cropped (Kit draws no margin there).
- Foundation goldens, next to the reference scene they relate to: every reference picture the golden manifest
  (`crates/gui/farhelm-gui/tests/goldens.txt`) names. The pictures are not in the repository, so `capture` copies them
  from the directory `FARHELM_GOLDENS_DIR` names when it is set (fetched elsewhere and copied over, on a machine that
  cannot reach the storage repository), and otherwise fetches them first with `scripts/gui-goldens.py fetch`. They are
  copies, so every platform's goldens are shown wherever the batch runs, each labelled with its platform (only Linux has
  goldens so far); a golden the manifest names for no platform, and a picture the manifest names that the directory
  lacks, are failed captures. `scripts/check-gui-goldens.py` is what proves them current.
- A capture this batch did not make says "not captured in this batch", and a failed one says "capture failed" with the
  reason; neither is "not yet ported", which only the scene list's own state decides.
- A mockup is labelled MOCKUP and never counts as evidence that something is built.
- The commit the capture ran on: the jj working-copy commit in a jj checkout, or git's HEAD (marked when the tree had
  uncommitted changes).

The app scenes show the first slice over a fleet of four seeded sessions, one each running, waiting, idle and exited
(`gui-agent.py start --fleet`, `docs/gui-agent.md`, "Seeded sessions"): each one open with its terminal, the idle one's
row menu, Delete asked from the waiting one's menu, and the rename dialog; and, in an app run of its own, the window
over an empty fleet. A scene's reference scene is marked ported when the scene's subject is complete, not when the whole
picture matches: the Delete question and the rename dialog are, while the main window and the selected sessions still
lack the host rail, the app bar, the list header and the session header, and the row menu lacks most of its commands, so
those stay unported with their app scenes beside them.

## The scene list

`docs/gpui/screenshot-scenes.json` is the committed scene set: `app_variants` (each a theme, a size name matching the
reference set's, and the window size in pixels), `app_start` (the `gui-agent.py start` options, such as the fleet to
seed, of every app run whose scenes name none), `reference_scenes` (each reference id and whether it is ported),
`app_scenes` (an id, a title, the related reference id, whether it is a mockup, the bridge commands that reach the scene
before the screenshot, for a scene that needs another app run than `app_start`'s its own `start` options, and for a
scene limited to some window sizes its `sizes` and `sizes_why`) and `foundation_goldens`. When an area ports a scene, it
adds the app scene that shows it and marks the reference scene ported once that scene's subject is complete. Both
`capture` and `page` refuse a list that is not well formed (duplicate ids, a reference to an unlisted scene, malformed
steps or variants, `start` options the batch sets itself, a malformed `sizes` or one without its `sizes_why`, a golden
the golden manifest names but the list lacks, or a listed one the manifest does not name), and the page flags any
reference scene the list forgot.

The scenes that share start options share one app run per variant and run in list order, so each scene's steps begin
where the one before it left the app (the Delete scene closes the menu the row-menu scene opened, the rename scene
answers Cancel to the Delete question). A scene that needs another fleet names its own `start` options, `--sessions 0`
for the empty one, and gets an app run of its own; the template and quick-launch scenes start with `--templates`, which
seeds their launch templates (`docs/gui-agent.md`, "Seeded templates"). A new scene goes into the group whose state it
can start from, and its steps leave the app the way the next scene in that group expects.

A scene that cannot be shown at some window size lists the size names it is captured at in `sizes` (a non-empty list of
sizes the app variants have) and says why in `sizes_why`, one line; each needs the other. `app-terminal-tabs` is
captured only at `wide`, for example: the narrow window's terminal pane is below the width down to which SPEC.md keeps
the session's top row whole, so its tab strip has no room for New terminal. Such a scene is not run in the other sizes'
variants and is not counted as failed there, and the page says "not captured at narrow" with the reason where that
capture would be, never "not captured in this batch" or "not yet ported" for that size. In those variants every later
scene of its group runs without the limited scene's steps (from where the scene before it left the app, or from a fresh
app when the limited scene was the group's first), so none may depend on what a limited scene leaves behind. A `sizes`
that lists every size is refused: it limits nothing. Keep `sizes` for a scene the size cannot hold at all, not one that
fails there for a reason that can be fixed.

`python3 -B scripts/test-gui-screenshot-batch.py` tests the page from fixtures and the capture step against a fake
harness; it needs neither the app nor the reference set.
