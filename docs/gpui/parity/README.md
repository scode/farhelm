# Parity checklists

One file per area maps the old browser UI's Playwright specs (deleted with that UI; reference commit `24bc0580`) and the
SPEC.md behavior they covered to their replacements in the GPUI app. Each area's port updates its own file; the
orchestrator owns this index after M1. Every one of the 145 spec files at the reference commit is assigned to exactly
one area, except where an area's entry names only some of a spec's cases: the row-action cases of
`tests/terminal.spec.ts` are also listed under Sidebar and session view, `tests/host-updates.spec.ts` is split between
Settings and hosts and Sidebar and session view, and `tests/host-picker.spec.ts` between Launcher and Sidebar and
session view, each file saying which cases are its.

- [Terminal](terminal.md)
- [Sidebar and session view](sidebar-and-session-view.md)
- [Launcher](launcher.md)
- [Templates and quick launch](templates-and-quick-launch.md)
- [Settings and hosts](settings-and-hosts.md)
- [Desktop integration and Linux packaging](desktop-integration-and-linux-packaging.md)
- [Approvals and agent-spawned sessions](approvals-and-agent-spawned-sessions.md)
- [Other dialogs](other-dialogs.md)
- [Foundations (M1)](foundations.md)

Each spec has an entry of its own: a heading holding the spec's path (`` ### `tests/templates.spec.ts` ``, one level
deeper under a `###` section), a paragraph with its number of tests and what it covers (its first test's title, or its
`describe` title where that says more), as in "Tests: 48. Covers: template bolts use a fixed outline or filled drawing
for their mark.", and its status as a paragraph of its own. A new replacing test goes in a bullet of its own under the
status. Statuses written before this layout name their tests inline; when you change one, keep its inline list and add
the new tests as bullets after it. A section listing cases of specs whose entries sit in another area's file uses a
"Case: ..." paragraph (plus "Its file: ..." where the heading does not name the file) in place of "Tests: ... Covers:
...", and the old native app's tests and the desktop smoke legs use the same shape.

The files used to be tables, one row per spec, and prose written before this layout still calls an entry its row. dprint
pads every cell of a table to its column's widest cell, and many rows were over 2,000 characters, so any change that
moved a column's width rewrote every row and any two PRs touching one file conflicted. With an entry per spec, a change
touches only that spec's lines; do not add spec tables back.

A status is one of:

- not yet ported;
- partly, naming the replacing tests;
- replaced, naming them;
- superseded: the GPUI design or a later SPEC.md removes the need; the remaining acceptance cases are still ported;
- dropped, with the reason;
- test infrastructure: a helper of the old suite, not GUI behavior; nothing to port;
- docs media: a docs screenshot, README hero or demo video capture, rebuilt on the GPUI tooling at cutover (M4), not as
  part of the GUI port.

The last two are not progress on the port: a spec with either status was never a GUI behavior to replace.
