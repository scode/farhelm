# Another local account can hold Farhelm's installation lock

Reviewed commit: 237ac795335024ba9954f02bd114c6d3d9061159

## TLDR

Another account on the Mac that can read the user's Applications folder can hold Farhelm's installation lock
indefinitely. Installs, updates, the app's update checks and `farhelm uninstall` then refuse as busy. Making the
fallback lock file private, which this commit does, protects only volumes where the folder cannot be locked. On ordinary
volumes the folder lock carries the same exposure the commit set out to remove.

On a Mac whose Applications folder sits on a volume that cannot lock folders, releases since 0.32.0 left a lock file
there that other accounts on the machine can open. This fix makes newly created lock files private, but an existing file
is reopened as it is, so on a machine that already has one, another local account can still hold the lock and make the
user's installs, updates and `farhelm uninstall` refuse as busy. The fix only protects machines that never ran an
earlier release in that configuration.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer records `t07c-hosts-provisioning` `f7`, `t07c-hosts-provisioning`
`f3` (Claude Code `claude -p`, `claude-opus-5-5`, effort high). Findings were validated by source and contract analysis;
nothing was built, run or reproduced.

### Recording notes

Two reviewer records merged: f7 (the primary lock is a flock on the readable `~/Applications` folder) and f3 (237ac79533
makes only newly created fallback lock files private, so a file shipped releases created stays world-openable). f3's
consequence was narrowed in synthesis to the same local denial of service; the independent drop check AGREED it is
outside `highest`, consistent with the retired item it follows up (installer-fallback-lock-world-openable.md, bucketed
`other`). Confidence `likely`: that home and `~/Applications` are readable by other accounts on real machines is not
established. No coverage source covers it.

### Reviewer record `t07c-hosts-provisioning` `f7`: Another account that can read ~/Applications can hold the primary install lock (the folder itself)

Location: `crates/farhelm-supervisor/src/installation/mod.rs:195`

TLDR: Another account on the Mac that can read the user's Applications folder can hold Farhelm's installation lock
indefinitely. Installs, updates, the app's update checks and `farhelm uninstall` then refuse as busy. Making the
fallback lock file private, which this commit does, protects only volumes where the folder cannot be locked. On ordinary
volumes the folder lock carries the same exposure the commit set out to remove.

`lock_installation` (crates/farhelm-supervisor/src/installation/mod.rs:181-242 at 237ac79533) creates `~/Applications`
with `DirBuilder::new().create` (default mode 0777 & umask, normally 0755). It then opens it with
`OFlags::RDONLY | OFlags::DIRECTORY` and takes `flock(LOCK_EX|LOCK_NB)` on that descriptor. Only on
ENOTSUP/EOPNOTSUPP/ENOLCK does it fall back to the file that `open_fallback_lock` now creates as 0600. flock needs only
an open descriptor, and opening a directory read-only needs only its read permission. So any account with search
permission on the home and read permission on `~/Applications` can `open(O_RDONLY|O_DIRECTORY)` the folder and keep
`flock(LOCK_EX)` on it. The user's installer then reports busy after its five-second wait, and the uninstall command and
update checks refuse (callers: installation/install.rs:208, installation/mod.rs:645,
farhelm/src/uninstall/locks.rs:170). The commit's own reasoning for the file ("A read-only descriptor can take the same
exclusive lock as the installer") applies to the folder as well. SPEC.md's threat-model paragraph ("Farhelm is not
designed for install directories that other local accounts can write to ... assume no other account can create or
replace entries in them") is about write access, not read, so it does not obviously accept this. A fix would need a lock
object only the user can open, for example a 0600 lock file in a private per-user directory taken alongside the folder
lock (older tools only check the folder, so the folder lock must stay), or a documented acceptance in SPEC.md. Verify by
reading lock_installation and the open(2)/flock(2) permission rules.

