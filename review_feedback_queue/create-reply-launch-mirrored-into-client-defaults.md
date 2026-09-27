# The client mirrors a remote create reply's launch choices into its own remembered defaults

Reviewed commit: b529cfd641a6f02e4b035852e0ddf501f2068046

## TLDR

After creating one session on a misbehaving remote machine, the New dialog in that same browser tab or desktop window
preselects whatever permissions mode (for example "yolo", approvals off) and workspace-trust choice that remote machine
put in its reply, for every host, until the page is reloaded, even when the helm's own stored default is untouched.

## Details

Paths are relative to `crates/farhelm-ui/src/` unless they start with `crates/`.

This is the client-side half of the defect queued as `create-reply-sets-remembered-yolo.md`. That item is about the helm
writing the remote reply's `launch` into the `preferences` table. Its suggested fix (derive the remembered defaults from
the helm's own accepted selection, never from the reply) closes the durable write but leaves this path open, so the two
should land in one change; this file exists so the UI half is not missed when the helm half is executed.

`list/view.rs:2534-2544` (`ListView`'s `on_created` handler): on a successful create the UI reads `session.launch` off
the create REPLY and writes `launch.permissions` into `preferences.0.write().remembered_permissions` and
`launch.workspace_trust` into `remembered_workspace_trust`. The comment above it says this is a "local mirror" of the
write the helm makes itself, and deliberately issues no PUT.

The reply is peer text. `crates/farhelm-helm/src/sessions.rs:1786-1862` (`accept_created_session`) returns the
supervisor's `SessionInfo` unchanged (`Ok(session)` at :1862) after `created_session` in `client.rs` has checked only
the id, and `api.rs:1608-1622` (`create_session`) decodes that body straight into `Session`. So `session.launch` is
whatever the remote supervisor chose to report. Nothing in `on_created` compares it with the selection the form actually
submitted (`bound.agent`), which the handler no longer has: it receives only the enriched reply
(`list/create_form.rs:3431-3435`).

The mirrored value is then what a fresh New dialog seeds from. `list/create_form.rs:1533-1539` calls
`initial_structured_permissions(&preferences.0.peek())` on mount when there is no prefill, `:1559-1564` seeds
`structured_workspace_trust` the same way, and the "reset choices" button re-applies both (`:3555-3564`). The
preferences signal is seeded once per page by `PreferencesGate` and never refetched (`list/view.rs:138-171`), so the
client-side value persists for the life of the page regardless of what the helm stores.

Trigger: any user-initiated create whose reply comes from a supervisor that reports
`launch: { permissions: "yolo", workspace_trust: true, … }` regardless of what was asked. Structured, profile, raw
command and replace-with creates all go through `on_created`; a plain Replace via `api::replace_session` goes through
`on_open` only and is not affected. A raw-command or profile create sends no launch selection at all, yet a reply
carrying one is still mirrored.

SPEC.md "Remote input, session defaults, and availability": a remote supervisor's metadata "must not override an
explicit user choice or indefinitely determine that default". The launch-composer carve-out remembers the last
SUCCESSFUL structured launch's permissions mode, meaning the mode the user chose, not the mode a peer reported.

To verify: with a fake supervisor whose create reply sets `launch.permissions = Yolo` and `workspace_trust = true` for a
create submitted with `permissions: None`, open New in the same page after the create; the permissions segment
preselects yolo and the summary reads "permissions: yolo", and "reset choices" returns to yolo.

Fix: mirror from the submitted selection rather than the reply. Have `CreateSessionForm` pass `bound.agent` (the
`LaunchIntent::Structured` it sent, or nothing for command/profile creates) alongside the session to `on_created`, and
write `remembered_permissions`/`remembered_workspace_trust` only from that. Alternatively drop the local mirror and
refetch preferences after a create. Either way the mirror must ignore `session.launch`.

Not exercised at runtime; every hop was read at the reviewed commit.
