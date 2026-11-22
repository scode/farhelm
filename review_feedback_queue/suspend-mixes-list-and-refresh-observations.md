# The Suspend patch narrows a pre-existing race and the provider routes do not recheck activity

Reviewed commit: fe63efdef0691208e093bb665ffe064deae511dd

## TLDR

Suspend can use an old idle session list alongside a newer successful refresh and skip asking before it freezes active
work.

## Details

**high.** Source inspection.

fe63efdef0691208e093bb665ffe064deae511dd crates/gui/farhelm-gui/src/session_list/environment.rs:478-517;
23ba5ede59ebae6523ad7f7bfd35f4ff7dc74352 same file:478-520; 1a53d9d35889d7dde96e31320cc1681dd013737d
crates/farhelm-helm/src/ui_channel.rs:1049-1055 and crates/farhelm-helm/src/environment/lifecycle.rs:46-72,92-110

A Suspend can still freeze an active sandbox or container without the activity question. Before #4481 the UI accepted
any Connected host with a complete idle cached GET. #4481 requires RefreshView::Ok, so Pending and Failed now cause a
question when they are the pushed state at reply time. #4486 changes tests only here. Neither change binds the host
health to the list's observation. The GET is cache-backed (pinned sessions.rs:881-913); connected retained rows are not
stale (aggregate.rs:217-234; manager.rs:5687-5712); channel collection refresh samples Hosts after the GET reply was
constructed, then sends the collections before that reply (ui_channel.rs:832-857,1049-1055). A refresh that succeeds
between those observations can publish new health with a reply still describing old idle sessions. Plain Suspend carries
no listing fingerprint, activity condition, or confirmation flag (GUI environment.rs:626-635). The helm dispatches from
the registered provider, withdraws the connection, and applies the transition. Tensorlake checks account and sandbox
state (tensorlake/lifecycle.rs:108-147,188-231). Podman checks canonical container ownership and its running/paused
state (environment/podman.rs:677-751,1399-1405). Neither asks the supervisor for session activity or compares the list.
Stop and suspend has the two ordinary Stop guards on the chosen session and requires cleanup success before the same
host transition, with no check of other sessions (sessions/sandbox.rs:559-591). This is residual pre-existing behavior
narrowed by #4481. SPEC.md at the assigned commit:352-361 requires complete, conclusive activity evidence. The reviewer
suggested high for an unasked freeze of active work that preserves processes. Confidence confirmed from allowed source
ordering and downstream route, timing frequency unmeasured; ; Possible coverage: original suspend refresh concern.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `session-prompts/SP-03`.
