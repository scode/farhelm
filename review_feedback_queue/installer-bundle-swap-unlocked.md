# The macOS bundle swap runs unlocked and non-atomically

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Interrupting the installer, or running it twice at once, on a Mac can leave a Farhelm.app that mixes versions, is nested
inside another, or is half-deleted, which later installer or uninstall runs refuse.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F3 / COR-BUNDLE-SWAP-UNLOCKED`, tagged **possible**. Anchors and title: `scripts/install.sh:1161`,
`scripts/install.sh:1219`, `scripts/install.sh:1266`, `scripts/install.sh:1267` — The macOS app-bundle step runs after
the lock is released and swaps non-atomically, so overlapping or interrupted runs leave a mixed, nested or half-deleted
bundle

On macOS the installer builds `~/Applications/Farhelm.app` only after it has released the install lock (L1161). It
copies the _currently installed_ `$INSTALL_DIR/farhelm-desktop` and `$INSTALL_DIR/farhelm` into a staging bundle
(L1219–1220), stamps it with this run's version in `Info.plist`, and swaps it in with `rm -rf "$app_path"` followed by
`mv "$bundle_stage" "$app_path"` (L1266–1267). The install lock is per install directory, but the bundle path is one per
home directory, so any other installer, including one targeting a different `FARHELM_INSTALL_DIR`, can run this step at
the same time.

Three bad outcomes follow:

- **Mixed versions.** If another installer commits new binaries between the two `cp` calls, the bundle contains one
  binary from each version, labelled with this run's version. Its receipt hashes the copies, so uninstall accepts it.
- **Nested bundle.** If both runs do `rm -rf` before either does `mv`, the first `mv` creates `Farhelm.app`, and the
  second `mv` moves its stage _into_ it as `Farhelm.app/Farhelm.app` (ordinary `mv` semantics when the destination is a
  directory). Both runs report success. Uninstall's layout check (`reject_unexpected`) then refuses the bundle because
  of the unexpected entry.
- **Half-deleted husk.** Ctrl-C or SIGTERM during `rm -rf` stops deletion part-way; if `Contents/Info.plist` was already
  gone, the next installer refuses the bundle at L1204 ("does not look like a farhelm app bundle") and uninstall rejects
  it too.

The script's comment calls the bundle a derived artifact "rebuilt wholesale", which needs a swap that cannot interleave
or stop half done. Suggested direction: hold the lock through L1267 or take a separate home-directory lock for the
bundle; swap by renaming the old bundle aside, moving the stage in, then deleting the old copy; re-check that the
destination is absent immediately before the final rename; and build from this run's own verified binaries rather than
whatever is in `$INSTALL_DIR` at that moment.

Restater note: the finding suggests building "from the verified binaries already staged in `$STAGING_DIR`", but the
replace loop _moves_ (L1131, `mv "$STAGING_DIR/$name" "$dest"`) rather than copies the staged binaries, so they are no
longer in `$STAGING_DIR` by the bundle step. That part of the fix needs a copy kept before the commit, or a check that
the installed files still match this run's digests.
