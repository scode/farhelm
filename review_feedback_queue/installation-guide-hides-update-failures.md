# Installation guide contradicts visible automatic update failures

Reviewed commit: 77069b3acaf0cecf7cc1741956451039dd23772e

## TLDR

The installation guide says automatic update failures appear only in logs, although Farhelm now shows qualifying
failures beside the version.

## Details

Suggested bucket: `other`. Confidence: Confirmed by inspection; documentation mismatch. Source: dedicated `gpt-6.1-sol`
high adversarial review of this commit. Proposed drop: none.

`docs/install_uninstall.md:47–50` still describes manual results only in the hover and says a failed automatic check is
only logged. The new behavior is in `crates/farhelm-ui/src/desktop/updater.rs:836–882` and revised
SPEC.md:321–327,350–359. SPEC.md:284–291 requires this guide to remain accurate and change alongside installation/update
behavior.

Update the guide for immediate installation failures, the three-day probe grace period, persistence, retry and
dismissal. The stale guide also omits the new version-button check action introduced separately.

Possible cover: None found in Planned, BUGS, queue, relevant ledger entries or filters.

Current-main status: persists by inspection at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; no fixing commit found. The
reviewer read the full commit diff and relevant traces; this is not an exhaustive review of unchanged surrounding code.
