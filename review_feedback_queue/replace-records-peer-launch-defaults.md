# Plain Replace records the remote host's launch selection as defaults

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Pressing Replace on a session from a compromised remote machine can quietly switch your remembered launch defaults to
"yolo" with workspace trust on every machine.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F25 / SEC-REPLACE-PEER-LAUNCH-DEFAULTS`, tagged **definite**. Anchors and title: `sessions.rs:3089`,
`sessions.rs:3125`, `sessions.rs:2017` — A plain Replace records the remote host's listed launch selection as the user's
helm-wide permission and trust choice

**Replace** (`POST /api/sessions/{id}/replace`) recreates a session under a new id and deletes the old one. With no
`with` body, it reuses the source session's settings, which it reads live from the owning host's `ListSessions`: data
the remote supervisor supplies. `mode_from_source` (`sessions.rs:2017`) copies the source row's
`launch: Some(selection)` verbatim into `CreateMode::Structured` (called from `sessions.rs:3089`). `do_replace_session`
then creates with `origin: CreateOrigin::User` (`sessions.rs:3125`). As in F24, `accept_created_session` treats a
user-origin create as authority to write the selection's permissions and workspace trust into the helm-wide remembered
preferences.

The F24 fix alone does not close this. Here the _request_ selection itself was copied from the peer's row, so "derive
from the helm's own request" still yields the peer's values. The user never chose these values in this action, and
Replace asks nothing about permissions. The same SPEC rule applies as in F24.

Suggested fix: derived replacements must not move the remembered launch defaults. Either pass
`remember_launch_choices = false` or add a distinct origin (for example `CreateOrigin::Derived`). Only an explicit
"replace with" body or the composer should update them.

User-visible consequence: pressing Replace on a session from a compromised remote machine can quietly switch your
remembered launch defaults to "yolo" with workspace trust on every machine.
