# Installer fallback lock file is created world-openable

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

On a volume where the ~/Applications folder itself cannot be locked, the stand-in lock file is created so another local
account can hold it and block the owner's installs/uninstalls (and, under a permissive umask, break mutual exclusion).

## Details

**other.** Source inspection; commissioner confirmed the creation modes. Local DoS / weakened exclusion; no data loss
established. Premises named (kept from the reviewer): a lock-unsupported volume (the only time the fallback is used)
plus a second local account; umask 0 for the exclusion-break variant. Would rise only with a realistic multi-user
deployment on such a volume — not established here. FILTER.md rare-glitch filter does not match: a cross-account DoS is
a trust-boundary consequence (excluded), and the Busy refusal persists while the lock is held (not self-correcting).

`crates/farhelm-supervisor/src/installation/mod.rs:208-216` (`lock_installation` fallback) creates
`.farhelm-install.lock` with `OpenOptions::new().read(true).write(true).create(true).truncate(false)` → mode 0o666 &
umask (0644 under the usual 022); the folder is created at mod.rs:175 via `DirBuilder` (0777 & umask → 0755). flock on a
read-only fd succeeds, so any account that can traverse to the file can open it RDONLY and hold an exclusive flock,
making the owner's run report Busy and refuse (local DoS while held). Under umask 0 the file is 0666 and folder 0777,
letting another account rename the lock file away and substitute its own, breaking exclusion between two of the owner's
runs. SPEC.md's ownership section assumes other accounts cannot create/replace entries in install directories, but
opening an existing world-readable file RDONLY is neither — and SPEC.md's 2026-10-10 confirmation requires good
permission practice for files Farhelm creates.

Verify: on a flock-unsupported mount, hold the fallback as another user, run install → Busy; inspect the created mode.
Fix shape: create the fallback 0o600 (and the folder 0o700 when Farhelm creates it); keep it never-deleted as designed.

Reviewer metadata (area 5 chunk F): suggested bucket: other; confidence: likely (premises named); proposed_drop: none;
possible_cover: none. Commissioner validation: modes confirmed at the cited lines; coverage searched (queue, TODO.md,
BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no match.
