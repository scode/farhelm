# Plain Replace makes the remote host's claimed profile the default

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Pressing Replace on a session from a compromised remote machine can change which profile the new-session dialog suggests
by default on every machine.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F26 / SEC-REPLACE-PEER-DEFAULT-PROFILE`, tagged **possible**. Anchors and title: `sessions.rs:1752`,
`sessions.rs:1856`, `sessions.rs:3125` — A plain Replace makes the remote host's claimed profile the helm-wide
remembered default profile

A **profile** is a named, saved launch configuration in the helm-wide catalog. The New dialog suggests a **remembered
default profile**. When a plain Replace's source row has no structured launch but names a `source_profile`,
`mode_from_source` looks that profile id up in the catalog and returns `CreateMode::ResolvedProfile`. The
`source_profile` here is as reported in the owning host's `ListSessions`. `do_create_session` then sets
`remembered_profile = Some(profile.id)` (`sessions.rs:1752`). Because Replace uses `CreateOrigin::User`
(`sessions.rs:3125`), `accept_created_session` calls `remember_default_profile` (`sessions.rs:1856`) and overwrites the
helm-wide default.

A remote supervisor can list a session claiming any catalog profile id, such as a "yolo" profile. Profile ids are
discoverable to agents by design. This is a different database write from F25, so it needs its own gate.

SPEC.md names this case explicitly: remote "profile references … must not override an explicit user choice or
indefinitely determine that default for other hosts". Agent-relay creates are already kept away from this default, and
Replace is a side door.

Suggested fix: skip `remember_default_profile` when the create mode was derived from a source row. Keep it for the
composer, and for a "replace with" body naming a profile the user chose.

User-visible consequence: pressing Replace on a session from a compromised remote machine can change which profile the
New-session dialog suggests by default on every machine.
