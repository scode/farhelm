# The installer rm -rf's Farhelm.app on a substring match

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Re-running the installer on a Mac can permanently delete a Farhelm.app the user built or customised, including anything
they added inside, just because its Info.plist mentions Farhelm, while refusing to repair its own half-removed bundle.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F22 / SEC-APP-BUNDLE-GREP-RM`, tagged **definite**. Anchors and title: `scripts/install.sh:1203`,
`scripts/install.sh:1204`, `scripts/install.sh:1266` — The installer rm -rf's any ~/Applications/Farhelm.app whose
Info.plist merely mentions "farhelm", ignoring its own receipt

Before rebuilding the macOS app bundle, the installer's only test of whether an existing `~/Applications/Farhelm.app` is
its own is at L1203–1204: `Contents/Info.plist` must exist and `grep -qi farhelm` must match somewhere in it. That is
true for almost any app named Farhelm, since the name normally appears as `CFBundleName` or the executable name. It then
runs `rm -rf "$app_path"` (L1266). The step runs outside the install lock and never reads
`Contents/.farhelm-installation`, the receipt the installer itself writes and that uninstall relies on. There is no
check of bundle identifier, layout, or file owner. (A dangling symlink at that path skips the check entirely, but then
only the link itself is removed.)

The check also fails in the other direction: an installer-built bundle whose `Info.plist` was already removed by an
interrupted uninstall is refused even though its receipt is valid. And the uninstaller's refusal for a receipt-less
bundle says "rerun the installer" (F10); following that advice is what triggers this deletion.

SPEC.md "Operator prerequisites and failure behavior" says "A foreign file or bundle must not be deleted merely because
its name matches an expected artifact". The uninstaller enforces receipts, digests, a fixed layout and non-recursive
`rmdir` for this same path; the installer has broader deletion power with none of those safeguards. A user-built app
(`dx bundle`), a re-signed or customised one, a wrapper app, and anything the user added inside it would all be deleted
without a prompt.

Suggested change: replace the bundle only when it contains a regular, user-owned, not group- or world-writable receipt
with `farhelm-app` magic (internal, or the uninstall sibling from F4) naming this install directory. For bundles from
releases before receipts existed, keep a narrow legacy check (exact `CFBundleIdentifier` `org.scode.farhelm.desktop`
plus the fixed layout). Otherwise refuse, or rename the old bundle aside instead of deleting it.

Restater note: the code comment at L1198–1202 shows the loose match is deliberate, to accept the pre-installer "trial"
bundle; that is exactly the legacy case the suggested identifier-plus-layout check would keep supporting.
