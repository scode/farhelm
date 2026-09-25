# ADD ignores requested or recorded binary and state paths

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Re-running setup on a host with a custom state directory or binary path (or entering one in the add form) silently
installs to the default locations, starting an empty supervisor and hiding the host's existing sessions.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F6 / COR-ADD-IGNORES-COORDS`, tagged **definite**. Anchors and title: `provisioning/service.rs:413-428`,
`provisioning/service.rs:511-526`, `provisioning/plan.rs:293-311`, `store.rs:3665-3677`,
`crates/farhelm-ui/src/hosts.rs:2578-2583` — ADD ignores requested or recorded remote_farhelm/remote_state_dir and
rewrites the row to the default layout

The add-host form lets the user type an optional binary path and state directory, and a rerun against an existing row
fills them from that row (service.rs:321-332). The probe uses them. But when the probe finds nothing, the ADD plan is
built with `self.layout.plan(Add, …)` (service.rs:423-428) — the default layout with no overrides — so it installs to
`~/.local/lib/farhelm/farhelm` and `~/.local/state/farhelm` no matter what was typed or recorded. `register` then
prefers the plan's paths over the request's (service.rs:511-526), and the store's converge branch (store.rs:3665-3677)
overwrites the existing row's `remote_farhelm` and `remote_state_dir` with them.

The UI's own comment on the add form (`crates/farhelm-ui/src/hosts.rs:2578-2583`) says the optional fields are sent
because "a retained setup plan uses them as its installation coordinates"; the server ignores them for exactly that
purpose. The practical case: a known host with a custom layout whose supervisor is merely down. Re-running setup
installs a fresh supervisor on a fresh state directory, which mints a new identity with no sessions, and the row forgets
the old paths — so the user's existing sessions are no longer reachable from the helm. SPEC.md promises that re-running
provisioning is idempotent recovery that re-registers the host "with all its sessions intact", and SPEC_impl.md says
provisioning starts from recorded coordinates. The confirmation shows the new default paths but does not say the typed
or recorded ones were discarded.

Suggested change (any of): apply the `plan_for_row`-style overrides when coordinates were supplied or recorded (without
chmodding shared directories, see F28); refuse ADD for an already-registered destination and direct the user to UPDATE;
or at least warn in the plan when the paths differ.

User-visible consequence: re-running setup on a host with a custom state directory or binary path (or entering one in
the add form) silently installs to the default locations, starting an empty supervisor and hiding the host's existing
sessions.
