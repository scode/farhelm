# The archive directory's owner and mode are not checked

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With a checkout root other local users can write to, one of them can end up owning, and locking you out of, the folder
your deleted sessions' repositories are moved into.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F16 / SEC-ARCHIVE-ROOT-OWNERSHIP`, tagged **possible**. Anchors and title:
`working_copies.rs:1231-1255`, `working_copies.rs:1190-1204`, `working_copies.rs:1432-1438` — The archive directory is
trusted without checking its owner or mode

When the last session of a managed checkout is deleted, the checkout is moved into
`<root>/farhelm-archived-working-copies`. `ensure_archive_root` accepts an existing directory at that path as long as it
is a real directory (not a symlink) on the same filesystem. It does not check who owns it or what its permissions are.
When the directory does not exist, it is created with a plain `create_dir`, which gives mode `0777` minus the umask.

This matters when the configured checkout root can be written by another Unix account, such as a group-shared folder or
a sticky, `/tmp`-style directory:

1. The other account creates `farhelm-archived-working-copies` first, and makes it writable by the victim.
2. The victim's next last-reference Delete moves their checkout into that directory, which the other account owns.
3. The other account runs `chmod 700` on the archive directory.

The victim can then no longer reach the only preserved copy of the checkout, which may contain unpushed work. SPEC.md
makes the Unix account the security boundary, so other accounts are untrusted. Same-account attackers are out of scope
and are not what this finding is about.

Open premise: the user configured a checkout root that other local accounts can write to.

Suggested change: require an existing archive directory to be owned by the supervisor's effective uid, and refuse it
when it is group- or world-writable. Create it with mode `0700`, and re-apply the permissions afterwards so the umask
cannot loosen them. `ensure_private_dir` already does this for the attachment quarantine.
