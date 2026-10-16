# Restored terminal snapshots reject every link click

Reviewed commit: 293d5aa752cc46817fa7affe6a5fe31a8405e604

## TLDR

After connection recovery restores the old terminal screen, links can still show hover effects, but clicking them no
longer opens websites or downloads files. Reconnecting or reopening the terminal repairs the problem.

## Details

Suggested bucket: `other`. Confidence: likely. The gesture gate and lifecycle are confirmed statically; restored-screen
browser interaction was not reproduced. Source: separate adversarial trailing reviewer. Proposed drop: none.

`crates/farhelm-ui/assets/terminal.js:3535–3541,3604–3611,3687–3691` makes activation depend on `linkGesture`. Its
observer is installed at `5429–5440` and removed by `stopCopyOnSelect` at `5449–5454`. Recovery retains the terminal at
`1714–1721`; restoration at `1772–1785` installs only `installSnapshotCopyOnSelect`. That helper (`1269–1300`) never
updates the retained gesture. New mouse releases therefore fail event-identity admission, while the surviving link
providers still supply hover callbacks. This remains unchanged through review tip
`991835f826b25bc93eb088f6d3984477a9dc2fab`.

Verify using an owned terminal whose reconnect is refused after retaining a screen containing web and file links.
Establish that the restored screen is visible, then hover and click those links. Gesture observation should survive for
the retained terminal's lifetime, or be reinstalled with the snapshot, while continuing to reject drags.

Possible cover: FILTER.md, “Rare, self-correcting glitches and imprecise diagnostics,” may apply because leaving the
view repairs it. Coverage is unclear: repeated clicks fail for the restored screen's whole lifetime. The completed
`restored-terminal-snapshots-lose-select-to-copy.md` ledger outcome concerns copying, not link activation. No exact
specification exception, Planned item, BUGS entry or queue item was found.
