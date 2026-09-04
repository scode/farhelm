# Host aliases: a shorter label for a machine

Anchor: commit `1e4a1340fa29ddc65b3373feb80e5a2432e78e82`, 2026-09-03.

Covers the "add host aliases" entry under "near term" in TODO.md. Effort: medium. That rests on the helm side being one
nullable column, one route in the shape of the destination edit, and the display-name function growing a third input
that has to reach the three places the name is derived from a snapshot; the UI side being one host-menu item that reuses
the destination editor; and the agent CLI needing no code change but one deliberate test. The spread across crates, not
any single piece, is what makes it medium rather than low.

NOTE: a plan, written against the anchor commit; check `git log <anchor>..main` over the paths below before executing.
Two other plans in this directory add helm schema migrations (`plans/session-dot-read-state.md`); whichever lands first
takes schema version 16 and the other takes 17. The title-line host slot this plan's alias reuses on session rows
already exists (`.session-host` in app.css, commit `3e5e4f653a8b`): every row now carries a locality glyph, and a remote
or unknown row names its host in that bounded slot rather than on a line of its own.

## Decisions already taken

Settled with the maintainer on 2026-09-03:

- The alias IS the host's display name once set, everywhere the name is derived, including the hosts listing agents see
  and resolve targets against (`farhelm agent create --host <name>`). The helm refuses an alias that would collide with
  another host's display name, so resolution stays one exact match. A host's raw destination stops resolving once it has
  an alias; a script that named the destination gets the ordinary "no host named" refusal, whose message lists the known
  names, which now include the alias.
- The local host can be aliased too. "this machine" stays its default label.
- The alias is edited from the host row's "⋯" menu with the same inline text field the destination edit uses. An empty
  submission clears it.

## What an alias is, precisely

An optional string on a registry row, `NULL` for none. Its only job is display: it replaces the derived label (the SSH
destination as entered, or "this machine") wherever that label is shown, except in the host details view, which keeps
the real destination visible under it so an alias can never hide which machine a row reaches. It is not an identity (the
per-install identity stays what it is), it is not a second destination (ssh never sees it), and it does not participate
in locality (the sidebar decides local versus remote by host id, never by name, which is exactly why a renamed local row
cannot break that decision).

Validation, applied at the write and mirrored in the client's field: trimmed; empty means clear; no control characters
or line breaks, because `farhelm agent` prints host names on stdout and a name carrying a newline forges a second line
(SPEC_impl.md makes the same argument for session ids); at most 64 characters, an arbitrary cap that exists so the
sidebar's title line and the CLI's known-hosts list stay readable; and unique against every OTHER host's current display
name, alias or derived, including the literal "this machine". The symmetric check belongs on the destination write too:
a destination edit that lands on another host's alias must be refused the same way, or the uniqueness holds only in one
direction.

## Helm side

### `crates/farhelm-helm/src/store.rs`

A migration adding `alias TEXT` to `hosts` (nullable, no default needed), added to the schema-history doc comment, the
fresh-create branch, the migration ladder, and the downgrade-test drop lists (an `ALTER TABLE hosts DROP COLUMN alias`
beside the existing `cache_truncated` drop around store.rs:5210). `HostRow` and `RawHostRow` gain the field; the
`list_hosts` read decodes it.

`update_alias(host, Option<&str>)` in the shape of `update_ssh_destination` at store.rs:2613: one transaction that
validates the string, reads every other row's `(kind, destination, alias)`, computes each one's display name with the
same function the rest of the helm uses, refuses on a match with a `HostStoreError::AliasTaken`-style error naming the
colliding host, and writes. `update_ssh_destination` gets the mirror-image check against other rows' aliases. Unit tests
for each validation rule, the clear, both directions of the collision, and that the local row accepts an alias.

### `crates/farhelm-helm/src/aggregate.rs`

`host_display_name(kind, destination, alias)`: the alias when present, else today's derivation. Every caller passes the
snapshot's alias.

### `crates/farhelm-helm/src/manager.rs`

