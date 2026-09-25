# A failed canonicalization stores the literal path as verified

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Restart can be permanently refused for that session.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F13 / COR-CANONICALIZE-FAILURE`, tagged **possible**. Anchors and title: `service/core.rs:7215-7232`,
`service/core.rs:3109-3140` — A failed canonicalization at create stores the literal path as the verified directory,
blocking later restarts

A session records two spellings of its working directory. `cwd` is what the user typed. `canonical_cwd` is the same path
with symlinks, `.`, `..` and trailing slashes resolved. The canonical form serves two purposes. Conversation capture
compares it against the `getcwd()` the agent reports. It is also the directory's identity: before every restart and
every keyed create retry, `ensure_cwd_identity` (core.rs:3109-3140) re-resolves `cwd` and refuses unless the result
equals the stored `canonical_cwd` exactly, with "working directory … now resolves to …, refusing to relaunch its agent
somewhere else". This guards against a symlink that was repointed between launches. A session with no stored canonical
path (`None`) skips the check.

At create, `validate_create` calls `tokio::fs::canonicalize` right after confirming the directory is usable
(core.rs:7215-7232). If that call fails, it logs a warning and stores `Some(cwd)`, the literal spelling, instead of
`None`. The comment says this fallback "costs capture for that session, never correctness". That is only true when the
literal path happens to be canonical already. If it contains a symlink component, a trailing slash, `.` or `..`, every
later identity check canonicalizes it successfully to something different from the stored literal and refuses. A one-off
failure at create time becomes a permanent refusal to restart the session or retry its keyed create.

The trigger is rare. Canonicalization has to fail transiently just after the usability check passed, for example because
something in the path changed in between, and the literal path must not already be canonical. The fix is small: on
canonicalization failure, store `None`, which the identity check already treats as "nothing recorded to verify", or fail
the create as a transient error. The literal spelling should not be persisted as a verified identity.
