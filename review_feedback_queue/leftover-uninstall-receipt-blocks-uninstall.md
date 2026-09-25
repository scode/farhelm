# A leftover uninstall receipt blocks uninstall after a reinstall

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After an interrupted uninstall followed by a reinstall of a different version, `farhelm uninstall` refuses forever with
advice (rerun the installer) that never helps.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F4 / COR-LEFTOVER-UNINSTALL-RECEIPT`, tagged **definite**. Anchors and title:
`crates/farhelm/src/uninstall/ownership.rs:491`, `crates/farhelm/src/uninstall/ownership.rs:494`,
`crates/farhelm/src/uninstall/removal.rs:79-87`, `scripts/install.sh:1265-1267` — A leftover uninstall retry receipt
blocks every later uninstall after a reinstall, and the advised remedy cannot clear it

When `farhelm uninstall` removes the macOS app bundle, it deletes the files and subdirectories first. Just before it
deletes the bundle's internal receipt and the last two directories, it copies the receipt to a sibling file,
`~/Applications/.Farhelm.app.uninstall-receipt`, so that an interrupted removal can still prove ownership on retry
(removal.rs:79–87). That sibling copy is deleted only as the very last bundle step. If uninstall is interrupted between
removing the bundle root and deleting the sibling, the sibling survives.

If the user then reinstalls instead of retrying the uninstall, the installer builds a fresh bundle and never looks at or
removes the sibling file (install.sh contains no reference to it). If the new bundle came from a different release or a
different install directory, its internal receipt differs from the stale sibling (the version string is inside
`Info.plist`, whose digest the receipt carries). From then on, `inspect_bundle_at` sees both receipts, finds they
disagree (ownership.rs:491–494), and refuses every uninstall with "…it disagrees with the app's internal receipt; rerun
the installer, or upgrade once to a release that writes uninstall metadata". Rerunning the installer changes nothing,
because it does not touch the sibling. A different interruption point, where the bundle directory still exists but
`Info.plist` is already gone, instead makes the installer itself refuse the half-removed bundle (install.sh:1203–1208).

SPEC.md "Operator prerequisites and failure behavior" requires that retries continue cleanup safely and that refusals be
actionable. Here one uninstall attempt's retry state blocks a later installation's removal, and the advice loops.
Options: have the installer's bundle rebuild remove a sibling receipt it can verify as its own (regular file, owned by
the user, `farhelm-app` magic); or have uninstall treat a disagreeing sibling as stale when the internal receipt fully
verifies the bundle that is present; at minimum, name the sibling file in the refusal so the user can delete it.

Restater note: SPEC_impl.md "Standalone uninstall" says explicitly "If both receipts survive, they must agree", so the
second option (uninstall ignoring a disagreeing sibling) needs a spec change; the installer-side cleanup does not. The
trigger is narrow: uninstall must be interrupted in a window of a few syscalls at the very end of bundle removal, and
the user must then reinstall a different version rather than rerun uninstall.
