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
