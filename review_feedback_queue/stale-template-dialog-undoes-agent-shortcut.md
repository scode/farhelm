# An open Templates dialog can undo an approved shortcut move

Reviewed commit: b375716cc08ae5d14931d4f7b56264b3e169cdf4

## TLDR

After approving an agent's shortcut change, a person editing another shortcut in an already open Templates dialog can
silently restore the old assignment.

## Details

The agent updates shared preferences, but the open dialog keeps its loaded snapshot. Its number-key handler saves a copy
of the old entire map, and its bolt handler decides whether to clear the mark from the old holder
(`crates/farhelm-ui/src/list/templates.rs:1265–1273,1294–1319,1684–1701`). The next edit can therefore overwrite the
approved move. The landing report in `plans/reports/agent-quick-launch-templates.report.md:87–94` also identifies this
sequence.

Suggested bucket: other; a lost shortcut assignment is saved configuration, but can be restored quickly. Confidence:
confirmed by static state flow; no two-client reproduction was run. Provenance: trailing review of the stated commit.
`SPEC.md` allows concurrent windows to lose an edit to a whole shortcut map and says loaded clients keep old preferences
until reload. An independent coverage audit found those passages ambiguous for an approved agent move followed by a
stale write, including the scalar bolt mark. Refreshing the bindings before editing or reconciling the write with
current bindings would keep the approved choice.
