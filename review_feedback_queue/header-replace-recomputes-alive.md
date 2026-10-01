# Replace from the session header can kill a session that came back to life

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If Replace in the session header told the user nothing was running, and then asked a YOLO confirmation, an agent or
terminal that another window started in the meantime is killed without warning when the user confirms.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F49 / COR-REPLACE-ALIVE-RECOMPUTE`, tagged **definite**. Anchor and title:
`crates/farhelm-ui/src/session_view.rs:1291` — the session header's Replace recomputes "nothing is alive" each time it
runs, including when the YOLO confirmation re-submits it.

Replace creates a fresh session from an existing one and then deletes the source. Deleting the source kills whatever is
still running in it, so the UI has a safeguard: when the confirmation prompt told the user "nothing in this session is
alive" (the agent has ended and no terminal tab is open), the request carries a flag, `only_if_nothing_alive`, that
tells the supervisor to refuse the source delete if something turns out to be running after all.
`crates/farhelm-ui/src/status.rs:368-377` states this contract: whenever a delete or Replace goes ahead on the strength
of a nothing-alive prompt, the request must carry the precondition.

In the session view's header, the `replace` closure (`session_view.rs:1291-1295`) does not take that flag from the
prompt the user answered. It recomputes it from the session's current state each time the closure runs. That matters
because the closure runs a second time when the helm refuses a YOLO launch on a sensitive host: the YOLO confirmation's
buttons (`:2226`, `:2241`) call `replace` again, possibly much later, and that confirmation talks only about YOLO, not
about what is alive. The failing sequence:

1. The session has exited and has no tabs; the Replace prompt says nothing is alive, and the user confirms.
2. The target host is sensitive, so the helm refuses and the UI shows the YOLO confirmation.
3. Meanwhile another client restarts the session, or opens a tab in it, and a detail refresh updates this view.
4. The user confirms the YOLO question. `replace` recomputes, now finds something alive, and sends
   `only_if_nothing_alive = false`.
5. The helm creates the replacement and deletes the source unconditionally, killing the agent or shell that was just
   started, with no warning.

The header's Delete button already had exactly this race fixed: its handler captures the nothing-alive value from the
same render that drew the prompt text (`session_view.rs:1736-1744`) rather than reading fresh state at click time. The
suggested fix is to do the same for Replace: capture `only_if_nothing_alive` (together with the source host fields) when
the Replace prompt is confirmed, carry it through the YOLO confirmation's state, and pass that captured value into
`replace` instead of recomputing it. F50 is the same bug in the sidebar.

## Additional detail merged from a second review (de774a1ee8815ce833da77deac593a55d82f7be3)

A separate whole-codebase review found this path independently (as part of
`confirmed-nothing-alive-prompt-kills-live-agent.md`, which now covers only the sidebar delete prompt and the header
Restart confirm, with the same capture-from-the-rendered-prompt fix). It adds:

- Since 35b41ff the YOLO confirmation has a second button, "Start, and don't ask again on this host"
  (`on_confirm_and_stop_asking`), which goes through the same recomputation, so the fix must cover both buttons.
- A browser verification recipe: mark the host as asking before YOLO launches, Replace an exited YOLO session, restart
  it through the API while the YOLO question is shown, then confirm; the replace body carries
  `"only_if_nothing_alive": false`.
