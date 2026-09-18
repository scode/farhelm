# An ssh destination of "this machine" collides with the local host's display name

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Adding an ssh host with a certain name silently breaks targeting the local machine: the local host — the default place
new sessions run — stops accepting agent commands, and the new host entry can never connect either.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: definite. Coordinator verified.

Every host's display name is its alias, else the derived name: destination verbatim, or `"this machine"` for the local
row (`host_display_name`, `crates/farhelm-helm/src/aggregate.rs:136` — the one function all layers share).
`add_ssh_host`'s collision check (`alias_collision`, store.rs:1185-1200) reads the `alias` column ALONE; the unaliased
local row's derived `"this machine"` is compared against nothing. So `POST /api/hosts {"ssh": "this machine"}` passes
`destination_is_usable` (non-empty, no leading `-`, no NUL; REST adds no check, hosts.rs:449) and inserts — likewise
retarget, `--ensure-hosts`, and probed registration via the same helper. Two rows then display `"this machine"` and
`resolve_host` answers `Conflict` for both (agent_requests.rs:855-859: second match → `Conflict`, "the fleet is the
thing that is incoherent") — the local host, the default create target, silently stops being an agent target. The
check's own rationale names exactly this outcome for aliases (store.rs:3295-3301); the local derived name is the one
display name the checks omit. The ssh destination never connects either (space), so the row is pure damage until
removed. Aliases cannot take the name (the SET path does the wide comparison, store.rs:3773-3783) — this is a
destinations-only gap.

Suggested fix: refuse destination candidates equal to the local row's current `host_display_name` in the four
destination writes (keeping retarget's skip-when-aliased); add `AliasTaken` tests mirroring the alias cases.
