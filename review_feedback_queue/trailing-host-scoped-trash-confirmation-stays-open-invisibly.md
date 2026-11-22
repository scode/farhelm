# Host-scoped Trash confirmation stays open invisibly when its host leaves or empties

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If you open a host's 'Delete all on' confirmation inside the Trash dialog and that host is then removed or its archived
checkouts are cleared elsewhere so the host's section disappears, the hidden confirmation stays active: every individual
Delete and Retry button on other hosts remains disabled with no confirmation visible on screen, and clicking Close or
pressing Escape silently fails to close the Trash dialog on the first attempt.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/trash/dialog.rs:241-256`.

In `crates/gui/farhelm-gui/src/trash/dialog.rs`, `TrashDialog::asked` (`dialog.rs:112`) stores
`Option<(Option<i64>, Inline<Vec<Target>>)>`. In `TrashDialog::render` (`dialog.rs:405-407`), `self.asked` is cleared
only when `self.open_question(cx)` is `None`, which checks `inline.is_open(cx)` (`dialog.rs:241-243`). Per
`crates/gui/farhelm-gui/src/prompt.rs:1407-1411`, `Inline::is_open` returns `!self.prompt.read(cx).answered` and
explicitly stays `true` when an owner stops drawing the inline prompt. When a host-scoped confirmation (`Some(host_id)`)
is open and either (1) that host leaves `snapshot.hosts` (a case commit `f7cc03b3e36d14c5d55c2ed3b4fd5e6107d0cc5b`
explicitly acknowledged in `focus_delete_opener` / `dismissed`) or (2) that host stays connected and becomes empty with
no issues or notices so `host_section` returns `None` (`dialog.rs:597-606`), the confirmation band is no longer drawn
anywhere, yet `self.asked` remains `Some`. Consequently: (a) `deletes_allowed` (`dialog.rs:416-419`) stays `false` and
`Handles::delete` (`dialog.rs:381-387`) drops clicks, disabling every single-entry Delete and Retry button on all other
hosts with no visible confirmation; (b) `TrashDialog::dismissed` (`dialog.rs:248-256`) sees
`self.open_question(cx).is_some()`, clears `self.asked`, and returns `Dismissal::Stay`, swallowing the user's first
Close/Escape; (c) in case (2), `self.host_deletes` still retains the hidden host's handle
(`dialog.rs:467-471, 495-496`), so `focus_delete_opener` (`dialog.rs:163-178`) calls `focus::focus_in_layer` on an
undrawn handle, queueing a pending focus move (`focus.rs:662-664`) that would fire if that host's section later
reappears before another user input; and (d) before dismissal, if a new archive later arrives on that host, the stale
hidden confirmation band reappears inside its card. This conflicts with SPEC.md, "Keyboard focus" -> "Opening and
closing" (a confirmation goes away when the thing it is about is gone). Fix: in `TrashDialog::render`, before computing
`deletes_allowed` and `self.fence`, clear `self.asked` whenever it names `Some(host_id)` whose host is absent from
`snapshot.hosts` or would not render a `host_section` (and retain `host_deletes` only for rendered host sections, or
guard `focus_delete_opener` against undrawn hosts). Verify with a GPUI test in
`crates/gui/farhelm-gui/src/trash/dialog/tests.rs` that opens a host's 'Delete all on' confirmation, drops that host or
empties its listing, and asserts another host's Delete button becomes enabled (after settle) and a single Close/Escape
closes the dialog.

Reviewer metadata (g28-gui-refactors-input-focus/l1): suggested bucket: other (not highest: no security impact or
unintended data loss, since the hidden prompt cannot be activated while undrawn and freezes its own targets and
wording); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g28-gui-refactors-input-focus/l1`.
