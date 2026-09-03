# Replace: a fresh session that takes the old one's place

Anchor: commit `1e4a1340fa29ddc65b3373feb80e5a2432e78e82`, 2026-09-03.

Covers the "add replace on sessions" entry under "near term" in TODO.md. Effort: medium. That rests on the helm side
being one route composed from two operations the helm already performs (the agent-clone derivation of a create from a
source session, and delete), the UI side being one menu item with one inline confirmation in the pattern archive and
delete already use, and the rest being spec text and tests. Nothing on the supervisor or the protocol changes.

NOTE: a plan, written against the anchor commit; check `git log <anchor>..main` over the paths below before executing.

## Decisions already taken

Settled with the maintainer on 2026-09-03:

- The old session is DELETED, not archived. Replace is a fresh create with the old session's settings followed by the
  old session's removal; the fleet ends up with the same number of sessions and the old conversation is gone.
- Replace acts directly from the row menu, behind one inline confirmation. It does not open the create form; the whole
  point is "the same settings", so there is nothing to edit.
- The new session has a NEW id. The maintainer was emphatic that this is the entire point: restart is the operation that
  keeps a session's identity and conversation, replace is the one that does not. This is user-visible behaviour and goes
  into SPEC.md as such, beside restart's contract.

## What replace means, precisely

Replace on session S creates a new session S' on S's host, in S's working directory, with S's title, and with S's agent,
then deletes S. "S's agent" follows the rule the UI's clone already applies: the profile S was created from when that
profile is still in the helm's catalog, otherwise S's raw invocation. The agent-CLI clone deliberately refuses the raw
fallback because a command line written for one machine may not run on another; replace never changes machine, so the
fallback is safe here and matches what the user sees when they clone from the UI. Title is carried as-is; like clone,
replace does not deduplicate titles, and since the old row is gone a moment later there is no duplicate to see.

What replace does NOT carry: the conversation (that is restart), terminal tabs (they are not durable metadata even
across a reboot, and a fresh session starts with none), and the old session's id and anything keyed on it (an
agent-spawned child's parent reference now points at a deleted session, exactly as it would after a delete) — which
includes the helm's own `session_seen` read/unread mark (SPEC.md, Status): S' has a new id, so it starts with no row in
that table at all, the same fresh-unset state a session that has simply never been opened reads.

Order of operations is create first, delete second, and the two failure modes are asymmetric on purpose:

- If the create fails, S is untouched and the error is the create's own (the supervisor's refusal for a vanished
  directory, an unreachable host named by state, a profile that resolves to nothing). Nothing was lost.
- If the create succeeds and the delete then fails, S' exists and S is still there. The reply is an error that names
  BOTH ids and says the new session was created but the old could not be removed. The user sees two rows and deletes the
  old by hand. Silently reporting success here would hide a session the user believes is gone, and rolling the create
  back would kill an agent the user asked for; the honest answer is the loud one.

A replace on a session whose host is unreachable is refused up front with the host's state named, through the same owner
lookup every lifecycle operation uses (`route_session`); SPEC.md already says nothing queues. An archived or exited
source is a legitimate replace target (there is no agent to kill, only a record to discard), and the menu offers it
wherever clone is offered, which is every retention state.

Idempotency: the route accepts the same optional `intent_key` a create does and forwards it to the create half, so a
retried request cannot double-create. The delete half has no such protection and needs none in practice: the UI fires
one request per confirmation and does not retry on its own. A retry after a fully successful replace answers 404 for the
source; say so in the route's docs rather than pretending the operation is idempotent end to end.

## Helm side

### `crates/farhelm-helm/src/sessions.rs` and `lib.rs`

One route, `POST /api/sessions/{id}/replace`, body `{"intent_key": <optional string>}`. The handler:

1. Routes the source through `route_session` (refuses non-connected hosts with the state named, 404 for an unknown id).
2. Reads the source row from the host's list (the same drain `clone_for_agent` does at agent_requests.rs:1075) and
   derives a `CreateMode` from it: `resolved_profile` when its `source_profile` snapshot still names a catalog profile,
   `Raw(invocation)` otherwise. That derivation is the piece to REFACTOR: today it lives inline in `clone_for_agent`
   with the agent-CLI's refuse-on-dangling rule; lift the source-to-mode step into a function in `sessions.rs` that
   takes the fallback policy as an argument (refuse, or fall back to raw), and have both callers use it, so a change to
   how a snapshot is matched cannot drift between the two.
