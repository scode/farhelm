---
kind: added
---

**Overflowing terminal tabs now stay visible and easy to reach.** Opening or switching to a tab brings it into view
automatically, and overflow controls let you navigate all open tabs in a session.

When a session has more terminal tabs than fit in the strip at the top of the session, the main Agent (or Shell) tab
stays at the left end while only the shell tabs scroll. Opening a tab, clicking one, restoring the tab you last had open
or falling back to a tab after a close scrolls that tab into view, as the tab-switching shortcuts already did. While the
tabs overflow, the strip fades the edge where tabs are hidden and shows scroll buttons and an all-tabs button whose list
shows every tab, marks the shown one, and can show or close any shell tab.

In a tight strip the scroll buttons give way and only the all-tabs button stays; in the narrowest session views, beside
the open-tunnels pill, the Agent tab and the new-terminal button scroll with the other tabs and no overflow controls
show, so every tab stays reachable. Those are edge cases the entry can leave out.

`kind: added` because the overflow controls are new; the TODO entry this covers called tabs going out of view a bug, so
curation may prefer `fixed`.

Context for curation: the first paragraph is the plan `terminal-tab-overflow-ux`'s settled release note; this fragment
covers both of its PRs (pinning and reveal, then the overflow controls).
