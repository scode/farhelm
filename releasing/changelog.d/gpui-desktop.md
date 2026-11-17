---
kind: changed
---

Farhelm's GUI is now a native desktop app for macOS and Linux, which replaces both the browser interface and the old Mac
app. Farhelm no longer serves a page or offers a browser sign-in, and the `farhelm helm token show` and
`farhelm helm token rotate` commands are removed. On Linux the desktop app needs a Wayland session: started without one,
it exits with a message saying so.

(Curator: this draft is written while the migration is under way and was last revised late on 2026-11-16, during its
third milestone. On Linux the desktop app runs its own helm, as it does on a Mac, but no Linux desktop package ships
yet. The new app now covers the old app's daily work: the session list with pinning, marking and bulk Stop and
Delete, the session launcher, templates and quick launch, the terminal with tabs, file links and attachments, Settings,
the Hosts panel, approval cards, Trash, notices and status sounds. What is still being ported is tracked in
`docs/gpui/parity/`. The paragraphs below are candidates for their own entries; most belong under Changed.)

Five settings the old app kept per device reset to their defaults when you switch to the new app: the appearance
(System, Dark or Light), the sidebar width, whether the host rail is open, the status sounds on or off, and the terminal
text size. The new app keeps them with your other settings in Farhelm itself instead of in the window.

The window reopens where you left it. The first launch after the switch ignores the old app's saved window position and
opens at the default size, centered, and on a Mac a window left on a second display reopens on the main one.

Clone spells out invisible characters copied into Folder and Name, and launches with the original values unless
you edit those fields.

The number-key, Keyboard shortcuts and text size shortcuts (Cmd+digit, Cmd+/ and Cmd+Shift with = or − on a Mac;
Ctrl+Shift with the same keys on Linux) now go by the key's position on the keyboard, whatever its layout, and never reach
the program in a focused terminal. On Linux, Ctrl+Shift+− used to reach the terminal, where it is readline's undo.

Pasting into a terminal is Cmd+V on a Mac and Ctrl+Shift+V or Shift+Insert on Linux, so on Linux Shift+Insert no longer
reaches the program.

The prompt boxes in the session launcher and the template editor grow with their text up to a limit and then scroll;
you can no longer drag them to resize.

A launch onto a suspended sandbox or container now asks you to resume it in Hosts first.

When Enter or Escape closes the last open dialog or menu, the same key pressed again within half a second does nothing,
so a double-tapped Escape no longer reaches the terminal and interrupts the agent in it.

Opening a session whose host cannot be reached, or one that has ended, now marks its output as seen, as opening any other
session does.

A question that stays open while Farhelm works, such as Remove host, no longer moves your focus back to Cancel when the
request is refused, if you had already moved on to another control. The version shown in the window's corner explains
itself in full on hover instead of a cut-off line.

Block characters in the terminal are drawn as exact cell-sized rectangles, so progress bars and graphs no longer show
thin stripes at text sizes other than the default. (Curator: confirm the released app striped before keeping this; if
only early builds of the new app did, drop it.)

The questions that delete a sandbox or a container start with Cancel focused, so pressing Enter right after one opens no
longer destroys the machine, its files and its sessions.

Sandbox and container hosts are called Sandbox and Container rather than Remote, in Hosts and in the session list. A
machine that is being deleted, or whose deletion failed, no longer offers Update or a setup rerun, and a recipe's
summary says "Managed Ubuntu" rather than "Tensorlake Ubuntu".

A new sandbox or container session's launch progress names its steps for its kind, such as "Creating a web container
on build-box…", instead of the generic step names.

When a session's host is suspending or resuming, the session card names the provider Farhelm asked.

On Linux, when the desktop does not draw window title bars (GNOME, for example), the app draws its own, with the title
and the minimize, maximize and close buttons. (Curator: KDE, which draws its own title bars, has not been tried yet.)

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

(Curator: the maintainer ruled on 2026-11-17 that the move to the desktop app is not a breaking change; list it under Changed in a normal release.)
