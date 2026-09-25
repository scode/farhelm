# An interrupt before the record publish leaves stale digests

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If an update is cancelled at the wrong instant, a later `farhelm uninstall` refuses with a checksum-mismatch error that
does not say rerunning the installer fixes it.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F12 / COR-INTERRUPT-BEFORE-RECORD`, tagged **possible**. Anchors and title: `scripts/install.sh:1152`,
`scripts/install.sh:1157`, `crates/farhelm/src/uninstall/ownership.rs:415` — An interrupt between commit and record
publish leaves new binaries with the old ownership record, and uninstall's refusal reads like tampering

An update commits by deleting the journal (L1152). Only afterwards does it delete the `.old` backups, hash both
binaries, and publish the new ownership record `.farhelm-installation` (L1157). A Ctrl-C, HUP or TERM in that window
runs `cleanup`, which finds no journal (so, correctly, does not roll back) and just releases the lock. The directory now
holds the new binaries beside the _previous_ record, whose digests describe the old ones.

A later `farhelm uninstall` then fails in `verify_payload` (ownership.rs:415) with "… does not match its recorded
SHA-256 digest". Unlike the other record failures, which go through `repair_refusal` and say "rerun the installer", this
message gives no advice, and to a user it reads like tampering. SPEC.md "Operator prerequisites and failure behavior"
asks refusals to say how to handle them.

Two complementary fixes: move record publication into the protected phase (stage it before the commit and journal it, or
ignore catchable signals from journal deletion until the record is published); and add "if an install or update was
interrupted, rerun the installer" to the flat-binary digest-mismatch error.

Restater note: the window is short (two `rm -f` and two SHA-256 passes over the binaries), and rerunning the installer
repairs the record, since it republishes it unconditionally. The problem is mainly that nothing tells the user that.
