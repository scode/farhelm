# Installer interruption test rejects its supported no-swap fallback

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

The same-version interruption regression can reject the installer's documented fallback on a filesystem without atomic
directory exchange.

## Details

`crates/farhelm-supervisor/src/installation/install/tests.rs:685` rejects `VersionNameEmpty`, although the supported
exchange fallback emits that interruption point. The fallback is valid product behavior; the test's expected sequence
assumes the stronger swap capability. The temporary test filesystem's actual capability was not established, so this is
a conditional portability/oracle issue rather than an observed failure.

Suggested bucket: `other`. Confidence: confirmed source-contract mismatch with the affected substrate unverified.
Possible cover: the accepted implementation fallback explains production behavior, not the regression's rejection of it.
Provenance: `g012/f1`. Test the fallback through injected capability or allow its intended trace while preserving the
real recovery assertions. No test or filesystem reproduction ran.
