# Installer leftover assertions reject a supported fallback lock file

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

Installer tests can report leftover debris when the installer correctly retains its permanent fallback lock file on a
filesystem without directory locking.

## Details

`crates/farhelm-supervisor/src/installation/install/tests.rs:115–166`, especially `125–131`, allows only the app and
named extra entries. `installation/mod.rs:190–232` intentionally creates and retains `.farhelm-install.lock` when file
locking works but directory locking does not. That legitimate entry fails the helper's no-leftovers assertion.

Suggested bucket: `other`. Confidence: confirmed conditional oracle mismatch; no supported test volume was shown to take
this branch and no failure was reproduced. Possible cover: permanent fallback-lock retention is specified behavior, not
acceptance that tests must fail there. Provenance: `g012/f2`, found during discovery and followed by a separate empty
search. Capability-aware expectations must continue rejecting actual temporary debris.
