---
kind: changed
---

When a session has more terminal tabs than fit in the strip at the top of the session, the main Agent (or Shell) tab
now stays at the left end while only the shell tabs scroll. Opening a tab, clicking one, restoring the tab you last had
open or falling back to a tab after a close now scrolls that tab into view, as the tab-switching shortcuts already did.

In the narrowest session views, beside the open-tunnels pill, the Agent tab and the new-terminal button scroll with the
other tabs instead, so every tab stays reachable; that is an edge case the entry can leave out.

`kind: changed` because pinning the main tab changes how the strip behaves; the TODO entry this covers called tabs going
out of view a bug, so curation may prefer `fixed`.

Context for curation: this is the first of two PRs for the plan `terminal-tab-overflow-ux`, whose settled release note
reads "**Overflowing terminal tabs now stay visible and easy to reach.** Opening or switching to a tab brings it into
view automatically, and overflow controls let you navigate all open tabs in a session." The second PR adds the overflow
controls (edge fades, scroll arrows and an all-tabs menu) and is expected to amend this fragment rather than add
another, at which point the plan's note fits as written.