The manager's host snapshot carries `alias`, filled by `sync_registry` from the store row. This is what makes the alias
reach the three derivations that read snapshots rather than store rows: the session row's `host_name`
(aggregate.rs:246), the agent relay's session view (agent_requests.rs:1291), and the single-session read
(sessions.rs:1481). Deriving from the snapshot rather than joining the store in each place keeps the alias consistent
with the connection state it is shown beside.

### `crates/farhelm-helm/src/hosts.rs` and `lib.rs`

`HostView` gains `alias: Option<String>`, always serialized, so a client can show the editor's current value and the
details view's real name without parsing `name`; `name` is the alias when set. One route, `POST /api/hosts/{id}/alias`,
body `{"alias": "<string>" | null}`, held under `host_write_lock` and followed by `sync_registry` exactly as
`set_destination` at hosts.rs:478 is, then an explicit `events().bump()`: a registry reconcile that changes no host's
shape or state does not bump on its own (events.rs pins "no-op reconciles do not"), and every client's session rows and
host panel need to redraw the name. Tests through `rest_harness`: set, list shows the alias as `name` and the
destination unchanged; clear restores the derived name; collision answers a conflict naming the other host; the revision
bumps; the local host accepts one.

### `crates/farhelm-helm/src/agent_requests.rs`

No code change: `resolve_host` matches `view.name`, which is now the alias when set, and its ambiguity refusal at
agent_requests.rs:749 already tells the user to "rename one in the Farhelm UI", advice that becomes possible for the
first time with this plan. Two tests pin the chosen behaviour: a host with an alias resolves by the alias, and its raw
destination no longer resolves, with the refusal's known-hosts list showing the alias.

## UI side

### `crates/farhelm-ui/src/lib.rs` and `api.rs`

`Host` gains `alias: Option<String>` (`#[serde(default)]`; an older helm sends no key and the editor is not offered).
`api::set_alias(base, id, Option<String>)`.

### `crates/farhelm-ui/src/hosts.rs`

`HostMenuAction::Alias`, offered on every host row including the local one, placed beside `Edit`. The inline editor that
`on_edit_start` opens for a destination (hosts.rs:1076 and the row's `edit_start` at 1497) is generalized to carry WHICH
field it edits (an enum of destination or alias) rather than duplicated: the same text field, submit, cancel and error
line, with the placeholder, the validation, and the API call chosen by the field. The row's name renders `name` as
today, which is the alias when set; the details view adds a "destination" line showing the raw destination whenever an
alias is set (and "this machine" needs none). Unit tests for the editor's field switch and the client-side validation
mirror.

### Session rows and the header

Nothing to change: the row's `.session-host` and the session header's stale notice render the helm's `host_name`, which
carries the alias. The session row's locality glyph (commit `3e5e4f653a8b`) means a local row shows the local glyph and
no name even when the local host is aliased; the alias for the local host is visible in the host panel and the details
view, which is where "this machine" was visible before.

## Spec and docs

`SPEC.md`, Topology (the host registry paragraph around line 116): a host can carry an alias, shown in place of its
destination everywhere except the host details view; aliases are unique among display names; the local host can carry
one. Agent-spawned sessions (line 577, "naming the target by the display NAME the hosts listing reports"): unchanged in
wording and now covers the alias by definition; add a clause that an aliased host is named by its alias only.
`SPEC_impl.md`, Helm internals: the column, why uniqueness is checked against derived names and not only other aliases,
and why the alias rides the manager's snapshot.

## Browser tests

In `provisioning.spec.ts` or `sidebar.spec.ts`, wherever the host row menu is already driven: set an alias on the remote
host from the row menu, assert the host panel row, the session rows' host slot, and the create form's host selector all
show it while the details view shows the destination; clear it and assert the destination returns; alias the local host
and assert the host panel shows it. `agent-relay.spec.ts` has the CLI host-naming cases; add one that creates by alias
and one that shows the destination refused after aliasing.

## PR shape

Two PRs: helm (store, snapshot, display name, route, relay tests, SPEC_impl), then UI and SPEC.md. The entry and this
file are removed in the second.
