---
kind: changed
---

Terminal tab close buttons now appear on the left on hover. Previously there was a close button to the right (always
visible), which was easy to accidentally click instead of the + button used to open a new terminal tab.

Each shell tab now shows a terminal icon at its left, which turns into the close button while the pointer is over the
tab, while the tab or its close button has keyboard focus, while the tab's close confirmation is open, and while the tab
is closing. Nothing in the tab strip moves when it appears. Closing still asks first unless Skip session confirmations
is on, and the close button can still be reached with Tab right after its tab. The Agent (or Shell) tab still has no
close button.

New terminal (+) now stays at the right end of the tab strip however many terminal tabs are open, instead of following
the last tab, so pressing it twice in a row opens two terminals rather than landing the second press on the new tab's
close button, and + stays in view when there are more tabs than fit. When the session pane is so narrow that the strip
has no room for + and a sliver of the tabs (a 650 pixel pane with the tunnel pill showing, for instance), + goes back to
following the last tab so the tabs stay reachable. The plan's release note does not mention the moved +.
