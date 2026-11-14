---
kind: breaking
---

Farhelm no longer has a browser interface. `farhelm helm run` serves no page and offers no browser sign-in, and the
`farhelm helm token show` and `farhelm helm token rotate` commands are removed. A Linux helm installed with
`farhelm helm setup` keeps running, but nothing can connect a GUI to it.

The GUI is being rebuilt as a native desktop app for macOS and Linux, which replaces both the browser interface and the
old Mac app. The old Mac app's window is already gone: until the new app lands, the desktop app starts its helm and
local supervisor and opens a window that lists your sessions in your chosen order, with their status (a running
session's dot pulses, as in the old app), shows the terminal of the one you pick, live, for you to type into, and
lets you delete a session from the ⋯ menu on its row (the open one also with Cmd+W on a Mac or Ctrl+Shift+Backspace on
Linux). It has no terminal tabs, text size control or other session controls yet. That window does connect to its
helm, says when the connection is lost or another window has taken over, and offers to take the helm back. This draft
is written while that migration is under way, so builds from the development branch in this period have a GUI that
still lacks most of what the old app could do, and the entry is revised as the new app lands. Agents and the command
line keep working as before, except that agent requests that need your approval are refused while the new app is open
(below).

Five settings the old app kept per device reset to their defaults when you switch to the new app: the appearance
(System, Dark or Light), the sidebar width, whether the host rail is open, the status sounds on or off, and the terminal
text size. The new app will keep them with your other settings in Farhelm itself instead of in the old app's window.
(Curator: none of the five exists in the new app yet; say "keeps" once they have moved into the helm's preferences.)

Until the new app shows approval cards, an agent request that needs your approval is refused at once while the desktop
app is open, with a message saying the window cannot show approval requests yet. Requests a host's "without asking"
settings already allow still run, but the new app cannot change those settings yet, so only what a host already has
from the old app applies. Some requests no setting allows, so they are always refused for now: adding, editing or
removing a host, launches with a wrapper prefix the agent chose, new disposable environments, and template changes that
leave a wrapper prefix in the template. (Curator: interim, decided by the maintainer on 2026-11-13; drop this paragraph
once the app's approval cards land, or, if it has shipped, say they are back. "Already has from the old app" assumes
the host settings carry over unchanged in the helm's database, which nothing in the switch resets.)

The new `farhelm licenses` command prints the license notices for the open-source code Farhelm is built from: the
crates its binaries depend on, and code it ported from xterm.js or derived from Unicode data, whose notices used to ship
inside the browser interface. (Curator: this sentence belongs under Added.)

Text from hosts and agents that Farhelm shows, such as session titles in `farhelm agent sessions`, now also makes the
rest of Unicode's invisible format characters visible as escapes (for example Arabic number signs and musical symbol
format controls), not only the controls, direction overrides and zero-width characters it already showed. Saving a
template whose session name, launch command or resume command contains one of these characters is now refused, as it
already was for the others; templates saved earlier still apply. (Curator: this paragraph fixes behavior that shipped in
released versions and belongs under Fixed, not with the GUI migration above. The same wider set is also dropped from
ntfy alerts and from Tensorlake error messages, escaped in the helm's audit log and in the error of a launch that never
started, and replaced with `_` in the supervisor's hook log; those are probably not worth an entry of their own.)
