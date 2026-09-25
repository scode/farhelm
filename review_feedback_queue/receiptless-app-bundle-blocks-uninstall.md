# A receipt-less Farhelm.app blocks the whole uninstall

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A Mac user with an older or self-built Farhelm.app in ~/Applications (for example after installing with the no-bundle
opt-out) cannot run `farhelm uninstall` at all, and the suggested fix either does nothing or deletes their app.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F10 / COR-RECEIPTLESS-BUNDLE-BLOCKS-UNINSTALL`, tagged **possible**. Anchors and title:
`crates/farhelm/src/uninstall/ownership.rs:244`, `crates/farhelm/src/uninstall/ownership.rs:479`,
`crates/farhelm/src/uninstall/ownership.rs:502` — On macOS a Farhelm.app without a receipt blocks the whole uninstall,
and the advised repair cannot work under the app-bundle opt-out

On macOS, `farhelm uninstall` always inspects `~/Applications/Farhelm.app` (ownership.rs:244), and any problem it finds
there refuses the entire uninstall, including removal of the flat binaries that are fully verified by their own record.
A bundle without an internal receipt fails either at the layout check (`reject_unexpected`, L479: any extra entry such
as `Contents/PkgInfo` or `Contents/_CodeSignature` from a signed or `dx bundle`-built app) or with "ownership receipt …
is missing; rerun the installer to repair it" (L502).

Such bundles exist in practice. The installer built receipt-less bundles from #310 (2026-09-01) until receipts were
added in #673 (2026-09-16), and a user may have their own Farhelm.app, for example one built from source. For a user who
updated with the supported `FARHELM_NO_APP_BUNDLE=1` opt-out, or with a `FARHELM_VERSION` pinned to a release from
before the app icon existed, rerunning the installer skips the bundle step entirely, so the refusal repeats forever.
Rerunning without the opt-out either refuses (if `Info.plist` lacks "farhelm") or deletes the user's app outright (see
F22).

The finding argues that a bundle with no receipt of either kind is simply not Farhelm's to judge, and should be reported
as a retained foreign bundle, the way Linux already reports a foreign `farhelm-desktop` as retained. The refusal would
stay for a bundle that has a receipt and fails verification. At minimum the refusal should stop advising a reinstall
under the opt-out, or say that the bundle can be moved aside.

Restater note: for the common case (a pre-#673 installer bundle and no opt-out), "rerun the installer" does work: the
current installer rebuilds the bundle with a receipt. SPEC.md asks for an "actionable refusal" when ownership is
ambiguous and removes only "the recognized installer-created macOS app bundle"; a receipt-less bundle made by an old
installer is genuinely ambiguous, so whether to retain or refuse it is partly a spec-interpretation question.
