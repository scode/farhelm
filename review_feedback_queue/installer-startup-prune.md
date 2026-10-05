# An interrupted update can prune a version whose supervisor is starting

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

An interrupted update can prune a version whose supervisor is starting.

## Details

`F34 / COR-INSTALLER-STARTUP-PRUNE` — **possible** — `scripts/install.sh:1315` — An interrupted update can prune a
version whose supervisor is starting

A possible startup race can leave a running supervisor without the program it needs to launch agents. Suppose an update
from version A to B stops after replacing the desktop program but before advancing the Installed record, which still
names A. Opening that desktop starts B's supervisor from B's own version directory. Meanwhile, a subsequent update to C
retains A as the previously installed version and C as the new version. It reads the Running record once to decide which
additional version to keep.

If B's supervisor has passed its own-program existence check but has not yet published its Running record when C reads
that record, C can delete B's directory. B can then publish its record and finish starting from its already running
process. Its agent-launch helper still uses the deleted B program. A later session Restart can stop the old agent and
then fail to launch the replacement; commands forwarded to B can fail too.

Coordinate supervisor startup and publication of its version with pruning, or retain an intermediate version during
interrupted-update recovery. The installer lock serializes installers, but the supervisor's startup and Running-record
publication do not take that lock.

Suggested bucket: highest

Possible cover: `SPEC_impl.md:2901–2907` describes interrupted updates and which versions the Installed and Running
records protect. Whether that accepted policy covers this publication gap remains unresolved.

Caveats: This exact interleaving on native macOS with the default state directory has not been reproduced. It is
distinct from the documented limitation that a supervisor using another custom state directory is protected for only one
update.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_lifecycle p1`.

Possible cover recorded during collection: SPEC_impl2901–2907 interruptedupdate/defaultRunning policy..

Collection caveats: Native default-state interleaving unverified; customstate oneupdate waiver differs.

## Filed reviewer metadata

- `cli_lifecycle p1`: confidence as filed: possible / likely; the unverified premise is the native macOS startup/update
  interleaving described below. No runtime reproduction. Suggested bucket as filed: `highest`.