3. Calls `do_create_session` with the source's `cwd`, `title`, host claim and client, the `intent_key`, and whatever
   further fields clone forwards from a source today (`agent_kind`, the resume template if a raw create carries one).
   Terminal size takes `CreateReq`'s defaults; the first attach resizes it like any other create.
4. Calls `client.delete_session(&source_id)` and `forget_session` exactly as `delete_session` at sessions.rs:1753 does.
5. Answers with the new session row (the same shape `POST /api/sessions` answers with, so the client's existing decoder
   applies), or, on a failed delete, an error whose message names both ids as described above. The fleet-events revision
   is bumped by the create and the forget as it is today; no extra bump is needed.

Tests in `sessions_tests.rs` through `rest_harness` and the fake supervisor: a live raw-invocation source is replaced
(new id, same cwd and title and invocation, old id gone from the list); a profile-backed source follows its profile; a
profile-backed source whose profile was deleted falls back to its invocation (the divergence from the agent-CLI clone,
pinned so nobody "fixes" it into a refusal); an archived source is replaced; a create refusal leaves the source listed;
a delete failure after a successful create reports both ids and leaves both rows; an unreachable host is refused before
anything is created; and `intent_key` reaching the create half (a repeated request with the same key yields one new
session).

## UI side

### `crates/farhelm-ui/src/api.rs`

`replace_session(base, id) -> Result<Session, String>`, a POST in the shape of `archive_session` at api.rs:1670, with an
`intent_key` minted per confirmation the way the create form mints one.

### `crates/farhelm-ui/src/status.rs`

`replace_consequence(status) -> &'static str`, beside `confirm_consequence`, and worded under the same no-guessing rule:
a live status says the agent is killed and the conversation discarded; `Unknown` says the agent may still be running;
exited and interrupted say the conversation is discarded; `Error` says nothing ran. Every arm ends by saying a fresh
session with the same settings takes its place, because that is the half that distinguishes this prompt from delete's.
Unit tests mirror `confirm_consequence`'s.

### `crates/farhelm-ui/src/list/row.rs`

`MenuAction::Replace`, placed directly after `Clone` in `MENU_ACTIONS` and offered under the same rule (every retention
state; `session_menu_order` answers `true` unconditionally as it does for `Clone`). A `confirming_replace` state
alongside `confirming_archive`, rendered through the same inline prompt shape (consequence line in its own untruncatable
element ahead of the truncatable title, cancel as the only way back, the open button inert while it shows), and an
`on_replace: EventHandler<Session>` up to the view. Extend the menu-order unit tests for the new position and the
retention rule.

### `crates/farhelm-ui/src/list/view.rs`

The handler calls `api::replace_session` under the global nav lock the other operations take, and on success records the
NEW session as the selection through `remember_selection` (a user-initiated choice, which is also what writes the helm's
shared `last_selected`). The list's next read drops the old row; the selection-reconciliation path that already handles
a vanished selected row must not fire first and pick something else, so the selection write happens before the re-read
is requested. On the two-id error the message is shown in the row's error line like any other operation's failure, and
nothing is selected.

## Spec and docs

`SPEC.md`, Lifecycle operations: add `replace` to the supported list and a **Replace** bullet after **Clone**, stating
the whole contract above in user terms: a brand-new session with a NEW id and a fresh conversation, same host,
directory, title and agent (profile when it still exists, otherwise the original command line), the old session deleted
once the new one exists, the confirmation, and the failure rule (a failed create changes nothing; a failed removal after
a successful create is reported with both sessions left in place). Contrast it with restart in one sentence, since the
two are the pair the maintainer wants kept apart. Also add replace to the Agent-spawned sessions section's note on what
the CLI verbs do NOT include: there is no `farhelm agent replace` in this plan, and an agent replacing its own session
would be killing itself mid-request, which is a design question for another day.

`SPEC_impl.md`, Helm internals: a paragraph on the route being a composition of the existing create and delete, the
shared source-to-mode derivation and its two fallback policies, and why the create goes first.

## Browser tests

One case in sidebar.spec.ts or the lifecycle spec that holds the other operations: replace a live session from the row
menu, confirm, and assert a new row with the same title and a different `data-session-id` is selected, the old id is
gone, the terminal shows a fresh agent (the fake agent's READY line with no prior scrollback), and the cwd and
invocation fields match. A second case cancels the prompt and asserts nothing changed. The two-id failure is a
helm-level test, not a browser one.

## PR shape

Two PRs: the helm route with its refactor and tests (SPEC_impl paragraph included), then the UI and SPEC.md wording. The
entry and this file are removed in the second.
