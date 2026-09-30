# Replace from the sidebar can kill a session that came back to life

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If Replace from the sidebar told the user nothing was running, and then asked a YOLO confirmation, an agent or terminal
that another window started in the meantime is killed without warning when the user confirms.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F50 / COR-SIDEBAR-REPLACE-RECOMPUTE`, tagged **definite**. Anchor and title:
`crates/farhelm-ui/src/list/view.rs:1995` — the sidebar's Replace has the same "nothing is alive" recompute.

This is the sidebar counterpart of F49. The sidebar row's Replace runs through `do_replace` in
`crates/farhelm-ui/src/list/view.rs`, which works out the `only_if_nothing_alive` safeguard (the flag that makes the
supervisor refuse to delete the replace source if something in it is still running) by looking the row up in the current
session listing each time it runs (`listing.peek()`, around line 1995). Its own comment says the prompt was worded "from
this same row", which is only true the first time.

When the target host is sensitive and the helm refuses the YOLO launch, the sidebar's YOLO confirmation re-enters the
same `do_replace` (via `yolo_do_replace`, lines 2095, 3176 and 3188) after however long the user takes to answer. If a
listing refresh in the meantime shows the row live again (another client restarted it, or a tab was opened), the re-run
sends `only_if_nothing_alive = false`, and the source delete silently kills that new agent or shell, even though the
prompt the user answered said nothing was alive. That breaks the same `status.rs` contract described in F49.

The suggested fix is to store the value captured when the user confirmed the Replace prompt alongside the row's
confirmation state, and reuse that stored value on the YOLO path instead of recomputing it from the listing.

## Additional detail merged from a second review (de774a1ee8815ce833da77deac593a55d82f7be3)

A separate whole-codebase review found this path independently (as part of
`confirmed-nothing-alive-prompt-kills-live-agent.md`, which now covers only the sidebar delete prompt and the header
Restart confirm, with the same capture-from-the-rendered-prompt fix). It adds:

- Since 35b41ff the YOLO confirmation has a second button, "Start, and don't ask again on this host"
  (`on_confirm_and_stop_asking`), which goes through the same recomputation, so the fix must cover both buttons.
- A browser verification recipe: mark the host as asking before YOLO launches, Replace an exited YOLO session, restart
  it through the API while the YOLO question is shown, then confirm; the replace body carries
  `"only_if_nothing_alive": false`.
