# Trusted agent-requested Antigravity launches do not pin the canonical folder before approval

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When Antigravity's automatic folder-trust switch is turned on and an agent requests a new session, spawn, or clone in a
folder given through a symlink, the approval card shows the symlink path instead of the real destination folder, and the
target machine only resolves the symlink later when Antigravity actually starts. An existing symlink on a remote target
can disguise which folder will be granted workspace trust, and repointing a symlink while the approval card waits
redirects the trusted launch to a different folder than the user approved so its hooks, settings, and commands run
without prompting.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/launches.rs:735`.

In commit 4aff3f1b1e0b778d4f6265b6a0660ebfc7ac4e1e, `workspace_trust_requires_managed_checkout` was renamed to
`workspace_trust_pins_approval_cwd` (`crates/farhelm-helm/src/launches.rs:732-749`), keeping
`LaunchHarness::Antigravity => false` while returning `true` for `Claude | Codex | Muse | Pi` (at
`66ad3aaa95ef03a2130e2d0194d58353912ae9da`). Its doc comment says
`Antigravity already resolves its directory in the supervisor.`

That rationale is incorrect for approval-card pinning:

1. At spawn time (`crates/farhelm-supervisor/src/agent_kind/mod.rs:2331-2356`), the supervisor resolves `cwd` via
   `std::fs::canonicalize(cwd)` in the exact same place for both `{antigravity:trusted-cwd}` and `{codex:trusted-cwd}`.
   Spawn-time canonicalization does not freeze what the approval card shows: Codex uses the same spawn-time
   canonicalization and still returns `true` from `workspace_trust_pins_approval_cwd` so the card displays and pins the
   canonical path.
2. Because `TrustResolvedLaunch::pins_approval_cwd` (`crates/farhelm-helm/src/workspace_trust.rs:53-58`) delegates to
   `workspace_trust_pins_approval_cwd`, both `create_for_agent` (`crates/farhelm-helm/src/agent_requests.rs:2092-2099`,
   covering Create, Spawn, and SpawnInherited) and `clone_for_agent`
   (`crates/farhelm-helm/src/agent_requests.rs:3935-3937`) skip `pin_trusted_approval_cwd`
   (`client.canonical_launch_directory(cwd)`) for Antigravity.
3. Established route (source analysis): With `antigravity_trust` enabled in Settings, an agent invokes
   `farhelm agent create --host <target> --agent antigravity --cwd <symlink>` (or `spawn` / `clone`). The approval card
   shows `<symlink>` with the Trusted workspace (from Settings) chip, and the helm dispatches `<symlink>` verbatim after
   Allow. At spawn, the supervisor canonicalizes `<symlink>` and passes
   `--settings {"trustedWorkspaces":["<canonical>"]}` to `agy`. Cross-host, a remote agent can exploit any existing
   symlink on `<target>` (e.g., inside a checked-out repo pointing to another repo or `/`) to hide the true canonical
   directory being trusted from the approval card; if `<symlink>` is repointed on `<target>` during the approval wait,
   the trusted launch is redirected to the new directory. Caveat: on the same host, same-account processes already share
   local Unix authority (`SPEC.md`, "Local authority and trust between hosts"), whereas the cross-host symlink-masking
   route requires no write access on `<target>`.
4. Spec and tests: `SPEC.md:5662-5664` and `SPEC_impl.md:3562-3565` enumerate Claude, Codex, Muse, and Pi for canonical
   pinning across approval and omit Antigravity; `SPEC_impl.md:5209-5210` notes only that the supervisor resolves the
   directory when filling Antigravity's slot. Neither spec section explicitly accepts unpinned symlink display or
   symlink redirection during approval for Antigravity. `crates/farhelm-helm/src/agent_requests.rs:12372`
   (`inherited_spawn_normalizes_trust_before_approval_and_dispatch`) currently asserts
   `harness != LaunchHarness::Antigravity` skips `BrowseDirectory`, and
   `trusted_agent_create_and_clone_pin_a_plain_alias_before_approval`
   (`crates/farhelm-helm/src/agent_requests.rs:11218-11418`) tests only `LaunchHarness::Codex`.
5. Fix direction: Return `true` for `LaunchHarness::Antigravity` in `workspace_trust_pins_approval_cwd`
   (`crates/farhelm-helm/src/launches.rs:735-749`), update `SPEC.md:5662` and `SPEC_impl.md:3562` to include Antigravity
   alongside Claude, Codex, Muse, and Pi, update `inherited_spawn_normalizes_trust_before_approval_and_dispatch`
   (`agent_requests.rs:12372`), and extend `trusted_agent_create_and_clone_pin_a_plain_alias_before_approval`
   (`agent_requests.rs:11218`) to cover `LaunchHarness::Antigravity`.

Reviewer metadata (g14-trust-codex-muse-pi-helm/l1): suggested bucket: highest (could be in highest: yes — security
impact on the agent approval-card trust boundary); confidence: confirmed; possible cover: none.

Additional source record at `6f9918a88100c8d8a69ae471fe12cfde4c7bf76d`, `crates/farhelm-helm/src/launches.rs:735-749`
(g16-trust-sessions-and-claude-guard/l7):

In commit `4aff3f1b1e0b778d4f6265b6a0660ebfc7ac4e1e`, `workspace_trust_requires_managed_checkout` was renamed to
`workspace_trust_pins_approval_cwd` (`crates/farhelm-helm/src/launches.rs:735-749`), and commit
`6f9918a88100c8d8a69ae471fe12cfde4c7bf76d` exposed `TrustResolvedLaunch::pins_approval_cwd`
(`crates/farhelm-helm/src/workspace_trust.rs:53-58`) — its sole caller. Every agent-initiated directory launch path
routes through `pins_approval_cwd` / `pin_trusted_approval_cwd`: `create_for_agent`
(`crates/farhelm-helm/src/agent_requests.rs:2092-2099`, which handles `Create`, `Spawn`, and `SpawnInherited` via
`edits.spawn` at lines 1970 and 2120-2124) and `clone_for_agent` (`agent_requests.rs:3935-3937`).
(`ParsedAgentVerb::Restart` at lines 717-809 is plain Restart only and cannot change `cwd` or compose trust.) Because
`workspace_trust_pins_approval_cwd` kept `LaunchHarness::Antigravity => false` from the old managed-only check — with
the inaccurate comment "Antigravity already resolves its directory in the supervisor" (`launches.rs:733`) — Create,
Spawn, SpawnInherited, and Clone all skip canonical cwd pinning and `composition_incarnation` recording for Antigravity.
In fact, the supervisor resolves `cwd` via `std::fs::canonicalize(cwd)` at spawn time for Codex and Antigravity in the
exact same function (`crates/farhelm-supervisor/src/agent_kind/mod.rs:2336-2354`, `fill_cwd`) and for Claude in
`launch.rs:1233`/`trust.rs:100`. Resolving in the supervisor at spawn time is precisely what allows a symlink repointed
during the approval wait to redirect trust to a different directory unless the helm canonicalizes and pins `cwd` before
`approve_launch` and ties the check to `composition_incarnation`. Note: `SPEC_impl.md:3562-3565` currently lists only
"Claude, Codex, Muse or Pi trust enabled" without mentioning Antigravity, reflecting the rename in `4aff3f1b` rather
than an explicit acceptance of symlink-retargeting for Antigravity. Fix direction: Return `true` for
`LaunchHarness::Antigravity` in `launches::workspace_trust_pins_approval_cwd` (so every harness with
`offers_workspace_trust()` pins its approval cwd and records `composition_incarnation`), update `SPEC_impl.md:3562`, and
extend `trusted_agent_create_and_clone_pin_a_plain_alias_before_approval` (`agent_requests.rs:11218`) to cover
Antigravity as well.

Reviewer metadata (g16-trust-sessions-and-claude-guard/l7): suggested bucket: highest (could be in highest: security —
trust-boundary / TOCTOU bypass on user-approved agent Create, Spawn, SpawnInherited, and Clone with Antigravity
workspace trust enabled: a requesting agent can repoint a symlink directory during the approval wait so Farhelm grants
Antigravity workspace trust to a different directory than the user inspected and approved); confidence: confirmed;
possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g14-trust-codex-muse-pi-helm/l1`,
`g16-trust-sessions-and-claude-guard/l7`.
