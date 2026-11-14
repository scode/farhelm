---
kind: breaking
---

Farhelm no longer has a browser interface. `farhelm helm run` serves no page and offers no browser sign-in, and the
`farhelm helm token show` and `farhelm helm token rotate` commands are removed. A Linux helm installed with
`farhelm helm setup` keeps running, but nothing can connect a GUI to it.

The GUI is being rebuilt as a native desktop app for macOS and Linux, which replaces both the browser interface and the
old Mac app. The old Mac app's window is already gone: until the new app lands, the desktop app starts its helm and
local supervisor and opens a window that shows no sessions yet. That window does connect to its helm, says when the
connection is lost or another window has taken over, and offers to take the helm back. This draft is written while that
migration is under way, so builds from the development branch in this period have no working GUI, and the entry is
revised as the new app lands. Agents and the command line keep working as before.

Five settings the old app kept per device reset to their defaults when you switch to the new app: the appearance
(System, Dark or Light), the sidebar width, whether the host rail is open, the status sounds on or off, and the terminal
text size. The new app will keep them with your other settings in Farhelm itself instead of in the old app's window.
(Curator: none of the five exists in the new app yet; say "keeps" once they have moved into the helm's preferences.)

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