Suggested bucket: other. It is a local denial of service by another account on the same Mac: nothing is lost or exposed,
and the user can recover once the other account's process ends. Showing that cross-account interference with installs
counts as a security impact under the maintainer's threat model would move it up.

Confidence: likely. The unverified premise is that the home and `~/Applications` are readable by other local accounts on
real machines (macOS's default home mode is believed to be 0755). The lock mechanics are source-established.

proposed_drop: none

possible_cover: possibly the triage outcome behind the retired queue item installer-fallback-lock-world-openable.md (not
read, per the brief), which may have scoped or accepted the cross-account case.

### Reviewer record `t07c-hosts-provisioning` `f3`: Private fallback install lock applies only to new files; a shipped release's world-readable lock file stays openable

Location: `crates/farhelm-supervisor/src/installation/mod.rs:164`

TLDR: On a Mac whose Applications folder sits on a volume that cannot lock folders, releases since 0.32.0 left a lock
file there that other accounts on the machine can open. This fix makes newly created lock files private, but an existing
file is reopened as it is, so on a machine that already has one, another local account can still hold the lock and make
the user's installs, updates and `farhelm uninstall` refuse as busy. The fix only protects machines that never ran an
earlier release in that configuration.

`open_fallback_lock` (crates/farhelm-supervisor/src/installation/mod.rs:164-176 at 237ac79533) passes `.mode(0o600)` to
`OpenOptions`, which applies only when `open` creates the file; its doc comment says so ("Restrict newly created
fallback files"). `lock_installation` (mod.rs:222-229) reuses an existing `~/Applications/.farhelm-install.lock` without
checking or tightening its mode. The file has existed since 0ae92a2a7 (first shipped in v0.32.0), was created with the
default 0666 & umask (normally 0644), and is never deleted (SPEC_impl.md near line 7902: "a fixed lock file in
`~/Applications` that is never deleted"). The commit's own comment says why mode matters: "A read-only descriptor can
take the same exclusive lock as the installer", and flock on Linux and macOS accepts a read-only descriptor. So on an
upgraded machine, another account that can traverse the home and `~/Applications` (macOS creates homes 0755 by default,
and a `create_dir`-made `~/Applications` is 0755 under the usual umask) can open the old 0644 file and hold an exclusive
flock, so every install, update check and uninstall refuses or waits out its timeout. The new test
`a_new_fallback_lock_file_is_private` (mod.rs:767-776) checks only a fresh directory. Fix: after opening, if the file is
owned by the current uid and `mode & 0o077 != 0`, `fchmod` it to 0600 (refuse if another uid owns it), and add a test
that starts from a pre-existing 0644 file. SPEC.md's "Confirmed 2026-10-10" paragraph under the install-directory rules
asks only for good permission practice on files Farhelm creates, which includes this file, created by an earlier
release.

Suggested bucket: other. Another local account can block installs and uninstalls (a local denial of service) only on
volumes that need the fallback lock, and no data or credential is exposed. Showing that cross-account interference
counts as a security impact under the project's threat model would move this to `high` or `highest`.

Confidence: likely. Source establishes the create-only mode and that the file persists. The unverified premise is that a
real user has the fallback-lock configuration with home and `~/Applications` traversable by other accounts.

proposed_drop: none

possible_cover: the commit retires a queue item, installer-fallback-lock-world-openable.md (not read, per the brief);
its triage outcome may have scoped the fix to new files.

Revision (turn 2): The impact is narrower than first stated. On ordinary volumes the primary lock is a flock on the
~/Applications folder itself, which any account able to read that folder can also hold (recorded as f7). The stale 0644
fallback file therefore leaves fallback-lock volumes only as exposed as ordinary volumes already are, rather than adding
a new kind of exposure. The finding stands as an incomplete fix of the fallback file specifically; bucket stays other.
If f7 is accepted as a non-issue under the threat model, f3 should be dropped on the same basis.
