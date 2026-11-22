# A post-exit Alt release clears a short selection and queues cursor keys

Reviewed commit: 0592c5c6d667abe2f08706185b7566febb3434f7

## TLDR

A quick Alt/Option release over the final screen can clear the highlight for a just-copied character and dispatch cursor
keys.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/terminal/attached.rs:4215`.

attached.rs 3979–4018 now admits a plain local press after exit even with retained mouse reporting. End selection writes
the clipboard first, then a quick Alt release with at most one UTF-16 unit selected reaches alt_click while following
(4204–4249). Supervisor grid_output.rs 329–362 and 441–451 export the final emulator cursor even when invisible, and
grid_stream.rs 453–462, 754–757, 910–920 overlays the exit separately. A state-only update leaves the cursor in the
model (lib.rs 355–357, 460–463). In a nonempty primary grid, any different clamped cell yields a nonzero movement count
(cursor.rs 161–193) and nonzero runs give ordinary Key messages (31–53). Terminal.rs 744–751 materializes those
messages. Then send_user clears the selection locally before dispatch (attached.rs 1428–1441), regardless of whether the
host ultimately uses the keys. No clipboard rollback is implied. Existing attached/tests.rs 3653–3683 asserts nonempty
keys for the live path, including a one-character selection, while 3877–3906 covers only plain drag and bare motion in
post-exit history. The parent already had the stale Alt path for reporting-off or forced selection, but with reporting
on its plain press went to the program (parent attached.rs 3974–4016). This patch newly allows the plain press to arm
the gesture. SPEC.md tip 3467–3470 assigns post-exit clicks to the terminal. Gate the Alt cursor action on program exit.
Bucket other for the confirmed local selection effect and attempted input dispatch. Transport: program Exited keeps the
attachment alive (grid_stream.rs 46–53, 604–626), and a closed PTY discards keys (terminal-host host.rs 581–605). No
consumed input, PTY write, or runtime effect was observed. Later in-range 063203912059cb23101d9a458d8821f02f1bc18f
classifies a known-gone terminal as missing (service/status.rs 319–326 at tip). A retained exited terminal still has a
handle and the change does not guard this path. Confidence confirmed by source for nonempty commands and local clearing.
Remote effect unverified. Possible coverage none. Transport distinction: terminal-host host.rs 454–486 journals the
direct child's exit after the output drain or its bounds. The exit record does not by itself close the PTY. Input
eligibility at 581–605 depends on the PTY's actual state. Thus the closed-PTY discard is conditional, and neither
unconditional dropping nor delivery to any surviving reader is established by the exit banner. The local selection clear
occurs first in either case. The helm client queues the group under the original attachment stamp
(crates/farhelm-helm-client/src/terminal.rs 206–217). It cannot move with recovery onto a replacement attachment. This
limits transport impact but cannot undo the GUI-side clear.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l4`.
