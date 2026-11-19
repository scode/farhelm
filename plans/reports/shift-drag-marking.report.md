### What this was about

Shift-click marks a range from the open session. Dragging across session rows with Shift held did not mark a range. You
can now press on the body of a session row with Shift held and drag to mark from that row to the one under the pointer.
The row grip still moves the session. The sweep replaces the previous marks and shrinks again when you move back. A
Shift press that stays on its starting row still behaves like Shift-click.

### Things you should know

The list holds its order during a sweep and scrolls when you reach an edge. Collapsed group members stay out of the
range, and the anchor can scroll out of view. Escape, an interrupted drag, or the starting row disappearing restores the
marks you had before the press. Finishing the sweep keeps the current session open and discards pending clicks from the
gesture, even if the release lands on another row or on Clear.

The sweep uses the same pointer capture and edge scrolling as the row grips. The small refactor and the gesture are in
one draft because the shared code has no independent behavior to deliver. The draft also updates the spec, adds the
release note, and clears the TODO entry.

### Open questions and possible follow-ups

None.

### PRs

- [#4199](https://github.com/scode/farhelm/pull/4199/changes) adds Shift-drag marking to the sidebar. Delivered as an
  open draft at `f12288ba`.

### Checks run, reused and skipped

- Passed 244 focused GUI tests, including the new sweep cases, the existing grip and group drags, and shared button and
  row activation. The tests exercise Escape, losing a release, and a starting row deleted or hidden by a group collapse
  just before redraw. Recorder `f41b076e-7ca3-468e-8b51-a2013f852901`.
- The real Linux desktop smoke passed 49 steps, including pin reorder and dragging an unpinned session into a group. The
  headless compositor skipped its scripted clipboard-paste step because it cannot give the app clipboard ownership.
  Recorder `7d511f11-c031-47b9-8bf2-874f41e00a4d`. Two earlier attempts stopped during compilation and provided no
  desktop result. Building the binaries separately let this run reach the app.
- Passed GUI Clippy with all targets and warnings denied, Rust and scoped Markdown formatting, diff whitespace,
  changelog format, and the test-delay checker.
- No results reused from other revisions. The two runtime records match the exact draft diff. The rest of the workspace
  suite and screenshot goldens were skipped because the change is scoped to sidebar input with the existing marked
  appearance. Native macOS input was not run on this Linux host.

### Review gate

The designated fresh high-effort GPT-6.2 Sol reviewer checked the feature and verified the fixes for its findings. An
independent scope review checked reuse of the existing drag capture, edge scrolling, and click handling. The title and
commit message passed their cold read.

### Landing

#### What landed with it

It landed in a round of three plans: shift-drag-marking, model-family-shorthand and unusable-folder-refusals. Since its
base, main gained earlier rounds' plans (session group dragging, model families, the covered-link refusal, the demo
video recorder, the Replace with checkout hint and others) and bookkeeping. It was built on the final session group drag
code, and a reviewer compared every grip-move path before and after: moving a session by its grip behaves as before. One
new behavior is a fix: if the app never sees the mouse button come up (released outside the window, say), the next click
now ends the drag, where before the drag kept following the pointer. The reviews found no other interaction.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The sweep (Shift+drag across
rows to mark them) is correct: a reviewer traced lost releases, stale presses, cancelling, scrolling during the press,
crossing group headers and listing updates.

#### Fixes made while landing

- **A Shift+click that wobbled became a sweep.** A sweep started the moment the pointer crossed a row's edge, by even a
  pixel, and a sweep marks a different range than a Shift+click (from the row pressed rather than from the open
  session). A Shift+click near a row's bottom edge that moved a pixel or two could mark the wrong range with no warning.
  A sweep now starts only once the pointer has moved the same distance the grip's drag needs before it lifts.
- **A shared rule lost its warning.** Buttons and rows ignore a click whose mouse release something else has already
  claimed. Only a sweep claims releases, so the row a sweep ends on is not also clicked. The comment that warned against
  relaxing this had been replaced by one that did not explain it. It now explains it, and says nothing else should claim
  a release.
- **Two comments were cut or left wrong:** the one explaining why Escape is caught for the whole sidebar during a drag
  (it now covers sweeps too), and the row function's, which still described a parameter the plan removed.

#### Things you should know that the report does not say

- **A sweep that starts on the grip moves the session instead.** The grip shows in a row's leftmost 12 points on hover.
  Shift+drag that starts there is the grip's move, which can change a session's group. The plan asks for this, but now
  that Shift+drag means "mark", people starting at the left edge will hit it. Worth deciding whether Shift on the grip
  should sweep or do nothing.
- **Some race hardening goes beyond the plan.** If the start row leaves the list during a sweep, the sweep is undone and
  the earlier marks come back. The plan asked for that check when the list next redraws. The plan also added a second
  copy that runs when the mouse is released, for the rare case where the release comes before that redraw. This copy
  works out on its own which rows are showing, so a later change to the list could make it disagree with what is on
  screen. Without it, that rare case keeps the swept marks. Possible follow-up: remove it and accept that.
- After a sweep, a Shift+click ranges from the open session, not from the sweep's end, unlike file managers.
- The docs page's "Mark several sessions" section does not mention Shift+drag, and says Escape in the terminal goes to
  the agent, which is not true while a sweep is on.

#### Checks

The three plans change the desktop app's session list and launcher, the helm, the shared protocol, the supervisor and
its end-to-end tests. The landing ran the lints, the whole Rust workspace suite, the desktop process's tests, the app's
end-to-end smoke and the reference screenshots (goldens). Builds ran locally; the workspace suite and the smoke ran on
boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python3 -B scripts/test-gui-e2e-smoke.py`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`
  once without and once with `--features agent`: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  2761, 2658 and 2636 tests, 8055 in all (`cc2d7e45-c34d-471b-9521-e292a5d3eabc`,
  `ad12af75-4c3f-479f-9c87-e8df09421a5a`, `89433db2-2d08-49cc-a1af-a5b8696a6edb`). One failed: the macro
  compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat does not have,
  so it fails there every time. It did not run anywhere this round; nothing in this round touches it, and it passed
  locally in an earlier round today. One test skipped itself at runtime because it needs a recreated file to get a new
  inode number and the boat's filesystem reused the old one; it did not run anywhere else this round.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`6610603b-7df3-43e5-ab7c-cf4756a7b544`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`a05cf1f2-6da9-4972-988a-f814bff20a5e`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`1c6c16a9-9ed6-4d42-9ce3-0399f6a2bc61`).

On the fixed tree (the Shift sweep's start threshold, the folder check's access call, comments and SPEC_impl.md):

- `cargo fmt --all -- --check`, `dprint check` and
  `cargo clippy --locked -p farhelm-gui -p farhelm-supervisor --all-targets -- -D warnings`: passed.
- The session list's whole test module, locally: 516 passed (`69a3bcc4-c0af-480b-9732-b36393d20617`), the sweep and grip
  drag tests included.
- The supervisor's session lifecycle and terminal tab end-to-end modules, which exercise the folder check on create,
  restart and tab open, locally: 96 passed (`a83797de-4468-4cb7-8c6d-df759c257c5b`).

The boat suites, the smoke, the desktop tests and the goldens were reused for the fixed tree: after them only the two
small code changes above and comments changed, and the focused runs above cover both.

Skipped:

- The Mac-only checks: no Mac-specific code changed.
