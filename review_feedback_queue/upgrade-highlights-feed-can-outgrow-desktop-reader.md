# A cumulative v1 highlights feed can exceed the shipped reader budget without failing promotion

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The cumulative highlights feed can exceed the desktop reader’s limit while still passing the release promotion check.

## Details

website/.agents/release-notes.md:15–18 documents finalized stable releases beginning at v0.42.0 and continued use by
existing desktops. This is a fixed earliest version, with no rolling window, maximum release count or byte budget.
website/scripts/release-notes.mjs:158–166,272–278,299–304 serializes every eligible release to one asset.
releasing/.agents/changelog.md:25–34,104–109,137–142 protects and curates history without a numeric highlights limit.
SPEC.md:5836–5854 defines the installation upgrade floor only. It neither bounds this asset nor exempts otherwise
eligible older readers. SPEC.md:837–842 describes the release interval and recoverable fetch failure.
SPEC_impl.md:7627–7634 fixes the whole-response cap and requires the running release.
crates/farhelm-desktop/src/whats_new.rs:13–16,36–46,79–105 and crates/farhelm-desktop/src/updater.rs:1691–1702 reject
the over-budget response before version selection. The v1 check in releasing/.agents/stable.md:248–256 checks format and
current-release membership with a 20-second request deadline, without the desktop byte limit or a bounded fallback. The
generator, reader and relevant promotion check have no in-range fix. Enforce the shipped budget on the generated
artifact and at promotion, and decide what running-version coverage v1 must continue to provide before any retention or
versioned-feed change.

Confidence: confirmed by inspection conditional on the body exceeding the cap. Neither the deployed size nor a threshold
date is established.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `releaseWeb/l2`, `upgradeBackend/l6`.
