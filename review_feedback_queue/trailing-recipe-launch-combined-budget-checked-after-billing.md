# The early recipe-launch check ignores the combined 64 KiB create budget

Reviewed commit: 90dc9a41f2d18c16476229395b094c9a83e827c1

## TLDR

A new-sandbox or new-container launch whose title, initial prompt and command together exceed 64 KiB (each one under 64
KiB on its own) passes every check Farhelm makes before it creates the disposable host. The sandbox is allocated (and on
Tensorlake, billed), set up, and then the session create is refused by the new host's supervisor, and the fresh host is
torn down. An agent request goes further: the user is shown the approval card and allows a launch that was already
impossible. The commit aims to refuse an unusable title before any host exists or any approval is asked, but only checks
the title by itself.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t08c-sandbox-env` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. No coverage source covers it; the retired item the commit addressed may have scoped
itself to the title alone.

### Reviewer record

Location: `crates/farhelm-helm/src/sessions.rs:1214`

At 90dc9a41f2, `CreateReq::prepare_sandbox_launch` (crates/farhelm-helm/src/sessions.rs:1214-1222) refuses a title with
control characters or longer than `MAX_SESSION_TITLE_BYTES` (64 KiB, crates/farhelm-proto/src/text.rs:23). Its docstring
(sessions.rs:1194) says "A title must fit before a disposable host exists." The supervisor, however, enforces one
combined cap: crates/farhelm-supervisor/src/service/handlers.rs:396-411 sums parent + cwd + launch bytes
(`create_launch_field_bytes`) + named start + prompt + checkout + title and refuses when the total exceeds
`CREATE_FIELD_CAP` (64 KiB, handlers.rs:86). SPEC.md (Title bullet, around line 1141-1145) states the same: "the text
you send on creation — working directory, command and resume command, title, and the rest of the launch — must fit in 64
KiB between them". The helm has no counterpart of the combined check (no helm use of
`CREATE_FIELD_CAP`/`create_launch_field_bytes`; prompt validation in crates/farhelm-proto/src/session_launch.rs:271-291
checks the prompt on its own). Route: the REST route `/api/environment-sessions`
(crates/farhelm-helm/src/sessions/sandbox.rs:125 -> provisioning/tensorlake_add.rs:359) and the agent route
(agent_requests.rs:4646-4653, before the approval card) both call only `prepare_sandbox_launch`; `register_session` and
provider work follow; `finish_session_launch` -> `create_in_session_sandbox` -> supervisor create is refused with "...
together are N bytes, exceeding the 65536-byte limit" and cleanup forgets the new host. Example: title of 40 KiB plus
initial prompt of 30 KiB, or an initial prompt near 64 KiB plus any command. Verify by reading the two checks side by
side; a test like the commit's `recipe_home_title_is_checked_before_rest_provisioning` with a 40 KiB title and a 30 KiB
prompt would reach host registration. Fix: compute the same combined byte total in `prepare_sandbox_launch` (share the
accounting through farhelm-proto so helm and supervisor cannot drift) and refuse there, covering cwd "~", the compiled
launch, prompt, named start, checkout fields and title. The pre-existing gap for prompt+command is not introduced by
this commit; what the commit adds is a title check whose docstring and tests claim the title "fits" when it only fits
alone.

Suggested bucket: other — the consequence is a short billed sandbox and a refused launch (plus a pointless approval card
for agent launches) for unusually large inputs, with no data or credential loss; it would move to high only if ordinary
launches (e.g. long agent prompts) were shown to hit it routinely.

Confidence: confirmed (source-established: the helm checks title alone, the supervisor checks the sum; no helm-side
combined check exists at the commit).

proposed_drop: none

Revision (route scoping): for agent launches the asking supervisor already caps the serialized `edits` at 64 KiB
(crates/farhelm-supervisor/src/service/handlers.rs:3729-3731), so an agent reaches the gap only when saved templates
merged in, or the compiled launch, push the total over; the commit test that sends a 65537-byte title through
`HelmAgentRequests::handle` bypasses that doorway. The REST route used by the GUI launcher has no combined check
anywhere (no GUI-side budget check found in crates/gui/farhelm-gui/src at the commit), so a pasted long prompt plus a
long title is the main reachable case.

Revision (turn 2 synthesis, route re-check): `/api/environment-sessions` (crates/farhelm-helm/src/lib.rs:568 at the
commit) carries no route-specific `DefaultBodyLimit` layer, unlike the 8/16 KiB routes at lib.rs:800-914, so axum's
default 2 MB body limit applies and a request with a 40 KiB title and 30 KiB prompt reaches `prepare_sandbox_launch`.
Interaction with t08a/t08b f18807f6 (size consent): the size-confirmation check runs after `prepare_sandbox_launch` in
`start_session_environment` (provisioning/tensorlake_add.rs:359-371), so the user confirms the shown size, the launch is
admitted, and only the new host's supervisor refuses it. Finding, bucket and confidence unchanged.

possible_cover: none known; the commit's own removed queue item about the early title check may have scoped itself to
the title alone.
