# Parity checklists

One file per area maps the old browser UI's Playwright specs (deleted with that UI; reference commit `24bc0580`) and the
SPEC.md behavior they covered to their replacements in the GPUI app. Each area's port updates its own file; the
orchestrator owns this index after M1. Every one of the 145 spec files at the reference commit is assigned to exactly
one area.

- [Terminal](terminal.md)
- [Sidebar and host rail](sidebar-and-host-rail.md)
- [Launcher and templates](launcher-and-templates.md)
- [Settings and hosts](settings-and-hosts.md)
- [Other dialogs](other-dialogs.md)
- [Desktop integration and Linux packaging](desktop-integration-and-linux-packaging.md)
- [Approvals and agent-spawned sessions](approvals-and-agent-spawned-sessions.md)
- [Foundations (M1)](foundations.md)

Each row names the spec, its number of tests, what it covers (its first test's title, or its `describe` title where that
says more) and a status, which is one of:

- not yet ported;
- partly, naming the replacing tests;
- replaced, naming them;
- superseded: the GPUI design or a later SPEC.md removes the need; the remaining acceptance cases are still ported;
- dropped, with the reason;
- test infrastructure: a helper of the old suite, not GUI behavior; nothing to port;
- docs media: a docs screenshot, README hero or demo video capture, rebuilt on the GPUI tooling at cutover (M4), not as
  part of the GUI port.

The last two are not progress on the port: a spec with either status was never a GUI behavior to replace.
