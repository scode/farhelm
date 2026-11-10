# Installer leftover assertions reject a supported fallback lock file

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

Installer tests can report leftover debris when the installer correctly retains its permanent fallback lock file on a
filesystem without directory locking. The native legacy-refusal acceptance has the same conditional mismatch; a Mac test
volume requiring that fallback remains unverified after investigation.

## Details

### Native legacy-refusal snapshot

Additional reviewed commit: 6940d803421bb25a4a9ce5a298c0168ae6e7573a, checked through
0bc984b71384e7b754b55d007b53ebbeb9a076ca. Suggested bucket: **other**. Confidence: **unverified after investigation**
for native reachability, with the conditional source/oracle mismatch confirmed.

`scripts/test-uninstall.py:190-218,285-296` seeds the legacy installation without the fallback lock, snapshots the home,
requires status 76 and its upgrade-floor/uninstall diagnostics, then rejects every new entry. Installer locking occurs
before the floor check (`installation/install.rs:207-240,329`), and `installation/mod.rs:199-224` intentionally retains
the regular-file fallback when directory flock is unsupported. Its descriptor owner (`:132-141`) does not unlink it; the
permanent filename is documented at `:68-71`. The shell's ephemeral-stage cleanup is separate
(`scripts/install.sh:680-682,998-1007`). SPEC_impl.md's side-by-side installation contract explicitly permits that
permanent file and separately requires the dedicated floor refusal.

The missing premise is a supported Mac test filesystem that refuses directory flock with a recognized unsupported error
but permits file flock and the remaining setup to reach status 76. The ordinary lock test
(`installation/mod.rs:849-866`) exercises directory locking; it neither witnesses nor refutes that native fallback.
Source inspection cannot settle actual filesystem errors, and this review ran no substrate probes. A retained native run
on such a volume would settle reachability; fault injection alone would establish only the conditional mismatch.

Account narrowly for the permanent artifact or establish it before the baseline. Preserve the runnable old fixture,
status 76, exact floor reason and actionable uninstall advice, old bytes/modes/symlinks/ownership, user data and
unrelated entries, and ephemeral cleanup. Do not allow generic leftovers or delete the permanent lock to satisfy the
assertion. Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, `g041/g041-legacy-refusal-snapshot`.

`crates/farhelm-supervisor/src/installation/install/tests.rs:115–166`, especially `125–131`, allows only the app and
named extra entries. `installation/mod.rs:190–232` intentionally creates and retains `.farhelm-install.lock` when file
locking works but directory locking does not. That legitimate entry fails the helper's no-leftovers assertion.

Suggested bucket: `other`. Confidence: confirmed conditional oracle mismatch; no supported test volume was shown to take
this branch and no failure was reproduced. Possible cover: permanent fallback-lock retention is specified behavior, not
acceptance that tests must fail there. Provenance: `g012/f2`, found during discovery and followed by a separate empty
search. Capability-aware expectations must continue rejecting actual temporary debris.
