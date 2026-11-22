# Interrupted uploads stay invisible when the selected session loses its terminal to a card

Reviewed commit: e3d03d5651e9f0b417514e145ce847a1e45fffe8

## TLDR

If an upload is interrupted because the open session loses its terminal (for example, a host reboot or update puts the
session on its recovery card, a container session becomes retired, or a connection reset reconnects onto an unreachable
host or recovery card), Farhelm shows neither a terminal strip message nor a window notice until the user eventually
leaves that session—and says nothing at all if they quit first.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/terminal/attachments.rs:402-415`.

`post_departed_uploads` (`crates/gui/farhelm-gui/src/terminal/attachments.rs:402-442`) only drains `Carried.interrupted`
into the notice center when `local_state::selected(cx).as_ref() != Some(*key)`. Meanwhile, `SessionPane::sync`
(`crates/gui/farhelm-gui/src/session_pane.rs:425-445`) can drop or refuse to mount `TerminalSet` while
`local_state::selected(cx)` remains `Some(key)`:

1. While an upload is in flight, the open session transitions to its recovery card (`Interrupted` or `terminal_missing`)
   or retired card (`retired::is`): `SessionPane::sync` sets `mounted.set = None` (`session_pane.rs:438-439`), dropping
   `TerminalSet` and calling `Attachments::hand_over`, which abandons the upload and writes `Interrupted` into
   `Carried[key].interrupted`. Because `key` is still selected, `post_departed_uploads` does nothing, and no terminal
   exists to call `take_carried(true)`.
2. A channel reset retires the terminal (`hand_over` stores `Interrupted` in `Carried[key]`) while preserving
   `local_state::selected(cx) == Some(key)`. On reconnect, if the session is still listed (so `keep_survivors` keeps it
   selected) but `card || unavailable || retired` is true (or `terminals()` returns `None` when font measurement fails),
   `Mounted::new` mounts `set: None` (`session_pane.rs:425-427`). Again, neither a terminal strip nor a notice reports
   the cut-off upload. For a retired containerized session, `SessionPane` will never mount a terminal again
   (`session_pane/retired.rs:12`). SPEC.md, "Attachments" (line 3641) requires that upload failures must be visible and
   an attachment must never disappear silently. SPEC_impl.md, "One failure path: the reset" (lines 498-504) only covers
   a still-open session whose terminal set delivers them or a selection that moves on/clears, missing the
   still-selected-without-terminal state. Fix direction: When the window is connected and `SessionPane` has no
   `TerminalSet` for the selected session (dropped to a card or mounted without terminals), promote that session's
   `Carried.interrupted` entries to the notice center rather than waiting for `LocalState` to change.

Reviewer metadata (g19-uploads-terminal-attachments/l1): suggested bucket: high (not highest: local files remain intact
on disk and no user-owned data, credentials, or processes are lost; however, an interrupted upload silently vanishes
from view while the user stays on the card or quits from it, materially degrading UX and violating SPEC.md's visibility
rule); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g19-uploads-terminal-attachments/l1`.
