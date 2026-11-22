# A focused hint can open the next agent's Settings page before that session is shown

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

After automatic session selection, the focused hook hint can open the next agent's Settings page before that selection
is visible.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b crates/gui/farhelm-gui/src/session_pane/hook_band.rs:36-55,139-150;
crates/gui/farhelm-gui/src/session_pane.rs:354-358,403-426,495-508,648-663

If the selected session is removed elsewhere while focus is on the hint's Settings button, a key can open the successor
session's agent settings before the pane showing that successor is presented. At
bb31959c2f22c96bbbb72da2e60eee4a0b288a6b, session_list.rs:569-592,612-617 and session_list/selection.rs:143-162,304-315
automatically choose the successor from the refreshed list without moving focus. SessionPane mounts the successor under
the same GUI session (session_pane.rs:403-426); its set selects the agent tab by default
(terminal/set.rs:337-359,420-430). Focus stays on the retained, window-owned Settings handle because new home never
steals focus from a control that still holds it (focus.rs:500-512). The redraw tracks that same handle, but its handler
already captures the successor's agent (hook_band.rs:139-150; settings.rs:328-342). Normal Enter/Space dispatch calls
that handler (components/button.rs:1292-1314; components/mod.rs:425-439). Pinned SPEC_impl.md:984-988 and reviewed
prompt.rs:1159-1179,1193-1209 document that the dirty frame can install handlers before presentation and the action
snapshot must wait. The affected control has no such gate. This complements HS-06's post-presentation target binding and
needs neither a restart of the same session nor a GUI reconnect. Gate the Settings action on presentation of its agent
and session context. Source and documented frame behavior establish the route; its timing has not been measured. The
reviewer suggested other: the consequence is opening the wrong settings page and does not itself write any setting. The
reviewed GUI paths remain present at pinned tip 1a53d9d35889d7dde96e31320cc1681dd013737d.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-session/HS-09`.
